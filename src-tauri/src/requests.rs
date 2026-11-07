//! The app side of the design request (Machine `design-request`): the editor's commands, what
//! `Send` starts, and what the rest of the app tells the requests.
//!
//! The rules and the files are in `weaver_core::project::requests`; this module carries them out
//! in the project's queue and connects them to the rest of the app:
//!
//! * `developer.NEW_REQUEST`, `SEND_REQUEST`, `DISCARD_REQUEST`, `LEAVE_REQUEST` (an empty request
//!   is discarded when the developer leaves it or its window closes) and every save are commands. A
//!   write runs in the project's mutation queue ([`crate::mutations`]); a save starts no process.
//!   `Send` plans the job, starts it through [`RequestJobs`] (a setup job when the Model is empty,
//!   otherwise a design job) and then records the job in `.state.json`, all under the project lock
//!   and under the gate that keeps the Board from showing the draft and the ghost together.
//! * The requests being written are the draft cards of the Board's Backlog: [`RequestsService::drafts`]
//!   works them out for the Board snapshot ([`crate::board::board_snapshot`]) and [`refresh_board`]
//!   sends the Board again when they changed.
//! * `proposal.SUBMITTED`: [`proposal_submitted`], called by the hand-over of a Proposal bound to
//!   the request (see [`crate::proposals::answer_cli`]).
//! * `job.ENDED`: [`JobEnd`], installed with [`install`]. A request whose job ended without a
//!   Proposal bound to it goes back to `Writing`; the editor says so with `returned`.
//! * `workspace.REQUEST_DELETED`: [`files_changed`], called with every batch of file changes of a
//!   project; a sent request whose file is gone is forgotten and the editor is told. The same
//!   function sends the Board again (its slice only) when a draft appeared, vanished or changed
//!   its title or whether it is empty, whoever wrote the file.
//! * `weaver job new design` from the working General: [`GeneralSender`].
//! * Closing a window or quitting: [`flush_window`] asks the editor to save what is pending and
//!   waits for its answer before the window goes, so that closing never loses text and never sends.
//!
//! Logs name request ids and never their text.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

use serde::Serialize;
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager, State};
use ts_rs::TS;
use weaver_cli::spec::{Call, Failure};
use weaver_core::assets::tags;
use weaver_core::command::CommandReply;
use weaver_core::job_instruction::Job;
use weaver_core::paths::project_hash_of_canonical;
use weaver_core::project::image::ProjectImage;
use weaver_core::project::requests::image::{base64_decode, base64_encode, ImageError};
use weaver_core::project::requests::{
    model_is_empty, LocalTime, RequestError, RequestState, RequestStore, SendPlan,
};

use crate::agent_work::AgentWork;
use crate::blocking;
use crate::error::{AppError, ErrorCode};
use crate::events::EventName;
use crate::jobs::starts::Ctx;
use crate::jobs::wire::{JobInfo, JobObserver};
use crate::jobs::{
    authority::{CommandExtension, RequestSender},
    Jobs,
};
use crate::pictures::Rebuild;
use crate::state::AppState;
use crate::windows::registry::label_of;
use crate::{log_info, log_warn};

/// How long closing a window waits for the editor to save what is pending.
pub const FLUSH_LIMIT: Duration = Duration::from_secs(2);

/// A job a request started.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartedJob {
    pub id: String,
    /// The fixed name of the job, as the Jobs list shows it.
    pub name: String,
}

/// Where a job stands, as far as a request cares.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobLife {
    /// It works, waits for the developer, or is suspended.
    Active,
    Finished,
}

/// What the requests need from the jobs of the agent.
pub trait RequestJobs: Send + Sync {
    /// Starts the setup or design job (`Job::Setup`, `Job::Design`) on a request; the refusal is
    /// the sentence the developer reads.
    fn start(&self, ctx: &Ctx, job: &Job) -> Result<StartedJob, String>;
    /// The name and life of a job; `None` when the jobs do not know it.
    fn life(&self, job_id: &str) -> Option<(String, JobLife)>;
    /// The names of the assets of a project now: the tags of a request are recognised against
    /// them when it is sent. None in the tests of the requests.
    fn asset_names(&self, _project_id: &str) -> Vec<String> {
        Vec::new()
    }
    /// A request was recorded as sent to the job just started: the rows of the Jobs list say it.
    fn recorded_sent(&self, _project_id: &str) {}
}

/// The jobs of the agent.
struct AppJobs(Arc<Jobs>);

impl RequestJobs for AppJobs {
    fn start(&self, ctx: &Ctx, job: &Job) -> Result<StartedJob, String> {
        let started = match job {
            Job::Setup { request, assets } => self.0.start_setup(ctx, request, assets.clone()),
            Job::Design { request, assets } => self.0.start_design(ctx, request, assets.clone()),
            other => return Err(format!("{other:?} is not a job a request starts")),
        };
        let id = started.map_err(|refusal| format!("Could not send the request: {refusal}."))?;
        let name = self.0.info(&id).map(|info| info.name).unwrap_or_default();
        Ok(StartedJob { id, name })
    }

    fn life(&self, job_id: &str) -> Option<(String, JobLife)> {
        self.0.record(job_id).map(|record| {
            (
                record.name,
                if record.state.is_active() {
                    JobLife::Active
                } else {
                    JobLife::Finished
                },
            )
        })
    }

    fn asset_names(&self, project_id: &str) -> Vec<String> {
        self.0.asset_names(project_id)
    }

    fn recorded_sent(&self, project_id: &str) {
        self.0.rows_changed(project_id);
    }
}

/// What the refusal of a job a request starts begins with (`RequestJobs::start`).
const SEND_REFUSED_PREFIX: &str = "Could not send the request: ";

/// A request the agent of Chat sent (`agent.SEND_REQUEST`) and the job that works on it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentSent {
    pub request: String,
    pub title: String,
    pub job: StartedJob,
    /// `design`, or `setup` when the Model was empty.
    pub kind: &'static str,
}

/// Why the agent of Chat could not send a request: nothing was written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentSendRefusal {
    /// The kind of job that would have started.
    pub kind: &'static str,
    pub message: String,
}

impl AgentSendRefusal {
    /// What the Chat shows after `Weaver did not start <kind>:`: the message without its lead-in
    /// and its final full stop.
    fn reason(&self) -> &str {
        self.message
            .strip_prefix(SEND_REFUSED_PREFIX)
            .unwrap_or(&self.message)
            .trim_end_matches('.')
    }
}

/// Whether a request is being written or was sent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "kebab-case")]
pub enum RequestStatus {
    Writing,
    Sent,
}

/// One open request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RequestView {
    pub id: String,
    pub text: String,
    pub status: RequestStatus,
    pub job_id: Option<String>,
    pub job_name: Option<String>,
    pub returned: bool,
    /// The Model has no Feature and no Actor: a request sent now is a setup request.
    pub model_empty: bool,
}

/// Payload of `requests-changed` and `requests-flush`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RequestsEvent {
    pub project_id: String,
}

/// The requests of the app: the jobs they start and what is only in memory.
pub struct RequestsService {
    jobs: RwLock<Arc<dyn RequestJobs>>,
    /// `(project id, request id)` of the requests that went back to `Writing` because their job
    /// ended without a Proposal.
    returned: Mutex<HashSet<(String, String)>>,
    /// The windows closing, by project id, waiting for their editor to save.
    flushing: Mutex<HashMap<String, tokio::sync::oneshot::Sender<()>>>,
    /// The drafts of each open project as the Board shows them, the last time they were read: the
    /// Board is touched only when they differ.
    drafts: Mutex<HashMap<String, Vec<Draft>>>,
    /// The projects whose requests were brought up to date for the first Board of their window
    /// (the sent requests whose job ended while the app was closed went back to `Writing`).
    opened: Mutex<HashSet<String>>,
    /// Held while a window opens, so that two first Boards do the bringing up to date once.
    opening: tokio::sync::Mutex<()>,
    /// A Board snapshot reads (ghosts and drafts together); starting the job of a request and
    /// recording the request as sent write: no snapshot shows the draft and the ghost together.
    board_gate: RwLock<()>,
}

/// A request being written as a card of the Board's Backlog (`requirements/tech#design-request`):
/// what the Board shows of it. A change of anything else (the text below the title) leaves the
/// Board as it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Draft {
    pub id: String,
    /// The first heading, else the first line, else `Untitled request`.
    pub title: String,
    /// The local time of creation, `YYYY-MM-DDTHH:MM:SS`.
    pub created: String,
    /// Nothing is left of the text after removing whitespace.
    pub empty: bool,
    /// It came back to `Writing` because its job ended without a Proposal.
    pub returned: bool,
}

fn failed(error: RequestError) -> AppError {
    match error {
        RequestError::Io(e) => AppError::internal(e),
        RequestError::Image(e) => AppError::new(ErrorCode::InvalidInput, e.to_string()),
        RequestError::NotAnId(_) => AppError::new(ErrorCode::InvalidInput, error.to_string()),
        other => AppError::new(ErrorCode::Refused, other.to_string()),
    }
}

fn status_of(state: RequestState) -> RequestStatus {
    match state {
        RequestState::Sent => RequestStatus::Sent,
        _ => RequestStatus::Writing,
    }
}

impl RequestsService {
    pub fn new(jobs: Arc<dyn RequestJobs>) -> Self {
        Self {
            jobs: RwLock::new(jobs),
            returned: Mutex::new(HashSet::new()),
            flushing: Mutex::new(HashMap::new()),
            drafts: Mutex::new(HashMap::new()),
            opened: Mutex::new(HashSet::new()),
            opening: tokio::sync::Mutex::new(()),
            board_gate: RwLock::new(()),
        }
    }

    /// Remembers the drafts of a project as the Board shows them now. `true` when they differ from
    /// the ones remembered (or none were remembered): the Board has to be sent again.
    pub fn remember_drafts(&self, project_id: &str, drafts: Vec<Draft>) -> bool {
        let mut known = self.drafts.lock().expect("drafts poisoned");
        match known.get(project_id) {
            Some(last) if *last == drafts => false,
            _ => {
                known.insert(project_id.to_owned(), drafts);
                true
            }
        }
    }

    /// The window of a project is gone: its drafts are forgotten and its next window starts again.
    fn forget_drafts(&self, project_id: &str) {
        self.drafts
            .lock()
            .expect("drafts poisoned")
            .remove(project_id);
        self.opened
            .lock()
            .expect("opened projects poisoned")
            .remove(project_id);
    }

    /// What a Board snapshot holds while it reads the ghosts and the drafts.
    pub fn board_read(&self) -> std::sync::RwLockReadGuard<'_, ()> {
        self.board_gate.read().unwrap_or_else(|e| e.into_inner())
    }

    fn board_write(&self) -> std::sync::RwLockWriteGuard<'_, ()> {
        self.board_gate.write().unwrap_or_else(|e| e.into_inner())
    }

    fn is_opened(&self, project_id: &str) -> bool {
        self.opened
            .lock()
            .expect("opened projects poisoned")
            .contains(project_id)
    }

    fn jobs(&self) -> Arc<dyn RequestJobs> {
        self.jobs.read().expect("request jobs poisoned").clone()
    }

    fn returned(&self) -> std::sync::MutexGuard<'_, HashSet<(String, String)>> {
        self.returned.lock().expect("returned requests poisoned")
    }

    fn is_returned(&self, project_id: &str, request: &str) -> bool {
        self.returned()
            .contains(&(project_id.to_owned(), request.to_owned()))
    }

    fn mark_returned(&self, project_id: &str, requests: &[String]) {
        let mut returned = self.returned();
        for request in requests {
            returned.insert((project_id.to_owned(), request.clone()));
        }
    }

    fn forget_returned(&self, project_id: &str, request: &str) {
        self.returned()
            .remove(&(project_id.to_owned(), request.to_owned()));
    }

    /// The sent requests whose job finished meanwhile (the app was closed when it ended) go back
    /// to `Writing`.
    fn return_finished(&self, store: &RequestStore, project_id: &str) -> Result<(), RequestError> {
        let jobs = self.jobs();
        let state = store.state()?;
        let mut finished: Vec<&String> = state
            .sent
            .values()
            .filter(|job| matches!(jobs.life(job), Some((_, JobLife::Finished))))
            .collect();
        finished.sort();
        finished.dedup();
        for job in finished {
            let returned = store.job_ended(job, false)?;
            self.mark_returned(project_id, &returned);
        }
        Ok(())
    }

    /// The requests being written, as the cards of the Backlog, oldest first. `ghost_jobs` are the
    /// jobs the same Board shows a ghost for: a request sent to a job that works is no draft, and
    /// one sent to a job that finished and shows no ghost is a draft that came back, even before
    /// the end of the job was recorded in its file, so that its card is in the very picture in
    /// which the ghost leaves.
    pub fn drafts(
        &self,
        project_id: &str,
        store: &RequestStore,
        ghost_jobs: &HashSet<String>,
    ) -> Result<Vec<Draft>, RequestError> {
        let jobs = self.jobs();
        let mut drafts: Vec<Draft> = store
            .list()?
            .into_iter()
            .filter_map(|entry| {
                let returned = match entry.state {
                    RequestState::Writing => self.is_returned(project_id, &entry.id),
                    _ => {
                        let finished = entry.job_id.as_deref().is_some_and(|job| {
                            !ghost_jobs.contains(job)
                                && matches!(jobs.life(job), Some((_, JobLife::Finished)))
                        });
                        if !finished {
                            return None;
                        }
                        true
                    }
                };
                Some(Draft {
                    id: entry.id,
                    title: entry.title,
                    created: entry.created,
                    empty: entry.empty,
                    returned,
                })
            })
            .collect();
        // `list` is newest first.
        drafts.reverse();
        Ok(drafts)
    }

    /// The first Board of a window: the requests whose file is gone are forgotten and the sent
    /// ones whose job ended while the app was closed go back to `Writing`. Call it inside the
    /// project's queue.
    pub fn bring_up_to_date_locked(
        &self,
        project_id: &str,
        root: &Path,
    ) -> Result<(), RequestError> {
        let store = RequestStore::new(root);
        store.reconcile()?;
        self.return_finished(&store, project_id)
    }

    /// The text and standing of one open request; `model_empty` is whether the Model has no Feature
    /// and no Actor.
    pub fn view(
        &self,
        project_id: &str,
        root: &Path,
        request: &str,
        model_empty: bool,
    ) -> Result<RequestView, RequestError> {
        let store = RequestStore::new(root);
        let text = store.read(request)?;
        let job_id = store.state()?.sent.get(request).cloned();
        let status = status_of(store.state_of(request)?);
        let job_name = job_id
            .as_deref()
            .and_then(|job| self.jobs().life(job))
            .map(|(name, _)| name);
        Ok(RequestView {
            id: request.to_owned(),
            text,
            returned: status == RequestStatus::Writing && self.is_returned(project_id, request),
            status,
            job_id,
            job_name,
            model_empty,
        })
    }

    /// `developer.SEND_REQUEST`, under the project lock: the job it starts, `None` when the request
    /// is empty and nothing happens.
    pub fn send_locked(
        &self,
        project_id: &str,
        root: &Path,
        request: &str,
    ) -> Result<Option<String>, AppError> {
        let store = RequestStore::new(root);
        let image = ProjectImage::load(root);
        // The tags are recognised against the assets that exist now, in the one read of the
        // request; the request file stays as it is written.
        let names = self.jobs().asset_names(project_id);
        match store
            .plan_send_tagged(request, model_is_empty(image.model()), &|text| {
                tags::tagged(text, &names)
            })
            .map_err(failed)?
        {
            SendPlan::Nothing => Ok(None),
            SendPlan::Start(job) => {
                // No Board snapshot sees the job's first ghost and the draft together.
                let _board = self.board_write();
                let started = self
                    .jobs()
                    .start(
                        &Ctx {
                            project_id,
                            root,
                            image: &image,
                        },
                        &job,
                    )
                    .map_err(|why| AppError::new(ErrorCode::Refused, why))?;
                store.record_sent(request, &started.id).map_err(failed)?;
                self.jobs().recorded_sent(project_id);
                self.forget_returned(project_id, request);
                log_info!("the request {request} was sent to the job {}", started.id);
                Ok(Some(started.id))
            }
        }
    }

    /// `agent.SEND_REQUEST`, under the project lock: creates the request with the text the agent of
    /// Chat gave, starts its job (design, or setup when the Model is empty) and records the request
    /// as sent. When the job cannot start, the request is removed again and nothing is left.
    pub fn agent_send_locked(
        &self,
        project_id: &str,
        root: &Path,
        image: &ProjectImage,
        text: &str,
        now: &LocalTime,
    ) -> Result<AgentSent, AgentSendRefusal> {
        let refused = |kind: &'static str, message: String| AgentSendRefusal { kind, message };
        // The request exists only as sent: no Board snapshot sees it as a draft.
        let _board = self.board_write();
        let store = RequestStore::new(root);
        // The tags are recognised against the assets that exist now, in the one read of the text.
        let names = self.jobs().asset_names(project_id);
        let empty_model = model_is_empty(image.model());
        let kind = if empty_model { "setup" } else { "design" };
        let sent = store
            .plan_agent_send(now, text, empty_model, &|text| tags::tagged(text, &names))
            .map_err(|error| refused(kind, failed(error).message))?
            .ok_or_else(|| refused(kind, "The request is empty: write it first.".to_owned()))?;
        let started = match self.jobs().start(
            &Ctx {
                project_id,
                root,
                image,
            },
            &sent.job,
        ) {
            Ok(started) => started,
            Err(why) => {
                // Nothing is left of a request whose job did not start.
                if let Err(error) = store.discard(&sent.id) {
                    log_warn!("the request {} could not be removed: {error}", sent.id);
                }
                return Err(refused(kind, why));
            }
        };
        store
            .record_sent(&sent.id, &started.id)
            .map_err(|error| refused(kind, failed(error).message))?;
        self.jobs().recorded_sent(project_id);
        log_info!(
            "the request {} of Chat was sent to the job {}",
            sent.id,
            started.id
        );
        Ok(AgentSent {
            request: sent.id,
            title: sent.title,
            job: started,
            kind,
        })
    }

    /// `job.ENDED` of `job_id`: the requests it received that no Proposal consumed go back to
    /// `Writing`. Under the project lock. Returns their ids.
    pub fn job_ended_locked(
        &self,
        project_id: &str,
        root: &Path,
        job_id: &str,
    ) -> Result<Vec<String>, RequestError> {
        let returned = RequestStore::new(root).job_ended(job_id, false)?;
        self.mark_returned(project_id, &returned);
        Ok(returned)
    }
}

/// `proposal.SUBMITTED` of a Proposal bound to `request`: the first one consumes the request. Under
/// the project lock, at the hand-over.
pub fn proposal_submitted(root: &Path, request: &str) {
    match RequestStore::new(root).consume(request) {
        Ok(true) => log_info!("the request {request} was consumed by a Proposal"),
        Ok(false) => {}
        Err(error) => log_warn!("the request {request} could not be recorded as consumed: {error}"),
    }
}

/// Tells the editor of a project that its requests changed, and the Jobs list, whose rows say
/// whether the request of a job is still sent to it.
pub fn emit_changed(app: &AppHandle, project_id: &str) {
    let event = RequestsEvent {
        project_id: project_id.to_owned(),
    };
    if let Err(error) = app.emit(EventName::RequestsChanged.as_str(), &event) {
        log_warn!("the editor of the requests was not told about a change: {error}");
    }
    if let Some(jobs) = app.try_state::<Arc<Jobs>>() {
        jobs.rows_changed(project_id);
    }
}

/// Whether a batch of file changes touches what the requests are made of: `.weaver/requests/`.
pub fn touches_requests(root: &Path, batch: &Rebuild) -> bool {
    match batch {
        Rebuild::All => true,
        Rebuild::Paths(paths) => {
            let requests = weaver_core::project::layout::weaver_dir(root)
                .join(weaver_core::project::layout::REQUESTS_DIR);
            paths.iter().any(|path| path.starts_with(&requests))
        }
    }
}

/// The drafts of a project as a Board built now would show them. The ghosts and the drafts are
/// read under the gate that keeps a request being sent out of one of them, and the drafts are
/// remembered under it too; `true` when they differ from the ones the last Board showed.
fn remember_now(
    service: &RequestsService,
    work: &dyn AgentWork,
    project_id: &str,
    root: &Path,
) -> Result<bool, RequestError> {
    let _board = service.board_read();
    let ghost_jobs = work
        .ghosts(project_id)
        .into_iter()
        .map(|ghost| ghost.job_id)
        .collect();
    let drafts = service.drafts(project_id, &RequestStore::new(root), &ghost_jobs)?;
    Ok(service.remember_drafts(project_id, drafts))
}

/// Sends the Board of a project again (its slice only) when the drafts it shows are no longer the
/// ones the last Board showed: a request appeared, vanished, came back, or changed its title or
/// whether it is empty. The sent requests whose file is gone are forgotten first. A save that
/// changes none of these touches nothing (the editor saves about every 500 ms).
async fn refresh_board(app: &AppHandle, project_id: &str, root: &Path) {
    let state = app.state::<AppState>();
    let service = app.state::<Arc<RequestsService>>().inner().clone();
    let queue_root = root.to_path_buf();
    let reconciled = state
        .mutations
        .run(project_id, move || {
            RequestStore::new(&queue_root)
                .reconcile()
                .map(|_| ())
                .map_err(failed)
        })
        .await;
    if let Err(error) = reconciled {
        log_warn!("the requests of a project were not reconciled: {error}");
    }
    let (work, project, root) = (
        state.agent_work.get(),
        project_id.to_owned(),
        root.to_path_buf(),
    );
    let changed = blocking::disk(move || remember_now(&service, work.as_ref(), &project, &root))
        .await
        .map_err(|error| error.message)
        .and_then(|outcome| outcome.map_err(|error| error.to_string()));
    match changed {
        // A project whose window closed meanwhile has no picture: nothing to tell.
        Ok(true) => {
            let _ = crate::board::publish_agent_change(app, &state.pictures, project_id);
        }
        Ok(false) => {}
        Err(error) => log_warn!("the drafts of a project were not read: {error}"),
    }
}

/// A batch of file changes of a project, whoever made them (the editor, another editor, the
/// command line, an agent): the sent requests whose file is gone are forgotten
/// (`workspace.REQUEST_DELETED`), the editor reads again and the Board is sent again when its
/// drafts changed ([`refresh_board`]).
pub fn files_changed(app: &AppHandle, project_id: &str, root: &Path, batch: &Rebuild) {
    if !touches_requests(root, batch) {
        return;
    }
    let (app, project, root) = (app.clone(), project_id.to_owned(), root.to_path_buf());
    tauri::async_runtime::spawn(async move {
        refresh_board(&app, &project, &root).await;
        emit_changed(&app, &project);
    });
}

/// A project window opened: the drafts its Board shows are the ones to compare the next changes of
/// the requests with.
pub fn project_opened(app: &AppHandle, project_id: &str, root: &Path) {
    let (app, project, root) = (app.clone(), project_id.to_owned(), root.to_path_buf());
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        let service = app.state::<Arc<RequestsService>>().inner().clone();
        let work = state.agent_work.get();
        match blocking::disk(move || remember_now(&service, work.as_ref(), &project, &root)).await {
            Ok(Ok(_)) => {}
            Ok(Err(error)) => log_warn!("the drafts of a project were not read: {error}"),
            Err(error) => log_warn!("the drafts of a project were not read: {}", error.message),
        }
    });
}

/// A project window closed: its drafts are forgotten.
pub fn project_closed(app: &AppHandle, project_id: &str) {
    if let Some(service) = app.try_state::<Arc<RequestsService>>() {
        service.forget_drafts(project_id);
    }
}

/// The first Board of a window (`requirements/tech#design-request`, **Return on opening**): the
/// requests whose job ended while the app was closed go back to `Writing`, in the project's
/// mutation queue, before the Board is worked out, so that their cards show from the start. Later
/// Boards of the same window do nothing here.
pub async fn bring_up_to_date(
    state: &AppState,
    service: &Arc<RequestsService>,
    project_id: &str,
    root: &Path,
) {
    if service.is_opened(project_id) {
        return;
    }
    // Two first Boards at the same time bring the requests up to date once.
    let _opening = service.opening.lock().await;
    if service.is_opened(project_id) {
        return;
    }
    let (service_in, project, root) = (service.clone(), project_id.to_owned(), root.to_path_buf());
    let outcome = state
        .mutations
        .run(project_id, move || {
            service_in
                .bring_up_to_date_locked(&project, &root)
                .map_err(failed)
        })
        .await;
    if let Err(error) = outcome {
        log_warn!("the requests of a project were not brought up to date: {error}");
    }
    service
        .opened
        .lock()
        .expect("opened projects poisoned")
        .insert(project_id.to_owned());
}

/// Asks the editor of a project window to save what is pending and waits (at most
/// [`FLUSH_LIMIT`]) until it says it did. Called before a window closes and when the app quits.
pub async fn flush_window(app: &AppHandle, project_id: &str) {
    let Some(service) = app.try_state::<Arc<RequestsService>>() else {
        return;
    };
    let (answer, receiver) = tokio::sync::oneshot::channel();
    service
        .flushing
        .lock()
        .expect("flushing poisoned")
        .insert(project_id.to_owned(), answer);
    let event = RequestsEvent {
        project_id: project_id.to_owned(),
    };
    if app
        .emit_to(
            label_of(project_id),
            EventName::RequestsFlush.as_str(),
            &event,
        )
        .is_ok()
    {
        let _ = tokio::time::timeout(FLUSH_LIMIT, receiver).await;
    }
    service
        .flushing
        .lock()
        .expect("flushing poisoned")
        .remove(project_id);
}

/// `weaver job new design` from the working General.
struct GeneralSender(Arc<RequestsService>);

impl RequestSender for GeneralSender {
    fn send(&self, project_id: &str, root: &Path, request: &str) -> Result<String, String> {
        match self.0.send_locked(project_id, root, request) {
            Ok(Some(job)) => Ok(job),
            Ok(None) => Err("The request is empty: write it first.".to_owned()),
            Err(error) => Err(error.message),
        }
    }
}

/// The local time of the computer, which names a request the agent of Chat sends.
fn local_now() -> LocalTime {
    use chrono::{Datelike, Timelike};
    let now = chrono::Local::now();
    LocalTime {
        year: u16::try_from(now.year()).unwrap_or(0),
        month: now.month() as u8,
        day: now.day() as u8,
        hour: now.hour() as u8,
        minute: now.minute() as u8,
        // A leap second reads as 60.
        second: now.second().min(59) as u8,
    }
}

/// `weaver request new` from the Chat: the request is written and sent as the developer's Send
/// does, and the Chat is told which job started, or why none did, as for any job it asks for.
struct RequestCli {
    app: AppHandle,
    jobs: Arc<Jobs>,
    service: Arc<RequestsService>,
}

impl CommandExtension for RequestCli {
    fn run(&self, call: &Call) -> Option<Result<CommandReply, Failure>> {
        (call.command.object == "request" && call.command.verb == "new").then(|| self.send(call))
    }
}

impl RequestCli {
    fn send(&self, call: &Call) -> Result<CommandReply, Failure> {
        let project_id = project_hash_of_canonical(call.root);
        let text = call
            .input
            .get("text")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let outcome =
            self.service
                .agent_send_locked(&project_id, call.root, call.image, text, &local_now());
        if call.caller.general.is_some() {
            if let Ok(sent) = &outcome {
                self.jobs.mark_by_chat(&sent.job.id);
            }
        }
        let general = self
            .jobs
            .hooks
            .general
            .read()
            .expect("hooks poisoned")
            .clone();
        if let (Some(general), Some(token)) = (general, call.caller.general.as_deref()) {
            match &outcome {
                Ok(sent) => general.job_requested(
                    call.root,
                    token,
                    sent.kind,
                    Ok(&[(sent.job.id.clone(), sent.job.name.clone())]),
                ),
                Err(refusal) => {
                    general.job_requested(call.root, token, refusal.kind, Err(refusal.reason()))
                }
            }
        }
        emit_changed(&self.app, &project_id);
        let sent = outcome.map_err(|refusal| Failure::refused(refusal.message))?;
        Ok(CommandReply {
            markdown: format!(
                "Sent the request `{}` and started the {} job `{}`. The job writes the Proposal and asks the developer its own questions; the agent does not wait for it. Tell the developer which job started.\n",
                sent.request, sent.kind, sent.job.id
            ),
            json: json!({
                "request": sent.request,
                "job": sent.job.id,
                "kind": sent.kind,
                "title": sent.title,
            }),
            exit: 0,
            note: None,
        })
    }
}

/// `job.ENDED`: the requests the job received go back to `Writing` unless a Proposal consumed
/// them.
struct JobEnd {
    app: AppHandle,
    service: Arc<RequestsService>,
}

impl JobObserver for JobEnd {
    fn job_ended(&self, job: &JobInfo) {
        if !matches!(
            job.kind,
            weaver_core::jobs::JobKind::Setup | weaver_core::jobs::JobKind::Design
        ) {
            return;
        }
        let (app, service) = (self.app.clone(), self.service.clone());
        let (project, root, job_id) = (job.project_id.clone(), job.root.clone(), job.id.clone());
        let board_root = root.clone();
        tauri::async_runtime::spawn(async move {
            let state = app.state::<AppState>();
            let (service_in, project_in) = (service.clone(), project.clone());
            let outcome = state
                .mutations
                .run(&project, move || {
                    service_in
                        .job_ended_locked(&project_in, &root, &job_id)
                        .map_err(failed)
                })
                .await;
            if let Err(error) = outcome {
                log_warn!("the requests of an ended job were not updated: {error}");
            }
            emit_changed(&app, &project);
            refresh_board(&app, &project, &board_root).await;
        });
    }
}

/// Creates the requests, plugs them into the jobs and manages them. Call right after
/// [`crate::jobs::install`].
pub fn install(app: &AppHandle, jobs: &Arc<Jobs>) -> Arc<RequestsService> {
    let service = Arc::new(RequestsService::new(Arc::new(AppJobs(jobs.clone()))));
    app.manage(service.clone());
    jobs.set_request_sender(Arc::new(GeneralSender(service.clone())));
    jobs.add_extension(Arc::new(RequestCli {
        app: app.clone(),
        jobs: jobs.clone(),
        service: service.clone(),
    }));
    jobs.observe(Arc::new(JobEnd {
        app: app.clone(),
        service: service.clone(),
    }));
    service
}

// --- commands -------------------------------------------------------------------------------------

/// The root of an open project.
fn root_of(state: &AppState, project_id: &str) -> Result<std::path::PathBuf, AppError> {
    Ok(state.pictures.get(project_id)?.root)
}

/// The text and standing of one request.
#[tauri::command]
pub async fn request_read(
    state: State<'_, AppState>,
    service: State<'_, Arc<RequestsService>>,
    project_id: String,
    request_id: String,
) -> Result<RequestView, AppError> {
    let picture = state.pictures.get(&project_id)?;
    let model_empty = model_is_empty(picture.image.model());
    let service = service.inner().clone();
    blocking::disk(move || {
        service
            .view(&project_id, &picture.root, &request_id, model_empty)
            .map_err(failed)
    })
    .await?
}

/// `developer.NEW_REQUEST`: an empty request named after `local_time` (`YYYY-MM-DDTHH:MM:SS`, the
/// local time of the developer's computer); returns its id.
#[tauri::command]
pub async fn request_new(
    state: State<'_, AppState>,
    app: AppHandle,
    project_id: String,
    local_time: String,
) -> Result<String, AppError> {
    let time = LocalTime::parse(&local_time).ok_or_else(|| {
        AppError::new(
            ErrorCode::InvalidInput,
            "the local time is not `YYYY-MM-DDTHH:MM:SS`",
        )
    })?;
    let root = root_of(&state, &project_id)?;
    let board_root = root.clone();
    let id = state
        .mutations
        .run(&project_id, move || {
            RequestStore::new(&root).create(&time).map_err(failed)
        })
        .await?;
    refresh_board(&app, &project_id, &board_root).await;
    Ok(id)
}

/// Saves the text of a request being written. Starts no process.
#[tauri::command]
pub async fn request_save(
    state: State<'_, AppState>,
    project_id: String,
    request_id: String,
    text: String,
) -> Result<(), AppError> {
    let root = root_of(&state, &project_id)?;
    state
        .mutations
        .run(&project_id, move || {
            RequestStore::new(&root)
                .save(&request_id, &text)
                .map_err(failed)
        })
        .await
}

/// `developer.SEND_REQUEST`: starts a setup job on an empty Model, a design job otherwise; an
/// empty request does nothing.
#[tauri::command]
pub async fn request_send(
    state: State<'_, AppState>,
    service: State<'_, Arc<RequestsService>>,
    app: AppHandle,
    project_id: String,
    request_id: String,
) -> Result<(), AppError> {
    let root = root_of(&state, &project_id)?;
    let board_root = root.clone();
    let service = service.inner().clone();
    let id = project_id.clone();
    state
        .mutations
        .run(&project_id, move || {
            service.send_locked(&id, &root, &request_id).map(|_| ())
        })
        .await?;
    emit_changed(&app, &project_id);
    refresh_board(&app, &project_id, &board_root).await;
    Ok(())
}

/// `developer.DISCARD_REQUEST`: deletes the file and the images of a request being written.
#[tauri::command]
pub async fn request_discard(
    state: State<'_, AppState>,
    service: State<'_, Arc<RequestsService>>,
    app: AppHandle,
    project_id: String,
    request_id: String,
) -> Result<(), AppError> {
    let root = root_of(&state, &project_id)?;
    let board_root = root.clone();
    let service = service.inner().clone();
    let (id, request) = (project_id.clone(), request_id.clone());
    state
        .mutations
        .run(&project_id, move || {
            RequestStore::new(&root).discard(&request).map_err(failed)?;
            service.forget_returned(&id, &request);
            Ok(())
        })
        .await?;
    emit_changed(&app, &project_id);
    refresh_board(&app, &project_id, &board_root).await;
    Ok(())
}

/// `developer.LEAVE_REQUEST` (`closing` false: the developer left the editor) or `window.CLOSED`
/// (`closing` true: the window of the open request is closing): the request is discarded, with its
/// images and without a confirmation, only when it is in writing and its file is empty after
/// trimming (checked here, under the project lock). Anything else is left as it is.
#[tauri::command]
pub async fn request_leave(
    state: State<'_, AppState>,
    service: State<'_, Arc<RequestsService>>,
    app: AppHandle,
    project_id: String,
    request_id: String,
    closing: bool,
) -> Result<(), AppError> {
    let root = root_of(&state, &project_id)?;
    let board_root = root.clone();
    let service = service.inner().clone();
    let (id, request) = (project_id.clone(), request_id.clone());
    let discarded = state
        .mutations
        .run(&project_id, move || {
            let discarded = RequestStore::new(&root)
                .discard_if_empty(&request, closing)
                .map_err(failed)?;
            if discarded {
                service.forget_returned(&id, &request);
            }
            Ok(discarded)
        })
        .await?;
    if discarded {
        log_info!("the empty request {request_id} was discarded on leaving it");
        refresh_board(&app, &project_id, &board_root).await;
    }
    Ok(())
}

/// Saves a pasted image (`data` is its content in base64) in the request's images folder and
/// returns the path to link, relative to the request file.
#[tauri::command]
pub async fn request_image_add(
    state: State<'_, AppState>,
    project_id: String,
    request_id: String,
    data: String,
) -> Result<String, AppError> {
    let root = root_of(&state, &project_id)?;
    state
        .mutations
        .run(&project_id, move || {
            let bytes =
                base64_decode(&data).ok_or_else(|| failed(ImageError::NotAccepted.into()))?;
            RequestStore::new(&root)
                .add_image(&request_id, &bytes)
                .map_err(failed)
        })
        .await
}

/// Links a dropped image file: one inside the project is linked where it is, any other is copied.
#[tauri::command]
pub async fn request_image_import(
    state: State<'_, AppState>,
    project_id: String,
    request_id: String,
    path: String,
) -> Result<String, AppError> {
    let root = root_of(&state, &project_id)?;
    state
        .mutations
        .run(&project_id, move || {
            RequestStore::new(&root)
                .import_image(&request_id, Path::new(&path))
                .map_err(failed)
        })
        .await
}

/// An image of the request's own images folder as a data URL, for the preview.
#[tauri::command]
pub async fn request_image(
    state: State<'_, AppState>,
    project_id: String,
    request_id: String,
    link: String,
) -> Result<String, AppError> {
    let root = root_of(&state, &project_id)?;
    blocking::disk(move || {
        let (kind, bytes) = RequestStore::new(&root)
            .read_image(&request_id, &link)
            .map_err(failed)?;
        Ok::<_, AppError>(format!(
            "data:{};base64,{}",
            kind.mime(),
            base64_encode(&bytes)
        ))
    })
    .await?
}

/// The editor saved what was pending (the answer to `requests-flush`).
#[tauri::command]
pub async fn requests_flushed(
    service: State<'_, Arc<RequestsService>>,
    project_id: String,
) -> Result<(), AppError> {
    if let Some(waiting) = service
        .flushing
        .lock()
        .expect("flushing poisoned")
        .remove(&project_id)
    {
        let _ = waiting.send(());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    const FEATURE: &str = "# Shopping\n\nBuying things.\n";
    const A: &str = "2026-11-02T09-30-00";

    /// The jobs a request may start, scripted.
    #[derive(Default)]
    struct FakeJobs {
        started: Mutex<Vec<Job>>,
        finished: Mutex<HashSet<String>>,
        refuse: Mutex<Option<String>>,
        /// The assets of the project.
        assets: Mutex<Vec<String>>,
        /// Runs while a job starts.
        on_start: Mutex<Option<Box<dyn Fn() + Send + Sync>>>,
    }

    impl RequestJobs for FakeJobs {
        fn start(&self, _ctx: &Ctx, job: &Job) -> Result<StartedJob, String> {
            if let Some(why) = self.refuse.lock().unwrap().clone() {
                return Err(why);
            }
            if let Some(hook) = self.on_start.lock().unwrap().as_ref() {
                hook();
            }
            let mut started = self.started.lock().unwrap();
            started.push(job.clone());
            Ok(StartedJob {
                id: format!("job{:05}", started.len()),
                name: "Design".to_owned(),
            })
        }

        fn asset_names(&self, _project_id: &str) -> Vec<String> {
            self.assets.lock().unwrap().clone()
        }

        fn life(&self, job_id: &str) -> Option<(String, JobLife)> {
            let finished = self.finished.lock().unwrap().contains(job_id);
            Some((
                "Design".to_owned(),
                if finished {
                    JobLife::Finished
                } else {
                    JobLife::Active
                },
            ))
        }
    }

    struct Fixture {
        project: tempfile::TempDir,
        jobs: Arc<FakeJobs>,
        service: RequestsService,
    }

    fn put(root: &Path, rel: &str, text: &str) {
        let path = root.join(".weaver").join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    fn fixture(with_model: bool) -> Fixture {
        let project = tempfile::tempdir().unwrap();
        put(project.path(), "model/system.md", "# Shop\n\nA shop.\n");
        if with_model {
            put(project.path(), "model/shop/feature.md", FEATURE);
        }
        let jobs = Arc::new(FakeJobs::default());
        Fixture {
            service: RequestsService::new(jobs.clone()),
            jobs,
            project,
        }
    }

    fn written(f: &Fixture, text: &str) -> String {
        let store = RequestStore::new(f.project.path());
        let id = store
            .create(&LocalTime::parse("2026-11-02T09:30:00").unwrap())
            .unwrap();
        store.save(&id, text).unwrap();
        id
    }

    const P: &str = "0123456789abcdef";

    #[test]
    fn sending_on_an_empty_model_starts_a_setup_job() {
        let f = fixture(false);
        let id = written(&f, "# Make a shop\n");
        let job = f
            .service
            .send_locked(P, f.project.path(), &id)
            .unwrap()
            .unwrap();
        assert_eq!(
            *f.jobs.started.lock().unwrap(),
            [Job::Setup {
                request: id.clone(),
                assets: vec![]
            }]
        );
        let view = f.service.view(P, f.project.path(), &id, false).unwrap();
        assert_eq!(
            (
                view.status,
                view.job_id.as_deref(),
                view.job_name.as_deref()
            ),
            (RequestStatus::Sent, Some(job.as_str()), Some("Design"))
        );
    }

    #[test]
    fn sending_on_a_model_with_a_feature_starts_a_design_job() {
        let f = fixture(true);
        let id = written(&f, "Add a cart");
        f.service
            .send_locked(P, f.project.path(), &id)
            .unwrap()
            .unwrap();
        assert_eq!(
            *f.jobs.started.lock().unwrap(),
            [Job::Design {
                request: id,
                assets: vec![]
            }]
        );
    }

    #[test]
    fn the_assets_a_request_tags_when_it_is_sent_go_to_the_job_and_the_file_stays_as_written() {
        let f = fixture(true);
        *f.jobs.assets.lock().unwrap() = vec!["logo.png".to_owned(), "my notes.txt".to_owned()];
        let text = "Use @my notes.txt and @logo.png, also @missing.png and @LOGO.PNG";
        let id = written(&f, text);
        f.service
            .send_locked(P, f.project.path(), &id)
            .unwrap()
            .unwrap();
        assert_eq!(
            *f.jobs.started.lock().unwrap(),
            [Job::Design {
                request: id.clone(),
                assets: vec!["my notes.txt".to_owned(), "logo.png".to_owned()]
            }]
        );
        assert_eq!(RequestStore::new(f.project.path()).read(&id).unwrap(), text);
    }

    #[test]
    fn sending_something_empty_starts_nothing() {
        let f = fixture(true);
        let id = written(&f, "  \n\t\n");
        assert_eq!(
            f.service.send_locked(P, f.project.path(), &id).unwrap(),
            None
        );
        assert!(f.jobs.started.lock().unwrap().is_empty());
        assert_eq!(
            f.service.view(P, f.project.path(), &id, false).unwrap().status,
            RequestStatus::Writing
        );
    }

    #[test]
    fn a_request_made_only_of_an_image_is_sent() {
        let f = fixture(true);
        let id = written(&f, "![](x.png)\n");
        assert!(f
            .service
            .send_locked(P, f.project.path(), &id)
            .unwrap()
            .is_some());
    }

    #[test]
    fn a_sent_request_cannot_be_sent_again_nor_changed() {
        let f = fixture(true);
        let id = written(&f, "Add a cart");
        f.service.send_locked(P, f.project.path(), &id).unwrap();
        let error = f.service.send_locked(P, f.project.path(), &id).unwrap_err();
        assert_eq!(error.code, ErrorCode::Refused);
        assert_eq!(f.jobs.started.lock().unwrap().len(), 1);
        assert!(RequestStore::new(f.project.path())
            .save(&id, "other")
            .is_err());
    }

    #[test]
    fn a_refused_job_leaves_the_request_being_written() {
        let f = fixture(true);
        let id = written(&f, "Add a cart");
        *f.jobs.refuse.lock().unwrap() =
            Some("Could not send the request: the agent is busy.".to_owned());
        let error = f.service.send_locked(P, f.project.path(), &id).unwrap_err();
        assert_eq!(
            (error.code, error.message.as_str()),
            (
                ErrorCode::Refused,
                "Could not send the request: the agent is busy."
            )
        );
        assert_eq!(
            f.service.view(P, f.project.path(), &id, false).unwrap().status,
            RequestStatus::Writing
        );
        assert!(f.service.send_locked(P, f.project.path(), &id).is_err());
        *f.jobs.refuse.lock().unwrap() = None;
        assert!(f
            .service
            .send_locked(P, f.project.path(), &id)
            .unwrap()
            .is_some());
    }

    #[test]
    fn a_job_that_ends_without_a_proposal_returns_the_request_to_writing_and_says_so() {
        let f = fixture(true);
        let id = written(&f, "Add a cart");
        let job = f
            .service
            .send_locked(P, f.project.path(), &id)
            .unwrap()
            .unwrap();
        assert_eq!(
            f.service
                .job_ended_locked(P, f.project.path(), &job)
                .unwrap(),
            std::slice::from_ref(&id)
        );
        let view = f.service.view(P, f.project.path(), &id, false).unwrap();
        assert_eq!((view.status, view.returned), (RequestStatus::Writing, true));
        // It can be changed and sent again; the banner goes away once it is sent.
        RequestStore::new(f.project.path())
            .save(&id, "Add a cart and a wishlist")
            .unwrap();
        f.service
            .send_locked(P, f.project.path(), &id)
            .unwrap()
            .unwrap();
        assert!(!f.service.view(P, f.project.path(), &id, false).unwrap().returned);
    }

    #[test]
    fn a_job_that_finished_is_a_draft_that_came_back_as_soon_as_its_ghost_is_gone() {
        let f = fixture(true);
        let id = written(&f, "Add a cart");
        let job = f
            .service
            .send_locked(P, f.project.path(), &id)
            .unwrap()
            .unwrap();
        let store = RequestStore::new(f.project.path());
        let no_ghost = HashSet::new();
        // While the job works the request is no draft.
        assert!(f.service.drafts(P, &store, &no_ghost).unwrap().is_empty());
        f.jobs.finished.lock().unwrap().insert(job.clone());
        // The moment the job is finished the Board shows the draft, before anything is written...
        let drafts = f.service.drafts(P, &store, &no_ghost).unwrap();
        assert_eq!(
            drafts
                .iter()
                .map(|d| (d.id.as_str(), d.returned))
                .collect::<Vec<_>>(),
            [(id.as_str(), true)]
        );
        // ... but never together with the ghost of its job.
        let ghost: HashSet<String> = [job].into();
        assert!(f.service.drafts(P, &store, &ghost).unwrap().is_empty());
        // The first Board of a window writes it: back to Writing, as a request that came back.
        f.service
            .bring_up_to_date_locked(P, f.project.path())
            .unwrap();
        assert_eq!(store.state_of(&id).unwrap(), RequestState::Writing);
        let view = f.service.view(P, f.project.path(), &id, false).unwrap();
        assert_eq!((view.status, view.returned), (RequestStatus::Writing, true));
        assert_eq!(f.service.drafts(P, &store, &ghost).unwrap().len(), 1);
    }

    #[test]
    fn the_first_proposal_handed_over_consumes_the_request_for_good() {
        let f = fixture(true);
        let id = written(&f, "Add a cart");
        let job = f
            .service
            .send_locked(P, f.project.path(), &id)
            .unwrap()
            .unwrap();
        proposal_submitted(f.project.path(), &id);
        let store = RequestStore::new(f.project.path());
        assert_eq!(store.state().unwrap().consumed, std::slice::from_ref(&id));
        assert!(!f
            .project
            .path()
            .join(format!(".weaver/requests/{id}.md"))
            .exists());
        // Its job ending afterwards returns nothing, and a file that reappears is ignored.
        assert!(f
            .service
            .job_ended_locked(P, f.project.path(), &job)
            .unwrap()
            .is_empty());
        put(f.project.path(), &format!("requests/{id}.md"), "back again");
        assert!(f
            .service
            .drafts(P, &RequestStore::new(f.project.path()), &HashSet::new())
            .unwrap()
            .is_empty());
        assert!(f.service.view(P, f.project.path(), &id, false).is_err());
        // A second Proposal changes nothing.
        proposal_submitted(f.project.path(), &id);
        assert_eq!(store.state().unwrap().consumed, [id]);
    }

    #[test]
    fn a_sent_request_deleted_from_outside_is_forgotten_when_the_window_opens() {
        let f = fixture(true);
        let id = written(&f, "Add a cart");
        f.service.send_locked(P, f.project.path(), &id).unwrap();
        fs::remove_file(f.project.path().join(format!(".weaver/requests/{id}.md"))).unwrap();
        f.service
            .bring_up_to_date_locked(P, f.project.path())
            .unwrap();
        assert!(RequestStore::new(f.project.path())
            .state()
            .unwrap()
            .sent
            .is_empty());
    }

    #[test]
    fn the_drafts_are_oldest_first_with_title_creation_time_and_emptiness() {
        let f = fixture(false);
        let store = RequestStore::new(f.project.path());
        let older = store
            .create(&LocalTime::parse("2026-11-02T09:00:00").unwrap())
            .unwrap();
        let newer = store
            .create(&LocalTime::parse("2026-11-02T10:00:00").unwrap())
            .unwrap();
        store.save(&older, "# Older\n").unwrap();
        let drafts = f.service.drafts(P, &store, &HashSet::new()).unwrap();
        assert_eq!(
            drafts,
            [
                Draft {
                    id: older,
                    title: "Older".to_owned(),
                    created: "2026-11-02T09:00:00".to_owned(),
                    empty: false,
                    returned: false,
                },
                Draft {
                    id: newer,
                    title: "Untitled request".to_owned(),
                    created: "2026-11-02T10:00:00".to_owned(),
                    empty: true,
                    returned: false,
                },
            ]
        );
    }

    #[test]
    fn no_board_can_be_read_between_the_start_of_the_job_and_the_record_of_the_request() {
        let f = fixture(true);
        let service = Arc::new(RequestsService::new(f.jobs.clone()));
        let probe = service.clone();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let sink = seen.clone();
        *f.jobs.on_start.lock().unwrap() = Some(Box::new(move || {
            sink.lock().unwrap().push(probe.board_gate.try_read().is_err());
        }));
        let id = written(&f, "Add a cart");
        service
            .send_locked(P, f.project.path(), &id)
            .unwrap()
            .unwrap();
        assert_eq!(*seen.lock().unwrap(), [true], "closed while the job starts");
        assert!(service.board_gate.try_read().is_ok(), "open again afterwards");
        // The request sent by the agent of Chat is under the gate from its creation too.
        seen.lock().unwrap().clear();
        let image = ProjectImage::load(f.project.path());
        service
            .agent_send_locked(
                P,
                f.project.path(),
                &image,
                "# From chat\n",
                &LocalTime::parse("2026-11-02T10:00:00").unwrap(),
            )
            .unwrap();
        assert_eq!(*seen.lock().unwrap(), [true]);
        assert!(service.board_gate.try_read().is_ok());
    }

    #[test]
    fn the_general_sends_a_request_through_the_same_rules() {
        let f = fixture(true);
        let sender = GeneralSender(Arc::new(RequestsService::new(f.jobs.clone())));
        let id = written(&f, "Add a cart");
        let job = sender.send(P, f.project.path(), &id).unwrap();
        assert!(job.starts_with("job"));
        assert!(
            sender.send(P, f.project.path(), &id).is_err(),
            "a sent request is not sent again"
        );
        let empty = written(&f, "");
        assert!(sender
            .send(P, f.project.path(), &empty)
            .unwrap_err()
            .contains("empty"));
        assert!(sender.send(P, f.project.path(), "../x").is_err());
    }

    fn at(text: &str) -> LocalTime {
        LocalTime::parse(text).unwrap()
    }

    #[test]
    fn the_agent_of_chat_sends_a_request_that_is_one_like_the_others() {
        let f = fixture(true);
        let root = f.project.path();
        let image = ProjectImage::load(root);
        *f.jobs.assets.lock().unwrap() = vec!["logo.png".to_owned()];
        let sent = f
            .service
            .agent_send_locked(
                P,
                root,
                &image,
                "# Carts\r\n\r\nAdd carts, see @logo.png",
                &at("2026-11-02T09:30:00"),
            )
            .unwrap();
        assert_eq!(sent.request, A);
        assert_eq!(sent.title, "Carts");
        assert_eq!(sent.kind, "design");
        assert!(sent.job.id.starts_with("job"));
        assert_eq!(
            f.jobs.started.lock().unwrap().as_slice(),
            [Job::Design {
                request: A.to_owned(),
                assets: vec!["logo.png".to_owned()],
            }]
        );
        // The file is the text as given, with LF and one final newline, and no images folder.
        let folder = root.join(".weaver/requests");
        assert_eq!(
            fs::read_to_string(folder.join(format!("{A}.md"))).unwrap(),
            "# Carts\n\nAdd carts, see @logo.png\n"
        );
        assert!(!folder.join(format!("{A}.images")).exists());
        // It is recorded as sent with its job, and the editor shows it as any sent request.
        let store = RequestStore::new(root);
        assert_eq!(
            store.state().unwrap().sent.get(A).map(String::as_str),
            Some(sent.job.id.as_str())
        );
        let view = f.service.view(P, root, A, false).unwrap();
        assert_eq!(view.status, RequestStatus::Sent);
        assert_eq!(view.job_id.as_deref(), Some(sent.job.id.as_str()));
        // The developer cannot send it again.
        assert!(f.service.send_locked(P, root, A).is_err());
    }

    #[test]
    fn the_agent_of_chat_starts_a_setup_job_while_the_model_is_empty() {
        let f = fixture(false);
        let root = f.project.path();
        let sent = f
            .service
            .agent_send_locked(
                P,
                root,
                &ProjectImage::load(root),
                "Build a shop",
                &at("2026-11-02T09:30:00"),
            )
            .unwrap();
        assert_eq!(sent.kind, "setup");
        assert_eq!(
            f.jobs.started.lock().unwrap().as_slice(),
            [Job::Setup {
                request: A.to_owned(),
                assets: Vec::new(),
            }]
        );
    }

    #[test]
    fn a_refused_job_of_the_agent_of_chat_leaves_no_request_and_no_record() {
        let f = fixture(true);
        let root = f.project.path();
        *f.jobs.refuse.lock().unwrap() =
            Some("Could not send the request: omp is not ready.".to_owned());
        let refusal = f
            .service
            .agent_send_locked(
                P,
                root,
                &ProjectImage::load(root),
                "Add carts",
                &at("2026-11-02T09:30:00"),
            )
            .unwrap_err();
        assert_eq!(refusal.kind, "design");
        assert_eq!(refusal.reason(), "omp is not ready");
        let folder = root.join(".weaver/requests");
        assert!(!folder.join(format!("{A}.md")).exists());
        assert!(!folder.join(".state.json").exists());
        assert!(RequestStore::new(root).list().unwrap().is_empty());
        assert!(f.jobs.started.lock().unwrap().is_empty());
    }

    #[test]
    fn a_text_with_nothing_in_it_starts_nothing_and_writes_nothing() {
        let f = fixture(true);
        let root = f.project.path();
        let refusal = f
            .service
            .agent_send_locked(
                P,
                root,
                &ProjectImage::load(root),
                "  \n ",
                &at("2026-11-02T09:30:00"),
            )
            .unwrap_err();
        assert!(refusal.message.contains("empty"));
        assert!(!root.join(".weaver/requests").exists());
        assert!(f.jobs.started.lock().unwrap().is_empty());
    }

    #[test]
    fn two_requests_of_the_same_second_get_different_ids() {
        let f = fixture(true);
        let root = f.project.path();
        let now = at("2026-11-02T09:30:00");
        let first = f
            .service
            .agent_send_locked(P, root, &ProjectImage::load(root), "One", &now)
            .unwrap();
        let second = f
            .service
            .agent_send_locked(P, root, &ProjectImage::load(root), "Two", &now)
            .unwrap();
        assert_eq!(first.request, A);
        assert_eq!(second.request, format!("{A}-2"));
    }

    #[test]
    fn the_local_time_names_a_valid_request() {
        let now = local_now();
        assert!(LocalTime::parse(&now.iso()).is_some());
        assert!(now.year >= 2026);
    }

    #[test]
    fn a_save_touches_the_board_only_when_it_changes_the_title_or_the_emptiness() {
        // `refresh_board`, called for every batch of file changes of `.weaver/requests/`, touches
        // the Board exactly when `remember_now` says the drafts differ from the last Board.
        let f = fixture(false);
        let root = f.project.path();
        let store = RequestStore::new(root);
        let work = crate::agent_work::NoAgentWork;
        let id = store
            .create(&LocalTime::parse("2026-11-02T09:30:00").unwrap())
            .unwrap();
        assert!(remember_now(&f.service, &work, P, root).unwrap(), "a new request");
        store.save(&id, "# Cart").unwrap();
        assert!(remember_now(&f.service, &work, P, root).unwrap(), "title and emptiness");
        store.save(&id, "# Cart\n\nmore").unwrap();
        assert!(!remember_now(&f.service, &work, P, root).unwrap(), "same title, still not empty");
        store.save(&id, "# Cart\n\nmore and more").unwrap();
        assert!(!remember_now(&f.service, &work, P, root).unwrap());
        store.save(&id, "").unwrap();
        assert!(remember_now(&f.service, &work, P, root).unwrap(), "empty again");
        store.discard(&id).unwrap();
        assert!(remember_now(&f.service, &work, P, root).unwrap(), "discarded");
    }

    #[test]
    fn the_board_is_touched_only_when_a_draft_appears_vanishes_or_changes_title_or_emptiness() {
        let f = fixture(false);
        let root = f.project.path();
        let store = RequestStore::new(root);
        let read = || f.service.drafts(P, &store, &HashSet::new()).unwrap();
        let id = store
            .create(&LocalTime::parse("2026-11-02T09:30:00").unwrap())
            .unwrap();
        // Nothing known yet: the first look tells the Board.
        assert!(f.service.remember_drafts(P, read()));
        assert!(!f.service.remember_drafts(P, read()));
        // The first text makes it not empty and gives it a title.
        store.save(&id, "# Cart\n\nFirst").unwrap();
        assert!(f.service.remember_drafts(P, read()));
        // Typing below the title changes neither the title nor the emptiness.
        store.save(&id, "# Cart\n\nFirst and second").unwrap();
        assert!(!f.service.remember_drafts(P, read()));
        store.save(&id, "# Basket\n\nFirst and second").unwrap();
        assert!(f.service.remember_drafts(P, read()));
        // Another project has its own memory.
        assert!(f.service.remember_drafts("fedcba9876543210", read()));
        // A request that is sent is no draft any more; one that vanishes neither.
        store.record_sent(&id, "job00001").unwrap();
        assert!(f.service.remember_drafts(P, read()));
        let other = written(&f, "Another");
        assert!(f.service.remember_drafts(P, read()));
        store.discard(&other).unwrap();
        assert!(f.service.remember_drafts(P, read()));
        f.service.forget_drafts(P);
        assert!(f.service.remember_drafts(P, read()));
    }

    #[test]
    fn a_batch_touches_the_requests_only_when_it_names_their_folder() {
        let root = Path::new("/p");
        let inside = Rebuild::Paths(vec![
            "/p/.weaver/model/system.md".into(),
            "/p/.weaver/requests/.state.json".into(),
        ]);
        assert!(touches_requests(root, &inside));
        assert!(touches_requests(root, &Rebuild::All));
        assert!(!touches_requests(
            root,
            &Rebuild::Paths(vec!["/p/.weaver/tasks/a.md".into()])
        ));
        assert!(!touches_requests(
            root,
            &Rebuild::Paths(vec!["/p/.weaver/requests-old/a.md".into()])
        ));
        // The Proposals do not hold a request: none of their files counts.
        for other in [
            "/p/.weaver/proposals/pppppppp-cart/proposal.md",
            "/p/.weaver/proposals/pppppppp-cart",
            "/p/.weaver/proposals",
        ] {
            assert!(
                !touches_requests(root, &Rebuild::Paths(vec![other.into()])),
                "{other}"
            );
        }
    }

    #[tokio::test]
    async fn the_commands_save_send_and_discard_in_the_projects_queue() {
        let base = tempfile::tempdir().unwrap();
        let f = fixture(true);
        let state = AppState::new(weaver_core::paths::AppDirs::from_bases(
            base.path(),
            base.path(),
            None,
        ));
        let id = state
            .pictures
            .open(f.project.path().to_path_buf())
            .await
            .unwrap();
        let root = root_of(&state, &id).unwrap();
        let created = {
            let root = root.clone();
            state
                .mutations
                .run(&id, move || {
                    RequestStore::new(&root)
                        .create(&LocalTime::parse("2026-11-02T09:30:00").unwrap())
                        .map_err(failed)
                })
                .await
                .unwrap()
        };
        assert_eq!(created, A);
        state
            .mutations
            .run(&id, {
                let (root, created) = (root.clone(), created.clone());
                move || {
                    RequestStore::new(&root)
                        .save(&created, "# Cart\n")
                        .map_err(failed)
                }
            })
            .await
            .unwrap();
        let service = Arc::new(RequestsService::new(f.jobs.clone()));
        let sent = {
            let (service, id, root, created) =
                (service.clone(), id.clone(), root.clone(), created.clone());
            state
                .mutations
                .run(&id.clone(), move || {
                    service.send_locked(&id, &root, &created)
                })
                .await
                .unwrap()
        };
        assert!(sent.is_some());
        assert_eq!(f.jobs.started.lock().unwrap().len(), 1);
    }
}

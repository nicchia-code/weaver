//! The app side of the design request (Machine `design-request`): the editor's commands, what
//! `Send` starts, and what the rest of the app tells the requests.
//!
//! The rules and the files are in `weaver_core::project::requests`; this module carries them out
//! in the project's queue and connects them to the rest of the app:
//!
//! * `developer.NEW_REQUEST`, `SEND_REQUEST`, `DISCARD_REQUEST` and every save are commands. A
//!   write runs in the project's mutation queue ([`crate::mutations`]); a save starts no process.
//!   `Send` plans the job, starts it through [`RequestJobs`] (a setup job when the Model is empty,
//!   otherwise a design job) and then records the job in `.state.json`, all under the project lock.
//! * `proposal.SUBMITTED`: [`proposal_submitted`], called by the hand-over of a Proposal bound to
//!   the request (see [`crate::proposals::answer_cli`]).
//! * `job.ENDED`: [`JobEnd`], installed with [`install`]. A request whose job ended without a
//!   Proposal bound to it goes back to `Writing`; the editor says so with `returned`.
//! * `workspace.REQUEST_DELETED`: [`files_changed`], called with every batch of file changes of a
//!   project; a sent request whose file is gone is forgotten and the editor is told.
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
use tauri::{AppHandle, Emitter, Manager, State};
use ts_rs::TS;
use weaver_core::assets::tags;
use weaver_core::job_instruction::Job;
use weaver_core::project::image::ProjectImage;
use weaver_core::project::requests::image::{base64_decode, base64_encode, ImageError};
use weaver_core::project::requests::{
    model_is_empty, LocalTime, RequestEntry, RequestError, RequestState, RequestStore, SendPlan,
};

use crate::blocking;
use crate::error::{AppError, ErrorCode};
use crate::events::EventName;
use crate::jobs::starts::Ctx;
use crate::jobs::wire::{JobInfo, JobObserver};
use crate::jobs::{authority::RequestSender, Jobs};
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
}

/// Whether a request is being written, was sent or is held by Chat.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "kebab-case")]
pub enum RequestStatus {
    Writing,
    Sent,
    /// The agent of Chat is writing a Proposal bound to it: it can be read, not changed, sent or
    /// discarded.
    HeldByChat,
}

/// A row of the list of open requests.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RequestSummary {
    pub id: String,
    /// The first heading, else the first line, else the id.
    pub title: String,
    /// The local time of creation, `YYYY-MM-DDTHH:MM:SS`.
    pub created: String,
    pub status: RequestStatus,
    /// The job that received it, when it was sent.
    pub job_id: Option<String>,
    pub job_name: Option<String>,
    /// It went back to `Writing` because its job ended without a Proposal.
    pub returned: bool,
}

/// The open requests of a project, newest first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RequestsSnapshot {
    pub project_id: String,
    pub requests: Vec<RequestSummary>,
    /// The Model has no Feature and no Actor: a request sent now is a setup request.
    pub model_empty: bool,
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
        RequestState::HeldByChat => RequestStatus::HeldByChat,
        _ => RequestStatus::Writing,
    }
}

impl RequestsService {
    pub fn new(jobs: Arc<dyn RequestJobs>) -> Self {
        Self {
            jobs: RwLock::new(jobs),
            returned: Mutex::new(HashSet::new()),
            flushing: Mutex::new(HashMap::new()),
        }
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

    fn summary(&self, project_id: &str, entry: RequestEntry) -> RequestSummary {
        let job_name = entry
            .job_id
            .as_deref()
            .and_then(|job| self.jobs().life(job))
            .map(|(name, _)| name);
        RequestSummary {
            returned: entry.state == RequestState::Writing
                && self.is_returned(project_id, &entry.id),
            status: status_of(entry.state),
            id: entry.id,
            title: entry.title,
            created: entry.created,
            job_id: entry.job_id,
            job_name,
        }
    }

    /// The list of the open requests. Writes only when a job ended or a file vanished since the
    /// last look: call it inside the project's queue.
    pub fn list_locked(
        &self,
        project_id: &str,
        root: &Path,
        model_empty: bool,
    ) -> Result<RequestsSnapshot, RequestError> {
        let store = RequestStore::new(root);
        store.reconcile()?;
        self.return_finished(&store, project_id)?;
        let requests = store
            .list()?
            .into_iter()
            .map(|entry| self.summary(project_id, entry))
            .collect();
        Ok(RequestsSnapshot {
            project_id: project_id.to_owned(),
            requests,
            model_empty,
        })
    }

    /// The text and standing of one open request.
    pub fn view(
        &self,
        project_id: &str,
        root: &Path,
        request: &str,
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
                self.forget_returned(project_id, request);
                log_info!("the request {request} was sent to the job {}", started.id);
                Ok(Some(started.id))
            }
        }
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
/// the project lock, at the hand-over. `from_chat` is whether the Proposal was a Draft the agent of
/// Chat was writing (`chatWriting`) until the hand-over: that Draft held the request.
pub fn proposal_submitted(root: &Path, request: &str, from_chat: bool) {
    match RequestStore::new(root).consume(request, from_chat) {
        Ok(true) => log_info!("the request {request} was consumed by a Proposal"),
        Ok(false) => {}
        Err(error) => log_warn!("the request {request} could not be recorded as consumed: {error}"),
    }
}

/// Tells the editor of a project that its requests changed.
pub fn emit_changed(app: &AppHandle, project_id: &str) {
    let event = RequestsEvent {
        project_id: project_id.to_owned(),
    };
    if let Err(error) = app.emit(EventName::RequestsChanged.as_str(), &event) {
        log_warn!("the editor of the requests was not told about a change: {error}");
    }
}

/// Whether a batch of file changes touches what the requests are made of: `.weaver/requests/`, and
/// the `proposal.md` of the Proposals, because a Draft of Chat that names a request holds it while
/// it exists (so the Draft being created, discarded or handed over changes the request).
pub fn touches_requests(root: &Path, batch: &Rebuild) -> bool {
    match batch {
        Rebuild::All => true,
        Rebuild::Paths(paths) => {
            let weaver = weaver_core::project::layout::weaver_dir(root);
            let requests = weaver.join(weaver_core::project::layout::REQUESTS_DIR);
            let proposals = weaver.join(weaver_core::project::layout::PROPOSALS_DIR);
            paths.iter().any(|path| {
                path.starts_with(&requests)
                    || path.strip_prefix(&proposals).is_ok_and(|inside| {
                        let mut parts = inside.iter();
                        let (_folder, file, rest) = (parts.next(), parts.next(), parts.next());
                        rest.is_none()
                            && file.is_none_or(|file| file == weaver_core::project::layout::PROPOSAL_FILE)
                    })
            })
        }
    }
}

/// A batch of file changes of a project (`workspace.REQUEST_DELETED` among them): the sent requests
/// whose file is gone are forgotten, and the editor reads again.
pub fn files_changed(app: &AppHandle, project_id: &str, root: &Path, batch: &Rebuild) {
    if !touches_requests(root, batch) {
        return;
    }
    let (app, project, root) = (app.clone(), project_id.to_owned(), root.to_path_buf());
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        let store_root = root.clone();
        let outcome = state
            .mutations
            .run(&project, move || {
                RequestStore::new(&store_root).reconcile().map_err(failed)
            })
            .await;
        if let Err(error) = outcome {
            log_warn!("the requests of a project were not reconciled: {error}");
        }
        emit_changed(&app, &project);
    });
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
        });
    }
}

/// Creates the requests, plugs them into the jobs and manages them. Call right after
/// [`crate::jobs::install`].
pub fn install(app: &AppHandle, jobs: &Arc<Jobs>) -> Arc<RequestsService> {
    let service = Arc::new(RequestsService::new(Arc::new(AppJobs(jobs.clone()))));
    app.manage(service.clone());
    jobs.set_request_sender(Arc::new(GeneralSender(service.clone())));
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

/// The open requests of a project and whether its Model is empty.
#[tauri::command]
pub async fn requests_list(
    state: State<'_, AppState>,
    service: State<'_, Arc<RequestsService>>,
    project_id: String,
) -> Result<RequestsSnapshot, AppError> {
    let picture = state.pictures.get(&project_id)?;
    let model_empty = model_is_empty(picture.image.model());
    let service = service.inner().clone();
    let id = project_id.clone();
    state
        .mutations
        .run(&project_id, move || {
            service
                .list_locked(&id, &picture.root, model_empty)
                .map_err(failed)
        })
        .await
}

/// The text and standing of one request.
#[tauri::command]
pub async fn request_read(
    state: State<'_, AppState>,
    service: State<'_, Arc<RequestsService>>,
    project_id: String,
    request_id: String,
) -> Result<RequestView, AppError> {
    let root = root_of(&state, &project_id)?;
    let service = service.inner().clone();
    blocking::disk(move || {
        service
            .view(&project_id, &root, &request_id)
            .map_err(failed)
    })
    .await?
}

/// `developer.NEW_REQUEST`: an empty request named after `local_time` (`YYYY-MM-DDTHH:MM:SS`, the
/// local time of the developer's computer); returns its id.
#[tauri::command]
pub async fn request_new(
    state: State<'_, AppState>,
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
    state
        .mutations
        .run(&project_id, move || {
            RequestStore::new(&root).create(&time).map_err(failed)
        })
        .await
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
    let service = service.inner().clone();
    let id = project_id.clone();
    state
        .mutations
        .run(&project_id, move || {
            service.send_locked(&id, &root, &request_id).map(|_| ())
        })
        .await?;
    emit_changed(&app, &project_id);
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
    }

    impl RequestJobs for FakeJobs {
        fn start(&self, _ctx: &Ctx, job: &Job) -> Result<StartedJob, String> {
            if let Some(why) = self.refuse.lock().unwrap().clone() {
                return Err(why);
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
        let view = f.service.view(P, f.project.path(), &id).unwrap();
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
            f.service.view(P, f.project.path(), &id).unwrap().status,
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

    /// A `proposal.md` of the Draft Chat writes (or of a job when `chat_writing` is `false`) that
    /// names `request`, with `status`.
    fn proposal(f: &Fixture, status: &str, chat_writing: bool, request: &str) {
        let writing = if chat_writing { "chatWriting: true\n" } else { "" };
        put(
            f.project.path(),
            "proposals/pppppppp-cart/proposal.md",
            &format!("---\nstatus: {status}\nkind: change\nrequest: {request}\n{writing}---\n\nBody\n"),
        );
    }

    #[test]
    fn a_request_the_draft_of_chat_names_is_held_and_refuses_save_send_and_discard() {
        let f = fixture(true);
        let id = written(&f, "Add a cart");
        proposal(&f, "draft", true, &id);
        let store = RequestStore::new(f.project.path());

        let view = f.service.view(P, f.project.path(), &id).unwrap();
        assert_eq!(
            (view.status, view.returned, view.text.as_str()),
            (RequestStatus::HeldByChat, false, "Add a cart")
        );
        let listed = f.service.list_locked(P, f.project.path(), false).unwrap();
        assert_eq!(
            listed.requests.iter().map(|r| r.status).collect::<Vec<_>>(),
            [RequestStatus::HeldByChat]
        );

        let error = f.service.send_locked(P, f.project.path(), &id).unwrap_err();
        assert_eq!(error.code, ErrorCode::Refused);
        assert!(f.jobs.started.lock().unwrap().is_empty());
        assert!(matches!(
            store.save(&id, "other"),
            Err(RequestError::HeldByChat)
        ));
        assert!(matches!(store.discard(&id), Err(RequestError::HeldByChat)));
        assert_eq!(store.read(&id).unwrap(), "Add a cart");
        // Nothing was written about it: the record stays what it was.
        assert!(!f.project.path().join(".weaver/requests/.state.json").exists());
    }

    #[test]
    fn a_draft_of_a_job_does_not_hold_the_request() {
        let f = fixture(true);
        let id = written(&f, "Add a cart");
        proposal(&f, "draft", false, &id);
        let view = f.service.view(P, f.project.path(), &id).unwrap();
        assert_eq!(view.status, RequestStatus::Writing);
    }

    #[test]
    fn discarding_the_draft_of_chat_makes_the_request_writable_again() {
        let f = fixture(true);
        let id = written(&f, "Add a cart");
        proposal(&f, "draft", true, &id);
        assert_eq!(
            f.service.view(P, f.project.path(), &id).unwrap().status,
            RequestStatus::HeldByChat
        );

        proposal(&f, "discarded", true, &id);
        let view = f.service.view(P, f.project.path(), &id).unwrap();
        // The job did not end: there is no "job ended" banner.
        assert_eq!((view.status, view.returned), (RequestStatus::Writing, false));
        RequestStore::new(f.project.path())
            .save(&id, "Add a cart and a wishlist")
            .unwrap();
        assert!(f
            .service
            .send_locked(P, f.project.path(), &id)
            .unwrap()
            .is_some());
    }

    #[test]
    fn handing_over_the_draft_of_chat_consumes_the_request_it_held() {
        let f = fixture(true);
        let id = written(&f, "Add a cart");
        proposal(&f, "draft", true, &id);
        // The hand-over: the Proposal is Open and `chatWriting` is gone; the call reads
        // `chatWriting` before it happens.
        proposal(&f, "open", false, &id);
        proposal_submitted(f.project.path(), &id, true);

        let store = RequestStore::new(f.project.path());
        assert_eq!(store.state_of(&id).unwrap(), RequestState::Consumed);
        assert!(!f
            .project
            .path()
            .join(format!(".weaver/requests/{id}.md"))
            .exists());
        assert!(f
            .service
            .list_locked(P, f.project.path(), false)
            .unwrap()
            .requests
            .is_empty());
    }

    #[test]
    fn a_request_being_written_is_not_consumed_by_a_proposal_that_is_not_from_chat() {
        let f = fixture(true);
        let id = written(&f, "Add a cart");
        proposal_submitted(f.project.path(), &id, false);
        assert_eq!(
            RequestStore::new(f.project.path()).state_of(&id).unwrap(),
            RequestState::Writing
        );
    }

    #[test]
    fn a_held_request_deleted_from_outside_is_discarded() {
        let f = fixture(true);
        let id = written(&f, "Add a cart");
        proposal(&f, "draft", true, &id);
        fs::remove_file(f.project.path().join(format!(".weaver/requests/{id}.md"))).unwrap();
        assert_eq!(
            RequestStore::new(f.project.path()).state_of(&id).unwrap(),
            RequestState::Discarded
        );
        assert!(f
            .service
            .list_locked(P, f.project.path(), false)
            .unwrap()
            .requests
            .is_empty());
        assert!(f.service.view(P, f.project.path(), &id).is_err());
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
            f.service.view(P, f.project.path(), &id).unwrap().status,
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
        let view = f.service.view(P, f.project.path(), &id).unwrap();
        assert_eq!((view.status, view.returned), (RequestStatus::Writing, true));
        // It can be changed and sent again; the banner goes away once it is sent.
        RequestStore::new(f.project.path())
            .save(&id, "Add a cart and a wishlist")
            .unwrap();
        f.service
            .send_locked(P, f.project.path(), &id)
            .unwrap()
            .unwrap();
        assert!(!f.service.view(P, f.project.path(), &id).unwrap().returned);
    }

    #[test]
    fn a_job_that_finished_while_the_app_was_closed_is_noticed_at_the_next_list() {
        let f = fixture(true);
        let id = written(&f, "Add a cart");
        let job = f
            .service
            .send_locked(P, f.project.path(), &id)
            .unwrap()
            .unwrap();
        let listed = f.service.list_locked(P, f.project.path(), false).unwrap();
        assert_eq!(listed.requests.len(), 1);
        assert_eq!(
            (
                listed.requests[0].status,
                listed.requests[0].job_name.as_deref()
            ),
            (RequestStatus::Sent, Some("Design"))
        );
        f.jobs.finished.lock().unwrap().insert(job);
        let listed = f.service.list_locked(P, f.project.path(), false).unwrap();
        assert_eq!(
            (listed.requests[0].status, listed.requests[0].returned),
            (RequestStatus::Writing, true)
        );
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
        proposal_submitted(f.project.path(), &id, false);
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
            .list_locked(P, f.project.path(), false)
            .unwrap()
            .requests
            .is_empty());
        assert!(f.service.view(P, f.project.path(), &id).is_err());
        // A second Proposal changes nothing.
        proposal_submitted(f.project.path(), &id, false);
        assert_eq!(store.state().unwrap().consumed, [id]);
    }

    #[test]
    fn a_sent_request_deleted_from_outside_is_forgotten_at_the_next_list() {
        let f = fixture(true);
        let id = written(&f, "Add a cart");
        f.service.send_locked(P, f.project.path(), &id).unwrap();
        fs::remove_file(f.project.path().join(format!(".weaver/requests/{id}.md"))).unwrap();
        let listed = f.service.list_locked(P, f.project.path(), false).unwrap();
        assert!(listed.requests.is_empty());
        assert!(RequestStore::new(f.project.path())
            .state()
            .unwrap()
            .sent
            .is_empty());
    }

    #[test]
    fn the_list_is_newest_first_and_says_whether_the_model_is_empty() {
        let f = fixture(false);
        let store = RequestStore::new(f.project.path());
        let older = store
            .create(&LocalTime::parse("2026-11-02T09:00:00").unwrap())
            .unwrap();
        let newer = store
            .create(&LocalTime::parse("2026-11-02T10:00:00").unwrap())
            .unwrap();
        store.save(&older, "# Older\n").unwrap();
        let listed = f.service.list_locked(P, f.project.path(), true).unwrap();
        assert!(listed.model_empty);
        let titles: Vec<(&str, &str)> = listed
            .requests
            .iter()
            .map(|r| (r.id.as_str(), r.title.as_str()))
            .collect();
        assert_eq!(
            titles,
            [(newer.as_str(), newer.as_str()), (older.as_str(), "Older")]
        );
        assert_eq!(listed.requests[0].created, "2026-11-02T10:00:00");
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
        // A Draft of Chat that names a request holds it: its `proposal.md` (and its folder) count,
        // what else a Proposal holds does not.
        for named in [
            "/p/.weaver/proposals/pppppppp-cart/proposal.md",
            "/p/.weaver/proposals/pppppppp-cart",
            "/p/.weaver/proposals",
        ] {
            assert!(
                touches_requests(root, &Rebuild::Paths(vec![named.into()])),
                "{named}"
            );
        }
        for other in [
            "/p/.weaver/proposals/pppppppp-cart/tasks/a.md",
            "/p/.weaver/proposals/pppppppp-cart/model/shop/feature.md",
            "/p/.weaver/proposals/pppppppp-cart/notes.md",
            "/p/.weaver/proposals-old/pppppppp-cart/proposal.md",
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

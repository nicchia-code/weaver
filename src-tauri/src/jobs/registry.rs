//! The jobs of the app: the registry in memory, the Machine `job` of each, and what the rest of the
//! app and the UI ask of them.
//!
//! All state lives in [`Jobs`], behind one lock. Everything that changes the state of a job goes
//! through [`Jobs::dispatch`]: the pure Machine decides ([`weaver_core::jobs::machine`]), the
//! record is updated and written (by a thread of its own, so that no lock waits on the disk), and
//! the effects are carried out: bookkeeping here, I/O by the actor of the job ([`super::actor`]).
//! Events to the UI and calls to the observers happen after the change, in order.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::mpsc as std_mpsc;
use std::sync::{Arc, Mutex, MutexGuard, RwLock, Weak};

use tokio::sync::{mpsc, watch};
use weaver_core::assets::tags;
use weaver_core::job_instruction;
use weaver_core::jobs::admission::StartPlan;
use weaver_core::jobs::machine::{Effect, Event, JobMachine, MessageId, PermissionEnd, RequestId};
use weaver_core::jobs::notes::{self, Normalised};
use weaver_core::jobs::{
    measure, names, AnnouncementRecord, FiledCard, JobKind, JobRecord, JobState, JobStore,
};
use weaver_core::project::board::AgentFacts;
use weaver_core::project::clock::{now_instant, Clock, SystemClock};
use weaver_core::project::ids;
use weaver_core::project::random::OsRandom;
use weaver_core::project::requests::store::UNTITLED;
use weaver_core::project::requests::RequestStore;

use super::actor::{Cmd, Mode, Msg};
use super::conversation::{Conversation, Delivery, EndLine, PermissionState};
use super::host::Host;
use super::wire::{ConversationSnapshot, EntriesEvent, JobInfo, JobObserver, JobRow, JobsSnapshot};
use crate::agent_work::{Ghost, GhostKind, GhostPhase};
use crate::subagents::poller::{Access, Poller};
use crate::subagents::reader::{Done, Location};
use crate::subagents::tree::Plan;
use crate::subagents::wire::{Outgoing, Owner, Step, SubAgentConversation, SubAgentsEvent};
use crate::subagents::Agents;
use crate::{log_info, log_warn};

/// The most finished jobs the Jobs list keeps.
pub const LISTED_FINISHED: usize = 30;

/// A job to create: what to run and where.
#[derive(Debug, Clone)]
pub struct NewJob {
    /// The project hash.
    pub project_id: String,
    /// The canonical root.
    pub root: PathBuf,
    pub plan: StartPlan,
}

pub(crate) struct Live {
    pub record: JobRecord,
    pub project_id: String,
    pub root: PathBuf,
    pub machine: JobMachine,
    pub conversation: Conversation,
    /// To the actor of the job, while it runs.
    pub channel: Option<mpsc::UnboundedSender<Msg>>,
    /// `true` once the actor has ended.
    pub done: Option<watch::Receiver<bool>>,
    /// The texts of the messages not delivered yet.
    pub texts: HashMap<MessageId, String>,
    pub next_message: MessageId,
    /// Not listed in the Jobs view: finished jobs of an earlier window, or removed with `X`.
    pub hidden: bool,
    pub instruction: String,
    /// The model the developer chose for the Proposal the job works for.
    pub proposal_model: Option<String>,
    /// omp's last error lines after an exit.
    pub exit_details: Option<String>,
    /// The order of the announcements, for the ghosts.
    pub announced: HashMap<String, u64>,
    /// Where the job was created among the announcements, for Weaver's own ghost of a setup or an
    /// import: it sits where it would sit if it had been announced at creation.
    pub order: u64,
    /// The title of the request a setup or design job received, as its draft card showed it when
    /// the job was created: the title of Weaver's own ghost. Only in memory, never stored.
    pub ghost_title: Option<String>,
    /// Its sub-agents, and the loop that reads their transcripts.
    pub agents: Agents,
    pub poller: Arc<Poller>,
}

impl Live {
    fn info(&self) -> JobInfo {
        JobInfo {
            id: self.record.id.clone(),
            project_id: self.project_id.clone(),
            root: self.root.clone(),
            kind: self.record.kind,
            name: self.record.name.clone(),
            state: self.record.state,
            target: self.record.target.clone(),
            reason: self.record.reason.clone(),
            announcements: self.record.announcements.clone(),
            empty_model_at_start: self.record.empty_model_at_start,
        }
    }

    /// `sent` are the requests sent to a job now (`RequestStore::sent_now`).
    fn row(&self, sent: &BTreeMap<String, String>) -> JobRow {
        let request = self.request().map(str::to_owned);
        let request_sent = request
            .as_ref()
            .is_some_and(|request| sent.get(request) == Some(&self.record.id));
        JobRow {
            id: self.record.id.clone(),
            kind: self.record.kind,
            name: self.record.name.clone(),
            state: self.record.state,
            reason: self.record.reason.clone(),
            created: self.record.created.clone(),
            started: self.record.started.clone(),
            ended: self.record.ended.clone(),
            note: self.record.latest_note().map(|note| note.text.clone()),
            target: self.record.target.clone(),
            request,
            request_sent,
        }
    }

    /// The request a setup or design job received.
    fn request(&self) -> Option<&str> {
        match self.record.kind {
            JobKind::Setup | JobKind::Design => self.record.target.as_deref(),
            _ => None,
        }
    }
}

#[derive(Default)]
pub(crate) struct Inner {
    pub jobs: HashMap<String, Live>,
    /// Projects whose registry records were read in this run.
    loaded: HashSet<String>,
    next_announcement: u64,
    /// Tasks finished since the last Scan System started, by project.
    done_since_scan: HashMap<String, u32>,
}

/// What a dispatch leaves to do outside the lock.
#[derive(Default)]
pub(crate) struct Outcome {
    waiting: Option<JobInfo>,
    ended: Option<JobInfo>,
    projects: Vec<String>,
    /// The Machine asked for omp to start again: the caller starts the actor, outside the lock.
    resume: bool,
    /// The record of a measured job that just ended: its measure is written after the end was
    /// applied and shown (`requirements/tech#job`, "Measure").
    measure: Option<Box<JobRecord>>,
}

/// The thread that writes the records, one after the other.
enum Write {
    Record(Box<JobRecord>),
    Prune(String),
    Flush(std_mpsc::Sender<()>),
}

pub struct Jobs {
    pub(crate) host: Arc<dyn Host>,
    store: JobStore,
    inner: Mutex<Inner>,
    writer: std_mpsc::Sender<Write>,
    clock: Arc<dyn Clock>,
    pub(crate) observers: RwLock<Vec<Arc<dyn JobObserver>>>,
    pub(crate) hooks: super::authority::Hooks,
    me: Weak<Jobs>,
}

/// A message that goes to the actor, or the reason it cannot.
fn send(live: &Live, message: Msg) {
    if let Some(channel) = &live.channel {
        let _ = channel.send(message);
    }
}

impl Jobs {
    pub fn new(host: Arc<dyn Host>) -> Arc<Jobs> {
        Self::with_clock(host, Arc::new(SystemClock))
    }

    pub fn with_clock(host: Arc<dyn Host>, clock: Arc<dyn Clock>) -> Arc<Jobs> {
        let store = JobStore::new(host.dirs());
        let (writer, queue) = std_mpsc::channel::<Write>();
        let thread_store = store.clone();
        std::thread::Builder::new()
            .name("weaver-jobs-registry".into())
            .spawn(move || {
                for write in queue {
                    match write {
                        Write::Record(record) => {
                            if let Err(error) = thread_store.write(&record) {
                                log_warn!(
                                    "the record of job {} could not be written: {error}",
                                    record.id
                                );
                            }
                        }
                        Write::Prune(project) => {
                            if let Err(error) = thread_store.prune_finished(&project) {
                                log_warn!("old job records could not be deleted: {error}");
                            }
                        }
                        Write::Flush(done) => {
                            let _ = done.send(());
                        }
                    }
                }
            })
            .expect("the thread of the registry starts");
        Arc::new_cyclic(|me| Jobs {
            host,
            store,
            inner: Mutex::new(Inner::default()),
            writer,
            clock,
            observers: RwLock::new(Vec::new()),
            hooks: super::authority::Hooks::default(),
            me: me.clone(),
        })
    }

    pub(crate) fn arc(&self) -> Arc<Jobs> {
        self.me.upgrade().expect("the jobs live as long as the app")
    }

    pub fn host(&self) -> &Arc<dyn Host> {
        &self.host
    }

    pub(crate) fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().expect("jobs poisoned")
    }

    pub(crate) fn now(&self) -> String {
        now_instant(self.clock.as_ref())
            .unwrap_or_else(|_| weaver_core::project::clock::format_instant(1))
    }

    /// Who wants to know when a job waits or ends.
    pub fn observe(&self, observer: Arc<dyn JobObserver>) {
        self.observers
            .write()
            .expect("observers poisoned")
            .push(observer);
    }

    /// Waits until every record written so far is on the disk.
    pub fn flush(&self) {
        let (done, wait) = std_mpsc::channel();
        if self.writer.send(Write::Flush(done)).is_ok() {
            let _ = wait.recv();
        }
    }

    fn persist(&self, record: &JobRecord) {
        let _ = self.writer.send(Write::Record(Box::new(record.clone())));
    }

    // -----------------------------------------------------------------------------------------
    // Queries
    // -----------------------------------------------------------------------------------------

    /// The Jobs list of a project, in the order the view shows it.
    pub fn snapshot(&self, project_id: &str) -> JobsSnapshot {
        // The record of the requests is read before the lock is taken (no lock waits on the disk),
        // and only when a job of the project has a request.
        let root = self
            .lock()
            .jobs
            .values()
            .find(|l| l.project_id == project_id && l.request().is_some())
            .map(|l| l.root.clone());
        let sent = root
            .map(|root| RequestStore::new(&root).sent_now())
            .unwrap_or_default();
        let inner = self.lock();
        self.snapshot_of(&inner, project_id, &sent)
    }

    fn snapshot_of(
        &self,
        inner: &Inner,
        project_id: &str,
        sent: &BTreeMap<String, String>,
    ) -> JobsSnapshot {
        let mut working: Vec<&Live> = inner
            .jobs
            .values()
            .filter(|l| l.project_id == project_id && l.record.state.is_active())
            .collect();
        let rank = |state: JobState| match state {
            JobState::Waiting => 0,
            JobState::Running => 1,
            JobState::Queued => 2,
            _ => 3,
        };
        working.sort_by(|a, b| {
            rank(a.record.state)
                .cmp(&rank(b.record.state))
                .then_with(|| {
                    b.record
                        .started
                        .as_ref()
                        .unwrap_or(&b.record.created)
                        .cmp(a.record.started.as_ref().unwrap_or(&a.record.created))
                })
                .then_with(|| b.record.id.cmp(&a.record.id))
        });
        let mut finished: Vec<&Live> = inner
            .jobs
            .values()
            .filter(|l| l.project_id == project_id && l.record.state.is_final() && !l.hidden)
            .collect();
        finished.sort_by(|a, b| {
            b.record
                .ended
                .cmp(&a.record.ended)
                .then_with(|| b.record.id.cmp(&a.record.id))
        });
        finished.truncate(LISTED_FINISHED);
        let listed: Vec<&Live> = working.into_iter().chain(finished).collect();
        let rows = listed.into_iter().map(|live| live.row(sent)).collect();
        JobsSnapshot {
            project_id: project_id.to_owned(),
            jobs: rows,
            omp_ready: self.host.omp_is_ready(),
        }
    }

    /// The conversation of a job of the project, as it is now.
    pub fn conversation(
        &self,
        project_id: &str,
        job_id: &str,
    ) -> Result<ConversationSnapshot, String> {
        let inner = self.lock();
        let live = inner
            .jobs
            .get(job_id)
            .filter(|l| l.project_id == project_id)
            .ok_or_else(|| format!("the job {job_id} does not exist"))?;
        Ok(ConversationSnapshot {
            job_id: job_id.to_owned(),
            seq: live.conversation.seq(),
            entries: live.conversation.entries().to_vec(),
            arrivals: live.conversation.arrivals().to_vec(),
            notes: live.record.notes.clone(),
        })
    }

    // -----------------------------------------------------------------------------------------
    // Sub-agents
    // -----------------------------------------------------------------------------------------

    /// The sub-agents of a job, as the list shows them.
    pub fn sub_agents(&self, project_id: &str, job_id: &str) -> Result<SubAgentsEvent, String> {
        let mut inner = self.lock();
        let live = inner
            .jobs
            .get_mut(job_id)
            .filter(|l| l.project_id == project_id)
            .ok_or_else(|| format!("the job {job_id} does not exist"))?;
        Ok(SubAgentsEvent {
            project_id: project_id.to_owned(),
            owner: Owner::Job {
                id: job_id.to_owned(),
            },
            nodes: live.agents.list(&live.conversation),
        })
    }

    /// The conversation of the sub-agent at `path` of a job, as it is now.
    pub fn sub_agent_conversation(
        &self,
        project_id: &str,
        job_id: &str,
        path: &[Step],
    ) -> Result<SubAgentConversation, String> {
        let mut inner = self.lock();
        let live = inner
            .jobs
            .get_mut(job_id)
            .filter(|l| l.project_id == project_id)
            .ok_or_else(|| format!("the job {job_id} does not exist"))?;
        let owner = Owner::Job {
            id: job_id.to_owned(),
        };
        live.agents
            .conversation(&live.conversation, project_id, &owner, path)
    }

    /// What the Jobs view shows now: whether the list of the sub-agents of the job is on screen,
    /// and the conversation of which sub-agent. Transcripts are read only for what is on screen.
    pub fn sub_agents_view(
        &self,
        project_id: &str,
        job_id: &str,
        list_shown: bool,
        shown: Option<Vec<Step>>,
    ) -> Result<(), String> {
        let poller = {
            let mut inner = self.lock();
            let live = inner
                .jobs
                .get_mut(job_id)
                .filter(|l| l.project_id == project_id)
                .ok_or_else(|| format!("the job {job_id} does not exist"))?;
            live.agents.interest(list_shown, shown);
            live.poller.clone()
        };
        poller.poke(|| self.access(job_id));
        Ok(())
    }

    fn access(&self, job_id: &str) -> Arc<dyn Access> {
        Arc::new(JobAccess {
            jobs: self.arc(),
            id: job_id.to_owned(),
        })
    }

    /// The session of the job is open: its sub-agents' transcripts are under it.
    pub(crate) fn locate_agents(&self, id: &str, session: &str) {
        let Some(root) = self.host.sessions_root() else {
            return;
        };
        if let Some(live) = self.lock().jobs.get_mut(id) {
            live.agents.locate(Location {
                root,
                session: session.to_owned(),
                owner_name: live.record.name.clone(),
            });
        }
    }

    /// The conversation lists sub-agents that may need reading: starts the loop that reads them.
    pub(crate) fn wake_agents(&self, id: &str) {
        let poller = self.lock().jobs.get(id).map(|l| l.poller.clone());
        if let Some(poller) = poller {
            self.wake_poller(&poller, id);
        }
    }

    fn wake_poller(&self, poller: &Arc<Poller>, id: &str) {
        poller.wake(|| self.access(id));
    }

    /// How many jobs of the project work (the agent runs or waits): what closing the window asks
    /// the developer about.
    pub fn working_count(&self, project_id: &str) -> usize {
        self.lock()
            .jobs
            .values()
            .filter(|l| l.project_id == project_id && l.record.state.is_working())
            .count()
    }

    /// What the Board needs to know about the agent's work in the project. The merge of Solo mode
    /// is read from the project folder when the project is open; whether its files still have
    /// conflicts is asked of git by whoever starts a `merge` job, never here.
    pub fn facts_of(&self, project_id: &str) -> AgentFacts {
        let merging = self
            .host
            .picture(project_id)
            .is_some_and(|(root, _)| weaver_core::solo_merge::in_progress(&root));
        let inner = self.lock();
        let active = inner
            .jobs
            .values()
            .filter(|l| l.project_id == project_id && l.record.state.is_active());
        let mut facts = AgentFacts {
            done_since_last_scan: inner.done_since_scan.get(project_id).copied().unwrap_or(0),
            merge_in_progress: merging,
            ..AgentFacts::default()
        };
        for live in active {
            facts.working_jobs.insert(live.record.id.clone());
            facts.run_in_progress |= live.record.kind.is_run();
            facts.resolve_working |= live.record.kind == JobKind::Resolve;
            facts.merge_job_unended |= live.record.kind == JobKind::Merge;
            if live.record.kind == JobKind::Resolve {
                facts
                    .resolving_drifts
                    .extend(live.record.target.iter().cloned());
            }
        }
        facts
    }

    /// The cards the jobs of the project announced and that have not arrived, in announcement order,
    /// and, for a setup, design or import job that has none on the Board, its own ghost, in the
    /// place it would have if it had been announced when the job was created.
    pub fn ghosts_of(&self, project_id: &str) -> Vec<Ghost> {
        let picture = self.host.picture(project_id).map(|(_, image)| image);
        let inner = self.lock();
        let mut ghosts: Vec<(u64, Ghost)> = Vec::new();
        for live in inner
            .jobs
            .values()
            .filter(|l| l.project_id == project_id && l.record.state.is_active())
        {
            let before = ghosts.len();
            for announcement in &live.record.announcements {
                if let (Some(card), Some(image)) = (announcement.card.as_ref(), picture.as_ref()) {
                    if card_is_on_the_board(image, card) {
                        continue;
                    }
                }
                let kind = ghost_kind(&announcement.kind).unwrap_or(GhostKind::Task);
                let phase = if live.record.state == JobState::Waiting {
                    GhostPhase::WaitingForAnswers
                } else if announcement.card.is_some() {
                    GhostPhase::Writing
                } else if live.record.kind == JobKind::Refinement {
                    GhostPhase::BeingRefined
                } else {
                    GhostPhase::Preparing
                };
                ghosts.push((
                    live.announced.get(&announcement.id).copied().unwrap_or(0),
                    Ghost {
                        id: announcement.id.clone(),
                        job_id: live.record.id.clone(),
                        kind,
                        title: announcement.title.clone(),
                        phase,
                        note: live.record.latest_note().map(|note| note.text.clone()),
                    },
                ));
            }
            if ghosts.len() == before {
                if let Some(ghost) = weaver_ghost(live) {
                    ghosts.push((live.order, ghost));
                }
            }
        }
        ghosts.sort_by_key(|(order, _)| *order);
        ghosts.into_iter().map(|(_, ghost)| ghost).collect()
    }

    /// The names of the assets of a project now (`requirements/tech#assets`).
    pub fn asset_names(&self, project_id: &str) -> Vec<String> {
        self.host.asset_names(project_id)
    }

    pub fn info(&self, job_id: &str) -> Option<JobInfo> {
        self.lock().jobs.get(job_id).map(Live::info)
    }

    pub fn record(&self, job_id: &str) -> Option<JobRecord> {
        self.lock().jobs.get(job_id).map(|l| l.record.clone())
    }

    /// Whether `job` is a job of the project at `root` that runs or waits for the developer.
    pub fn accepts_job(&self, root: &Path, job: &str) -> bool {
        self.lock()
            .jobs
            .get(job)
            .is_some_and(|l| l.root == root && l.machine.accepts_agent_calls())
    }

    /// The jobs of the project at `root`: the ones that are not finished, and with `all` the
    /// finished ones too, newest first.
    pub fn list_for(&self, root: &Path, all: bool) -> Vec<JobRecord> {
        let inner = self.lock();
        let mut records: Vec<JobRecord> = inner
            .jobs
            .values()
            .filter(|l| l.root == root && (all || l.record.state.is_active()))
            .map(|l| l.record.clone())
            .collect();
        records.sort_by(|a, b| b.created.cmp(&a.created).then_with(|| b.id.cmp(&a.id)));
        records
    }

    /// The Chat of the project started this job, or sent the request that started it: kept in its
    /// record, so that the Chat shows the job's line again after Weaver restarts.
    pub fn mark_by_chat(&self, job_id: &str) {
        let mut inner = self.lock();
        if let Some(live) = inner.jobs.get_mut(job_id) {
            if !live.record.by_chat {
                live.record.by_chat = true;
                self.persist(&live.record);
            }
        }
    }

    /// The jobs the Chat of the project started, oldest first (finished ones included).
    pub fn chat_jobs(&self, project_id: &str) -> Vec<JobRecord> {
        let inner = self.lock();
        let mut records: Vec<JobRecord> = inner
            .jobs
            .values()
            .filter(|l| l.project_id == project_id && l.record.by_chat)
            .map(|l| l.record.clone())
            .collect();
        records.sort_by(|a, b| a.created.cmp(&b.created).then_with(|| a.id.cmp(&b.id)));
        records
    }

    /// The identities of the jobs not finished in the project.
    pub fn active_ids(&self, project_id: &str) -> Vec<String> {
        self.lock()
            .jobs
            .values()
            .filter(|l| l.project_id == project_id && l.record.state.is_active())
            .map(|l| l.record.id.clone())
            .collect()
    }

    /// omp became ready or not: the Jobs list of every project with a Queued job says so.
    pub fn omp_changed(&self) {
        let projects: HashSet<String> = self
            .lock()
            .jobs
            .values()
            .filter(|l| l.record.state == JobState::Queued)
            .map(|l| l.project_id.clone())
            .collect();
        for project in projects {
            let snapshot = self.snapshot(&project);
            self.host.emit_jobs(&snapshot);
        }
    }

    // -----------------------------------------------------------------------------------------
    // Creation
    // -----------------------------------------------------------------------------------------

    /// Creates a job: Queued, with its fixed name and its instruction, and starts its actor, which
    /// waits for omp to be ready. Returns its identity.
    pub fn create(&self, new: NewJob) -> Result<String, String> {
        let settings = self.host.settings();
        let language = Some(settings.language.trim().to_owned()).filter(|l| !l.is_empty());
        let instruction = job_instruction::compose(
            &new.plan.job,
            &job_instruction::Settings {
                max_agents: new.plan.max_agents,
                language,
            },
        )
        .map_err(|error| error.to_string())?;
        let created = now_instant(self.clock.as_ref()).map_err(|error| error.to_string())?;

        // Read now, while the request is still as the developer sent it.
        let ghost_title = request_title(new.plan.kind, &new.root, new.plan.target.as_deref());
        let id = {
            let mut inner = self.lock();
            let id = ids::draw_identity(&mut OsRandom, &|candidate| {
                inner.jobs.contains_key(candidate) || self.store.file(candidate).exists()
            });
            let env = weaver_core::jobs::launch::weaver_env(
                &id,
                "",
                new.plan.kind,
                new.plan.task.as_deref(),
            );
            let record = JobRecord {
                id: id.clone(),
                kind: new.plan.kind,
                name: new.plan.name.clone(),
                project: new.root.to_string_lossy().into_owned(),
                target: new.plan.target.clone(),
                merged: new.plan.merged.clone(),
                state: JobState::Queued,
                reason: None,
                created,
                started: None,
                ended: None,
                session_id: None,
                omp: String::new(),
                args: Vec::new(),
                env,
                announcements: Vec::new(),
                notes: Vec::new(),
                pid: None,
                pid_started: None,
                empty_model_at_start: false,
                overlay: String::new(),
                running: None,
                running_incomplete: false,
                by_chat: false,
            };
            self.persist(&record);
            inner.next_announcement += 1;
            let order = inner.next_announcement;
            inner.jobs.insert(
                id.clone(),
                Live {
                    record,
                    project_id: new.project_id.clone(),
                    root: new.root.clone(),
                    machine: JobMachine::new(),
                    conversation: Conversation::new(),
                    channel: None,
                    done: None,
                    texts: HashMap::new(),
                    next_message: 1,
                    hidden: false,
                    instruction,
                    proposal_model: new.plan.proposal_model.clone(),
                    exit_details: None,
                    agents: Agents::new(),
                    poller: Arc::new(Poller::default()),
                    announced: HashMap::new(),
                    order,
                    ghost_title,
                },
            );
            if new.plan.kind == JobKind::Scan {
                // The count restarts when a Scan System job starts.
                inner.done_since_scan.insert(new.project_id.clone(), 0);
                self.write_counter(&new.project_id, 0);
            }
            id
        };
        log_info!(ctx: crate::logging::Context { project: None, job: Some(&id) }, "job created: {:?}", new.plan.kind);
        self.spawn_actor(&id, Mode::Start);
        self.changed(&new.project_id, Some(&id));
        Ok(id)
    }

    /// Starts the actor of a job.
    pub(crate) fn spawn_actor(&self, id: &str, mode: Mode) {
        let (tx, rx) = mpsc::unbounded_channel();
        let (done_tx, done_rx) = watch::channel(false);
        {
            let mut inner = self.lock();
            let Some(live) = inner.jobs.get_mut(id) else {
                return;
            };
            live.channel = Some(tx.clone());
            live.done = Some(done_rx);
        }
        let jobs = self.arc();
        let id = id.to_owned();
        tauri::async_runtime::spawn(async move {
            super::actor::run(jobs.clone(), id.clone(), mode, tx, rx).await;
            {
                let mut inner = jobs.lock();
                if let Some(live) = inner.jobs.get_mut(&id) {
                    live.channel = None;
                }
            }
            let _ = done_tx.send(true);
        });
    }

    // -----------------------------------------------------------------------------------------
    // The Machine
    // -----------------------------------------------------------------------------------------

    /// Applies an event to the Machine of a job and carries out what it asks.
    pub(crate) fn dispatch(&self, id: &str, event: Event) {
        let outcome = {
            let mut inner = self.lock();
            self.dispatch_in(&mut inner, id, event)
        };
        self.finish(outcome, id);
    }

    pub(crate) fn dispatch_in(&self, inner: &mut Inner, id: &str, event: Event) -> Outcome {
        let mut outcome = Outcome::default();
        let Some(live) = inner.jobs.get_mut(id) else {
            return outcome;
        };
        let (before, turn_before) = (live.machine.state(), live.machine.turn_running());
        let effects = live.machine.handle(event);
        let now_state = live.machine.state();
        if now_state != before {
            let at = self.now();
            if before == JobState::Running {
                live.record.left_running(&at);
            }
            if now_state == JobState::Running {
                live.record.entered_running(&at);
            }
            live.record.state = now_state;
            live.record.reason = live.machine.reason().map(str::to_owned);
            if before == JobState::Queued && now_state == JobState::Running {
                live.record.started = Some(self.now());
                if matches!(live.record.kind, JobKind::Setup | JobKind::Import) {
                    live.record.empty_model_at_start = self
                        .host
                        .picture(&live.project_id)
                        .is_some_and(|(_, image)| {
                            let model = image.model();
                            model.features.is_empty() && model.actors.is_empty()
                        });
                }
            }
            outcome.projects.push(live.project_id.clone());
        }
        for effect in effects {
            match effect {
                Effect::StartOmp => {}
                Effect::Steer(message) | Effect::FollowUp(message) => {
                    let follow_up = matches!(effect, Effect::FollowUp(_));
                    let text = live.texts.get(&message).cloned().unwrap_or_default();
                    send(
                        live,
                        Msg::Cmd(Cmd::Steer {
                            message,
                            text,
                            follow_up,
                        }),
                    );
                }
                Effect::Prompt(messages) => {
                    let text = messages
                        .iter()
                        .filter_map(|m| live.texts.get(m))
                        .cloned()
                        .collect::<Vec<_>>()
                        .join("\n\n");
                    send(live, Msg::Cmd(Cmd::Prompt { text }));
                }
                Effect::Delivered(message) => {
                    live.conversation.set_delivery(message, Delivery::Delivered);
                    live.texts.remove(&message);
                }
                Effect::NotDelivered(message, reason) => {
                    live.conversation
                        .set_delivery(message, Delivery::NotDelivered { reason });
                    live.texts.remove(&message);
                }
                Effect::ClosePermissions(how) => {
                    let to = match how {
                        PermissionEnd::Cancelled => PermissionState::Cancelled,
                        PermissionEnd::Void => PermissionState::Void,
                    };
                    for request in live.conversation.asking() {
                        live.conversation.close_permission(request, to.clone());
                    }
                    send(live, Msg::Cmd(Cmd::ClosePermissions));
                }
                Effect::ShutDown => send(
                    live,
                    Msg::Cmd(Cmd::ShutDown {
                        turn_running: turn_before,
                    }),
                ),
                Effect::Resume => outcome.resume = true,
                Effect::ResumePrompt => {
                    live.conversation.reloaded();
                    send(live, Msg::Cmd(Cmd::ResumePrompt));
                }
                Effect::Waiting => outcome.waiting = Some(live.info()),
                Effect::Ended => {
                    let end = match live.record.state {
                        JobState::Done => EndLine::Finished,
                        JobState::Stopped => EndLine::Stopped,
                        _ => EndLine::Failed {
                            reason: live
                                .record
                                .reason
                                .clone()
                                .unwrap_or_else(|| "the job failed".to_owned()),
                            details: live.exit_details.clone(),
                        },
                    };
                    live.conversation.finish(end);
                    live.agents.interrupt();
                    live.record.ended = Some(self.now());
                    live.record.pid = None;
                    live.record.pid_started = None;
                    live.texts.clear();
                    outcome.ended = Some(live.info());
                    if live.record.kind.is_measured() {
                        outcome.measure = Some(Box::new(live.record.clone()));
                    }
                    // The cards it announced and never filed are withdrawn, and those that arrived
                    // with them.
                    live.record.announcements.clear();
                }
            }
        }
        let live = inner.jobs.get_mut(id).expect("the job is still there");
        self.persist(&live.record);
        if outcome.ended.is_some() {
            let _ = self.writer.send(Write::Prune(live.record.project.clone()));
        }
        self.emit_entries(live, false);
        outcome
    }

    /// What follows a change, outside the lock: observers, the release of what the job held, the
    /// events to the UI and the Board.
    pub(crate) fn finish(&self, outcome: Outcome, id: &str) {
        let observers: Vec<Arc<dyn JobObserver>> =
            self.observers.read().expect("observers poisoned").clone();
        if let Some(info) = &outcome.waiting {
            for observer in &observers {
                observer.job_waiting(info);
            }
        }
        if let Some(info) = outcome.ended.clone() {
            log_info!(ctx: crate::logging::Context { project: None, job: Some(&info.id) }, "job ended: {:?}", info.state);
            self.release(&info);
            // `job.ENDED` (Done, Failed, Stopped): what its agent was changing is not held any more.
            if let Some(locks) = self.host.file_locks() {
                locks.conversation_ended(
                    &info.project_id,
                    &weaver_core::file_lock::job_conversation(&info.id),
                );
            }
            for observer in &observers {
                observer.job_ended(&info);
            }
        }
        if outcome.resume {
            self.spawn_actor(id, Mode::Resume);
        }
        let mut projects = outcome.projects;
        projects.sort();
        projects.dedup();
        for project in projects {
            self.changed(&project, None);
        }
        if let Some(record) = outcome.measure {
            self.measure(*record);
        }
    }

    /// Measures an ended job (`requirements/tech#job`, "Measure"): its session files are read once,
    /// on a thread of their own and after the end was applied and shown, and its line is appended
    /// to `.weaver/timings/jobs.jsonl`. A failure is logged and changes nothing of the job.
    pub(crate) fn measure(&self, record: JobRecord) {
        if !record.kind.is_measured() {
            return;
        }
        let sessions_root = self.host.sessions_root();
        tauri::async_runtime::spawn_blocking(move || {
            let root = PathBuf::from(&record.project);
            let session = measure::read_session(
                &root,
                sessions_root.as_deref(),
                record.session_id.as_deref(),
                record.kind.is_run(),
            );
            let Some(line) = measure::line_for(&record, session) else {
                return;
            };
            if let Err(error) = measure::append(&root, &line) {
                log_warn!(
                    "the measure of job {} could not be written: {error}",
                    record.id
                );
            }
        });
    }

    /// Releases what an ended job held: its Tasks go back to To do, in the project's queue.
    fn release(&self, info: &JobInfo) {
        let host = self.host.clone();
        let job = info.id.clone();
        let project = info.project_id.clone();
        tauri::async_runtime::spawn(async move {
            let id = job.clone();
            let result = host
                .mutate(
                    &project,
                    Box::new(move |root| {
                        let image = weaver_core::project::image::ProjectImage::load(root);
                        let changes = weaver_core::jobs::admission::plan_release(&image, &id);
                        if changes.is_empty() {
                            return Ok(None);
                        }
                        weaver_core::project::changes::apply(root, &changes, |_| Ok::<(), ()>(()))
                            .map(Some)
                            .map_err(|error| {
                                format!("the Tasks of job {id} could not be released: {error}")
                            })
                    }),
                )
                .await;
            if let Err(message) = result {
                log_warn!("{message}");
            }
            host.board_changed(&project);
        });
    }

    /// Emits the conversation changes of a job, if any, and what changed in its sub-agents.
    pub(crate) fn emit_entries(&self, live: &mut Live, reset: bool) {
        let changes = live.conversation.take_changes();
        if !changes.is_empty() || reset {
            self.host.emit_entries(&EntriesEvent {
                project_id: live.project_id.clone(),
                job_id: live.record.id.clone(),
                seq: live.conversation.seq(),
                reset,
                changes,
            });
        }
        let owner = Owner::Job {
            id: live.record.id.clone(),
        };
        for outgoing in live
            .agents
            .collect(&live.conversation, &live.project_id, &owner)
        {
            match outgoing {
                Outgoing::List(event) => self.host.emit_sub_agents(&event),
                Outgoing::Entries(event) => self.host.emit_sub_agent_entries(&event),
            }
        }
    }

    /// The list of a project changed: tells the UI and the Board.
    pub(crate) fn changed(&self, project_id: &str, _job: Option<&str>) {
        let snapshot = self.snapshot(project_id);
        self.host.emit_jobs(&snapshot);
        self.host.board_changed(project_id);
    }

    /// What the rows say about the requests changed (a request was sent, consumed, discarded or
    /// gone): tells the UI the list again.
    pub fn rows_changed(&self, project_id: &str) {
        self.host.emit_jobs(&self.snapshot(project_id));
    }

    /// Runs `f` on the conversation of a job and emits what it changed.
    pub(crate) fn with_conversation<T>(
        &self,
        id: &str,
        f: impl FnOnce(&mut Conversation) -> T,
    ) -> Option<T> {
        let mut inner = self.lock();
        let live = inner.jobs.get_mut(id)?;
        let value = f(&mut live.conversation);
        self.emit_entries(live, false);
        Some(value)
    }

    /// Runs `f` on the record of a job and writes it.
    pub(crate) fn with_record<T>(
        &self,
        id: &str,
        f: impl FnOnce(&mut JobRecord) -> T,
    ) -> Option<T> {
        let mut inner = self.lock();
        let live = inner.jobs.get_mut(id)?;
        let value = f(&mut live.record);
        self.persist(&live.record);
        Some(value)
    }

    pub(crate) fn proposal_model(&self, id: &str) -> Option<String> {
        self.lock()
            .jobs
            .get(id)
            .and_then(|l| l.proposal_model.clone())
    }

    pub(crate) fn instruction(&self, id: &str) -> Option<String> {
        self.lock().jobs.get(id).map(|l| l.instruction.clone())
    }

    pub(crate) fn set_exit_details(&self, id: &str, details: Option<String>) {
        if let Some(live) = self.lock().jobs.get_mut(id) {
            live.exit_details = details;
        }
    }

    /// Replaces the conversation by the history a reloaded session replayed, with the notes of the
    /// record after it; the UI fetches it again.
    pub(crate) fn adopt_replay(&self, id: &str, mut replayed: Conversation) {
        let mut inner = self.lock();
        let Some(live) = inner.jobs.get_mut(id) else {
            return;
        };
        if !live.conversation.is_empty() {
            // The conversation of this run is complete: the replay adds nothing.
            return;
        }
        // The history ends where omp's did: what the agent says next is a message of its own.
        replayed.end_turn();
        for note in &live.record.notes {
            replayed.note(&note.text, &note.at);
        }
        replayed.mark_sent();
        live.conversation = replayed;
        // The sub-agents are rebuilt from the replayed history, and their transcripts are read
        // again from the start.
        live.agents.reset();
        self.emit_entries(live, true);
        let poller = live.poller.clone();
        drop(inner);
        self.wake_poller(&poller, id);
    }

    // -----------------------------------------------------------------------------------------
    // The developer
    // -----------------------------------------------------------------------------------------

    /// `developer.SEND_MESSAGE`.
    pub fn send_message(&self, project_id: &str, job_id: &str, text: &str) -> Result<(), String> {
        let text = text.trim();
        if text.is_empty() {
            return Err("a message cannot be empty".to_owned());
        }
        // The assets the message tags are the ones that exist now, never recognised again later.
        let tagged = tags::tagged(text, &self.host.asset_names(project_id));
        let outcome = {
            let mut inner = self.lock();
            let live = inner
                .jobs
                .get_mut(job_id)
                .filter(|l| l.project_id == project_id)
                .ok_or_else(|| format!("the job {job_id} does not exist"))?;
            if !live.machine.accepts_messages() {
                return Err("The job does not take messages now.".to_owned());
            }
            let message = live.next_message;
            live.next_message += 1;
            // What goes to omp has the block of tagged assets; the conversation shows what the
            // developer wrote.
            live.texts.insert(message, tags::with_block(text, &tagged));
            live.conversation
                .developer_message(message, text, Delivery::Queued);
            self.dispatch_in(&mut inner, job_id, Event::SendMessage(message))
        };
        self.finish(outcome, job_id);
        Ok(())
    }

    /// `developer.STOP_AGENT`.
    pub fn stop(&self, project_id: &str, job_id: &str) -> Result<(), String> {
        self.check_job(project_id, job_id)?;
        self.dispatch(job_id, Event::StopAgent);
        Ok(())
    }

    /// `developer.ANSWER_PERMISSION`: `option` is one of the options of the request.
    pub fn answer_permission(
        &self,
        project_id: &str,
        job_id: &str,
        request: RequestId,
        option: &str,
    ) -> Result<(), String> {
        let outcome = {
            let mut inner = self.lock();
            let live = inner
                .jobs
                .get_mut(job_id)
                .filter(|l| l.project_id == project_id)
                .ok_or_else(|| format!("the job {job_id} does not exist"))?;
            let name = live
                .conversation
                .option_name(request, option)
                .ok_or_else(|| "this permission has no such option".to_owned())?;
            if !live.conversation.asking().contains(&request) {
                return Err("This permission is no longer asked.".to_owned());
            }
            live.conversation
                .close_permission(request, PermissionState::Chosen { name });
            send(
                live,
                Msg::Cmd(Cmd::AnswerPermission {
                    request,
                    option: option.to_owned(),
                }),
            );
            self.dispatch_in(&mut inner, job_id, Event::PermissionAnswered(request))
        };
        self.finish(outcome, job_id);
        Ok(())
    }

    /// The row's `X`: a finished job leaves the list; a job that works cannot be removed.
    pub fn remove(&self, project_id: &str, job_id: &str) -> Result<(), String> {
        {
            let mut inner = self.lock();
            let live = inner
                .jobs
                .get_mut(job_id)
                .filter(|l| l.project_id == project_id)
                .ok_or_else(|| format!("the job {job_id} does not exist"))?;
            if !live.record.state.is_final() {
                return Err("A job that works cannot be removed.".to_owned());
            }
            live.hidden = true;
            // Removing the job removes its sub-agents.
            live.agents = Agents::new();
        }
        self.changed(project_id, Some(job_id));
        Ok(())
    }

    fn check_job(&self, project_id: &str, job_id: &str) -> Result<(), String> {
        let inner = self.lock();
        inner
            .jobs
            .get(job_id)
            .filter(|l| l.project_id == project_id)
            .map(|_| ())
            .ok_or_else(|| format!("the job {job_id} does not exist"))
    }

    // -----------------------------------------------------------------------------------------
    // The agent
    // -----------------------------------------------------------------------------------------

    /// `question-round.OPENED`: only a round of this job makes it wait.
    pub fn round_opened(&self, job_id: &str) {
        self.dispatch(job_id, Event::RoundOpened);
    }

    /// `question-round.CLOSED`.
    pub fn round_closed(&self, job_id: &str) {
        self.dispatch(job_id, Event::RoundClosed);
    }

    /// `agent.DECLARE_NOTE`: the note as kept. A repeat of the last note only renews its instant.
    pub fn declare_note(&self, job_id: &str, text: &str) -> Result<Normalised, String> {
        let kept = notes::normalise(text).ok_or_else(|| "the note is empty".to_owned())?;
        let now = self.now();
        let project = {
            let mut inner = self.lock();
            let live = inner
                .jobs
                .get_mut(job_id)
                .ok_or_else(|| format!("the job {job_id} does not exist"))?;
            if !live.machine.accepts_agent_calls() {
                return Err(format!("the job {job_id} is not working"));
            }
            let repeat = live
                .record
                .notes
                .last()
                .is_some_and(|last| last.text == kept.text);
            notes::record(&mut live.record.notes, &kept.text, &now);
            if !repeat {
                live.conversation.note(&kept.text, &now);
            }
            self.persist(&live.record);
            self.emit_entries(live, false);
            live.project_id.clone()
        };
        self.changed(&project, Some(job_id));
        Ok(kept)
    }

    /// `agent.ANNOUNCE_CARD`: a ghost of the card appears on the Board. Returns the identity of the
    /// announcement.
    pub fn announce(&self, job_id: &str, kind: &str, title: &str) -> Result<String, String> {
        let project = {
            let mut inner = self.lock();
            if !inner
                .jobs
                .get(job_id)
                .is_some_and(|l| l.machine.accepts_agent_calls())
            {
                return Err(format!("the job {job_id} is not working"));
            }
            let taken: HashSet<String> = inner
                .jobs
                .values()
                .flat_map(|l| l.announced.keys().cloned())
                .collect();
            let id = ids::draw_identity(&mut OsRandom, &|candidate| taken.contains(candidate));
            inner.next_announcement += 1;
            let order = inner.next_announcement;
            let live = inner.jobs.get_mut(job_id).expect("checked above");
            live.record.announcements.push(AnnouncementRecord {
                id: id.clone(),
                kind: kind.to_owned(),
                title: title.trim().to_owned(),
                card: None,
            });
            live.announced.insert(id.clone(), order);
            self.persist(&live.record);
            (live.project_id.clone(), id)
        };
        self.changed(&project.0, Some(job_id));
        Ok(project.1)
    }

    /// A card was filed with `announcement`: the ghost leaves when the card is on the Board. The
    /// text is a note for the agent when the announcement is unknown or another job's.
    pub fn card_filed(
        &self,
        job: Option<&str>,
        kind: &str,
        card: &str,
        announcement: &str,
    ) -> Option<String> {
        ghost_kind(kind)?;
        let project = {
            let mut inner = self.lock();
            let owner = inner
                .jobs
                .values()
                .find(|l| l.record.announcements.iter().any(|a| a.id == announcement))
                .map(|l| l.record.id.clone());
            match (owner, job) {
                (Some(owner), Some(job)) if owner == job => {
                    let live = inner.jobs.get_mut(&owner).expect("found above");
                    if let Some(announced) = live
                        .record
                        .announcements
                        .iter_mut()
                        .find(|a| a.id == announcement)
                    {
                        announced.card = Some(FiledCard {
                            kind: kind.to_owned(),
                            id: card.to_owned(),
                        });
                    }
                    self.persist(&live.record);
                    live.project_id.clone()
                }
                _ => {
                    return Some(format!(
                        "The announcement `{announcement}` is not one of this job's open announcements: the card was created without a ghost."
                    ));
                }
            }
        };
        self.changed(&project, job);
        None
    }

    /// A Task reached Done by `FINISHED`: the count of the Board.
    pub fn task_finished(&self, root: &Path) {
        let project = {
            let mut inner = self.lock();
            let Some(project) = inner
                .jobs
                .values()
                .find(|l| l.root == root)
                .map(|l| l.project_id.clone())
            else {
                return;
            };
            let count = inner.done_since_scan.entry(project.clone()).or_insert(0);
            *count += 1;
            let count = *count;
            self.write_counter(&project, count);
            project
        };
        self.host.board_changed(&project);
    }

    fn counter_file(&self, project_id: &str) -> PathBuf {
        self.host
            .dirs()
            .local_data_dir()
            .join("scan-counters")
            .join(format!("{project_id}.json"))
    }

    fn write_counter(&self, project_id: &str, count: u32) {
        let file = self.counter_file(project_id);
        let text = serde_json::json!({ "doneSinceLastScan": count }).to_string();
        tauri::async_runtime::spawn_blocking(move || {
            let _ = weaver_core::atomic::write(&file, text.as_bytes(), None);
        });
    }

    /// The count a project had when it was last open.
    fn read_counter(&self, project_id: &str) -> u32 {
        std::fs::read(self.counter_file(project_id))
            .ok()
            .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
            .and_then(|value| {
                value
                    .get("doneSinceLastScan")
                    .and_then(serde_json::Value::as_u64)
            })
            .and_then(|n| u32::try_from(n).ok())
            .unwrap_or(0)
    }

    // -----------------------------------------------------------------------------------------
    // Windows
    // -----------------------------------------------------------------------------------------

    /// `window.OPENED`: reads the registry of the project (once per run), settles what a stop of
    /// Weaver left, releases the Tasks held by jobs nobody knows, and resumes the jobs that were
    /// suspended, all together.
    pub fn window_opened(&self, project_id: &str, root: &Path) {
        let jobs = self.arc();
        let project_id = project_id.to_owned();
        let root = root.to_path_buf();
        tauri::async_runtime::spawn(async move {
            let adopted = {
                let (jobs, project_id, root) = (jobs.clone(), project_id.clone(), root.clone());
                tauri::async_runtime::spawn_blocking(move || jobs.adopt_records(&project_id, &root))
                    .await
            };
            if adopted.is_err() {
                log_warn!("the registry of the jobs could not be read");
            }
            jobs.release_orphans(&project_id).await;
            let suspended: Vec<String> = {
                let inner = jobs.lock();
                inner
                    .jobs
                    .values()
                    .filter(|l| l.project_id == project_id && l.record.state == JobState::Suspended)
                    .map(|l| l.record.id.clone())
                    .collect()
            };
            for id in suspended {
                jobs.resume(&id);
            }
            jobs.changed(&project_id, None);
        });
    }

    /// `Resume`: the job's omp starts again and its session is reloaded.
    fn resume(&self, id: &str) {
        self.dispatch(id, Event::WindowOpened);
    }

    /// Reads the records of the project: finished jobs stay in the registry (hidden), a job that was
    /// working when Weaver stopped is Suspended (its leftover process is ended), a Queued one never
    /// started and is Stopped.
    fn adopt_records(&self, project_id: &str, root: &Path) {
        if !self.lock().loaded.insert(project_id.to_owned()) {
            return;
        }
        let counter = self.read_counter(project_id);
        let registry = self.store.read_all();
        for (path, reason) in &registry.unreadable {
            log_warn!("the job record {} is ignored: {reason}", path.display());
        }
        let project = root.to_string_lossy().into_owned();
        let mut records: Vec<JobRecord> = registry
            .records
            .into_iter()
            .filter(|r| r.project == project)
            .collect();
        // Creation order, so that the ghosts of the jobs keep it after a restart.
        records.sort_by(|a, b| a.created.cmp(&b.created).then_with(|| a.id.cmp(&b.id)));
        // The titles of the requests of setup and design jobs, read before the lock is taken: a
        // request that is gone reads as `Untitled request` (the title is never stored).
        let mut titles: HashMap<String, Option<String>> = records
            .iter()
            .map(|r| (r.id.clone(), request_title(r.kind, root, r.target.as_deref())))
            .collect();
        let mut inner = self.lock();
        inner
            .done_since_scan
            .entry(project_id.to_owned())
            .or_insert(counter);
        for mut record in records {
            if inner.jobs.contains_key(&record.id) {
                continue;
            }
            let mut changed = false;
            if let (Some(pid), Some(started)) = (record.pid, record.pid_started.clone()) {
                // A process a crash left behind.
                weaver_core::process_group::kill_leftover(pid, &started);
                record.pid = None;
                record.pid_started = None;
                changed = true;
            }
            let state = match record.state {
                JobState::Queued => {
                    record.state = JobState::Stopped;
                    record.ended = Some(self.now());
                    changed = true;
                    // It never started: the measure has the durations and no session.
                    self.measure(record.clone());
                    JobState::Stopped
                }
                JobState::Running | JobState::Waiting => {
                    // Weaver stopped unexpectedly: the time since the last change of state is
                    // unknown and never counted.
                    record.running_interrupted();
                    record.state = JobState::Suspended;
                    changed = true;
                    JobState::Suspended
                }
                other => other,
            };
            if changed {
                self.persist(&record);
            }
            inner.next_announcement += 1;
            let order = inner.next_announcement;
            let announced = record
                .announcements
                .iter()
                .map(|a| {
                    inner.next_announcement += 1;
                    (a.id.clone(), inner.next_announcement)
                })
                .collect();
            inner.jobs.insert(
                record.id.clone(),
                Live {
                    project_id: project_id.to_owned(),
                    root: root.to_path_buf(),
                    machine: JobMachine::at(state),
                    conversation: Conversation::new(),
                    channel: None,
                    done: None,
                    texts: HashMap::new(),
                    next_message: 1,
                    hidden: state.is_final(),
                    instruction: String::new(),
                    proposal_model: None,
                    exit_details: None,
                    agents: Agents::new(),
                    poller: Arc::new(Poller::default()),
                    announced,
                    order,
                    ghost_title: titles.remove(&record.id).flatten(),
                    record,
                },
            );
        }
    }

    /// A Task In progress whose holder no job of the registry knows counts as held by a job that
    /// ended: it goes back to To do.
    async fn release_orphans(&self, project_id: &str) {
        let known: HashSet<String> = {
            let inner = self.lock();
            inner
                .jobs
                .values()
                .filter(|l| l.record.state.is_active())
                .map(|l| l.record.id.clone())
                .collect()
        };
        let result = self
            .host
            .mutate(
                project_id,
                Box::new(move |root| {
                    let image = weaver_core::project::image::ProjectImage::load(root);
                    let changes = weaver_core::project::tasks::plan_orphans(&image, &|holder| {
                        known.contains(holder)
                    });
                    if changes.is_empty() {
                        return Ok(None);
                    }
                    weaver_core::project::changes::apply(root, &changes, |_| Ok::<(), ()>(()))
                        .map(Some)
                        .map_err(|error| {
                            format!("the Tasks of ended jobs could not be released: {error}")
                        })
                }),
            )
            .await;
        if let Err(message) = result {
            log_warn!("{message}");
        }
    }

    /// `window.CLOSED`: the jobs of the project that work are suspended and their omp ended, the
    /// Queued ones are Stopped. Completes when every process is ended and the registry is written.
    pub fn window_closed(&self, project_id: &str) -> impl Future<Output = ()> + Send + 'static {
        let mut waits: Vec<watch::Receiver<bool>> = Vec::new();
        let ids: Vec<String> = {
            let inner = self.lock();
            inner
                .jobs
                .values()
                .filter(|l| l.project_id == project_id && l.record.state.is_active())
                .map(|l| l.record.id.clone())
                .collect()
        };
        for id in &ids {
            self.dispatch(id, Event::WindowClosed);
        }
        {
            let mut inner = self.lock();
            for live in inner
                .jobs
                .values_mut()
                .filter(|l| l.project_id == project_id)
            {
                if let Some(done) = &live.done {
                    waits.push(done.clone());
                }
                // The finished jobs leave the list with the window.
                if live.record.state.is_final() {
                    live.hidden = true;
                }
            }
        }
        let jobs = self.arc();
        async move {
            for mut wait in waits {
                let _ = wait.wait_for(|done| *done).await;
            }
            let flush = jobs.clone();
            let _ = tauri::async_runtime::spawn_blocking(move || flush.flush()).await;
        }
    }
}

/// The poller of the sub-agents of a job reads and applies through the registry, under its lock.
struct JobAccess {
    jobs: Arc<Jobs>,
    id: String,
}

impl Access for JobAccess {
    fn plan(&self) -> Option<Plan> {
        let mut inner = self.jobs.lock();
        let live = inner.jobs.get_mut(&self.id)?;
        live.agents.plan(&live.conversation)
    }

    fn apply(&self, generation: u64, done: Vec<Done>) {
        let mut inner = self.jobs.lock();
        let Some(live) = inner.jobs.get_mut(&self.id) else {
            return;
        };
        live.agents.apply(&mut live.conversation, generation, done);
        self.jobs.emit_entries(live, false);
    }
}

/// What `kind` of an announcement or of a card says.
fn ghost_kind(kind: &str) -> Option<GhostKind> {
    match kind {
        "proposal" => Some(GhostKind::Proposal),
        "drift" => Some(GhostKind::Drift),
        "task" => Some(GhostKind::Task),
        _ => None,
    }
}

/// Weaver's own ghost of a setup, design or import job (`requirements/ui#work`): a Proposal being
/// prepared that the job has not announced. Its identity comes from the job and never equals an
/// announcement's (those have no `:`); it is never stored. The title of a setup or design job is
/// the one of the request it received, kept in memory when the job was created.
fn weaver_ghost(live: &Live) -> Option<Ghost> {
    let title = match live.record.kind {
        JobKind::Setup | JobKind::Design => live
            .ghost_title
            .clone()
            .unwrap_or_else(|| UNTITLED.to_owned()),
        JobKind::Import => names::IMPORT_GHOST_TITLE.to_owned(),
        _ => return None,
    };
    Some(Ghost {
        id: format!("job:{}", live.record.id),
        job_id: live.record.id.clone(),
        kind: GhostKind::Proposal,
        title,
        phase: if live.record.state == JobState::Waiting {
            GhostPhase::WaitingForAnswers
        } else {
            GhostPhase::Preparing
        },
        note: live.record.latest_note().map(|note| note.text.clone()),
    })
}

/// The title of the request a setup or design job received, as its draft card shows it; `None`
/// for any other job.
fn request_title(kind: JobKind, root: &Path, request: Option<&str>) -> Option<String> {
    match (kind, request) {
        (JobKind::Setup | JobKind::Design, Some(request)) => {
            Some(RequestStore::new(root).display_title(request))
        }
        _ => None,
    }
}

/// Whether the card filed with an announcement is on the Board of the picture: a Task or a Drift
/// is as soon as the file is read, a Proposal once it is not a Draft any more.
fn card_is_on_the_board(
    image: &weaver_core::project::image::ProjectImage,
    filed: &FiledCard,
) -> bool {
    use weaver_core::project::cards::CardKind;
    match ghost_kind(&filed.kind) {
        Some(GhostKind::Task) => image.card(CardKind::Task, &filed.id).is_some(),
        Some(GhostKind::Drift) => image.card(CardKind::Drift, &filed.id).is_some(),
        Some(GhostKind::Proposal) => image
            .card(CardKind::Proposal, &filed.id)
            .is_some_and(|p| p.str_key("status") != Some("draft")),
        None => false,
    }
}

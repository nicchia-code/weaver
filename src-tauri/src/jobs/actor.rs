//! The actor of a job: the one task that owns its omp process and its ACP session.
//!
//! It does the I/O the Machine `job` asks for ([`super::Jobs::dispatch`] sends it [`Cmd`]s) and
//! turns what happens (omp's messages, the end of a turn, the exit of the process) into events of
//! the Machine. Everything arrives on one channel, so it is handled one at a time, in order.
//! Nothing here ends a job by time: a job ends on a fact that omp or the developer produced.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::Value;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use weaver_core::jobs::machine::{Event, MessageId, RequestId, SteeringOutcome, RESUME_PROMPT};
use weaver_core::jobs::{launch, record};
use weaver_core::omp::machine::ReadyOmp;

use super::conversation::Conversation;
use super::proc::{ExitInfo, OmpProcess, SpawnSpec};
use super::registry::Jobs;
use super::rpc::Incoming;
use super::session::{self, OpenError, TurnEnd, OPEN_LIMIT};
use crate::file_lock::ConversationTools;

/// How the actor starts omp.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mode {
    /// A new session with the instruction.
    Start,
    /// The session of a suspended job is reloaded.
    Resume,
}

/// What the Machine asks the actor to do.
#[derive(Debug)]
pub(crate) enum Cmd {
    Steer {
        message: MessageId,
        text: String,
        follow_up: bool,
    },
    Prompt {
        text: String,
    },
    ResumePrompt,
    /// Every permission request still pending is answered `cancelled`.
    ClosePermissions,
    AnswerPermission {
        request: RequestId,
        option: String,
    },
    /// End omp in order and finish.
    ShutDown {
        turn_running: bool,
    },
}

/// Everything the actor waits for, on one channel.
pub(crate) enum Msg {
    Cmd(Cmd),
    OmpReady(Arc<ReadyOmp>),
    Spawned(Result<(OmpProcess, UnboundedReceiver<Incoming>, Option<String>), String>),
    Opened(Result<String, OpenError>),
    /// `initialize` answered, and the kept session is to be loaded.
    Initialized(String),
    /// The limit of the opening passed; carries the id of the load it was set for.
    LoadTimeout(u64),
    Incoming(Incoming),
    Exited(ExitInfo),
    SteerDone(MessageId, SteeringOutcome),
}

/// Why a job cannot be resumed, when the reason is not a proxy problem.
const NOT_RESUMED: &str = "the session could not be resumed";

pub(crate) async fn run(
    jobs: Arc<Jobs>,
    id: String,
    mode: Mode,
    tx: UnboundedSender<Msg>,
    mut rx: UnboundedReceiver<Msg>,
) {
    let mut actor = Actor {
        jobs,
        id,
        mode,
        tx,
        process: None,
        session: None,
        prompts: HashSet::new(),
        pending: HashMap::new(),
        next_request: 1,
        asks: false,
        loading: false,
        load: None,
        replay: Conversation::new(),
        opened_at: Instant::now(),
        exit_reported: false,
        turn_running: false,
        started: false,
        agents_rev: 0,
        finished: false,
        root: PathBuf::new(),
        tools: None,
    };
    actor.begin();
    while let Some(message) = rx.recv().await {
        actor.handle(message).await;
        if actor.finished {
            break;
        }
    }
    actor.cleanup().await;
}

struct Actor {
    jobs: Arc<Jobs>,
    id: String,
    mode: Mode,
    tx: UnboundedSender<Msg>,
    process: Option<Arc<OmpProcess>>,
    session: Option<String>,
    /// The prompts sent and not answered yet.
    prompts: HashSet<u64>,
    /// The JSON-RPC ids of the permission requests that wait for the developer.
    pending: HashMap<RequestId, Value>,
    next_request: RequestId,
    /// `--approval-mode always-ask`: permissions go to the UI.
    asks: bool,
    /// A session is being reloaded: what omp sends is the replayed history.
    loading: bool,
    /// The request id and the session of the `session/load` sent and not answered yet.
    load: Option<(u64, String)>,
    replay: Conversation,
    opened_at: Instant,
    exit_reported: bool,
    turn_running: bool,
    /// The session started (or was reloaded).
    started: bool,
    /// The `agents_rev` of the conversation when the poller of the sub-agents was last woken.
    agents_rev: u64,
    finished: bool,
    root: PathBuf,
    /// Where the tools of the agent that change files are told, once the project is known.
    tools: Option<ConversationTools>,
}

impl Actor {
    fn begin(&mut self) {
        let host = self.jobs.host.clone();
        let tx = self.tx.clone();
        tauri::async_runtime::spawn(async move {
            let ready = host.omp_ready().await;
            let _ = tx.send(Msg::OmpReady(ready));
        });
    }

    async fn handle(&mut self, message: Msg) {
        match message {
            Msg::Cmd(cmd) => self.command(cmd).await,
            Msg::OmpReady(ready) => {
                // The CLI socket starts with the app; a job that resumes at start waits for it.
                let mut endpoint = self.jobs.host.endpoint();
                for _ in 0..600 {
                    if endpoint.is_some() {
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    endpoint = self.jobs.host.endpoint();
                }
                match endpoint {
                    Some(endpoint) => self.launch(ready, endpoint),
                    None => self.fail_opening("the CLI socket of Weaver did not start", None),
                }
            }
            Msg::Spawned(result) => self.spawned(result),
            Msg::Opened(result) => self.opened(result),
            Msg::Initialized(session) => self.initialized(session),
            Msg::LoadTimeout(id) => self.load_timeout(id),
            Msg::Incoming(incoming) => self.incoming(incoming),
            Msg::Exited(info) => self.exited(info).await,
            Msg::SteerDone(message, outcome) => self
                .jobs
                .dispatch(&self.id, Event::SteeringReplied(message, outcome)),
        }
    }

    // -----------------------------------------------------------------------------------------
    // Starting
    // -----------------------------------------------------------------------------------------

    /// omp is ready: write the overlay, spawn the process.
    fn launch(&mut self, ready: Arc<ReadyOmp>, endpoint: String) {
        if self.finished || self.process.is_some() {
            return;
        }
        self.opened_at = Instant::now();
        let prepared = self.prepare(&ready, &endpoint);
        let tx = self.tx.clone();
        tauri::async_runtime::spawn(async move {
            let spawned = tauri::async_runtime::spawn_blocking(move || {
                let (spec, overlay, overlay_file) = prepared?;
                weaver_core::atomic::write(&overlay_file, overlay.as_bytes(), Some(0o600))
                    .map_err(|error| format!("cannot write the overlay of the agent: {error}"))?;
                let (process, incoming) = OmpProcess::spawn(spec)
                    .map_err(|error| format!("could not start the agent: {error}"))?;
                let started = weaver_core::process_group::start_time(process.pid);
                Ok((process, incoming, started))
            })
            .await
            .unwrap_or_else(|_| Err("could not start the agent".to_owned()));
            let _ = tx.send(Msg::Spawned(spawned));
        });
    }

    /// The command line, the environment and the overlay of omp, copied from the settings now (a
    /// resumed job keeps the ones it started with).
    #[allow(clippy::type_complexity)]
    fn prepare(
        &mut self,
        ready: &ReadyOmp,
        endpoint: &str,
    ) -> Result<(SpawnSpec, String, PathBuf), String> {
        let host = &self.jobs.host;
        let record = self.jobs.record(&self.id).ok_or("the job is gone")?;
        self.root = PathBuf::from(&record.project);
        self.tools = host.file_locks().map(|locks| {
            ConversationTools::new(
                locks,
                record.project_hash(),
                self.root.clone(),
                weaver_core::file_lock::job_conversation(&self.id),
            )
        });
        let overlay_file = record::overlay_file(host.dirs(), &self.id);
        let (args, mut env, overlay) = match self.mode {
            Mode::Start => {
                let settings = host.settings();
                let proposal_model = self.jobs.proposal_model(&self.id);
                let plan = launch::LaunchPlan {
                    kind: record.kind,
                    models: &settings.models,
                    proposal_model: proposal_model.as_deref(),
                    permissions: settings.permissions,
                    overlay: &overlay_file,
                };
                let env = launch::weaver_env(&self.id, endpoint, record.kind, record.run_task());
                (
                    launch::args(&plan),
                    env,
                    launch::overlay_text(record.kind, &settings.models.scan),
                )
            }
            Mode::Resume => (
                record.args.clone(),
                record.env.clone(),
                record.overlay.clone(),
            ),
        };
        // The socket is new at each launch of Weaver.
        env.insert("WEAVER_ENDPOINT".to_owned(), endpoint.to_owned());
        self.asks = args
            .windows(2)
            .any(|pair| pair[0] == "--approval-mode" && pair[1] == "always-ask");
        if self.mode == Mode::Start {
            let (omp, args_kept, env_kept, overlay_kept) = (
                ready.omp.to_string_lossy().into_owned(),
                args.clone(),
                env.clone(),
                overlay.clone(),
            );
            self.jobs.with_record(&self.id, move |r| {
                r.omp = omp;
                r.args = args_kept;
                r.env = env_kept;
                r.overlay = overlay_kept;
            });
        }
        let path = host.child_path(&ready.omp);
        // Read at every start and resume, and added after the env the registry keeps.
        let certificates = host.certificate_variables();
        let spec = SpawnSpec {
            program: ready.omp.clone(),
            args,
            env: launch::child_env(host.inherited_env(), &env, &certificates, &path),
            cwd: self.root.clone(),
            name: format!("job-{}", self.id),
        };
        Ok((spec, overlay, overlay_file))
    }

    fn spawned(
        &mut self,
        result: Result<(OmpProcess, UnboundedReceiver<Incoming>, Option<String>), String>,
    ) {
        let (process, mut incoming, started) = match result {
            Ok(parts) => parts,
            Err(reason) => {
                if self.mode == Mode::Start {
                    self.jobs.host.report_omp_missing(Some(reason.clone()));
                }
                self.fail_opening(
                    &weaver_core::process::timed::Failure::NotStarted.reason(),
                    Some(reason),
                );
                return;
            }
        };
        if self.finished {
            process.kill_now();
            return;
        }
        let process = Arc::new(process);
        let (pid, pid_started) = (process.pid, started);
        self.jobs.with_record(&self.id, move |r| {
            r.pid = Some(pid);
            r.pid_started = pid_started;
        });
        let tx = self.tx.clone();
        tauri::async_runtime::spawn(async move {
            while let Some(message) = incoming.recv().await {
                if tx.send(Msg::Incoming(message)).is_err() {
                    break;
                }
            }
        });
        let (tx, watched) = (self.tx.clone(), process.clone());
        tauri::async_runtime::spawn(async move {
            let info = watched.wait_exit().await;
            let _ = tx.send(Msg::Exited(info));
        });

        let rpc = process.rpc.clone();
        let cwd = self.root.clone();
        let reload = (self.mode == Mode::Resume)
            .then(|| self.jobs.record(&self.id).and_then(|r| r.session_id))
            .flatten();
        self.loading = reload.is_some();
        let remaining = OPEN_LIMIT.saturating_sub(self.opened_at.elapsed());
        let tx = self.tx.clone();
        self.process = Some(process);
        tauri::async_runtime::spawn(async move {
            let message = match reload {
                None => {
                    let opening = async {
                        session::initialize(&rpc).await?;
                        session::new_session(&rpc, &cwd).await
                    };
                    Msg::Opened(
                        tokio::time::timeout(remaining, opening)
                            .await
                            .unwrap_or(Err(OpenError::Timeout)),
                    )
                }
                // The load itself is sent by the actor, ordered with the replayed updates.
                Some(session) => match tokio::time::timeout(remaining, session::initialize(&rpc))
                    .await
                    .unwrap_or(Err(OpenError::Timeout))
                {
                    Ok(()) => Msg::Initialized(session),
                    Err(error) => Msg::Opened(Err(error)),
                },
            };
            let _ = tx.send(message);
        });
    }

    /// `initialize` answered: the session is loaded, sent ordered with the notifications omp sends
    /// meanwhile, so that its answer is handled after every replayed update. The 30 s limit of the
    /// opening goes on running.
    fn initialized(&mut self, session: String) {
        let Some(process) = self.process.clone().filter(|_| !self.finished) else {
            return;
        };
        let id = session::load_session(&process.rpc, &session, &self.root);
        self.load = Some((id, session));
        let remaining = OPEN_LIMIT.saturating_sub(self.opened_at.elapsed());
        let tx = self.tx.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(remaining).await;
            let _ = tx.send(Msg::LoadTimeout(id));
        });
    }

    /// The limit passed while the load had no answer yet.
    fn load_timeout(&mut self, id: u64) {
        if self.load.as_ref().is_some_and(|(load, _)| *load == id) {
            self.load = None;
            self.opened(Err(OpenError::Timeout));
        }
    }

    fn opened(&mut self, result: Result<String, OpenError>) {
        if self.finished {
            return;
        }
        let session = match result {
            Ok(session) => session,
            Err(error) => {
                let reason = error.reason();
                self.fail_opening(&reason, None);
                return;
            }
        };
        self.session = Some(session.clone());
        self.started = true;
        self.jobs.locate_agents(&self.id, &session);
        match self.mode {
            Mode::Start => {
                self.jobs
                    .with_record(&self.id, |r| r.session_id = Some(session.clone()));
                let instruction = self.jobs.instruction(&self.id).unwrap_or_default();
                self.jobs
                    .with_conversation(&self.id, |c| c.instruction(&instruction));
                self.start_turn(instruction);
                self.jobs.dispatch(&self.id, Event::SessionStarted);
            }
            Mode::Resume => {
                self.loading = false;
                let replayed = std::mem::take(&mut self.replay);
                self.jobs.adopt_replay(&self.id, replayed);
                self.jobs.dispatch(&self.id, Event::SessionResumed);
            }
        }
    }

    /// The last lines of omp's stderr so far, empty with no process.
    fn stderr_tail(&self) -> String {
        self.process
            .as_ref()
            .map(|process| process.stderr_text())
            .unwrap_or_default()
    }

    /// The session could not be opened: the job fails (a start) or cannot be resumed. A
    /// certificate or HTTP error in `reason`, the spawn error or the stderr tail is explained
    /// with the setting of this moment; the plain reasons stay otherwise.
    fn fail_opening(&mut self, reason: &str, details: Option<String>) {
        let tail = self.stderr_tail();
        let details = details.or_else(|| Some(tail.clone()).filter(|text| !text.is_empty()));
        let on = self.jobs.host.system_certificates_enabled();
        let texts = [reason, details.as_deref().unwrap_or(""), &tail];
        let event = match self.mode {
            Mode::Start => Event::StartFailed(session::explained(reason, &texts, on)),
            Mode::Resume => Event::SessionNotResumed(session::explained(NOT_RESUMED, &texts, on)),
        };
        self.jobs.set_exit_details(&self.id, details);
        self.jobs.dispatch(&self.id, event);
        // The Machine asks for the shutdown; with no process there is nothing to wait for.
        if self.process.is_none() {
            self.finished = true;
        }
    }

    // -----------------------------------------------------------------------------------------
    // Turns and messages
    // -----------------------------------------------------------------------------------------

    fn start_turn(&mut self, text: String) {
        let (Some(process), Some(session)) = (self.process.clone(), self.session.clone()) else {
            return;
        };
        self.turn_running = true;
        let id = session::prompt(&process.rpc, &session, &text);
        self.prompts.insert(id);
    }

    fn turn_done(&mut self, end: TurnEnd) {
        self.turn_running = false;
        self.end_tools();
        self.jobs.with_conversation(&self.id, |c| c.end_turn());
        match end {
            TurnEnd::Ended => self.jobs.dispatch(&self.id, Event::TurnEnded),
            TurnEnd::Failed(reason) => self.jobs.dispatch(&self.id, Event::TurnFailed(reason)),
            // The process is gone: its exit (reported on its own) says why the job ended.
            TurnEnd::Closed => {}
        }
    }

    async fn command(&mut self, cmd: Cmd) {
        match cmd {
            Cmd::Steer {
                message,
                text,
                follow_up,
            } => {
                let (Some(process), Some(session)) = (self.process.clone(), self.session.clone())
                else {
                    let _ = self.tx.send(Msg::SteerDone(
                        message,
                        SteeringOutcome::Failed("the agent is not running".to_owned()),
                    ));
                    return;
                };
                let tx = self.tx.clone();
                let on = self.jobs.host.system_certificates_enabled();
                tauri::async_runtime::spawn(async move {
                    let outcome = session::steer(&process.rpc, &session, &text, follow_up).await;
                    let outcome = session::explained_outcome(outcome, &process.stderr_text(), on);
                    let _ = tx.send(Msg::SteerDone(message, outcome));
                });
            }
            Cmd::Prompt { text } => self.start_turn(text),
            Cmd::ResumePrompt => self.start_turn(RESUME_PROMPT.to_owned()),
            Cmd::ClosePermissions => {
                if let Some(process) = &self.process {
                    for (_, id) in self.pending.drain() {
                        process.rpc.respond(&id, session::cancelled());
                    }
                }
                self.pending.clear();
            }
            Cmd::AnswerPermission { request, option } => {
                if let (Some(process), Some(id)) = (&self.process, self.pending.remove(&request)) {
                    process.rpc.respond(&id, session::selected(&option));
                }
            }
            Cmd::ShutDown { turn_running } => {
                if let Some(process) = self.process.clone() {
                    process
                        .shutdown(self.session.as_deref(), turn_running || self.turn_running)
                        .await;
                }
                self.finished = true;
            }
        }
    }

    // -----------------------------------------------------------------------------------------
    // What omp sends
    // -----------------------------------------------------------------------------------------

    fn incoming(&mut self, message: Incoming) {
        match message {
            Incoming::Notification { method, params } => {
                if method == "session/update" {
                    if let Some(update) = params.get("update") {
                        if self.loading {
                            self.replay.apply_update(update);
                        } else {
                            // Only the live updates count for the lock, never the replayed ones.
                            if let Some(tools) = &self.tools {
                                tools.update(update);
                            }
                            let rev = self.jobs.with_conversation(&self.id, |c| {
                                c.apply_update(update);
                                c.agents_rev()
                            });
                            self.agents_changed(rev);
                        }
                    }
                }
            }
            Incoming::Request { id, method, params } => self.request(id, &method, &params),
            Incoming::Response { id, result } => {
                if self.load.as_ref().is_some_and(|(load, _)| *load == id) {
                    // Every replayed update read before this answer was handled before it.
                    let (_, session) = self.load.take().expect("the load was just matched");
                    self.opened(result.map(|_| session).map_err(OpenError::from));
                } else if self.prompts.remove(&id) {
                    let on = self.jobs.host.system_certificates_enabled();
                    let end = session::turn_end(result).explained(&self.stderr_tail(), on);
                    self.turn_done(end);
                }
            }
            Incoming::Closed => {
                // The output ended: the process is exiting, or is gone. Give it a moment, then count it as exited.
                let Some(process) = self.process.clone() else {
                    return;
                };
                if process.is_shutting_down() || self.exit_reported {
                    return;
                }
                let tx = self.tx.clone();
                tauri::async_runtime::spawn(async move {
                    let info = tokio::time::timeout(Duration::from_secs(2), process.wait_exit())
                        .await
                        .unwrap_or(ExitInfo { code: None });
                    let _ = tx.send(Msg::Exited(info));
                });
            }
        }
    }

    /// A request of omp to the client.
    fn request(&mut self, id: Value, method: &str, params: &Value) {
        let Some(process) = self.process.clone() else {
            return;
        };
        if method == "session/request_permission" {
            let options = params.get("options").cloned().unwrap_or(Value::Null);
            if !self.asks {
                // `yolo`: a request that still arrives is answered at once.
                match session::auto_option(&options) {
                    Some(option) => process.rpc.respond(&id, session::selected(&option)),
                    None => process.rpc.respond(&id, session::cancelled()),
                }
                return;
            }
            let request = self.next_request;
            self.next_request += 1;
            self.pending.insert(request, id);
            let tool_call = params.get("toolCall").cloned().unwrap_or(Value::Null);
            self.jobs
                .with_conversation(&self.id, |c| c.permission(request, &tool_call, &options));
            self.jobs
                .dispatch(&self.id, Event::PermissionRequested(request));
            self.jobs.tell(&self.id, true, |observer, job| {
                observer.permission_asked(job, request)
            });
        } else if session::is_elicitation(method) {
            process.rpc.respond(&id, session::decline());
        } else {
            process
                .rpc
                .respond_error(&id, -32601, "Weaver does not offer this method");
        }
    }

    async fn exited(&mut self, info: ExitInfo) {
        self.end_tools();
        let Some(process) = self.process.clone() else {
            return;
        };
        if self.exit_reported || process.is_shutting_down() {
            return;
        }
        self.exit_reported = true;
        process.kill_now();
        process.settle_stderr().await;
        let on = self.jobs.host.system_certificates_enabled();
        let (reason, details) = process.exit_reason(&info, on);
        let tail = details.clone().unwrap_or_default();
        self.jobs.set_exit_details(&self.id, details);
        let event = if self.started {
            Event::Exited(reason)
        } else if self.mode == Mode::Start {
            Event::StartFailed(reason)
        } else {
            Event::SessionNotResumed(session::explained(NOT_RESUMED, &[&tail], on))
        };
        self.jobs.dispatch(&self.id, event);
        self.finished = true;
    }

    // -----------------------------------------------------------------------------------------
    // Sub-agents
    // -----------------------------------------------------------------------------------------

    /// What the conversation's rows say of the sub-agents changed: their transcripts may need
    /// reading, and the poller of the job decides which (`crate::subagents`).
    fn agents_changed(&mut self, rev: Option<u64>) {
        if let Some(rev) = rev.filter(|rev| *rev != self.agents_rev) {
            self.agents_rev = rev;
            self.jobs.wake_agents(&self.id);
        }
    }

    /// The tools of this omp stop counting for the lock of the File view.
    fn end_tools(&self) {
        if let Some(tools) = &self.tools {
            tools.end();
        }
    }

    async fn cleanup(&mut self) {
        self.end_tools();
        if let Some(process) = &self.process {
            if !process.has_exited() {
                process.kill_now();
            }
        }
        let file = record::overlay_file(self.jobs.host.dirs(), &self.id);
        let _ = tauri::async_runtime::spawn_blocking(move || std::fs::remove_file(file)).await;
    }
}

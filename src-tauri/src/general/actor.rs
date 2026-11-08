//! The actor of a conversation with omp (the General of a project, or an Ask more conversation):
//! the one task that owns its omp process and its ACP session.
//!
//! It does the I/O the Machine `general-chat` asks for ([`super::General`] sends it [`Cmd`]s) and
//! turns what happens (omp's messages, the end of a turn, the exit of the process) into events of
//! the Machine. Everything arrives on one channel, so it is handled one at a time, in order. The
//! process, the JSON-RPC and the ACP calls are the ones of the jobs; what differs is what is
//! started (General: `--append-system-prompt` instead of an overlay and `WEAVER_GENERAL`; an Ask
//! more conversation: no instruction file at all and `WEAVER_ASK`), the session that is reloaded
//! when one is kept (an empty one starts when the load fails, and an Ask more conversation then
//! builds its instruction again), and the forms, declined with a text that asks the agent to ask in
//! its reply.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use weaver_core::jobs::ask;
use weaver_core::jobs::general::{self as machine, Event};
use weaver_core::jobs::launch;
use weaver_core::jobs::machine::{MessageId, RequestId, SteeringOutcome};
use weaver_core::omp::machine::ReadyOmp;

use super::service::{General, Launch, Rebuild};
use crate::file_lock::ConversationTools;
use crate::jobs::conversation::Conversation;
use crate::jobs::proc::{ExitInfo, OmpProcess, SpawnSpec};
use crate::jobs::rpc::{Incoming, RpcError};
use crate::jobs::session::{self, OpenError, TurnEnd, OPEN_LIMIT};

/// What a form or an ask of omp is declined with in General.
pub const DECLINE_TEXT: &str = "In General, ask the developer in your reply, in plain text, and wait for the answer. Do not use a form or a question tool.";

/// What a form or an ask of omp is declined with in an Ask more conversation.
pub const DECLINE_TEXT_ASK: &str = "In Ask more, ask the developer in your reply, in plain text, and wait for the answer. Do not use a form or a question tool.";

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
    /// `session/cancel`: the turn in flight ends; omp stays.
    Cancel,
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

/// The session omp opened, and why the kept one could not be reloaded when it could not.
pub(crate) struct Opened {
    session: String,
    fell_back: Option<String>,
    /// An Ask more conversation whose session was replaced: its instruction built again from the
    /// files (`Some(None)`: the chosen thing no longer exists).
    instruction: Option<Option<String>>,
}

/// Everything the actor waits for, on one channel.
pub(crate) enum Msg {
    Cmd(Cmd),
    OmpReady(Arc<ReadyOmp>),
    Spawned(Result<(OmpProcess, UnboundedReceiver<Incoming>), String>),
    Opened(Result<Opened, OpenError>),
    /// `initialize` answered, and the kept session is to be loaded.
    Initialized(String),
    /// The limit of the opening passed; carries the id of the load it was set for.
    LoadTimeout(u64),
    Incoming(Incoming),
    Exited(ExitInfo),
    SteerDone(MessageId, SteeringOutcome),
}

pub(crate) async fn run(
    general: Arc<General>,
    key: String,
    generation: u64,
    tx: UnboundedSender<Msg>,
    mut rx: UnboundedReceiver<Msg>,
) {
    let mut actor = Actor {
        general,
        key,
        generation,
        tx,
        process: None,
        session: None,
        prompts: HashSet::new(),
        pending: HashMap::new(),
        next_request: 1,
        asks: false,
        reload: None,
        loading: false,
        load: None,
        replay: Conversation::general(),
        opened_at: Instant::now(),
        exit_reported: false,
        spawning: false,
        closing: false,
        turn_running: false,
        started: false,
        agents_rev: 0,
        finished: false,
        root: PathBuf::new(),
        tools: None,
        rebuild: None,
        decline: DECLINE_TEXT,
    };
    actor.begin();
    while let Some(message) = rx.recv().await {
        actor.handle(message).await;
        if actor.finished {
            break;
        }
    }
    actor.cleanup();
}

struct Actor {
    general: Arc<General>,
    /// The key of the conversation in the service: the project hash for General, the identity of
    /// an Ask more conversation.
    key: String,
    generation: u64,
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
    /// The kept session this omp reloads.
    reload: Option<String>,
    /// The session is being reloaded: what omp sends is the replayed history.
    loading: bool,
    /// The request id and the kept session of the `session/load` sent and not answered yet.
    load: Option<(u64, String)>,
    replay: Conversation,
    opened_at: Instant,
    exit_reported: bool,
    /// The process is being spawned: `Msg::Spawned` is on its way.
    spawning: bool,
    /// `Cmd::ShutDown` came while it was spawning: it ends when it arrives.
    closing: bool,
    turn_running: bool,
    /// The session opened.
    started: bool,
    /// The `agents_rev` of the conversation when the poller of the sub-agents was last woken.
    agents_rev: u64,
    finished: bool,
    root: PathBuf,
    /// Where the tools of the agent that change files are told, once the project is known.
    tools: Option<ConversationTools>,
    /// An Ask more conversation: how its instruction is built again when its session is replaced.
    rebuild: Option<Rebuild>,
    /// The text the forms of omp are declined with.
    decline: &'static str,
}

impl Actor {
    fn begin(&mut self) {
        let host = self.general.host.clone();
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
                // The end of Waiting for omp: the kept conversation reloads without a message.
                self.dispatch(Event::OmpReady);
                // The CLI socket starts with the app: wait for it.
                let mut endpoint = self.general.host.endpoint();
                for _ in 0..600 {
                    if endpoint.is_some() {
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    endpoint = self.general.host.endpoint();
                }
                match endpoint {
                    Some(endpoint) => self.launch(&ready, &endpoint),
                    None => self.fail_opening("the CLI socket of Weaver did not start", None),
                }
            }
            Msg::Spawned(result) => self.spawned(result),
            Msg::Opened(result) => self.opened(result),
            Msg::Initialized(previous) => self.initialized(previous),
            Msg::LoadTimeout(id) => self.load_timeout(id),
            Msg::Incoming(incoming) => self.incoming(incoming),
            Msg::Exited(info) => self.exited(info).await,
            Msg::SteerDone(message, outcome) => {
                self.dispatch(Event::SteeringReplied(message, outcome))
            }
        }
    }

    fn dispatch(&self, event: Event) {
        self.general.dispatch(&self.key, self.generation, event);
    }

    fn conversation<T>(&self, f: impl FnOnce(&mut Conversation) -> T) -> Option<T> {
        self.general
            .with_conversation(&self.key, self.generation, f)
    }

    // -----------------------------------------------------------------------------------------
    // Starting
    // -----------------------------------------------------------------------------------------

    /// omp is ready: write the instruction (General only), spawn the process.
    fn launch(&mut self, ready: &ReadyOmp, endpoint: &str) {
        if self.finished || self.process.is_some() {
            return;
        }
        let Some(facts) = self.general.launch_facts(&self.key, self.generation) else {
            self.finished = true;
            return;
        };
        let Launch {
            identity,
            root,
            reload,
            rebuild,
        } = facts;
        self.opened_at = Instant::now();
        self.root = root.clone();
        self.reload = reload;
        let host = &self.general.host;
        let settings = host.settings();
        // An Ask more conversation has no instruction file and no system prompt: its instruction
        // goes in the first prompt, and nothing of it is written to disk.
        let (args, env, instruction_file, name) = match &rebuild {
            Some(_) => (
                ask::args(&settings.models, settings.permissions),
                ask::env(&identity, endpoint),
                None,
                format!("ask-{}", self.key),
            ),
            None => {
                let file = machine::instruction_file(host.dirs());
                (
                    machine::args(&settings.models, settings.permissions, &file),
                    machine::env(&identity, endpoint),
                    Some(file),
                    format!("general-{}", self.key),
                )
            }
        };
        if rebuild.is_some() {
            self.replay = Conversation::ask();
            self.decline = DECLINE_TEXT_ASK;
        }
        self.rebuild = rebuild;
        self.tools = host.file_locks().map(|locks| {
            let family = match &self.rebuild {
                Some(_) => weaver_core::file_lock::ask_conversation(&self.key),
                None => weaver_core::file_lock::GENERAL_CONVERSATION.to_owned(),
            };
            ConversationTools::new(
                locks,
                weaver_core::paths::project_hash_of_canonical(&self.root),
                self.root.clone(),
                weaver_core::file_lock::generation_conversation(&family, self.generation),
            )
        });
        let language =
            Some(settings.language.trim().to_owned()).filter(|language| !language.is_empty());
        // Built at every start with the sub-agent limit and the language of the Settings of that
        // moment.
        let instruction =
            machine::instruction(settings.max_parallel_agents.max(1), language.as_deref());
        self.asks = args
            .windows(2)
            .any(|pair| pair[0] == "--approval-mode" && pair[1] == "always-ask");
        let path = host.child_path(&ready.omp);
        // Read at every start of the process, also the one a later message makes.
        let certificates = host.certificate_variables();
        let spec = SpawnSpec {
            program: ready.omp.clone(),
            args,
            env: launch::child_env(host.inherited_env(), &env, &certificates, &path),
            cwd: root,
            name,
        };
        self.spawning = true;
        let tx = self.tx.clone();
        tauri::async_runtime::spawn(async move {
            let spawned = tauri::async_runtime::spawn_blocking(move || {
                if let Some(file) = instruction_file {
                    weaver_core::atomic::write(&file, instruction.as_bytes(), None).map_err(
                        |error| format!("cannot write the instruction of Chat: {error}"),
                    )?;
                }
                OmpProcess::spawn(spec).map_err(|error| format!("could not start the agent: {error}"))
            })
            .await
            .unwrap_or_else(|_| Err("could not start the agent".to_owned()));
            let _ = tx.send(Msg::Spawned(spawned));
        });
    }

    fn spawned(&mut self, result: Result<(OmpProcess, UnboundedReceiver<Incoming>), String>) {
        self.spawning = false;
        if self.closing || self.finished {
            // omp was asked to end while it was starting: whatever started ends at once.
            if let Ok((process, _)) = result {
                process.kill_now();
            }
            self.finished = true;
            return;
        }
        let (process, mut incoming) = match result {
            Ok(parts) => parts,
            Err(reason) => {
                self.general.host.report_omp_missing(Some(reason.clone()));
                self.fail_opening(
                    &weaver_core::process::timed::Failure::NotStarted.reason(),
                    Some(reason),
                );
                return;
            }
        };
        let process = Arc::new(process);
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
        let reload = self.reload.clone();
        self.loading = reload.is_some();
        let remaining = OPEN_LIMIT.saturating_sub(self.opened_at.elapsed());
        let tx = self.tx.clone();
        self.process = Some(process);
        tauri::async_runtime::spawn(async move {
            let message = match reload {
                None => {
                    let opening = async {
                        session::initialize(&rpc).await?;
                        session::new_session(&rpc, &cwd)
                            .await
                            .map(|session| Opened {
                                session,
                                fell_back: None,
                                instruction: None,
                            })
                    };
                    Msg::Opened(
                        tokio::time::timeout(remaining, opening)
                            .await
                            .unwrap_or(Err(OpenError::Timeout)),
                    )
                }
                // The load itself is sent by the actor, ordered with the replayed updates.
                Some(previous) => {
                    match tokio::time::timeout(remaining, session::initialize(&rpc))
                        .await
                        .unwrap_or(Err(OpenError::Timeout))
                    {
                        Ok(()) => Msg::Initialized(previous),
                        Err(error) => Msg::Opened(Err(error)),
                    }
                }
            };
            let _ = tx.send(message);
        });
    }

    /// `initialize` answered: the kept session is loaded, sent ordered with the notifications omp
    /// sends meanwhile, so that its answer is handled after every replayed update. The 30 s limit
    /// of the opening goes on running.
    fn initialized(&mut self, previous: String) {
        let Some(process) = self.process.clone().filter(|_| !self.finished) else {
            return;
        };
        let id = session::load_session(&process.rpc, &previous, &self.root);
        self.load = Some((id, previous));
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

    /// The answer of `session/load`. When omp refused the kept session, an empty one starts and a
    /// notice says so.
    fn load_answered(&mut self, previous: String, result: Result<Value, RpcError>) {
        match result {
            Ok(_) => self.opened(Ok(Opened {
                session: previous,
                fell_back: None,
                instruction: None,
            })),
            Err(error) => {
                let reason = error.text();
                let (rpc, cwd) = match &self.process {
                    Some(process) if !self.finished => (process.rpc.clone(), self.root.clone()),
                    _ => return,
                };
                let remaining = OPEN_LIMIT.saturating_sub(self.opened_at.elapsed());
                let tx = self.tx.clone();
                let rebuild = self.rebuild.clone();
                tauri::async_runtime::spawn(async move {
                    let result = tokio::time::timeout(remaining, session::new_session(&rpc, &cwd))
                        .await
                        .unwrap_or(Err(OpenError::Timeout));
                    // An Ask more conversation whose session is replaced: the message that opens
                    // the new one carries the instruction read again from the files now, or goes
                    // alone when the chosen thing is gone.
                    let instruction = match (&result, rebuild) {
                        (Ok(_), Some(rebuild)) => Some(
                            tauri::async_runtime::spawn_blocking(move || rebuild())
                                .await
                                .unwrap_or(None),
                        ),
                        _ => None,
                    };
                    let result = result.map(|session| Opened {
                        session,
                        fell_back: Some(reason),
                        instruction,
                    });
                    let _ = tx.send(Msg::Opened(result));
                });
            }
        }
    }

    fn opened(&mut self, result: Result<Opened, OpenError>) {
        if self.finished {
            return;
        }
        let opened = match result {
            Ok(opened) => opened,
            Err(error) => {
                let reason = error.reason();
                self.fail_opening(&reason, None);
                return;
            }
        };
        self.session = Some(opened.session.clone());
        self.started = true;
        let reloaded = self.loading && opened.fell_back.is_none();
        self.loading = false;
        let replay = reloaded.then(|| std::mem::replace(&mut self.replay, Conversation::general()));
        self.general.session_opened(
            &self.key,
            self.generation,
            &opened.session,
            replay,
            opened.fell_back.as_deref(),
            opened.instruction,
        );
    }

    /// The last lines of omp's stderr so far, empty with no process.
    fn stderr_tail(&self) -> String {
        self.process
            .as_ref()
            .map(|process| process.stderr_text())
            .unwrap_or_default()
    }

    /// The session could not be opened: omp stopped, and the next message starts it again. A
    /// certificate or HTTP error in `reason`, the spawn error or the stderr tail is explained
    /// with the setting of this moment; the plain reasons stay otherwise.
    fn fail_opening(&mut self, reason: &str, details: Option<String>) {
        let tail = self.stderr_tail();
        let details = details.or_else(|| Some(tail.clone()).filter(|text| !text.is_empty()));
        let on = self.general.host.system_certificates_enabled();
        let shown = session::explained(
            reason,
            &[reason, details.as_deref().unwrap_or(""), &tail],
            on,
        );
        self.general
            .set_exit_details(&self.key, self.generation, details);
        self.dispatch(Event::Exited(shown));
        self.finished = true;
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
        self.end_tools();
        self.turn_running = false;
        self.conversation(|c| c.end_turn());
        match end {
            TurnEnd::Ended => self.dispatch(Event::TurnEnded),
            TurnEnd::Failed(reason) => self.dispatch(Event::TurnFailed(reason)),
            // The process is gone: its exit (reported on its own) says why.
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
                let on = self.general.host.system_certificates_enabled();
                tauri::async_runtime::spawn(async move {
                    let outcome = session::steer(&process.rpc, &session, &text, follow_up).await;
                    let outcome = session::explained_outcome(outcome, &process.stderr_text(), on);
                    let _ = tx.send(Msg::SteerDone(message, outcome));
                });
            }
            Cmd::Prompt { text } => self.start_turn(text),
            Cmd::Cancel => {
                if let (Some(process), Some(session)) = (&self.process, &self.session) {
                    process
                        .rpc
                        .notify("session/cancel", json!({ "sessionId": session }));
                }
            }
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
                } else if self.spawning {
                    // The process is being spawned: it is ended when it arrives, so the actor
                    // stays until then.
                    self.closing = true;
                    return;
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
                            let rev = self.conversation(|c| {
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
                    let (_, previous) = self.load.take().expect("the load was just matched");
                    self.load_answered(previous, result);
                } else if self.prompts.remove(&id) {
                    let on = self.general.host.system_certificates_enabled();
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
            let tool_call = params.get("toolCall").cloned().unwrap_or(Value::Null);
            if self.general.permission_requested(
                &self.key,
                self.generation,
                request,
                &tool_call,
                &options,
            ) {
                self.pending.insert(request, id);
            } else {
                // No turn is in progress: nobody can answer.
                process.rpc.respond(&id, session::cancelled());
            }
        } else if session::is_elicitation(method) {
            process.rpc.respond(
                &id,
                json!({ "action": "decline", "_meta": { "message": self.decline } }),
            );
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
        let on = self.general.host.system_certificates_enabled();
        let (reason, details) = process.exit_reason(&info, on);
        self.general
            .set_exit_details(&self.key, self.generation, details);
        self.dispatch(Event::Exited(reason));
        self.finished = true;
    }

    // -----------------------------------------------------------------------------------------
    // Sub-agents
    // -----------------------------------------------------------------------------------------

    /// What the conversation's rows say of the sub-agents changed: their transcripts may need
    /// reading, and the poller of the General decides which (`crate::subagents`).
    fn agents_changed(&mut self, rev: Option<u64>) {
        if let Some(rev) = rev.filter(|rev| *rev != self.agents_rev) {
            self.agents_rev = rev;
            self.general.wake_agents(&self.key);
        }
    }

    /// The tools of this omp stop counting for the lock of the File view.
    fn end_tools(&self) {
        if let Some(tools) = &self.tools {
            tools.end();
        }
    }

    fn cleanup(&mut self) {
        self.end_tools();
        if let Some(process) = &self.process {
            if !process.has_exited() {
                process.kill_now();
            }
        }
    }
}

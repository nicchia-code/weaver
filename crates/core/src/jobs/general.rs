//! General chat (`requirements/tech#general-chat`, `requirements/data#general-chat`): the pure part.
//!
//! General is the free conversation with omp of a project. It is not a job: it holds no Task,
//! announces no card, asks no question round and has no instruction, skill or written request.
//! What is decided without a process lives here:
//!
//! * [`GeneralMachine`], the Machine `general-chat` (events in, effects out);
//! * the command line and the environment of omp ([`args`], [`env`]) and the embedded
//!   instruction ([`instruction`]);
//! * the record `general/<project hash>.json` ([`GeneralRecord`], [`GeneralStore`]).
//!
//! ```text
//!   window.OPENED (a conversation is kept)        SESSION_RESUMED · SESSION_NOT_RESUMED
//!   Idle ─────────────────────────────────▶ Loading ───────────────────────────────▶ Idle
//!    │ ▲                                  (Reloading, or Waiting for omp until omp.READY)
//!    │ │ TURN_ENDED · TURN_FAILED · EXITED ·          └─ with messages waiting ─▶ Working
//!    │ │ STOP_AGENT · CLEAR_GENERAL · window.CLOSED
//!    │ └────────────────────────────────────┐
//!    └─────────────▶ Working ───────────────┘   SEND_MESSAGE (omp starts with the message)
//! ```
//!
//! Loading sends nothing to the agent: a message sent meanwhile waits, and goes out as a prompt
//! when the conversation has come back. START_FAILED and EXITED, Clear and window.CLOSED leave
//! Loading for Idle.
//!
//! Messages follow the rules of a job's conversation: while the agent works a message goes out at
//! once as a steer; while a permission waits it is held and goes out as a follow-up when the last
//! permission is answered; one that arrives while omp starts goes with the first prompt.

use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::launch::approval_mode;
use super::machine::{MessageId, PermissionEnd, RequestId, SteeringOutcome};
use crate::atomic;
use crate::paths::AppDirs;
use crate::settings::{ModelSettings, Permissions};

/// The States of the Machine (Loading has the two sub-states Reloading and Waiting for omp, told
/// apart by [`GeneralMachine::waiting_for_omp`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    /// No turn is in progress; the conversation may be empty.
    Idle,
    /// The window just opened and the kept conversation is coming back from omp; nothing is sent
    /// to the agent meanwhile, and the messages sent wait.
    Loading,
    /// The agent answers (omp may still be starting).
    Working,
}

/// Where omp stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Omp {
    /// Not running: the next message starts it.
    Off,
    /// Spawned and opening its session.
    Starting,
    /// The session is open.
    Open,
}

/// What happened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// `developer.SEND_MESSAGE`.
    SendMessage(MessageId),
    /// `developer.CLEAR_GENERAL`.
    Clear,
    /// omp opened (or reloaded) its session.
    SessionOpened,
    /// The reply of omp to the steer of a message.
    SteeringReplied(MessageId, SteeringOutcome),
    /// `omp.PERMISSION_REQUESTED`.
    PermissionRequested(RequestId),
    /// `developer.ANSWER_PERMISSION`.
    PermissionAnswered(RequestId),
    /// `agent.START_JOB`: a job was started (or refused) on the developer's request.
    StartJob,
    /// `omp.TURN_ENDED`.
    TurnEnded,
    /// `omp.TURN_FAILED`.
    TurnFailed(String),
    /// `omp.EXITED`, also when omp could not start or open its session.
    Exited(String),
    /// `window.OPENED` with a conversation kept: whether omp is ready decides between Reloading
    /// and Waiting for omp.
    WindowOpened { omp_ready: bool },
    /// `omp.READY`: omp-setup became Ready while Waiting for omp.
    OmpReady,
    /// `developer.STOP_AGENT`.
    StopAgent,
    /// `window.CLOSED`.
    WindowClosed,
}

/// What the app must do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    /// Spawn omp and open its session (reloading the kept one).
    StartOmp,
    /// Send the message as `_session/steering` with no `_meta`.
    Steer(MessageId),
    /// Send the held message as `_session/steering` with `_meta: {steering: {queue: "followUp"}}`.
    FollowUp(MessageId),
    /// Start a new turn: `session/prompt` with these messages.
    Prompt(Vec<MessageId>),
    /// The message reached the agent.
    Delivered(MessageId),
    /// The message did not reach the agent, with the reason shown under it.
    NotDelivered(MessageId, String),
    /// Close the pending permission requests.
    ClosePermissions(PermissionEnd),
    /// `session/cancel`: the turn in flight is cancelled; omp stays.
    Cancel,
    /// End omp in order (`session/cancel` when a turn runs, `session/close`, the signals).
    ShutDown { turn_running: bool },
    /// End omp as `ShutDown` does, delete the record and start the conversation again empty.
    Clear { turn_running: bool },
    /// The turn failed; omp stays.
    TurnFailed(String),
    /// omp stopped; the next message starts it again.
    Stopped(String),
}

/// The reasons shown under a message that never reached the agent.
pub const NOT_DELIVERED_STOPPED: &str = "the agent was stopped";
pub const NOT_DELIVERED_CLEARED: &str = "the conversation was cleared";
pub const NOT_DELIVERED_CLOSED: &str = "Weaver stopped before it was delivered";
pub const NOT_DELIVERED_EXITED: &str = "omp stopped";
pub const NOT_DELIVERED_FAILED: &str = "the turn failed";

/// The Machine `general-chat`, one per project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneralMachine {
    state: State,
    omp: Omp,
    /// A turn is in flight (a `session/prompt` has not answered yet).
    turn_running: bool,
    /// `session/cancel` was sent and the cancelled turn has not answered yet.
    cancelling: bool,
    /// Loading and omp is not ready yet (Waiting for omp); otherwise Reloading.
    waiting_for_omp: bool,
    permissions: BTreeSet<RequestId>,
    /// Messages waiting for the next prompt, or for the end of a permission.
    held: Vec<MessageId>,
    /// Steered and not answered yet.
    in_flight: BTreeSet<MessageId>,
}

impl Default for GeneralMachine {
    fn default() -> Self {
        Self::new()
    }
}

impl GeneralMachine {
    pub fn new() -> Self {
        Self {
            state: State::Idle,
            omp: Omp::Off,
            turn_running: false,
            cancelling: false,
            permissions: BTreeSet::new(),
            waiting_for_omp: false,
            held: Vec::new(),
            in_flight: BTreeSet::new(),
        }
    }

    pub fn state(&self) -> State {
        self.state
    }

    pub fn omp(&self) -> Omp {
        self.omp
    }

    /// Whether omp is spawned and opening its session (or waited for): what `Starting omp…` says.
    pub fn starting(&self) -> bool {
        self.omp == Omp::Starting
    }

    /// Loading, and omp is not ready yet: the Machine's Waiting for omp.
    pub fn waiting_for_omp(&self) -> bool {
        self.state == State::Loading && self.waiting_for_omp
    }

    pub fn turn_running(&self) -> bool {
        self.turn_running
    }

    /// Whether the agent may ask a permission or start a job now.
    pub fn accepts_agent_calls(&self) -> bool {
        self.state == State::Working && self.omp == Omp::Open
    }

    /// Applies `event`; returns what the app must do, in order.
    pub fn handle(&mut self, event: Event) -> Vec<Effect> {
        match event {
            Event::SendMessage(id) => self.send(id),
            Event::Clear => self.clear(),
            Event::WindowOpened { omp_ready } => self.window_opened(omp_ready),
            Event::OmpReady => self.omp_ready(),
            Event::SessionOpened => self.opened(),
            Event::SteeringReplied(id, outcome) => self.steering_replied(id, outcome),
            Event::PermissionRequested(request) => {
                if self.accepts_agent_calls() {
                    self.permissions.insert(request);
                }
                Vec::new()
            }
            Event::PermissionAnswered(request) => self.permission_answered(request),
            Event::StartJob => Vec::new(),
            Event::TurnEnded => self.turn_ended(),
            Event::TurnFailed(reason) => self.turn_failed(reason),
            Event::Exited(reason) => self.exited(reason),
            Event::StopAgent => self.stop(),
            Event::WindowClosed => self.window_closed(),
        }
    }

    /// The messages go out as one prompt.
    fn prompt_held(&mut self) -> Vec<Effect> {
        let held = std::mem::take(&mut self.held);
        self.state = State::Working;
        self.turn_running = true;
        let mut effects = vec![Effect::Prompt(held.clone())];
        effects.extend(held.into_iter().map(Effect::Delivered));
        effects
    }

    fn send(&mut self, id: MessageId) -> Vec<Effect> {
        match (self.state, self.omp) {
            (State::Loading, _) => {
                // The conversation is coming back: the message waits and is sent below it.
                self.held.push(id);
                Vec::new()
            }
            (State::Idle, Omp::Open) if !self.cancelling => {
                self.held.push(id);
                self.prompt_held()
            }
            (State::Idle, Omp::Off) => {
                self.state = State::Working;
                self.omp = Omp::Starting;
                self.held.push(id);
                vec![Effect::StartOmp]
            }
            (State::Working, Omp::Off) => {
                self.omp = Omp::Starting;
                self.held.push(id);
                vec![Effect::StartOmp]
            }
            (State::Idle, _) => {
                // omp starts, or the cancelled turn has not answered: the message waits for the next prompt.
                self.state = State::Working;
                self.held.push(id);
                Vec::new()
            }
            (State::Working, Omp::Starting) => {
                self.held.push(id);
                Vec::new()
            }
            (State::Working, Omp::Open) => {
                if self.cancelling || !self.permissions.is_empty() {
                    self.held.push(id);
                    Vec::new()
                } else {
                    self.in_flight.insert(id);
                    vec![Effect::Steer(id)]
                }
            }
        }
    }

    /// `window.OPENED` with a kept conversation: omp starts and reloads it, sending nothing. When
    /// omp is not ready it is waited for (`Event::OmpReady` ends the wait).
    fn window_opened(&mut self, omp_ready: bool) -> Vec<Effect> {
        if self.state != State::Idle || self.omp != Omp::Off || self.cancelling {
            return Vec::new();
        }
        self.state = State::Loading;
        self.omp = Omp::Starting;
        self.waiting_for_omp = !omp_ready;
        vec![Effect::StartOmp]
    }

    /// `omp.READY`: the wait of Waiting for omp is over, and omp reloads the kept conversation.
    fn omp_ready(&mut self) -> Vec<Effect> {
        if self.state == State::Loading {
            self.waiting_for_omp = false;
        }
        Vec::new()
    }

    fn opened(&mut self) -> Vec<Effect> {
        if self.omp != Omp::Starting {
            return Vec::new();
        }
        self.omp = Omp::Open;
        self.waiting_for_omp = false;
        if self.held.is_empty() {
            self.state = State::Idle;
            return Vec::new();
        }
        self.prompt_held()
    }

    fn permission_answered(&mut self, request: RequestId) -> Vec<Effect> {
        self.permissions.remove(&request);
        if !self.permissions.is_empty()
            || self.state != State::Working
            || !self.turn_running
            || self.held.is_empty()
        {
            return Vec::new();
        }
        // The held messages go out as follow-ups now that the wait is over.
        std::mem::take(&mut self.held)
            .into_iter()
            .map(|id| {
                self.in_flight.insert(id);
                Effect::FollowUp(id)
            })
            .collect()
    }

    fn steering_replied(&mut self, id: MessageId, outcome: SteeringOutcome) -> Vec<Effect> {
        if !self.in_flight.remove(&id) {
            return Vec::new();
        }
        let effects = match outcome {
            SteeringOutcome::Injected => vec![Effect::Delivered(id)],
            SteeringOutcome::PromptRequired if self.turn_running || self.cancelling => {
                // A turn is in flight after all: the message waits for the next one.
                self.held.push(id);
                Vec::new()
            }
            SteeringOutcome::PromptRequired => {
                self.held.push(id);
                self.prompt_held()
            }
            SteeringOutcome::Failed(reason) => vec![Effect::NotDelivered(id, reason)],
        };
        self.idle_when_nothing_is_left();
        effects
    }

    fn idle_when_nothing_is_left(&mut self) {
        if self.state == State::Working
            && !self.turn_running
            && self.omp == Omp::Open
            && self.in_flight.is_empty()
            && self.held.is_empty()
        {
            self.state = State::Idle;
        }
    }

    fn turn_ended(&mut self) -> Vec<Effect> {
        self.turn_running = false;
        self.cancelling = false;
        // A permission still asked is void.
        let mut effects = self.close_permissions(PermissionEnd::Void);
        if self.omp == Omp::Open && !self.held.is_empty() {
            effects.extend(self.prompt_held());
        } else if self.in_flight.is_empty() {
            self.state = State::Idle;
        }
        effects
    }

    fn turn_failed(&mut self, reason: String) -> Vec<Effect> {
        self.turn_running = false;
        self.cancelling = false;
        let mut effects = self.close_permissions(PermissionEnd::Void);
        effects.extend(self.discard_messages(NOT_DELIVERED_FAILED));
        self.state = State::Idle;
        effects.push(Effect::TurnFailed(reason));
        effects
    }

    fn exited(&mut self, reason: String) -> Vec<Effect> {
        // omp is gone already: no shutdown to run, and a permission asked is void.
        let mut effects = self.close_permissions(PermissionEnd::Void);
        effects.extend(self.discard_messages(NOT_DELIVERED_EXITED));
        self.omp = Omp::Off;
        self.waiting_for_omp = false;
        self.turn_running = false;
        self.cancelling = false;
        self.state = State::Idle;
        effects.push(Effect::Stopped(reason));
        effects
    }

    fn stop(&mut self) -> Vec<Effect> {
        // Loading has no turn to stop: the conversation is coming back, and Clear ends it.
        if matches!(self.state, State::Idle | State::Loading) {
            return Vec::new();
        }
        let mut effects = self.close_permissions(PermissionEnd::Cancelled);
        effects.extend(self.discard_messages(NOT_DELIVERED_STOPPED));
        match self.omp {
            Omp::Starting | Omp::Off => {
                // omp never opened: it ends, the next message starts it again.
                if self.omp == Omp::Starting {
                    effects.push(Effect::ShutDown {
                        turn_running: false,
                    });
                }
                self.omp = Omp::Off;
            }
            Omp::Open => {
                if self.turn_running {
                    self.cancelling = true;
                    effects.push(Effect::Cancel);
                }
            }
        }
        self.turn_running = false;
        self.state = State::Idle;
        effects
    }

    fn window_closed(&mut self) -> Vec<Effect> {
        let mut effects = self.close_permissions(PermissionEnd::Cancelled);
        effects.extend(self.discard_messages(NOT_DELIVERED_CLOSED));
        if self.omp != Omp::Off {
            effects.push(Effect::ShutDown {
                turn_running: self.turn_running,
            });
        }
        self.reset();
        effects
    }

    fn clear(&mut self) -> Vec<Effect> {
        let mut effects = self.close_permissions(PermissionEnd::Cancelled);
        effects.extend(self.discard_messages(NOT_DELIVERED_CLEARED));
        effects.push(Effect::Clear {
            turn_running: self.turn_running,
        });
        self.reset();
        effects
    }

    fn reset(&mut self) {
        self.state = State::Idle;
        self.omp = Omp::Off;
        self.waiting_for_omp = false;
        self.turn_running = false;
        self.cancelling = false;
    }

    fn close_permissions(&mut self, how: PermissionEnd) -> Vec<Effect> {
        if self.permissions.is_empty() {
            return Vec::new();
        }
        self.permissions.clear();
        vec![Effect::ClosePermissions(how)]
    }

    fn discard_messages(&mut self, reason: &str) -> Vec<Effect> {
        let ids: Vec<MessageId> = std::mem::take(&mut self.held)
            .into_iter()
            .chain(std::mem::take(&mut self.in_flight))
            .collect();
        ids.into_iter()
            .map(|id| Effect::NotDelivered(id, reason.to_owned()))
            .collect()
    }
}

// ---------------------------------------------------------------------------------------------
// How omp is started
// ---------------------------------------------------------------------------------------------

/// The arguments after the program: `[--model <analysis>] --approval-mode <mode>
/// --append-system-prompt <file> acp`. No `--advisor`, no `--slow`, no overlay.
pub fn args(
    models: &ModelSettings,
    permissions: Permissions,
    instruction_file: &std::path::Path,
) -> Vec<String> {
    let mut args = Vec::new();
    let model = models.analysis.trim();
    if !model.is_empty() {
        args.push("--model".to_owned());
        args.push(model.to_owned());
    }
    args.push("--approval-mode".to_owned());
    args.push(approval_mode(permissions).to_owned());
    args.push("--append-system-prompt".to_owned());
    args.push(instruction_file.to_string_lossy().into_owned());
    args.push("acp".to_owned());
    args
}

/// The variables Weaver adds to the environment of omp: `WEAVER_GENERAL` and `WEAVER_ENDPOINT`, and
/// never `WEAVER_JOB`, so that `weaver` refuses the commands tied to a job.
pub fn env(identity: &str, endpoint: &str) -> BTreeMap<String, String> {
    BTreeMap::from([
        ("WEAVER_GENERAL".to_owned(), identity.to_owned()),
        ("WEAVER_ENDPOINT".to_owned(), endpoint.to_owned()),
    ])
}

/// The file the instruction is written to: `<app local data>/general.md`.
pub fn instruction_file(dirs: &AppDirs) -> PathBuf {
    dirs.local_data_dir().join("general.md")
}

/// The language the developer set, trimmed; none when it is empty or holds a control character
/// (so that the sentence it ends can never grow a line break).
pub fn clean_language(language: Option<&str>) -> Option<&str> {
    language
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.chars().any(char::is_control))
}

/// The instruction of General's agent, appended to omp's system prompt. Fixed text, and the
/// language sentence only when a language is set. It never names the first person.
pub fn instruction(language: Option<&str>) -> String {
    let mut text = String::from(INSTRUCTION);
    if let Some(language) = clean_language(language) {
        text.push_str(&format!(
            "\nWrite the texts for the developer in {language}.\n"
        ));
    }
    text
}

const INSTRUCTION: &str = "\
# Weaver General

This is General, the free conversation of a developer with the agent in a project that Weaver manages. The project keeps its Model, Requirements, Proposals, Tasks and Drifts in `.weaver/`.

- Do what the developer asks, directly in this conversation. No skill and no written request is attached to it. Weaver's job skills (`weaver-setup`, `weaver-design`, `weaver-import`, `weaver-scan`, `weaver-compare`, `weaver-implement`, `weaver-resolve-drifts`) work only inside a job: never load or follow them here. The Requirement skills `weaver-req-*` apply here as everywhere.
- Ask the developer in the reply, in plain text, and wait for the answer. A form or a question tool is declined here, and `weaver question new` does not work here.
- General is not a job. The commands that belong to a job (`weaver task move`, `weaver announcement new`, `weaver job edit`, `weaver question new`) are refused here.
- A change of the Model or the Requirements is never designed in this conversation and no Proposal is written here. The developer writes a design request with `+ Work Item` on the Board and sends it, or asks here to send one already written; Weaver's design job then interviews the developer and writes the Proposal.
- Start a job of Weaver with `weaver job new` only when the developer explicitly asks for that job in this conversation:
  - `weaver job new --input '{\"kind\":\"scan\"}'` scans the whole System;
  - `weaver job new --input '{\"kind\":\"run\",\"task\":\"<task>\"}'` runs one Task, and without `task` runs every runnable Task;
  - `weaver job new --input '{\"kind\":\"resolve\",\"drift\":\"<drift>\"}'` resolves one Drift;
  - `weaver job new --input '{\"kind\":\"design\",\"request\":\"<request>\"}'` sends a design request the developer already wrote and did not send.
- Weaver applies its own refusals to the start of a job. When one is refused, tell the developer the reason from the output of the command. Do not start the job in another way.
- A job started this way works on its own, with its own conversation in the Jobs view. Tell the developer which job was started, and do not wait for it to end.
";

// ---------------------------------------------------------------------------------------------
// The record
// ---------------------------------------------------------------------------------------------

/// `general/<project hash>.json`: the session omp opened for the project's General. Weaver keeps no
/// text of the conversation: omp keeps it in its session files.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralRecord {
    /// The canonical root of the project.
    pub project: String,
    pub session_id: String,
    /// UTC ISO 8601 with milliseconds: when omp opened or reloaded the session.
    pub updated: String,
}

/// The records of the General conversations, one file per project.
#[derive(Debug, Clone)]
pub struct GeneralStore {
    dir: PathBuf,
}

impl GeneralStore {
    pub fn new(dirs: &AppDirs) -> Self {
        Self {
            dir: dirs.local_data_dir().join("general"),
        }
    }

    pub fn file(&self, project_hash: &str) -> PathBuf {
        self.dir.join(format!("{project_hash}.json"))
    }

    /// Writes the record atomically (mode `0600` on Unix).
    pub fn write(&self, project_hash: &str, record: &GeneralRecord) -> io::Result<()> {
        let mut json = serde_json::to_vec_pretty(record).map_err(io::Error::other)?;
        json.push(b'\n');
        atomic::write(&self.file(project_hash), &json, Some(0o600))
    }

    /// The record of the project, `None` when there is none. An unreadable one is an error.
    pub fn read(&self, project_hash: &str) -> io::Result<Option<GeneralRecord>> {
        match std::fs::read(self.file(project_hash)) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map(Some)
                .map_err(io::Error::other),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error),
        }
    }

    /// Deletes the record; a missing one is not an error.
    pub fn delete(&self, project_hash: &str) -> io::Result<()> {
        match std::fs::remove_file(self.file(project_hash)) {
            Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Effect::*;
    use super::*;

    fn models(analysis: &str) -> ModelSettings {
        ModelSettings {
            analysis: analysis.into(),
            scan: "s/m".into(),
            execution: "e/m".into(),
        }
    }

    /// A machine whose omp is open and idle.
    fn idle_open() -> GeneralMachine {
        let mut machine = GeneralMachine::new();
        assert_eq!(machine.handle(Event::SendMessage(1)), [StartOmp]);
        assert_eq!(
            machine.handle(Event::SessionOpened),
            [Prompt(vec![1]), Delivered(1)]
        );
        assert!(machine.handle(Event::TurnEnded).is_empty());
        assert_eq!((machine.state(), machine.omp()), (State::Idle, Omp::Open));
        machine
    }

    fn working() -> GeneralMachine {
        let mut machine = idle_open();
        assert_eq!(
            machine.handle(Event::SendMessage(2)),
            [Prompt(vec![2]), Delivered(2)]
        );
        assert_eq!(machine.state(), State::Working);
        machine
    }

    #[test]
    fn the_first_message_starts_omp_and_goes_out_with_the_first_prompt() {
        let mut machine = GeneralMachine::new();
        assert_eq!((machine.state(), machine.omp()), (State::Idle, Omp::Off));
        assert_eq!(machine.handle(Event::SendMessage(1)), [StartOmp]);
        assert_eq!(
            (machine.state(), machine.omp(), machine.starting()),
            (State::Working, Omp::Starting, true)
        );
        // Messages sent while omp opens its session wait for the same prompt.
        assert!(machine.handle(Event::SendMessage(2)).is_empty());
        assert_eq!(
            machine.handle(Event::SessionOpened),
            [Prompt(vec![1, 2]), Delivered(1), Delivered(2)]
        );
        assert!(machine.turn_running() && !machine.starting());
        assert!(machine.handle(Event::TurnEnded).is_empty());
        assert_eq!(machine.state(), State::Idle);
    }

    #[test]
    fn omp_stays_for_the_next_message_and_a_message_while_it_works_is_steered() {
        let mut machine = idle_open();
        assert_eq!(
            machine.handle(Event::SendMessage(2)),
            [Prompt(vec![2]), Delivered(2)],
            "no second start"
        );
        assert_eq!(machine.handle(Event::SendMessage(3)), [Steer(3)]);
        assert_eq!(
            machine.handle(Event::SteeringReplied(3, SteeringOutcome::Injected)),
            [Delivered(3)]
        );
        assert!(machine.handle(Event::TurnEnded).is_empty());
        assert_eq!(machine.state(), State::Idle);
    }

    #[test]
    fn a_steer_the_agent_cannot_take_starts_a_turn_or_fails() {
        let mut machine = working();
        assert_eq!(machine.handle(Event::SendMessage(3)), [Steer(3)]);
        // The turn ended before the answer: the message starts the next one.
        assert!(machine.handle(Event::TurnEnded).is_empty());
        assert_eq!(machine.state(), State::Working, "an answer is awaited");
        assert_eq!(
            machine.handle(Event::SteeringReplied(3, SteeringOutcome::PromptRequired)),
            [Prompt(vec![3]), Delivered(3)]
        );
        assert_eq!(machine.handle(Event::SendMessage(4)), [Steer(4)]);
        assert_eq!(
            machine.handle(Event::SteeringReplied(
                4,
                SteeringOutcome::Failed("no turn".into())
            )),
            [NotDelivered(4, "no turn".into())]
        );
        assert!(
            machine
                .handle(Event::SteeringReplied(9, SteeringOutcome::Injected))
                .is_empty(),
            "an unknown message is ignored"
        );
    }

    #[test]
    fn a_permission_stays_in_working_and_holds_the_messages_until_it_is_answered() {
        let mut machine = working();
        assert!(machine.handle(Event::PermissionRequested(7)).is_empty());
        assert_eq!(machine.state(), State::Working);
        assert!(
            machine.handle(Event::SendMessage(3)).is_empty(),
            "held while the permission waits"
        );
        assert_eq!(machine.handle(Event::PermissionAnswered(7)), [FollowUp(3)]);
        assert_eq!(machine.state(), State::Working);
        // A second permission: nothing goes out until the last is answered.
        assert!(machine.handle(Event::PermissionRequested(8)).is_empty());
        assert!(machine.handle(Event::PermissionRequested(9)).is_empty());
        assert!(machine.handle(Event::SendMessage(4)).is_empty());
        assert!(machine.handle(Event::PermissionAnswered(8)).is_empty());
        assert_eq!(machine.handle(Event::PermissionAnswered(9)), [FollowUp(4)]);
    }

    #[test]
    fn a_permission_still_asked_when_the_turn_ends_is_void_and_the_held_messages_start_a_turn() {
        let mut machine = working();
        machine.handle(Event::PermissionRequested(7));
        machine.handle(Event::SendMessage(3));
        assert_eq!(
            machine.handle(Event::TurnEnded),
            [
                ClosePermissions(PermissionEnd::Void),
                Prompt(vec![3]),
                Delivered(3)
            ]
        );
        assert_eq!(machine.state(), State::Working);
    }

    #[test]
    fn an_agent_call_is_accepted_only_while_working() {
        let mut machine = idle_open();
        assert!(!machine.accepts_agent_calls());
        // A permission asked while Idle is not kept.
        machine.handle(Event::PermissionRequested(1));
        assert_eq!(
            machine.handle(Event::SendMessage(2)),
            [Prompt(vec![2]), Delivered(2)]
        );
        assert!(machine.accepts_agent_calls());
        assert!(machine.handle(Event::StartJob).is_empty());
        assert_eq!(
            machine.state(),
            State::Working,
            "START_JOB stays in Working"
        );
        assert!(machine.handle(Event::TurnEnded).is_empty());
        assert!(!machine.accepts_agent_calls());
    }

    #[test]
    fn a_failed_turn_goes_idle_with_its_reason_and_omp_stays() {
        let mut machine = working();
        machine.handle(Event::PermissionRequested(7));
        machine.handle(Event::SendMessage(3));
        assert_eq!(
            machine.handle(Event::TurnFailed("the agent refused".into())),
            [
                ClosePermissions(PermissionEnd::Void),
                NotDelivered(3, NOT_DELIVERED_FAILED.into()),
                TurnFailed("the agent refused".into())
            ]
        );
        assert_eq!((machine.state(), machine.omp()), (State::Idle, Omp::Open));
        assert_eq!(
            machine.handle(Event::SendMessage(4)),
            [Prompt(vec![4]), Delivered(4)]
        );
    }

    #[test]
    fn an_exit_goes_idle_with_its_reason_and_the_next_message_starts_omp_again() {
        let mut machine = working();
        machine.handle(Event::SendMessage(3));
        assert_eq!(
            machine.handle(Event::Exited("omp exited with code 3".into())),
            [
                NotDelivered(3, NOT_DELIVERED_EXITED.into()),
                Stopped("omp exited with code 3".into())
            ]
        );
        assert_eq!((machine.state(), machine.omp()), (State::Idle, Omp::Off));
        assert_eq!(machine.handle(Event::SendMessage(4)), [StartOmp]);
        // omp that never opened its session: the same.
        assert_eq!(
            machine.handle(Event::Exited("could not start omp".into())),
            [
                NotDelivered(4, NOT_DELIVERED_EXITED.into()),
                Stopped("could not start omp".into())
            ]
        );
        // An exit while idle shows its reason too.
        let mut idle = idle_open();
        assert_eq!(
            idle.handle(Event::Exited("gone".into())),
            [Stopped("gone".into())]
        );
        assert_eq!(idle.omp(), Omp::Off);
    }

    #[test]
    fn stop_cancels_the_turn_keeps_omp_and_holds_the_next_message_until_the_cancelled_turn_answers()
    {
        let mut machine = working();
        machine.handle(Event::PermissionRequested(7));
        machine.handle(Event::SendMessage(3));
        assert_eq!(
            machine.handle(Event::StopAgent),
            [
                ClosePermissions(PermissionEnd::Cancelled),
                NotDelivered(3, NOT_DELIVERED_STOPPED.into()),
                Cancel
            ]
        );
        assert_eq!((machine.state(), machine.omp()), (State::Idle, Omp::Open));
        // A message before the cancelled turn answered waits for it.
        assert!(machine.handle(Event::SendMessage(4)).is_empty());
        assert_eq!(machine.state(), State::Working);
        assert_eq!(
            machine.handle(Event::TurnEnded),
            [Prompt(vec![4]), Delivered(4)]
        );
        // Stop with nothing to stop does nothing.
        let mut idle = idle_open();
        assert!(idle.handle(Event::StopAgent).is_empty());
    }

    #[test]
    fn the_cancelled_turn_answering_alone_leaves_idle() {
        let mut machine = working();
        machine.handle(Event::StopAgent);
        assert!(machine.handle(Event::TurnEnded).is_empty());
        assert_eq!(machine.state(), State::Idle);
        assert_eq!(
            machine.handle(Event::SendMessage(5)),
            [Prompt(vec![5]), Delivered(5)]
        );
    }

    #[test]
    fn stop_while_omp_starts_ends_it() {
        let mut machine = GeneralMachine::new();
        machine.handle(Event::SendMessage(1));
        assert_eq!(
            machine.handle(Event::StopAgent),
            [
                NotDelivered(1, NOT_DELIVERED_STOPPED.into()),
                ShutDown {
                    turn_running: false
                }
            ]
        );
        assert_eq!((machine.state(), machine.omp()), (State::Idle, Omp::Off));
        assert_eq!(machine.handle(Event::SendMessage(2)), [StartOmp]);
    }

    #[test]
    fn closing_the_window_cancels_the_turn_and_ends_omp() {
        let mut machine = working();
        machine.handle(Event::PermissionRequested(7));
        assert_eq!(
            machine.handle(Event::WindowClosed),
            [
                ClosePermissions(PermissionEnd::Cancelled),
                ShutDown { turn_running: true }
            ]
        );
        assert_eq!((machine.state(), machine.omp()), (State::Idle, Omp::Off));
        // omp idle and open is ended too; with omp off nothing runs.
        let mut idle = idle_open();
        assert_eq!(
            idle.handle(Event::WindowClosed),
            [ShutDown {
                turn_running: false
            }]
        );
        assert!(idle.handle(Event::WindowClosed).is_empty());
        assert_eq!(idle.handle(Event::SendMessage(9)), [StartOmp]);
    }

    #[test]
    fn clear_starts_the_conversation_again_and_stops_a_turn_in_progress() {
        let mut idle = idle_open();
        assert_eq!(
            idle.handle(Event::Clear),
            [Clear {
                turn_running: false
            }]
        );
        assert_eq!((idle.state(), idle.omp()), (State::Idle, Omp::Off));
        let mut machine = working();
        machine.handle(Event::SendMessage(3));
        assert_eq!(
            machine.handle(Event::Clear),
            [
                NotDelivered(3, NOT_DELIVERED_CLEARED.into()),
                Clear { turn_running: true }
            ]
        );
        assert_eq!(machine.handle(Event::SendMessage(4)), [StartOmp]);
    }

    /// A machine that is loading a kept conversation, omp ready.
    fn reloading() -> GeneralMachine {
        let mut machine = GeneralMachine::new();
        assert_eq!(
            machine.handle(Event::WindowOpened { omp_ready: true }),
            [StartOmp]
        );
        machine
    }

    #[test]
    fn opening_the_window_with_a_kept_conversation_starts_omp_and_reloads_it_sending_nothing() {
        let mut machine = reloading();
        assert_eq!(
            (machine.state(), machine.omp(), machine.starting()),
            (State::Loading, Omp::Starting, true)
        );
        assert!(!machine.waiting_for_omp() && !machine.turn_running());
        // The conversation came back and no message waits: Idle, omp stays for the next message.
        assert!(machine.handle(Event::SessionOpened).is_empty());
        assert_eq!((machine.state(), machine.omp()), (State::Idle, Omp::Open));
        assert_eq!(
            machine.handle(Event::SendMessage(1)),
            [Prompt(vec![1]), Delivered(1)]
        );
    }

    #[test]
    fn opening_the_window_while_omp_is_not_ready_waits_for_omp_and_reloads_when_it_is_ready() {
        let mut machine = GeneralMachine::new();
        assert_eq!(
            machine.handle(Event::WindowOpened { omp_ready: false }),
            [StartOmp]
        );
        assert_eq!(machine.state(), State::Loading);
        assert!(machine.waiting_for_omp());
        assert!(machine.handle(Event::OmpReady).is_empty());
        assert_eq!(machine.state(), State::Loading);
        assert!(
            !machine.waiting_for_omp(),
            "Reloading, with no message needed"
        );
        assert!(machine.handle(Event::SessionOpened).is_empty());
        assert_eq!(machine.state(), State::Idle);
        // omp becoming ready outside of the wait changes nothing.
        assert!(machine.handle(Event::OmpReady).is_empty());
        assert_eq!((machine.state(), machine.omp()), (State::Idle, Omp::Open));
    }

    #[test]
    fn the_window_opening_again_while_omp_runs_or_a_turn_works_starts_nothing() {
        let mut machine = idle_open();
        assert!(machine
            .handle(Event::WindowOpened { omp_ready: true })
            .is_empty());
        assert_eq!((machine.state(), machine.omp()), (State::Idle, Omp::Open));
        let mut machine = working();
        assert!(machine
            .handle(Event::WindowOpened { omp_ready: true })
            .is_empty());
        assert_eq!(machine.state(), State::Working);
    }

    #[test]
    fn messages_sent_while_loading_wait_in_order_and_go_out_when_the_conversation_is_back() {
        let mut machine = reloading();
        assert!(machine.handle(Event::SendMessage(1)).is_empty());
        assert!(machine.handle(Event::SendMessage(2)).is_empty());
        assert_eq!(
            (machine.state(), machine.omp(), machine.starting()),
            (State::Loading, Omp::Starting, true),
            "the Machine stays in Loading, never Working"
        );
        assert_eq!(
            machine.handle(Event::SessionOpened),
            [Prompt(vec![1, 2]), Delivered(1), Delivered(2)]
        );
        assert_eq!(machine.state(), State::Working);
        assert!(machine.turn_running());
        assert!(machine.handle(Event::TurnEnded).is_empty());
        assert_eq!(machine.state(), State::Idle);
    }

    #[test]
    fn messages_wait_the_same_while_omp_is_waited_for_and_an_empty_session_starts_them_too() {
        // SESSION_NOT_RESUMED reaches the Machine as the same event: the session that opened is
        // an empty one, and the waiting messages go out.
        let mut machine = GeneralMachine::new();
        machine.handle(Event::WindowOpened { omp_ready: false });
        assert!(machine.handle(Event::SendMessage(1)).is_empty());
        machine.handle(Event::OmpReady);
        assert_eq!(
            machine.handle(Event::SessionOpened),
            [Prompt(vec![1]), Delivered(1)]
        );
        assert_eq!(machine.state(), State::Working);
    }

    #[test]
    fn a_failed_start_or_an_exit_while_loading_goes_idle_and_the_next_message_starts_omp_again() {
        for reason in ["could not start omp", "omp exited with code 1"] {
            let mut machine = reloading();
            machine.handle(Event::SendMessage(1));
            assert_eq!(
                machine.handle(Event::Exited(reason.into())),
                [
                    NotDelivered(1, NOT_DELIVERED_EXITED.into()),
                    Stopped(reason.into())
                ]
            );
            assert_eq!((machine.state(), machine.omp()), (State::Idle, Omp::Off));
            assert!(!machine.waiting_for_omp());
            assert_eq!(machine.handle(Event::SendMessage(2)), [StartOmp]);
        }
    }

    #[test]
    fn clear_while_loading_ends_that_omp_drops_the_waiting_messages_and_starts_empty() {
        let mut machine = reloading();
        machine.handle(Event::SendMessage(1));
        assert_eq!(
            machine.handle(Event::Clear),
            [
                NotDelivered(1, NOT_DELIVERED_CLEARED.into()),
                Clear {
                    turn_running: false
                }
            ]
        );
        assert_eq!((machine.state(), machine.omp()), (State::Idle, Omp::Off));
        assert_eq!(machine.handle(Event::SendMessage(2)), [StartOmp]);
    }

    #[test]
    fn closing_the_window_while_loading_ends_omp_and_a_stop_there_does_nothing() {
        let mut machine = reloading();
        machine.handle(Event::SendMessage(1));
        assert!(machine.handle(Event::StopAgent).is_empty());
        assert_eq!(machine.state(), State::Loading);
        assert_eq!(
            machine.handle(Event::WindowClosed),
            [
                NotDelivered(1, NOT_DELIVERED_CLOSED.into()),
                ShutDown {
                    turn_running: false
                }
            ]
        );
        assert_eq!((machine.state(), machine.omp()), (State::Idle, Omp::Off));
        // The window opens again: the kept conversation loads again.
        assert_eq!(
            machine.handle(Event::WindowOpened { omp_ready: true }),
            [StartOmp]
        );
    }

    #[test]
    fn omp_runs_with_the_analysis_model_the_approval_mode_and_the_instruction_file_and_nothing_else(
    ) {
        let file = std::path::Path::new("/data/general.md");
        assert_eq!(
            args(&models(" a/m:high "), Permissions::Yolo, file),
            [
                "--model",
                "a/m:high",
                "--approval-mode",
                "yolo",
                "--append-system-prompt",
                "/data/general.md",
                "acp"
            ]
        );
        assert_eq!(
            args(&models(""), Permissions::Ask, file),
            [
                "--approval-mode",
                "always-ask",
                "--append-system-prompt",
                "/data/general.md",
                "acp"
            ]
        );
        let line = args(&models("a/m"), Permissions::Yolo, file).join(" ");
        for forbidden in ["--advisor", "--slow", "--config"] {
            assert!(!line.contains(forbidden), "{forbidden}");
        }
    }

    #[test]
    fn the_environment_names_general_and_never_a_job() {
        let env = env("gggggggg", "/run/weaver.sock");
        assert_eq!(
            env.keys().map(String::as_str).collect::<Vec<_>>(),
            ["WEAVER_ENDPOINT", "WEAVER_GENERAL"]
        );
        assert_eq!(env["WEAVER_GENERAL"], "gggggggg");
    }

    #[test]
    fn the_instruction_is_fixed_text_with_the_language_only_when_one_is_set() {
        let plain = instruction(None);
        assert!(plain.contains("weaver job new") && plain.contains("explicitly asks"));
        assert!(!plain.contains("Write the texts"));
        assert_eq!(instruction(Some("  ")), plain);
        assert!(instruction(Some("Italian"))
            .ends_with("Write the texts for the developer in Italian.\n"));
        assert_eq!(
            instruction(Some("Ita\nlian")),
            plain,
            "a language with a control character adds nothing"
        );
        // No first person, as every text Weaver writes for the agent.
        let lower = plain.to_lowercase();
        for word in [" i ", " we ", " my ", " our ", "i'm", "i'll"] {
            assert!(!lower.contains(word), "{word:?}");
        }
    }

    #[test]
    fn the_record_is_written_read_and_deleted_per_project() {
        let base = tempfile::tempdir().unwrap();
        let dirs =
            AppDirs::from_bases(&base.path().join("config"), &base.path().join("data"), None);
        let store = GeneralStore::new(&dirs);
        assert_eq!(store.read("0123456789abcdef").unwrap(), None);
        let record = GeneralRecord {
            project: "/p".into(),
            session_id: "s1".into(),
            updated: "2026-11-02T07:00:00.000Z".into(),
        };
        store.write("0123456789abcdef", &record).unwrap();
        assert_eq!(
            store.read("0123456789abcdef").unwrap(),
            Some(record.clone())
        );
        assert_eq!(
            store.read("fedcba9876543210").unwrap(),
            None,
            "another project has no record"
        );
        let text = std::fs::read_to_string(store.file("0123456789abcdef")).unwrap();
        assert!(
            text.contains("\"sessionId\"")
                && text.contains("\"updated\"")
                && text.contains("\"project\"")
        );
        store.delete("0123456789abcdef").unwrap();
        store.delete("0123456789abcdef").unwrap();
        assert_eq!(store.read("0123456789abcdef").unwrap(), None);
        std::fs::create_dir_all(store.file("x").parent().unwrap()).unwrap();
        std::fs::write(store.file("bad"), "{").unwrap();
        assert!(store.read("bad").is_err());
    }
}

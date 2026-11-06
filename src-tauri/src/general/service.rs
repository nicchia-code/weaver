//! [`General`]: the General of every open project and the Ask more conversations, over the Machine
//! `general-chat`.
//!
//! All state lives here, behind one lock. A conversation is a [`Slot`] kept under a key: the
//! project hash for the General of a project, the identity generated at opening for an Ask more
//! conversation (`requirements/tech#ask-conversation`, which is not a job: it has no record, no
//! registry entry and never enters admission). Everything that changes the state of a slot goes
//! through [`General::dispatch`]: the pure Machine decides ([`weaver_core::jobs::general`]), the
//! conversation and the record are updated, and the effects are carried out: bookkeeping here, I/O
//! by the actor of the omp process ([`super::actor`]). The events to the UI follow each change.
//!
//! A General outlives its window in memory: closing the window ends omp and keeps the conversation
//! and the record, so the window that opens again finds both. Quitting Weaver keeps the record,
//! and the conversation comes back when omp reloads the session. An Ask more conversation keeps
//! nothing: it ends with the window.

use std::collections::HashMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::mpsc as std_mpsc;
use std::sync::{Arc, Mutex, MutexGuard, RwLock, Weak};

use serde_json::Value;
use tokio::sync::{mpsc, watch};
use weaver_core::assets::tags;
use weaver_core::jobs::ask;
use weaver_core::jobs::general::{
    self as machine, Effect, GeneralMachine, GeneralRecord, GeneralStore, State,
};
use weaver_core::jobs::machine::{MessageId, PermissionEnd, RequestId};
use weaver_core::paths::project_hash_of_canonical;
use weaver_core::project::clock::{now_instant, Clock, SystemClock};
use weaver_core::project::ids;
use weaver_core::project::random::OsRandom;

use super::actor::{self, Cmd, Msg};
use super::wire::{
    AskClosed, AskEntries, AskKind, AskRow, AskSnapshot, AskStatus, GeneralEntries, GeneralEvents,
    GeneralSnapshot, GeneralState,
};
use crate::jobs::authority::GeneralAuthority;
use crate::jobs::conversation::{Conversation, Delivery, PermissionState};
use crate::jobs::host::Host;
use crate::subagents::poller::{Access, Poller};
use crate::subagents::reader::{Done, Location};
use crate::subagents::tree::Plan;
use crate::subagents::wire::{Outgoing, Owner, Step, SubAgentConversation, SubAgentsEvent};
use crate::subagents::Agents;
use crate::{log_info, log_warn};

/// Who labels the assignment a sub-agent of General received.
const AGENTS_OWNER: &str = "Chat";

/// Builds the instruction of an Ask more conversation again from the files, when its session was
/// lost and replaced: `None` when the chosen thing no longer exists.
pub type Rebuild = Arc<dyn Fn() -> Option<String> + Send + Sync>;

/// What an Ask more conversation is: opened about one thing, never a job.
struct AskSlot {
    project_id: String,
    name: String,
    kind: AskKind,
    /// The instant it was opened.
    opened: String,
    /// Grows with the openings: the most recent conversation has the greatest.
    order: u64,
    /// The question round it was opened from.
    round: Option<u64>,
    /// The instruction the next prompt carries: prepared at opening, until a prompt took it.
    instruction: Option<String>,
    rebuild: Rebuild,
    /// The session omp opened, kept in memory to reload it after omp stopped.
    session: Option<String>,
}

/// What opens an Ask more conversation.
pub struct AskOpening {
    pub project_id: String,
    pub root: PathBuf,
    pub name: String,
    pub kind: AskKind,
    pub round: Option<u64>,
    pub instruction: String,
    pub rebuild: Rebuild,
}

/// The thread that writes the records, one after the other.
enum Write {
    Record(String, GeneralRecord),
    Delete(String),
    Flush(std_mpsc::Sender<()>),
}

/// What the Chat tells the rest of the app: the Events of the Machine `general-chat` that others
/// receive. Called on a thread of their own, in the order they happened and never with the lock
/// of the service held, so that an observer may ask the Chat anything.
pub trait ChatObserver: Send + Sync {
    /// `general-chat.WAITING`: the agent of the Chat of the project starts waiting for the answers
    /// of its round.
    fn chat_waiting(&self, _project_id: &str) {}

    /// The round the agent of the Chat of the project waited for ends without answers: its turn
    /// failed or ended, omp exited, or the developer stopped it, cleared the conversation or
    /// closed the window. A round that is closed in the normal way is not reported here.
    fn chat_round_ended(&self, _project_id: &str) {}

    /// The round the agent of the Chat waited for was closed in the normal way (answered, or
    /// skipped by the developer): the Chat goes on in the same turn.
    fn chat_round_closed(&self, _project_id: &str) {}
}

/// What the thread of the observers is told.
enum Notice {
    Waiting(String),
    RoundEnded(String),
    RoundClosed(String),
    Flush(std_mpsc::Sender<()>),
}

/// The General of one project, or an Ask more conversation.
struct Slot {
    root: PathBuf,
    /// The record was read: the General takes messages.
    ready: bool,
    machine: GeneralMachine,
    conversation: Conversation,
    /// The conversation in memory is the one of the session: a session that was kept and not
    /// reloaded yet is not shown, and the replay of its history fills the conversation.
    loaded: bool,
    /// The kept session.
    record: Option<GeneralRecord>,
    /// The identity of the live omp (`WEAVER_GENERAL`, or `WEAVER_ASK` for an Ask more
    /// conversation, where it is the identity of the conversation).
    identity: Option<String>,
    /// The actor of the latest start: what an older actor reports is ignored.
    generation: u64,
    channel: Option<mpsc::UnboundedSender<Msg>>,
    /// `true` once an actor has ended; every start adds one.
    done: Vec<watch::Receiver<bool>>,
    /// The texts of the messages not delivered yet.
    texts: HashMap<MessageId, String>,
    next_message: MessageId,
    /// omp's last error lines after an exit.
    exit_details: Option<String>,
    /// The conversation was replaced: the UI fetches it again.
    reset: bool,
    last_state: Option<GeneralState>,
    /// Set for an Ask more conversation.
    ask: Option<AskSlot>,
    /// The last row an Ask more conversation told the UI.
    last_row: Option<AskRow>,
    /// Its sub-agents, and the loop that reads their transcripts.
    agents: Agents,
    poller: Arc<Poller>,
}

impl Slot {
    fn new(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
            ready: false,
            machine: GeneralMachine::new(),
            conversation: Conversation::general(),
            loaded: true,
            record: None,
            identity: None,
            generation: 0,
            channel: None,
            done: Vec::new(),
            texts: HashMap::new(),
            next_message: 1,
            exit_details: None,
            reset: false,
            last_state: None,
            ask: None,
            last_row: None,
            agents: Agents::new(),
            poller: Arc::new(Poller::default()),
        }
    }

    /// An Ask more conversation: ready at once, with nothing kept.
    fn opened_ask(root: &Path, ask: AskSlot) -> Self {
        Self {
            ready: true,
            conversation: Conversation::ask(),
            ask: Some(ask),
            ..Self::new(root)
        }
    }

    /// The project hash the slot belongs to; `key` is its own key.
    fn project<'a>(&'a self, key: &'a str) -> &'a str {
        self.ask.as_ref().map_or(key, |ask| ask.project_id.as_str())
    }

    /// The owner of its sub-agents.
    fn owner(&self, key: &str) -> Owner {
        match self.ask {
            Some(_) => Owner::Ask { id: key.to_owned() },
            None => Owner::General,
        }
    }

    /// The row of an Ask more conversation.
    fn row(&self, key: &str) -> Option<AskRow> {
        let ask = self.ask.as_ref()?;
        let status = if !self.machine.turn_in_progress() {
            AskStatus::Idle
        } else if self.conversation.asking().is_empty() {
            AskStatus::Running
        } else {
            AskStatus::Waiting
        };
        Some(AskRow {
            id: key.to_owned(),
            project_id: ask.project_id.clone(),
            name: ask.name.clone(),
            kind: ask.kind,
            opened: ask.opened.clone(),
            status,
            starting: self.machine.starting(),
            round: ask.round,
        })
    }

    fn state(&self, project_id: &str) -> GeneralState {
        GeneralState {
            project_id: project_id.to_owned(),
            working: self.machine.turn_in_progress(),
            starting: self.machine.starting(),
            loading: self.machine.state() == State::Loading && !self.loaded,
            kept: self.record.is_some() && !self.loaded,
            waiting: self.machine.state() == State::WaitingForAnswers,
        }
    }

    fn send(&self, cmd: Cmd) {
        if let Some(channel) = &self.channel {
            let _ = channel.send(Msg::Cmd(cmd));
        }
    }
}

/// The Generals of the open projects and the Ask more conversations.
pub struct General {
    pub(crate) host: Arc<dyn Host>,
    events: Arc<dyn GeneralEvents>,
    store: GeneralStore,
    inner: Mutex<HashMap<String, Slot>>,
    writer: std_mpsc::Sender<Write>,
    notices: std_mpsc::Sender<Notice>,
    observers: Arc<RwLock<Vec<Arc<dyn ChatObserver>>>>,
    clock: Arc<dyn Clock>,
    me: Weak<General>,
}

impl General {
    pub fn new(host: Arc<dyn Host>, events: Arc<dyn GeneralEvents>) -> Arc<General> {
        Self::with_clock(host, events, Arc::new(SystemClock))
    }

    pub fn with_clock(
        host: Arc<dyn Host>,
        events: Arc<dyn GeneralEvents>,
        clock: Arc<dyn Clock>,
    ) -> Arc<General> {
        let store = GeneralStore::new(host.dirs());
        let (writer, queue) = std_mpsc::channel::<Write>();
        let thread_store = store.clone();
        std::thread::Builder::new()
            .name("weaver-general-record".into())
            .spawn(move || {
                for write in queue {
                    match write {
                        Write::Record(project, record) => {
                            if let Err(error) = thread_store.write(&project, &record) {
                                log_warn!("the record of the General of {project} could not be written: {error}");
                            }
                        }
                        Write::Delete(project) => {
                            if let Err(error) = thread_store.delete(&project) {
                                log_warn!("the record of the General of {project} could not be deleted: {error}");
                            }
                        }
                        Write::Flush(done) => {
                            let _ = done.send(());
                        }
                    }
                }
            })
            .expect("the thread of the General records starts");
        let observers: Arc<RwLock<Vec<Arc<dyn ChatObserver>>>> = Arc::default();
        let (notices, notified) = std_mpsc::channel::<Notice>();
        let listening = observers.clone();
        std::thread::Builder::new()
            .name("weaver-general-observers".into())
            .spawn(move || {
                for notice in notified {
                    if let Notice::Flush(done) = notice {
                        let _ = done.send(());
                        continue;
                    }
                    let observers = listening.read().expect("observers poisoned").clone();
                    for observer in observers {
                        match &notice {
                            Notice::Waiting(project) => observer.chat_waiting(project),
                            Notice::RoundEnded(project) => observer.chat_round_ended(project),
                            Notice::RoundClosed(project) => observer.chat_round_closed(project),
                            Notice::Flush(_) => {}
                        }
                    }
                }
            })
            .expect("the thread of the General observers starts");
        Arc::new_cyclic(|me| General {
            host,
            events,
            store,
            inner: Mutex::new(HashMap::new()),
            writer,
            notices,
            observers,
            clock,
            me: me.clone(),
        })
    }

    pub(crate) fn arc(&self) -> Arc<General> {
        self.me
            .upgrade()
            .expect("the General lives as long as the app")
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<String, Slot>> {
        self.inner.lock().expect("general poisoned")
    }

    pub fn host(&self) -> &Arc<dyn Host> {
        &self.host
    }

    /// Who wants to know the Events of the Chat (`general-chat.WAITING`...).
    pub fn observe(&self, observer: Arc<dyn ChatObserver>) {
        self.observers
            .write()
            .expect("observers poisoned")
            .push(observer);
    }

    fn notify(&self, notice: Notice) {
        let _ = self.notices.send(notice);
    }

    /// Waits until every record written so far is on the disk and every observer was told what
    /// happened before.
    pub fn flush(&self) {
        let (done, wait) = std_mpsc::channel();
        if self.writer.send(Write::Flush(done)).is_ok() {
            let _ = wait.recv();
        }
        let (done, wait) = std_mpsc::channel();
        if self.notices.send(Notice::Flush(done)).is_ok() {
            let _ = wait.recv();
        }
    }

    // -----------------------------------------------------------------------------------------
    // Queries
    // -----------------------------------------------------------------------------------------

    /// The conversation and the state of the project's General.
    pub fn snapshot(&self, project_id: &str) -> Result<GeneralSnapshot, String> {
        let inner = self.lock();
        let slot = inner.get(project_id).ok_or_else(not_open)?;
        Ok(GeneralSnapshot {
            state: slot.state(project_id),
            seq: slot.conversation.seq(),
            entries: slot.conversation.entries().to_vec(),
        })
    }

    /// The kept session of the project, if any.
    pub fn record_of(&self, project_id: &str) -> Option<GeneralRecord> {
        self.lock()
            .get(project_id)
            .and_then(|slot| slot.record.clone())
    }

    // -----------------------------------------------------------------------------------------
    // Sub-agents
    // -----------------------------------------------------------------------------------------

    /// The sub-agents of the General of a project or of an Ask more conversation (`key`), as the
    /// list shows them.
    pub fn sub_agents(&self, key: &str) -> Result<SubAgentsEvent, String> {
        let mut inner = self.lock();
        let slot = inner.get_mut(key).ok_or_else(not_open)?;
        Ok(SubAgentsEvent {
            project_id: slot.project(key).to_owned(),
            owner: slot.owner(key),
            nodes: slot.agents.list(&slot.conversation),
        })
    }

    /// The conversation of the sub-agent at `path` of the General or Ask more conversation `key`,
    /// as it is now.
    pub fn sub_agent_conversation(
        &self,
        key: &str,
        path: &[Step],
    ) -> Result<SubAgentConversation, String> {
        let mut inner = self.lock();
        let slot = inner.get_mut(key).ok_or_else(not_open)?;
        let (project, owner) = (slot.project(key).to_owned(), slot.owner(key));
        slot.agents
            .conversation(&slot.conversation, &project, &owner, path)
    }

    /// What the Chat view shows now of the sub-agents of the General: the conversation of which
    /// sub-agent, if any. The Chat has no list of the sub-agents on screen, and a transcript is
    /// read only while it is shown (or while only it can tell how its sub-agent ended).
    pub fn sub_agents_view(
        &self,
        project_id: &str,
        shown: Option<Vec<Step>>,
    ) -> Result<(), String> {
        self.sub_agents_interest(project_id, false, shown)
    }

    /// What a view shows of the sub-agents of the conversation `key`: whether their list is on
    /// screen and the conversation of which sub-agent, if any.
    pub fn sub_agents_interest(
        &self,
        key: &str,
        list_shown: bool,
        shown: Option<Vec<Step>>,
    ) -> Result<(), String> {
        let poller = {
            let mut inner = self.lock();
            let slot = inner.get_mut(key).ok_or_else(not_open)?;
            slot.agents.interest(list_shown, shown);
            slot.poller.clone()
        };
        poller.poke(|| self.access(key));
        Ok(())
    }

    fn access(&self, key: &str) -> Arc<dyn Access> {
        Arc::new(GeneralAccess {
            general: self.arc(),
            key: key.to_owned(),
        })
    }

    /// The conversation lists sub-agents that may need reading: starts the loop that reads them.
    pub(crate) fn wake_agents(&self, key: &str) {
        let poller = self.lock().get(key).map(|slot| slot.poller.clone());
        if let Some(poller) = poller {
            poller.wake(|| self.access(key));
        }
    }

    // -----------------------------------------------------------------------------------------
    // The window
    // -----------------------------------------------------------------------------------------

    /// `window.OPENED`: reads the record of the project (the first time in this run). A kept
    /// session is reloaded at once, sending nothing: omp starts now, or as soon as omp-setup is
    /// Ready. With no record omp starts only with the first message.
    pub async fn open(&self, project_id: &str, root: &Path) {
        if !self.lock().get(project_id).is_some_and(|slot| slot.ready) {
            let (store, hash) = (self.store.clone(), project_id.to_owned());
            let record = match tauri::async_runtime::spawn_blocking(move || store.read(&hash)).await
            {
                Ok(Ok(record)) => record,
                Ok(Err(error)) => {
                    log_warn!("the record of the General of {project_id} is ignored: {error}");
                    None
                }
                Err(_) => None,
            };
            let mut inner = self.lock();
            let slot = inner
                .entry(project_id.to_owned())
                .or_insert_with(|| Slot::new(root));
            // Another window may have read it meanwhile: the first reading stands.
            if !slot.ready {
                slot.loaded = record.is_none();
                slot.record = record;
                slot.ready = true;
            }
        }
        let mut inner = self.lock();
        let Some(slot) = inner.get_mut(project_id) else {
            return;
        };
        slot.root = root.to_path_buf();
        slot.last_state = None;
        if slot.record.is_some() {
            let omp_ready = self.host.omp_is_ready();
            self.apply(project_id, slot, machine::Event::WindowOpened { omp_ready });
        } else {
            self.emit(project_id, slot);
        }
        log_info!("the General of a project is open");
    }

    /// `window.CLOSED`: the turn is cancelled and omp ends in order; the conversation and the
    /// record of the General stay, and every Ask more conversation of the project ends (it keeps
    /// nothing). Completes when omp is ended.
    pub fn window_closed(&self, project_id: &str) -> impl Future<Output = ()> + Send + 'static {
        let waits = {
            let mut inner = self.lock();
            let mut waits = Vec::new();
            if let Some(slot) = inner.get_mut(project_id) {
                self.apply(project_id, slot, machine::Event::WindowClosed);
                waits.extend(slot.done.clone());
            }
            let asks: Vec<String> = inner
                .iter()
                .filter(|(_, slot)| {
                    slot.ask
                        .as_ref()
                        .is_some_and(|ask| ask.project_id == project_id)
                })
                .map(|(key, _)| key.clone())
                .collect();
            for key in asks {
                if let Some(slot) = inner.remove(&key) {
                    waits.extend(self.end_ask(&key, slot));
                }
            }
            waits
        };
        async move {
            for mut wait in waits {
                let _ = wait.wait_for(|done| *done).await;
            }
        }
    }

    // -----------------------------------------------------------------------------------------
    // The developer
    // -----------------------------------------------------------------------------------------

    /// `developer.SEND_MESSAGE`.
    pub fn send_message(&self, key: &str, text: &str) -> Result<(), String> {
        let text = text.trim();
        if text.is_empty() {
            return Err("a message cannot be empty".to_owned());
        }
        let mut inner = self.lock();
        let slot = inner
            .get_mut(key)
            .filter(|slot| slot.ready)
            .ok_or_else(not_open)?;
        // The assets the message tags are the ones that exist now, never recognised again later.
        let tagged = tags::tagged(text, &self.host.asset_names(slot.project(key)));
        let message = slot.next_message;
        slot.next_message += 1;
        // What goes to omp has the block of tagged assets, after the message (and so after the
        // instruction in the first message of an Ask more conversation); the conversation shows
        // what the developer wrote.
        slot.texts.insert(message, tags::with_block(text, &tagged));
        slot.conversation
            .developer_message(message, text, Delivery::Queued);
        self.apply(key, slot, machine::Event::SendMessage(message));
        Ok(())
    }

    /// `developer.STOP_AGENT`.
    pub fn stop(&self, key: &str) -> Result<(), String> {
        self.with_slot(key, |this, slot| {
            this.apply(key, slot, machine::Event::StopAgent)
        })
    }

    /// `developer.CLEAR_GENERAL`: also during a turn, which it stops.
    pub fn clear(&self, project_id: &str) -> Result<(), String> {
        self.with_slot(project_id, |this, slot| {
            this.apply(project_id, slot, machine::Event::Clear)
        })
    }

    /// `developer.ANSWER_PERMISSION`: `option` is one of the options of the request.
    pub fn answer_permission(
        &self,
        key: &str,
        request: RequestId,
        option: &str,
    ) -> Result<(), String> {
        self.with_slot(key, |this, slot| {
            let name = slot
                .conversation
                .option_name(request, option)
                .ok_or_else(|| "this permission has no such option".to_owned())?;
            if !slot.conversation.asking().contains(&request) {
                return Err("This permission is no longer asked.".to_owned());
            }
            slot.conversation
                .close_permission(request, PermissionState::Chosen { name });
            slot.send(Cmd::AnswerPermission {
                request,
                option: option.to_owned(),
            });
            this.apply(key, slot, machine::Event::PermissionAnswered(request));
            Ok(())
        })?
    }

    fn with_slot<T>(
        &self,
        key: &str,
        f: impl FnOnce(&General, &mut Slot) -> T,
    ) -> Result<T, String> {
        let mut inner = self.lock();
        let slot = inner.get_mut(key).ok_or_else(not_open)?;
        Ok(f(self, slot))
    }

    // -----------------------------------------------------------------------------------------
    // The Machine
    // -----------------------------------------------------------------------------------------

    /// Applies an event to the Machine of the slot `key` and carries out what it asks.
    fn apply(&self, key: &str, slot: &mut Slot, event: machine::Event) {
        let effects = slot.machine.handle(event);
        self.carry_out(key, slot, effects);
        self.emit(key, slot);
    }

    /// What an actor reports. An actor that is not the latest one is ignored.
    pub(crate) fn dispatch(&self, key: &str, generation: u64, event: machine::Event) {
        let mut inner = self.lock();
        let Some(slot) = inner
            .get_mut(key)
            .filter(|slot| slot.generation == generation)
        else {
            return;
        };
        self.apply(key, slot, event);
    }

    fn carry_out(&self, key: &str, slot: &mut Slot, effects: Vec<Effect>) {
        for effect in effects {
            match effect {
                Effect::StartOmp => self.start_actor(key, slot),
                Effect::Steer(message) | Effect::FollowUp(message) => {
                    let follow_up = matches!(effect, Effect::FollowUp(_));
                    let text = slot.texts.get(&message).cloned().unwrap_or_default();
                    slot.send(Cmd::Steer {
                        message,
                        text,
                        follow_up,
                    });
                }
                Effect::Prompt(messages) => {
                    let text = messages
                        .iter()
                        .filter_map(|m| slot.texts.get(m))
                        .cloned()
                        .collect::<Vec<_>>()
                        .join("\n\n");
                    // The instruction of an Ask more conversation goes once, with the first
                    // message that reaches a session: never shown, never written anywhere.
                    let text = match slot.ask.as_mut().and_then(|ask| ask.instruction.take()) {
                        Some(instruction) => ask::first_prompt(&instruction, &text),
                        None => text,
                    };
                    slot.send(Cmd::Prompt { text });
                }
                Effect::Delivered(message) => {
                    slot.conversation.set_delivery(message, Delivery::Delivered);
                    slot.texts.remove(&message);
                }
                Effect::NotDelivered(message, reason) => {
                    slot.conversation
                        .set_delivery(message, Delivery::NotDelivered { reason });
                    slot.texts.remove(&message);
                }
                Effect::ClosePermissions(how) => {
                    let to = match how {
                        PermissionEnd::Cancelled => PermissionState::Cancelled,
                        PermissionEnd::Void => PermissionState::Void,
                    };
                    for request in slot.conversation.asking() {
                        slot.conversation.close_permission(request, to.clone());
                    }
                    slot.send(Cmd::ClosePermissions);
                }
                Effect::Cancel => slot.send(Cmd::Cancel),
                Effect::ShutDown { turn_running } => {
                    slot.identity = None;
                    slot.send(Cmd::ShutDown { turn_running });
                    // omp ends: the sub-agents it ran are not going on.
                    slot.conversation.interrupt_working();
                    slot.agents.interrupt();
                }
                Effect::Clear { turn_running } => {
                    // `developer.CLEAR_GENERAL`: what its agent was changing is not held any more.
                    if let Some(locks) = self.host.file_locks() {
                        locks.family_ended(
                            slot.project(key),
                            weaver_core::file_lock::GENERAL_CONVERSATION,
                        );
                    }
                    slot.identity = None;
                    slot.send(Cmd::ShutDown { turn_running });
                    slot.conversation = Conversation::general();
                    // A read that was on its way is dropped; nothing is on screen any more.
                    slot.agents.reset();
                    slot.agents.interest(false, None);
                    slot.loaded = true;
                    slot.record = None;
                    slot.texts.clear();
                    slot.exit_details = None;
                    slot.reset = true;
                    let _ = self.writer.send(Write::Delete(key.to_owned()));
                }
                Effect::Waiting => self.notify(Notice::Waiting(slot.project(key).to_owned())),
                Effect::EndRound => self.notify(Notice::RoundEnded(slot.project(key).to_owned())),
                Effect::TurnFailed(reason) => slot.conversation.turn_failed(&reason),
                Effect::Stopped(reason) => {
                    slot.identity = None;
                    let details = slot.exit_details.take();
                    slot.conversation.interrupt_working();
                    slot.agents.interrupt();
                    slot.conversation.omp_stopped(&reason, details);
                }
            }
        }
    }

    /// Starts the actor of a new omp.
    fn start_actor(&self, key: &str, slot: &mut Slot) {
        slot.generation += 1;
        let generation = slot.generation;
        let (tx, rx) = mpsc::unbounded_channel();
        let (done_tx, done_rx) = watch::channel(false);
        slot.channel = Some(tx.clone());
        slot.done.retain(|done| !*done.borrow());
        slot.done.push(done_rx);
        slot.identity = None;
        slot.exit_details = None;
        let general = self.arc();
        let key = key.to_owned();
        tauri::async_runtime::spawn(async move {
            actor::run(general.clone(), key.clone(), generation, tx, rx).await;
            general.actor_ended(&key, generation);
            let _ = done_tx.send(true);
        });
    }

    fn actor_ended(&self, key: &str, generation: u64) {
        let mut inner = self.lock();
        if let Some(slot) = inner
            .get_mut(key)
            .filter(|slot| slot.generation == generation)
        {
            slot.channel = None;
            slot.identity = None;
        }
    }

    /// Tells the UI what changed: the entries first, then the sub-agents, then the state (for an
    /// Ask more conversation, its row).
    fn emit(&self, key: &str, slot: &mut Slot) {
        let changes = slot.conversation.take_changes();
        let reset = std::mem::take(&mut slot.reset);
        let project = slot.project(key).to_owned();
        if !changes.is_empty() || reset {
            let seq = slot.conversation.seq();
            if slot.ask.is_some() {
                self.events.ask_entries(&AskEntries {
                    project_id: project.clone(),
                    id: key.to_owned(),
                    seq,
                    reset,
                    changes,
                });
            } else {
                self.events.entries(&GeneralEntries {
                    project_id: project.clone(),
                    seq,
                    reset,
                    changes,
                });
            }
        }
        let owner = slot.owner(key);
        for outgoing in slot.agents.collect(&slot.conversation, &project, &owner) {
            match outgoing {
                Outgoing::List(event) => self.host.emit_sub_agents(&event),
                Outgoing::Entries(event) => self.host.emit_sub_agent_entries(&event),
            }
        }
        if let Some(row) = slot.row(key) {
            if slot.last_row.as_ref() != Some(&row) {
                slot.last_row = Some(row.clone());
                self.events.ask_changed(&row);
            }
            return;
        }
        let state = slot.state(key);
        if slot.last_state.as_ref() != Some(&state) {
            slot.last_state = Some(state.clone());
            self.events.state(&state);
        }
    }

    // -----------------------------------------------------------------------------------------
    // What the actor needs
    // -----------------------------------------------------------------------------------------

    /// What starts omp now: its identity, the root, the session to reload and, for an Ask more
    /// conversation, how its instruction is built again. `None` when the actor is not the latest
    /// one.
    pub(crate) fn launch_facts(&self, key: &str, generation: u64) -> Option<Launch> {
        let mut inner = self.lock();
        let taken: Vec<String> = inner
            .iter()
            .flat_map(|(key, slot)| [Some(key.clone()), slot.identity.clone()])
            .flatten()
            .collect();
        let slot = inner
            .get_mut(key)
            .filter(|slot| slot.generation == generation)?;
        let (identity, reload, rebuild) = match &slot.ask {
            // The identity of a conversation is its own, the same at every start of omp.
            Some(ask) => (
                key.to_owned(),
                ask.session.clone(),
                Some(ask.rebuild.clone()),
            ),
            None => (
                ids::draw_identity(&mut OsRandom, &|candidate| {
                    taken.iter().any(|t| t == candidate)
                }),
                slot.record.as_ref().map(|record| record.session_id.clone()),
                None,
            ),
        };
        slot.identity = Some(identity.clone());
        Some(Launch {
            identity,
            root: slot.root.clone(),
            reload,
            rebuild,
        })
    }

    /// Runs `f` on the conversation and tells the UI what it changed.
    pub(crate) fn with_conversation<T>(
        &self,
        key: &str,
        generation: u64,
        f: impl FnOnce(&mut Conversation) -> T,
    ) -> Option<T> {
        let mut inner = self.lock();
        let slot = inner
            .get_mut(key)
            .filter(|slot| slot.generation == generation)?;
        let value = f(&mut slot.conversation);
        self.emit(key, slot);
        Some(value)
    }

    pub(crate) fn set_exit_details(&self, key: &str, generation: u64, details: Option<String>) {
        if let Some(slot) = self
            .lock()
            .get_mut(key)
            .filter(|slot| slot.generation == generation)
        {
            slot.exit_details = details;
        }
    }

    /// `omp.PERMISSION_REQUESTED`: the card is shown and the Machine stays in Working. `false` when
    /// no turn is in progress: the actor cancels the request. Nothing is notified: a permission
    /// of General or of an Ask more conversation waits in its conversation.
    pub(crate) fn permission_requested(
        &self,
        key: &str,
        generation: u64,
        request: RequestId,
        tool_call: &Value,
        options: &Value,
    ) -> bool {
        let mut inner = self.lock();
        let Some(slot) = inner
            .get_mut(key)
            .filter(|slot| slot.generation == generation)
        else {
            return false;
        };
        if !slot.machine.accepts_agent_calls() {
            return false;
        }
        slot.conversation.permission(request, tool_call, options);
        self.apply(key, slot, machine::Event::PermissionRequested(request));
        true
    }

    /// omp opened its session: the history it replayed (when the kept session was reloaded)
    /// becomes the conversation above the messages already sent, the record is written, and the
    /// Machine goes on. `fallback` is why the kept session could not be reloaded and an empty one
    /// started: the notice that says so comes first, above those messages.
    ///
    /// For an Ask more conversation nothing is written: the session stays in memory, and when
    /// `fallback` says the session was replaced, `instruction` is the instruction built again
    /// from the files (`Some(None)`: the chosen thing is gone, the message goes alone).
    pub(crate) fn session_opened(
        &self,
        key: &str,
        generation: u64,
        session: &str,
        replay: Option<Conversation>,
        fallback: Option<&str>,
        instruction: Option<Option<String>>,
    ) {
        let updated = now_instant(self.clock.as_ref())
            .unwrap_or_else(|_| weaver_core::project::clock::format_instant(1));
        let mut inner = self.lock();
        let Some(slot) = inner
            .get_mut(key)
            .filter(|slot| slot.generation == generation)
        else {
            return;
        };
        let notice = fallback.map(|reason| {
            format!("The conversation could not be reloaded ({reason}). A new one started.")
        });
        if slot.loaded {
            // The conversation in memory is the one of the session: a replay is not shown again.
            if let Some(notice) = &notice {
                slot.conversation.notice(notice);
            }
        } else {
            let mut merged = replay.unwrap_or_else(Conversation::general);
            // The history ends where omp's did: what the agent says next is a message of its own.
            merged.end_turn();
            if let Some(notice) = &notice {
                merged.notice(notice);
            }
            for entry in slot.conversation.entries() {
                merged.append(entry);
            }
            // The merged conversation is new as a whole: the UI fetches it.
            merged.mark_sent();
            slot.conversation = merged;
            slot.reset = true;
            // The sub-agents are rebuilt from the replayed history, and their transcripts are
            // read again from the start.
            slot.agents.reset();
        }
        slot.loaded = true;
        if let Some(ask) = slot.ask.as_mut() {
            ask.session = Some(session.to_owned());
            if fallback.is_some() {
                ask.instruction = instruction.flatten();
            }
        }
        if let Some(root) = self.host.sessions_root() {
            slot.agents.locate(Location {
                root,
                session: session.to_owned(),
                owner_name: slot
                    .ask
                    .as_ref()
                    .map_or(AGENTS_OWNER, |ask| ask.name.as_str())
                    .to_owned(),
            });
        }
        if slot.ask.is_none() {
            let record = GeneralRecord {
                project: slot.root.to_string_lossy().into_owned(),
                session_id: session.to_owned(),
                updated,
            };
            slot.record = Some(record.clone());
            let _ = self.writer.send(Write::Record(key.to_owned(), record));
        }
        self.apply(key, slot, machine::Event::SessionOpened);
        // The history may list sub-agents whose transcripts must be read.
        slot.poller.wake(|| self.access(key));
    }

    // -----------------------------------------------------------------------------------------
    // Ask more
    // -----------------------------------------------------------------------------------------

    /// `developer.ASK_MORE`, once the chosen thing was read: the conversation opens empty and
    /// shows in the Jobs view. Nothing is sent and omp does not start. Returns its identity.
    pub fn open_ask(&self, opening: AskOpening) -> String {
        let opened = now_instant(self.clock.as_ref())
            .unwrap_or_else(|_| weaver_core::project::clock::format_instant(1));
        let mut inner = self.lock();
        let taken: Vec<String> = inner
            .iter()
            .flat_map(|(key, slot)| [Some(key.clone()), slot.identity.clone()])
            .flatten()
            .collect();
        let id = ids::draw_identity(&mut OsRandom, &|candidate| {
            taken.iter().any(|t| t == candidate)
        });
        let order = inner
            .values()
            .filter_map(|slot| slot.ask.as_ref().map(|ask| ask.order))
            .max()
            .map_or(1, |latest| latest + 1);
        let mut slot = Slot::opened_ask(
            &opening.root,
            AskSlot {
                project_id: opening.project_id,
                name: opening.name,
                kind: opening.kind,
                opened,
                order,
                round: opening.round,
                instruction: Some(opening.instruction),
                rebuild: opening.rebuild,
                session: None,
            },
        );
        self.emit(&id, &mut slot);
        inner.insert(id.clone(), slot);
        id
    }

    /// The Ask more conversations of the project, the most recent first.
    pub fn asks(&self, project_id: &str) -> Vec<AskRow> {
        let inner = self.lock();
        let mut asks: Vec<(u64, AskRow)> = inner
            .iter()
            .filter_map(|(key, slot)| {
                let ask = slot
                    .ask
                    .as_ref()
                    .filter(|ask| ask.project_id == project_id)?;
                Some((ask.order, slot.row(key)?))
            })
            .collect();
        asks.sort_by_key(|ask| std::cmp::Reverse(ask.0));
        asks.into_iter().map(|(_, row)| row).collect()
    }

    /// Whether `id` is an open Ask more conversation.
    pub fn is_ask(&self, id: &str) -> bool {
        self.lock().get(id).is_some_and(|slot| slot.ask.is_some())
    }

    /// One Ask more conversation as it is now.
    pub fn ask_snapshot(&self, id: &str) -> Result<AskSnapshot, String> {
        let inner = self.lock();
        let slot = inner.get(id).ok_or_else(not_open)?;
        Ok(AskSnapshot {
            row: slot.row(id).ok_or_else(not_open)?,
            seq: slot.conversation.seq(),
            entries: slot.conversation.entries().to_vec(),
        })
    }

    /// `developer.CLOSE_ASK`, once confirmed when its agent works: the turn is cancelled, omp is
    /// ended and the conversation is gone. Completes when omp is ended.
    pub fn close_ask(&self, id: &str) -> Result<impl Future<Output = ()> + Send + 'static, String> {
        let waits = {
            let mut inner = self.lock();
            if !inner.get(id).is_some_and(|slot| slot.ask.is_some()) {
                return Err(not_open());
            }
            let slot = inner.remove(id).expect("the conversation was just found");
            self.end_ask(id, slot)
        };
        Ok(async move {
            for mut wait in waits {
                let _ = wait.wait_for(|done| *done).await;
            }
        })
    }

    /// Ends an Ask more conversation that left the map: the turn is cancelled, omp ends and the
    /// UI is told. Returns what tells that omp has ended.
    fn end_ask(&self, key: &str, mut slot: Slot) -> Vec<watch::Receiver<bool>> {
        self.apply(key, &mut slot, machine::Event::WindowClosed);
        // `developer.CLOSE_ASK`: what its agent was changing is not held any more.
        if let Some(locks) = self.host.file_locks() {
            locks.family_ended(
                slot.project(key),
                &weaver_core::file_lock::ask_conversation(key),
            );
        }
        self.events.ask_closed(&AskClosed {
            project_id: slot.project(key).to_owned(),
            id: key.to_owned(),
        });
        slot.done.clone()
    }

    // -----------------------------------------------------------------------------------------
    // The agent
    // -----------------------------------------------------------------------------------------

    /// `agent.START_JOB`: the jobs the agent started on the developer's request, or the refusal.
    /// `token` is the identity of the General of the project or of one of its Ask more
    /// conversations.
    fn started_jobs(
        &self,
        root: &Path,
        token: &str,
        kind: &str,
        outcome: Result<&[(String, String)], &str>,
    ) {
        let project_id = project_hash_of_canonical(root);
        let mut inner = self.lock();
        let key = if inner
            .get(&project_id)
            .is_some_and(|slot| slot.ask.is_none() && slot.identity.as_deref() == Some(token))
        {
            project_id.clone()
        } else if inner.get(token).is_some_and(|slot| {
            slot.ask
                .as_ref()
                .is_some_and(|ask| ask.project_id == project_id)
        }) {
            token.to_owned()
        } else {
            return;
        };
        let Some(slot) = inner.get_mut(&key) else {
            return;
        };
        match outcome {
            Ok(started) => {
                for (id, name) in started {
                    slot.conversation.job_started(id, name);
                }
            }
            Err(reason) => slot.conversation.job_refused(kind, reason),
        }
        self.apply(&key, slot, machine::Event::StartJob);
    }
}

/// What starts an omp for a conversation.
pub(crate) struct Launch {
    pub identity: String,
    pub root: PathBuf,
    /// The session to reload.
    pub reload: Option<String>,
    /// Set for an Ask more conversation: how its instruction is built again.
    pub rebuild: Option<Rebuild>,
}

fn not_open() -> String {
    "The conversation is not open.".to_owned()
}

/// The poller of the sub-agents of a conversation reads and applies through the service, under
/// its lock.
struct GeneralAccess {
    general: Arc<General>,
    key: String,
}

impl Access for GeneralAccess {
    fn plan(&self) -> Option<Plan> {
        let mut inner = self.general.lock();
        let slot = inner.get_mut(&self.key)?;
        slot.agents.plan(&slot.conversation)
    }

    fn apply(&self, generation: u64, done: Vec<Done>) {
        let mut inner = self.general.lock();
        let Some(slot) = inner.get_mut(&self.key) else {
            return;
        };
        slot.agents.apply(&mut slot.conversation, generation, done);
        self.general.emit(&self.key, slot);
    }
}

impl GeneralAuthority for General {
    fn accepts(&self, root: &Path, token: &str) -> bool {
        let project_id = project_hash_of_canonical(root);
        self.lock().get(&project_id).is_some_and(|slot| {
            slot.identity.as_deref() == Some(token) && slot.machine.accepts_agent_calls()
        })
    }

    fn accepts_ask(&self, root: &Path, token: &str) -> bool {
        let project_id = project_hash_of_canonical(root);
        self.lock().get(token).is_some_and(|slot| {
            slot.ask
                .as_ref()
                .is_some_and(|ask| ask.project_id == project_id)
                && slot.machine.accepts_agent_calls()
        })
    }

    fn job_requested(
        &self,
        root: &Path,
        token: &str,
        kind: &str,
        outcome: Result<&[(String, String)], &str>,
    ) {
        self.started_jobs(root, token, kind, outcome);
    }
}

/// What the rounds of questions and the rest of the app ask of the Chat of a project.
impl General {
    /// The identity (`WEAVER_GENERAL`) of the live omp of the Chat of the project, when it has one.
    pub fn identity(&self, project_id: &str) -> Option<String> {
        self.lock()
            .get(project_id)
            .filter(|slot| slot.ask.is_none())
            .and_then(|slot| slot.identity.clone())
    }

    /// The project hash of the live Chat that `token` is the identity of, while a turn of its
    /// agent is in progress (Working, or Waiting for answers); `None` for any other token, for an
    /// Ask more conversation and for a Chat with no turn.
    pub fn chat_of(&self, token: &str) -> Option<String> {
        self.lock()
            .iter()
            .find(|(_, slot)| {
                slot.ask.is_none()
                    && slot.identity.as_deref() == Some(token)
                    && slot.machine.accepts_agent_calls()
            })
            .map(|(key, _)| key.clone())
    }

    /// `question-round.OPENED` for a round of the Chat of the project: the Working agent goes to
    /// Waiting for answers (and `general-chat.WAITING` follows). A Chat that is not Working
    /// ignores it.
    pub fn round_opened(&self, project_id: &str) {
        self.apply_to_chat(project_id, machine::Event::RoundOpened);
    }

    /// `question-round.CLOSED` for a round of the Chat: the agent goes on in the same turn, and
    /// the messages that waited reach it. A Chat that is not waiting ignores it.
    pub fn round_closed(&self, project_id: &str) {
        let waiting = self.lock().get(project_id).is_some_and(|slot| {
            slot.ask.is_none() && slot.machine.state() == State::WaitingForAnswers
        });
        self.apply_to_chat(project_id, machine::Event::RoundClosed);
        if waiting {
            self.notify(Notice::RoundClosed(project_id.to_owned()));
        }
    }

    fn apply_to_chat(&self, project_id: &str, event: machine::Event) {
        let mut inner = self.lock();
        if let Some(slot) = inner
            .get_mut(project_id)
            .filter(|slot| slot.ask.is_none() && slot.ready)
        {
            self.apply(project_id, slot, event);
        }
    }
}

/// The entries of a conversation, for the tests.
#[cfg(test)]
impl General {
    pub(crate) fn entries(&self, project_id: &str) -> Vec<crate::jobs::conversation::Entry> {
        self.lock()
            .get(project_id)
            .map(|slot| slot.conversation.entries().to_vec())
            .unwrap_or_default()
    }

    pub(crate) fn working(&self, project_id: &str) -> bool {
        self.lock()
            .get(project_id)
            .is_some_and(|slot| slot.machine.turn_in_progress())
    }
}

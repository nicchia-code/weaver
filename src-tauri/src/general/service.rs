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
use std::sync::{Arc, Mutex, MutexGuard, Weak};

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
use weaver_core::question::{self, Action, Card, CardRecord, CardState, Origin, RoundRequest};

use super::actor::{self, Cmd, Msg};
use super::cards::{self, ChatCards};
use super::wire::{
    AskClosed, AskEntries, AskKind, AskRow, AskSnapshot, AskStatus, GeneralEntries, GeneralEvents,
    GeneralSnapshot, GeneralState,
};
use crate::jobs::authority::GeneralAuthority;
use crate::jobs::conversation::{Conversation, Delivery, Entry, PermissionState};
use crate::jobs::host::Host;
use crate::rounds::service::{CardPublication, QuestionInView};
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
    /// The card of questions of the Chat it was opened from.
    card: Option<String>,
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
    pub card: Option<String>,
    pub instruction: String,
    pub rebuild: Rebuild,
}

/// The thread that writes the records, one after the other.
enum Write {
    Record(String, GeneralRecord),
    Delete(String),
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
    /// General: the cards of questions of the conversation, in publication order.
    cards: Vec<Card>,
    /// Grows with every change of the cards, drafts included.
    cards_revision: u64,
    /// The turns of the agent so far: a card belongs to the turn that published it.
    turn: u64,
    /// A turn of the agent is in progress.
    in_turn: bool,
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
            cards: Vec::new(),
            cards_revision: 0,
            turn: 0,
            in_turn: false,
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
            card: ask.card.clone(),
        })
    }

    fn state(&self, project_id: &str) -> GeneralState {
        GeneralState {
            project_id: project_id.to_owned(),
            working: self.machine.turn_in_progress(),
            starting: self.machine.starting(),
            loading: self.machine.state() == State::Loading && !self.loaded,
            kept: self.record.is_some() && !self.loaded,
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
        Arc::new_cyclic(|me| General {
            host,
            events,
            store,
            inner: Mutex::new(HashMap::new()),
            writer,
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

    /// Waits until every record written so far is on the disk.
    pub fn flush(&self) {
        let (done, wait) = std_mpsc::channel();
        if self.writer.send(Write::Flush(done)).is_ok() {
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
            arrivals: slot.conversation.arrivals().to_vec(),
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
                slot.cards = restored_cards(project_id, record.as_ref());
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
        let mut sent = tags::with_block(text, &tagged);
        // A message written while a card of questions is active closes it, under this lock: what
        // was confirmed and skipped goes with the message, every other question as not answered.
        let card = self
            .close_active_card(key, slot, CardState::AnsweredWithMessage)
            .map(|(card, block)| {
                sent = question::with_answers_block(&sent, &block);
                card
            });
        slot.texts.insert(message, sent);
        slot.conversation
            .developer_message_closing(message, text, Delivery::Queued, card);
        self.apply(key, slot, machine::Event::SendMessage(message));
        Ok(())
    }

    /// `developer.CHANGE_SETTINGS` for the analysis model, in whichever window it was made: the
    /// omp of every Chat that runs with no turn in progress ends, keeping its conversation, and
    /// the next message starts it again with the new model.
    pub fn analysis_model_changed(&self) {
        let mut inner = self.lock();
        for (key, slot) in inner.iter_mut() {
            if slot.ask.is_none() && slot.ready {
                self.apply(key, slot, machine::Event::ModelChanged);
            }
        }
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
        // A turn ends with the end, the failure or the stop of the agent; whatever makes it work
        // afterwards starts another.
        if matches!(
            event,
            machine::Event::TurnEnded
                | machine::Event::TurnFailed(_)
                | machine::Event::Exited(_)
                | machine::Event::StopAgent
                | machine::Event::Clear
                | machine::Event::WindowClosed
        ) {
            slot.in_turn = false;
        }
        let effects = slot.machine.handle(event);
        let working = slot.machine.turn_in_progress();
        if working && !slot.in_turn {
            slot.turn += 1;
        }
        slot.in_turn = working;
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
                    self.clear_cards(key, slot);
                    slot.texts.clear();
                    slot.exit_details = None;
                    slot.reset = true;
                    let _ = self.writer.send(Write::Delete(key.to_owned()));
                }
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
            // The jobs the Chat started in an earlier run come back as their `Started` lines, from
            // their records, so that the Proposals they handed over show below them again.
            if slot.ask.is_none() {
                let known: Vec<String> = slot
                    .conversation
                    .entries()
                    .iter()
                    .filter_map(|entry| match entry {
                        Entry::JobStarted { job, .. } => Some(job.clone()),
                        _ => None,
                    })
                    .collect();
                for (job, name, at) in self.host.chat_jobs(key) {
                    if !known.contains(&job) {
                        merged.job_started_at(&job, &name, at);
                    }
                }
            }
            for (entry, at) in slot
                .conversation
                .entries()
                .iter()
                .zip(slot.conversation.arrivals())
            {
                merged.append(entry, *at);
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
            // A kept conversation that could not be reloaded goes with its cards, and the answers
            // of a card that a held message already carried never reach the agent.
            if fallback.is_some() {
                self.clear_cards(key, slot);
                for text in slot.texts.values_mut() {
                    if let Some((written, _)) = question::split_answers_block(text) {
                        *text = written.to_owned();
                    }
                }
            }
            let record = GeneralRecord {
                project: slot.root.to_string_lossy().into_owned(),
                session_id: session.to_owned(),
                updated,
                cards: cards_record(&slot.cards),
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
                card: opening.card,
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

    /// `agent.ASK_QUESTIONS` for the Chat: the card of questions the agent of the Chat whose
    /// identity is `token` publishes during a turn. The card shows in the conversation at once; it
    /// never enters the queue of the rounds, holds no one and changes no state of the Chat. A second
    /// card in the same turn is refused whole.
    pub fn publish_card(&self, token: &str, request: RoundRequest) -> CardPublication {
        let mut inner = self.lock();
        let Some((key, slot)) = inner.iter_mut().find(|(_, slot)| {
            slot.ask.is_none()
                && slot.identity.as_deref() == Some(token)
                && slot.machine.accepts_agent_calls()
        }) else {
            return CardPublication::Unavailable;
        };
        if slot.cards.iter().any(|card| card.turn == slot.turn) {
            return CardPublication::AlreadyPublished;
        }
        let id = ids::draw_identity(&mut OsRandom, &|candidate| {
            slot.cards.iter().any(|card| card.id == candidate)
        });
        // Only the latest card is active (two never exist: a message closes the one before).
        for card in slot.cards.iter_mut().filter(|card| card.is_active()) {
            card.close(CardState::AnsweredWithMessage);
        }
        slot.cards
            .push(Card::publish(id.clone(), slot.turn, key, request));
        self.persist_cards(key, slot);
        self.emit_cards(key, slot, true);
        CardPublication::Published(id)
    }

    /// The cards of questions of the Chat of a project, in publication order.
    pub fn cards(&self, project_id: &str) -> Result<ChatCards, String> {
        let inner = self.lock();
        let slot = inner
            .get(project_id)
            .filter(|slot| slot.ask.is_none())
            .ok_or_else(not_open)?;
        Ok(cards_of(project_id, slot))
    }

    /// The questions of the card `card` of the Chat of a project, only while it is the active card
    /// and `index` is one of its questions: what an Ask more conversation about its question in
    /// view reads. The developer's side of the card is never part of it.
    pub fn active_card_request(
        &self,
        project_id: &str,
        card: &str,
        index: usize,
    ) -> Option<RoundRequest> {
        let inner = self.lock();
        let slot = inner.get(project_id).filter(|slot| slot.ask.is_none())?;
        let card = slot
            .cards
            .iter()
            .find(|candidate| candidate.id == card && candidate.is_active())?;
        (index < card.request().questions.len()).then(|| card.request().clone())
    }

    /// What the developer does in the active card of questions; the cards afterwards. Confirming
    /// and skipping are written to the record, a draft is not, and tells no one.
    pub fn card_act(
        &self,
        project_id: &str,
        card: &str,
        action: Action,
    ) -> Result<ChatCards, String> {
        let mut inner = self.lock();
        let slot = inner
            .get_mut(project_id)
            .filter(|slot| slot.ask.is_none())
            .ok_or_else(not_open)?;
        let answers = matches!(action, Action::Confirm { .. } | Action::Skip { .. });
        let draft = matches!(action, Action::Draft { .. });
        slot.cards
            .iter_mut()
            .find(|c| c.id == card)
            .ok_or_else(|| "This card of questions does not exist.".to_owned())?
            .act(action)
            .map_err(|error| error.to_string())?;
        if answers {
            self.persist_cards(project_id, slot);
        }
        self.emit_cards(project_id, slot, !draft);
        Ok(cards_of(project_id, slot))
    }

    /// `developer.SEND_ANSWERS` for the active card of the Chat: only when every question is
    /// answered or skipped. The card closes under the lock that takes the message, and its
    /// answers follow the path of a message of the developer; no message of his is shown.
    pub fn send_answers(&self, project_id: &str, card: &str) -> Result<(), String> {
        let mut inner = self.lock();
        let slot = inner
            .get_mut(project_id)
            .filter(|slot| slot.ready && slot.ask.is_none())
            .ok_or_else(not_open)?;
        let active = slot
            .cards
            .iter()
            .rev()
            .find(|c| c.is_active())
            .filter(|c| c.id == card)
            .ok_or_else(|| "This card of questions is no longer active.".to_owned())?;
        active.answers_to_send().map_err(|error| error.to_string())?;
        let (card, block) = self
            .close_active_card(project_id, slot, CardState::Answered)
            .ok_or_else(not_open)?;
        let message = slot.next_message;
        slot.next_message += 1;
        slot.texts.insert(message, block);
        slot.conversation
            .developer_message_closing(message, "", Delivery::Queued, Some(card));
        self.apply(project_id, slot, machine::Event::SendMessage(message));
        Ok(())
    }

    /// What an Ask more conversation reads of a question of the active card of the Chat of the
    /// project: its questions and options, never the developer's drafts, choices or answers.
    /// `None` when the card is no longer active or has no such question.
    pub fn question_of_card(
        &self,
        project_id: &str,
        card: &str,
        index: usize,
    ) -> Option<QuestionInView> {
        let inner = self.lock();
        let slot = inner.get(project_id).filter(|slot| slot.ask.is_none())?;
        let card = slot
            .cards
            .iter()
            .rev()
            .find(|c| c.is_active())
            .filter(|c| c.id == card && index < c.request().questions.len())?;
        Some(QuestionInView {
            request: card.request().clone(),
            origin: Origin::Chat {
                project: project_id.to_owned(),
            },
            job: None,
        })
    }

    /// Closes the active card of the Chat, when there is one: its identity and the answers block
    /// that goes to the agent with what was confirmed and skipped (every other question as not
    /// answered). The record is written and the UI told.
    fn close_active_card(
        &self,
        key: &str,
        slot: &mut Slot,
        state: CardState,
    ) -> Option<(String, String)> {
        if slot.ask.is_some() {
            return None;
        }
        let card = slot.cards.iter_mut().rev().find(|card| card.is_active())?;
        let closed = (card.id.clone(), card.answers_block());
        card.close(state);
        self.persist_cards(key, slot);
        self.emit_cards(key, slot, true);
        Some(closed)
    }

    /// The conversation is gone (`New chat`, or a kept one that could not be reloaded): so are its
    /// cards, in memory and, with the record the caller writes next, on disk.
    fn clear_cards(&self, key: &str, slot: &mut Slot) {
        if !slot.cards.is_empty() {
            slot.cards.clear();
            self.emit_cards(key, slot, true);
        }
    }

    /// Writes the cards to the record of the Chat; nothing of a card is logged.
    fn persist_cards(&self, key: &str, slot: &mut Slot) {
        let cards = cards_record(&slot.cards);
        if let Some(record) = slot.record.as_mut().filter(|_| slot.ask.is_none()) {
            record.cards = cards;
            let _ = self
                .writer
                .send(Write::Record(key.to_owned(), record.clone()));
        }
    }

    /// A change of the cards: the revision grows and, unless it is quiet, the UI is told.
    fn emit_cards(&self, key: &str, slot: &mut Slot, tell: bool) {
        slot.cards_revision += 1;
        if tell {
            self.events.cards(&cards_of(key, slot));
        }
    }
}

/// The cards of the Chat as the UI gets them.
fn cards_of(project_id: &str, slot: &Slot) -> ChatCards {
    ChatCards {
        project_id: project_id.to_owned(),
        revision: slot.cards_revision,
        cards: slot.cards.iter().map(cards::view_of).collect(),
    }
}

/// What the record keeps of the cards: nothing, when there are none.
fn cards_record(cards: &[Card]) -> Option<Vec<CardRecord>> {
    (!cards.is_empty()).then(|| cards.iter().map(Card::record).collect())
}

/// The cards a record keeps, back in memory: none when any of them cannot be read, and only the
/// newest of the active ones stays active.
fn restored_cards(project_id: &str, record: Option<&GeneralRecord>) -> Vec<Card> {
    let Some(kept) = record.and_then(|record| record.cards.as_ref()) else {
        return Vec::new();
    };
    let Some(mut cards) = kept
        .iter()
        .cloned()
        .map(|card| Card::restore(project_id, card))
        .collect::<Option<Vec<_>>>()
    else {
        return Vec::new();
    };
    let newest = cards.iter().rposition(Card::is_active);
    for (at, card) in cards.iter_mut().enumerate() {
        if card.is_active() && Some(at) != newest {
            card.close(CardState::AnsweredWithMessage);
        }
    }
    cards
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

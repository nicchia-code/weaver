//! General against the scripted fake ACP agent: the first message starts omp, the kept session is
//! reloaded after the window closed and after a restart of Weaver, an empty session starts when
//! the load fails, Stop, the exit of omp, Clear, permissions and forms, and `weaver job new` from
//! General. No omp, no network.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde_json::{json, Value};
use weaver_core::jobs::general::{GeneralRecord, GeneralStore};
use weaver_core::jobs::subagents::SubAgentState;
use weaver_core::settings::Permissions;

use super::service::General;
use super::wire::{AskClosed, AskEntries, AskRow, GeneralEntries, GeneralEvents, GeneralState};
use crate::jobs::conversation::{Delivery, Entry, PermissionState};
use crate::jobs::testing::{text_update, until, Project, TestHost, LIMIT, TASK};
use crate::jobs::Jobs;
use crate::subagents::wire::{Owner, Step};

#[derive(Default)]
struct Recorder {
    states: Mutex<Vec<GeneralState>>,
    entries: Mutex<Vec<GeneralEntries>>,
    rows: Mutex<Vec<AskRow>>,
    ask_entries: Mutex<Vec<AskEntries>>,
    closed: Mutex<Vec<AskClosed>>,
    cards: Mutex<Vec<super::cards::ChatCards>>,
}

impl GeneralEvents for Recorder {
    fn state(&self, state: &GeneralState) {
        self.states.lock().unwrap().push(state.clone());
    }

    fn entries(&self, entries: &GeneralEntries) {
        self.entries.lock().unwrap().push(entries.clone());
    }

    fn cards(&self, cards: &super::cards::ChatCards) {
        self.cards.lock().unwrap().push(cards.clone());
    }

    fn ask_changed(&self, row: &AskRow) {
        self.rows.lock().unwrap().push(row.clone());
    }

    fn ask_entries(&self, entries: &AskEntries) {
        self.ask_entries.lock().unwrap().push(entries.clone());
    }

    fn ask_closed(&self, closed: &AskClosed) {
        self.closed.lock().unwrap().push(closed.clone());
    }
}

struct Fixture {
    host: Arc<TestHost>,
    general: Arc<General>,
    events: Arc<Recorder>,
    _dir: tempfile::TempDir,
    project: Project,
}

impl Fixture {
    fn id(&self) -> &str {
        &self.project.id
    }

    fn send(&self, text: &str) {
        self.general.send_message(self.id(), text).unwrap();
    }

    fn entries(&self) -> Vec<Entry> {
        self.general.entries(self.id())
    }

    async fn working(&self) {
        until("a turn to be in progress", LIMIT, || {
            self.general.working(self.id())
        })
        .await;
    }

    async fn received(&self, what: &str, wanted: impl Fn(&Value) -> bool) {
        until(what, LIMIT, || self.host.received().iter().any(&wanted)).await;
    }

    fn methods(&self) -> Vec<String> {
        self.host
            .received()
            .iter()
            .filter_map(|m| m["method"].as_str().map(str::to_owned))
            .collect()
    }

    fn record(&self) -> Option<GeneralRecord> {
        self.general.flush();
        GeneralStore::new(&self.host.dirs).read(self.id()).unwrap()
    }
}

async fn fixture() -> Fixture {
    let host = TestHost::new();
    let events = Arc::new(Recorder::default());
    let general = General::new(host.clone(), events.clone());
    let (dir, project) = host.project(&[]).await;
    general.open(&project.id, &project.root).await;
    Fixture {
        host,
        general,
        events,
        _dir: dir,
        project,
    }
}

fn agent(text: &str) -> Value {
    json!({ "update": text_update("agent_message_chunk", text) })
}

fn developers(entries: &[Entry]) -> Vec<(&str, &Delivery)> {
    entries
        .iter()
        .filter_map(|entry| match entry {
            Entry::Developer { text, delivery, .. } => Some((text.as_str(), delivery)),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn an_empty_general_keeps_nothing_and_the_first_message_starts_omp_with_the_general_command_line_and_environment(
) {
    let f = fixture().await;
    f.host.script(json!({
        "log": f.host.received_log(),
        "logEnv": ["WEAVER_GENERAL", "WEAVER_JOB", "WEAVER_ENDPOINT", "WEAVER_RUN", "WEAVER_TASK"],
        "prompts": [[agent("Hello "), agent("there")], [agent("Again")]],
    }));
    let snapshot = f.general.snapshot(f.id()).unwrap();
    assert!(snapshot.entries.is_empty());
    assert_eq!(
        (
            snapshot.state.working,
            snapshot.state.starting,
            snapshot.state.kept
        ),
        (false, false, false)
    );
    assert!(
        f.methods().is_empty(),
        "omp does not run before the first message"
    );

    f.send("  Hi omp  ");
    // The message shows at once, queued, while omp starts.
    assert!(
        matches!(&f.entries()[0], Entry::Developer { text, delivery: Delivery::Queued, .. } if text == "Hi omp")
    );
    let first_working = f
        .events
        .states
        .lock()
        .unwrap()
        .iter()
        .find(|s| s.working)
        .cloned()
        .unwrap();
    assert!(
        first_working.starting,
        "the state that shows the message says omp is starting"
    );

    until("the turn to end", LIMIT, || {
        let entries = f.entries();
        !f.general.working(f.id())
            && entries.contains(&Entry::Agent {
                text: "Hello there".into(),
                complete: true,
            })
    })
    .await;
    assert_eq!(
        developers(&f.entries()),
        vec![("Hi omp", &Delivery::Delivered)]
    );
    assert!(!f.events.states.lock().unwrap().last().unwrap().starting);

    let received = f.host.received();
    let start = &received[0];
    assert_eq!(
        start["env"]["WEAVER_GENERAL"],
        json!(f.general.identity(f.id()).unwrap())
    );
    assert_eq!(start["env"]["WEAVER_ENDPOINT"], "/tmp/weaver-test.sock");
    // No job variable reaches omp, not even the one Weaver's own environment held.
    for name in ["WEAVER_JOB", "WEAVER_RUN", "WEAVER_TASK"] {
        assert_eq!(start["env"][name], Value::Null, "{name}");
    }
    let args: Vec<&str> = start["args"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(Value::as_str)
        .collect();
    let file = weaver_core::jobs::general::instruction_file(&f.host.dirs);
    assert_eq!(
        args,
        [
            "--approval-mode",
            "yolo",
            "--append-system-prompt",
            file.to_str().unwrap(),
            "acp"
        ]
    );
    let written = std::fs::read_to_string(&file).unwrap();
    assert!(
        written.starts_with("# Weaver Chat\n") && written.contains("weaver job new"),
        "the instruction of Chat is written to general.md"
    );
    let new = received
        .iter()
        .find(|m| m["method"] == "session/new")
        .unwrap();
    assert_eq!(
        new["params"]["cwd"],
        f.project.root.to_string_lossy().as_ref()
    );
    let prompt = received
        .iter()
        .find(|m| m["method"] == "session/prompt")
        .unwrap();
    assert_eq!(
        prompt["params"]["prompt"][0]["text"], "Hi omp",
        "no instruction goes in the conversation"
    );

    // The session is kept: the record holds the project, the session and the instant, and nothing of the conversation.
    let record = f.record().unwrap();
    assert_eq!(
        (record.project.as_str(), record.session_id.as_str()),
        (f.project.root.to_str().unwrap(), "fake-session")
    );
    let text = std::fs::read_to_string(GeneralStore::new(&f.host.dirs).file(f.id())).unwrap();
    assert!(!text.contains("Hi omp") && !text.contains("Hello"));

    // omp stays for the next message: the second one is a second prompt of the same process.
    f.send("And again");
    until("the second turn", LIMIT, || {
        f.entries().contains(&Entry::Agent {
            text: "Again".into(),
            complete: true,
        })
    })
    .await;
    assert_eq!(
        f.methods().iter().filter(|m| *m == "initialize").count(),
        1,
        "omp was not started again"
    );
    assert_eq!(
        f.methods()
            .iter()
            .filter(|m| *m == "session/prompt")
            .count(),
        2
    );
}

#[tokio::test]
async fn a_talk_only_general_never_reaches_the_board_the_jobs_or_a_notification() {
    let f = fixture().await;
    let jobs = Jobs::new(f.host.clone());
    f.host.script(json!({ "prompts": [[agent("Done")]] }));
    f.send("Hello");
    until("the turn to end", LIMIT, || {
        !f.general.working(f.id()) && !f.entries().is_empty() && f.entries().len() > 1
    })
    .await;
    assert_eq!(
        *f.host.board_changes.lock().unwrap(),
        0,
        "a turn of Chat never sends the Board again"
    );
    assert!(
        f.host.snapshots.lock().unwrap().is_empty(),
        "no job list changes"
    );
    assert!(
        jobs.list_for(&f.project.root, true).is_empty(),
        "General is not a job"
    );
}

#[tokio::test]
async fn the_conversation_is_reloaded_after_the_window_closed_without_repeating_what_the_window_kept(
) {
    let f = fixture().await;
    f.host
        .script(json!({ "log": f.host.received_log(), "prompts": [[agent("First answer")]] }));
    f.send("First question");
    until("the first turn", LIMIT, || {
        f.entries().contains(&Entry::Agent {
            text: "First answer".into(),
            complete: true,
        })
    })
    .await;

    // `window.CLOSED`: omp is ended in order, the conversation and the record stay.
    f.general.window_closed(f.id()).await;
    f.received("session/close", |m| m["method"] == "session/close")
        .await;
    assert!(
        f.general.identity(f.id()).is_none(),
        "the identity of the ended omp is no longer accepted"
    );
    assert!(f.record().is_some());
    assert_eq!(f.entries().len(), 2);

    f.host
        .script(json!({ "log": f.host.received_log(), "prompts": [[agent("Second answer")]] }));
    f.general.open(f.id(), &f.project.root).await;
    f.received("the load at window open", |m| m["method"] == "session/load")
        .await;
    assert_eq!(
        f.entries().len(),
        2,
        "the conversation is there when the window opens again"
    );
    let methods = f.methods();
    let load = methods.iter().position(|m| m == "session/load").unwrap();
    assert!(
        !methods[load..].contains(&"session/prompt".to_owned()),
        "the window opening sends no prompt"
    );
    f.send("Second question");
    until("the second turn", LIMIT, || {
        f.entries().contains(&Entry::Agent {
            text: "Second answer".into(),
            complete: true,
        })
    })
    .await;
    let load = f
        .host
        .received()
        .into_iter()
        .find(|m| m["method"] == "session/load")
        .unwrap();
    assert_eq!(
        load["params"]["sessionId"], "fake-session",
        "the kept session is reloaded"
    );
    let texts: Vec<String> = f
        .entries()
        .iter()
        .filter_map(|entry| match entry {
            Entry::Developer { text, .. } | Entry::Agent { text, .. } => Some(text.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(
        texts,
        [
            "First question",
            "First answer",
            "Second question",
            "Second answer"
        ],
        "nothing is repeated"
    );
}

#[tokio::test]
async fn a_change_of_the_analysis_model_ends_an_idle_omp_keeps_the_conversation_and_the_next_message_starts_it_again(
) {
    let f = fixture().await;
    f.host
        .script(json!({ "log": f.host.received_log(), "prompts": [[agent("First answer")]] }));
    f.send("First question");
    until("the first turn", LIMIT, || {
        !f.general.working(f.id())
            && f.entries().contains(&Entry::Agent {
                text: "First answer".into(),
                complete: true,
            })
    })
    .await;

    f.general.analysis_model_changed();
    f.received("session/close", |m| m["method"] == "session/close")
        .await;
    assert!(
        f.general.identity(f.id()).is_none(),
        "omp ended: its identity is no longer accepted"
    );
    assert!(f.record().is_some(), "the kept session stays");
    assert_eq!(f.entries().len(), 2, "the conversation stays");
    // With omp ended a second change has nothing to end.
    f.general.analysis_model_changed();
    assert_eq!(
        f.methods().iter().filter(|m| *m == "session/close").count(),
        1
    );

    f.host
        .script(json!({ "log": f.host.received_log(), "prompts": [[agent("Second answer")]] }));
    f.send("Second question");
    until("the second turn", LIMIT, || {
        f.entries().contains(&Entry::Agent {
            text: "Second answer".into(),
            complete: true,
        })
    })
    .await;
    assert!(
        f.methods().contains(&"session/load".to_owned()),
        "the next message reloads the kept conversation"
    );
    assert_eq!(f.entries().len(), 4, "the replay is not shown twice");
}

#[tokio::test]
async fn the_instants_the_entries_arrived_come_with_the_snapshot_and_with_the_changes() {
    let f = fixture().await;
    f.host.script(json!({ "prompts": [[agent("Answer")]] }));
    let before = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;
    f.send("Question");
    until("the turn", LIMIT, || {
        !f.general.working(f.id()) && f.entries().len() == 2
    })
    .await;
    let snapshot = f.general.snapshot(f.id()).unwrap();
    assert_eq!(snapshot.arrivals.len(), snapshot.entries.len());
    assert!(snapshot.arrivals[0] >= before && snapshot.arrivals[0] <= snapshot.arrivals[1]);
    let changes: Vec<_> = f
        .events
        .entries
        .lock()
        .unwrap()
        .iter()
        .flat_map(|batch| batch.changes.clone())
        .collect();
    assert!(changes.iter().all(|change| change.at >= before));
}

#[tokio::test]
async fn after_a_restart_the_jobs_the_chat_started_come_back_as_their_started_lines_after_the_history(
) {
    let f = fixture().await;
    f.host.script(json!({ "prompts": [[agent("Answer")]] }));
    f.send("Question");
    until("the turn", LIMIT, || {
        !f.general.working(f.id()) && f.entries().len() == 2
    })
    .await;
    f.general.window_closed(f.id()).await;
    f.record().unwrap();

    // The registry of the new run knows the job the Chat started, from its record.
    *f.host.chat_jobs.lock().unwrap() = vec![("jjjjjjjj".into(), "Design the order".into(), 1_700_000_000_000)];
    f.host.script(json!({
        "log": f.host.received_log(),
        "replay": [
            { "sessionUpdate": "user_message_chunk", "content": { "type": "text", "text": "Question" } },
            { "sessionUpdate": "agent_message_chunk", "content": { "type": "text", "text": "Answer" } },
        ],
    }));
    let general = General::new(f.host.clone(), Arc::new(Recorder::default()));
    general.open(f.id(), &f.project.root).await;
    until("the history to come back", LIMIT, || {
        let state = general.snapshot(f.id()).unwrap().state;
        !state.loading && !general.entries(f.id()).is_empty()
    })
    .await;
    let snapshot = general.snapshot(f.id()).unwrap();
    assert!(matches!(snapshot.entries[0], Entry::Developer { .. }));
    assert_eq!(
        snapshot.entries[2],
        Entry::JobStarted {
            job: "jjjjjjjj".into(),
            name: "Design the order".into()
        }
    );
    assert_eq!(snapshot.entries.len(), 3);
    assert_eq!(snapshot.arrivals[2], 1_700_000_000_000);
}

#[tokio::test]
async fn after_a_restart_of_weaver_the_history_comes_back_when_the_window_opens_without_a_message()
{
    let f = fixture().await;
    f.host.script(json!({ "prompts": [[agent("Answer")]] }));
    f.send("Question");
    until("the turn", LIMIT, || {
        f.entries().contains(&Entry::Agent {
            text: "Answer".into(),
            complete: true,
        })
    })
    .await;
    f.general.window_closed(f.id()).await;
    f.record().unwrap();

    // A new run of Weaver: a new General over the same folders. The replay is what omp keeps.
    f.host.script(json!({
        "log": f.host.received_log(),
        "replay": [
            { "sessionUpdate": "user_message_chunk", "content": { "type": "text", "text": "Question" } },
            { "sessionUpdate": "agent_message_chunk", "content": { "type": "text", "text": "Answer" } },
        ],
        "prompts": [[agent("Newer answer")]],
    }));
    let events = Arc::new(Recorder::default());
    let general = General::new(f.host.clone(), events.clone());
    general.open(f.id(), &f.project.root).await;
    let snapshot = general.snapshot(f.id()).unwrap();
    assert!(
        snapshot.state.loading && snapshot.state.starting && snapshot.state.kept,
        "the window opening starts the load: {:?}",
        snapshot.state
    );
    assert!(!snapshot.state.working);

    until("the history to come back", LIMIT, || {
        let state = general.snapshot(f.id()).unwrap().state;
        !state.loading && !general.entries(f.id()).is_empty()
    })
    .await;
    let entries = general.entries(f.id());
    assert!(
        matches!(&entries[0], Entry::Developer { text, delivery: Delivery::Delivered, .. } if text == "Question")
    );
    assert_eq!(
        entries[1],
        Entry::Agent {
            text: "Answer".into(),
            complete: true
        }
    );
    assert_eq!(entries.len(), 2);
    assert!(
        events.entries.lock().unwrap().iter().any(|e| e.reset),
        "the UI is told to fetch the rebuilt conversation"
    );
    let state = general.snapshot(f.id()).unwrap().state;
    assert!(!state.kept && !state.loading && !state.working && !state.starting);
    let methods = f.methods();
    let load = methods.iter().position(|m| m == "session/load").unwrap();
    assert!(
        !methods[load..].contains(&"session/prompt".to_owned()),
        "nothing is sent to the agent while the conversation comes back"
    );

    // The first message after the history is the first prompt of the reloaded session, and the
    // answer opens a new entry instead of continuing the replayed one.
    general.send_message(f.id(), "Newer question").unwrap();
    until("the reloaded turn", LIMIT, || {
        general.entries(f.id()).contains(&Entry::Agent {
            text: "Newer answer".into(),
            complete: true,
        })
    })
    .await;
    let entries = general.entries(f.id());
    assert!(
        !entries
            .iter()
            .any(|e| matches!(e, Entry::Instruction { .. })),
        "a General has no instruction, so the first user message is the developer's"
    );
    assert!(
        matches!(&entries[2], Entry::Developer { text, .. } if text == "Newer question"),
        "the message stays after the history"
    );
    assert_eq!(
        entries[1],
        Entry::Agent {
            text: "Answer".into(),
            complete: true
        }
    );
}

#[tokio::test]
async fn a_window_opening_with_no_kept_session_starts_no_omp() {
    let f = fixture().await;
    let state = f.general.snapshot(f.id()).unwrap().state;
    assert!(!state.loading && !state.starting && !state.kept && !state.working);
    assert!(
        f.general.identity(f.id()).is_none(),
        "omp starts only with the first message"
    );
    assert!(f.host.received().is_empty());
}

#[tokio::test]
async fn a_message_sent_while_the_conversation_loads_waits_and_goes_out_below_the_history() {
    let f = fixture().await;
    f.host.script(json!({ "prompts": [[agent("Answer")]] }));
    f.send("Question");
    until("the turn", LIMIT, || {
        f.entries().contains(&Entry::Agent {
            text: "Answer".into(),
            complete: true,
        })
    })
    .await;
    f.general.window_closed(f.id()).await;
    f.record().unwrap();

    // The agent holds the load until the gate file exists: nothing else orders the test.
    let gate = f.host.dirs.local_data_dir().join("load.gate");
    f.host.script(json!({
        "log": f.host.received_log(),
        "loadGate": gate,
        "replay": [
            { "sessionUpdate": "user_message_chunk", "content": { "type": "text", "text": "Question" } },
            { "sessionUpdate": "agent_message_chunk", "content": { "type": "text", "text": "Answer" } },
        ],
        "prompts": [[agent("Second answer")]],
    }));
    let events = Arc::new(Recorder::default());
    let general = General::new(f.host.clone(), events.clone());
    general.open(f.id(), &f.project.root).await;
    f.received("the load request", |m| m["method"] == "session/load")
        .await;
    general.send_message(f.id(), "Meanwhile").unwrap();
    let state = general.snapshot(f.id()).unwrap().state;
    assert!(
        state.loading && state.starting && !state.working,
        "the message waits in Loading, it is not Working: {state:?}"
    );
    assert!(
        matches!(&general.entries(f.id())[..], [Entry::Developer { text, delivery: Delivery::Queued, .. }] if text == "Meanwhile")
    );
    assert!(
        !f.methods()
            .iter()
            .skip_while(|m| *m != "session/load")
            .any(|m| m == "session/prompt" || m == "_session/steering"),
        "nothing is sent to the agent while the conversation loads"
    );

    std::fs::write(&gate, "open").unwrap();
    until("the message to be answered", LIMIT, || {
        general.entries(f.id()).contains(&Entry::Agent {
            text: "Second answer".into(),
            complete: true,
        })
    })
    .await;
    let entries = general.entries(f.id());
    assert!(matches!(&entries[0], Entry::Developer { text, .. } if text == "Question"));
    assert_eq!(
        entries[1],
        Entry::Agent {
            text: "Answer".into(),
            complete: true
        }
    );
    assert!(
        matches!(&entries[2], Entry::Developer { text, delivery: Delivery::Delivered, .. } if text == "Meanwhile"),
        "the history is above the message sent meanwhile: {entries:?}"
    );
    let methods = f.methods();
    let load = methods.iter().position(|m| m == "session/load").unwrap();
    assert!(
        methods[load..]
            .iter()
            .filter(|m| *m == "session/prompt")
            .count()
            == 1,
        "the waiting message is one prompt, after the load"
    );
}

#[tokio::test]
async fn clear_while_the_conversation_loads_ends_that_omp_and_starts_empty() {
    let f = fixture().await;
    f.host.script(json!({ "prompts": [[agent("Answer")]] }));
    f.send("Question");
    until("the turn", LIMIT, || {
        f.entries().contains(&Entry::Agent {
            text: "Answer".into(),
            complete: true,
        })
    })
    .await;
    f.general.window_closed(f.id()).await;
    f.record().unwrap();

    let gate = f.host.dirs.local_data_dir().join("never.gate");
    f.host.script(json!({
        "log": f.host.received_log(),
        "loadGate": gate,
        "replay": [
            { "sessionUpdate": "agent_message_chunk", "content": { "type": "text", "text": "Answer" } },
        ],
    }));
    let general = General::new(f.host.clone(), Arc::new(Recorder::default()));
    general.open(f.id(), &f.project.root).await;
    f.received("the load request", |m| m["method"] == "session/load")
        .await;
    general.send_message(f.id(), "Meanwhile").unwrap();

    general.clear(f.id()).unwrap();
    let state = general.snapshot(f.id()).unwrap().state;
    assert!(!state.loading && !state.starting && !state.kept && !state.working);
    assert!(
        general.entries(f.id()).is_empty(),
        "the conversation starts empty"
    );
    general.flush();
    assert!(
        GeneralStore::new(&f.host.dirs)
            .read(f.id())
            .unwrap()
            .is_none(),
        "the kept entry is deleted"
    );
    assert!(
        general.identity(f.id()).is_none(),
        "that omp is ended, its identity no longer accepted"
    );
    // The ended omp never answers the load: nothing comes back into the empty conversation.
    general.window_closed(f.id()).await;
    std::fs::write(&gate, "open").unwrap();
    assert!(general.entries(f.id()).is_empty());
    assert!(!general.snapshot(f.id()).unwrap().state.kept);
}

#[tokio::test]
async fn closing_the_window_while_the_conversation_loads_ends_omp_and_keeps_the_record() {
    let f = fixture().await;
    f.host.script(json!({ "prompts": [[agent("Answer")]] }));
    f.send("Question");
    until("the turn", LIMIT, || {
        f.entries().contains(&Entry::Agent {
            text: "Answer".into(),
            complete: true,
        })
    })
    .await;
    f.general.window_closed(f.id()).await;
    f.record().unwrap();

    let gate = f.host.dirs.local_data_dir().join("never.gate");
    f.host
        .script(json!({ "log": f.host.received_log(), "loadGate": gate }));
    let general = General::new(f.host.clone(), Arc::new(Recorder::default()));
    general.open(f.id(), &f.project.root).await;
    f.received("the load request", |m| m["method"] == "session/load")
        .await;
    general.window_closed(f.id()).await;
    let state = general.snapshot(f.id()).unwrap().state;
    assert!(!state.loading && !state.starting && !state.working);
    assert!(
        state.kept,
        "the conversation is kept and loads again when the window opens"
    );
    general.flush();
    assert!(GeneralStore::new(&f.host.dirs)
        .read(f.id())
        .unwrap()
        .is_some());
}

#[tokio::test]
async fn when_the_kept_session_cannot_be_loaded_an_empty_one_starts_with_a_notice_above_the_waiting_messages(
) {
    let f = fixture().await;
    f.host.script(json!({ "prompts": [[agent("Answer")]] }));
    f.send("Question");
    until("the turn", LIMIT, || {
        f.entries().contains(&Entry::Agent {
            text: "Answer".into(),
            complete: true,
        })
    })
    .await;
    f.general.window_closed(f.id()).await;
    f.record().unwrap();

    f.host.script(json!({ "log": f.host.received_log(), "loadError": "session not found", "sessionId": "fresh", "prompts": [[agent("Fresh answer")]] }));
    let general = General::new(f.host.clone(), Arc::new(Recorder::default()));
    general.open(f.id(), &f.project.root).await;
    until("the empty session with the notice", LIMIT, || {
        general.entries(f.id()).iter().any(|e| matches!(e, Entry::Notice { text } if text.contains("could not be reloaded") && text.contains("session not found")))
    })
    .await;
    assert!(
        !general.snapshot(f.id()).unwrap().state.loading,
        "the loading ends with the empty session"
    );
    general.send_message(f.id(), "Hello again").unwrap();
    until("the turn", LIMIT, || {
        general.entries(f.id()).contains(&Entry::Agent {
            text: "Fresh answer".into(),
            complete: true,
        })
    })
    .await;
    let entries = general.entries(f.id());
    assert!(
        matches!(&entries[0], Entry::Notice { .. }),
        "the notice comes first: {entries:?}"
    );
    let methods: Vec<String> = f.methods();
    let load = methods.iter().position(|m| m == "session/load").unwrap();
    assert!(
        methods[load..].contains(&"session/new".to_owned()),
        "an empty session starts after the failed load"
    );
    general.flush();
    assert_eq!(
        GeneralStore::new(&f.host.dirs)
            .read(f.id())
            .unwrap()
            .unwrap()
            .session_id,
        "fresh",
        "the record names the new session"
    );
}

#[tokio::test]
async fn stop_cancels_the_turn_without_a_dialog_and_omp_stays_for_the_next_message() {
    let f = fixture().await;
    f.host.script(json!({
        "log": f.host.received_log(),
        "prompts": [[agent("Working"), { "wait_cancel": true }], [agent("Next answer")]],
    }));
    f.send("Do a long thing");
    until("the agent to answer", LIMIT, || {
        f.entries().contains(&Entry::Agent {
            text: "Working".into(),
            complete: false,
        })
    })
    .await;
    f.general.stop(f.id()).unwrap();
    assert!(!f.general.working(f.id()), "the Machine is Idle at once");
    f.received("session/cancel", |m| m["method"] == "session/cancel")
        .await;
    until("the cancelled answer to be complete", LIMIT, || {
        f.entries().contains(&Entry::Agent {
            text: "Working".into(),
            complete: true,
        })
    })
    .await;
    assert!(
        !f.methods().contains(&"session/close".to_owned()),
        "omp is kept"
    );

    f.send("Another thing");
    until("the next answer", LIMIT, || {
        f.entries().contains(&Entry::Agent {
            text: "Next answer".into(),
            complete: true,
        })
    })
    .await;
    assert_eq!(f.methods().iter().filter(|m| *m == "initialize").count(), 1);
}

#[tokio::test]
async fn the_exit_of_omp_shows_the_reason_and_the_next_message_starts_omp_again() {
    let f = fixture().await;
    f.host.script(json!({ "log": f.host.received_log(), "prompts": [[agent("Starting"), { "stderr": "error: no provider" }, { "exit": 3 }]] }));
    f.send("Hello");
    until("omp to stop", LIMIT, || {
        f.entries()
            .iter()
            .any(|e| matches!(e, Entry::OmpStopped { .. }))
    })
    .await;
    let stopped = f.entries().into_iter().find_map(|e| match e {
        Entry::OmpStopped { reason, details } => Some((reason, details)),
        _ => None,
    });
    let (reason, details) = stopped.unwrap();
    assert_eq!(reason, "error: no provider");
    assert_eq!(details.as_deref(), Some("error: no provider"));
    assert!(!f.general.working(f.id()));
    assert!(f.general.identity(f.id()).is_none());

    f.send("Are you there?");
    until("omp to start again", LIMIT, || {
        f.methods().iter().filter(|m| *m == "initialize").count() == 2
    })
    .await;
    until("the second stop", LIMIT, || {
        f.entries()
            .iter()
            .filter(|e| matches!(e, Entry::OmpStopped { .. }))
            .count()
            == 2
    })
    .await;
    assert!(
        f.methods().contains(&"session/load".to_owned()),
        "the second start reloads the session the first one opened"
    );
}

#[tokio::test]
async fn omp_that_cannot_open_its_session_is_a_stop_with_the_reason_and_a_missing_omp_is_reported()
{
    let f = fixture().await;
    f.host
        .script(json!({ "newError": "no provider is logged in" }));
    f.send("Hello");
    until("omp to stop", LIMIT, || f.entries().iter().any(|e| matches!(e, Entry::OmpStopped { reason, .. } if reason == "no provider is logged in"))).await;
    assert!(!f.general.working(f.id()));
    assert!(
        matches!(
            &f.entries()[0],
            Entry::Developer {
                delivery: Delivery::NotDelivered { .. },
                ..
            }
        ),
        "the message did not reach the agent"
    );
    assert!(
        f.record().is_none(),
        "no session was opened, so none is kept"
    );
}

#[tokio::test]
async fn a_turn_that_fails_shows_its_reason_and_keeps_omp() {
    let f = fixture().await;
    f.host
        .script(json!({ "prompts": [[{ "end": "refusal" }]] }));
    f.send("Hello");
    until("the failure", LIMIT, || {
        f.entries()
            .iter()
            .any(|e| matches!(e, Entry::TurnFailed { reason } if reason == "the agent refused"))
    })
    .await;
    assert!(!f.general.working(f.id()));
    assert!(f.general.identity(f.id()).is_some(), "omp stays");
}

/// A certificate error and an HTTP error, each with the setting on and off, and the explanation
/// each one must produce.
fn proxy_cases() -> Vec<(String, bool, String)> {
    let mut cases = Vec::new();
    for text in [
        "fetch failed: UNABLE_TO_GET_ISSUER_CERT_LOCALLY",
        "request failed: HTTP 407",
    ] {
        for on in [false, true] {
            let expected = weaver_core::omp::failure::explain(text, on).unwrap();
            assert_eq!(expected.contains("turning on System Certificates"), !on);
            cases.push((text.to_owned(), on, expected));
        }
    }
    cases
}

fn stopped(f: &Fixture) -> Option<(String, Option<String>)> {
    f.entries().into_iter().find_map(|e| match e {
        Entry::OmpStopped { reason, details } => Some((reason, details)),
        _ => None,
    })
}

#[tokio::test]
async fn an_exit_or_an_opening_with_a_proxy_error_stops_omp_with_the_explanation_and_omps_lines_as_details(
) {
    for (text, on, expected) in proxy_cases() {
        // omp exits: the error is not on the last line, the whole tail is tested.
        let f = fixture().await;
        f.host.settings.lock().unwrap().system_certificates.enabled = on;
        f.host.script(json!({ "prompts": [[agent("Starting"), { "stderr": text }, { "stderr": "error: giving up" }, { "exit": 3 }]] }));
        f.send("Hello");
        until("omp to stop", LIMIT, || stopped(&f).is_some()).await;
        let (reason, details) = stopped(&f).unwrap();
        assert_eq!(reason, expected, "exit {text} {on}");
        assert_eq!(details, Some(format!("{text}\nerror: giving up")));

        // The session cannot be opened.
        let f = fixture().await;
        f.host.settings.lock().unwrap().system_certificates.enabled = on;
        f.host.script(json!({ "newError": text }));
        f.send("Hello");
        until("omp to stop", LIMIT, || stopped(&f).is_some()).await;
        assert_eq!(stopped(&f).unwrap().0, expected, "opening {text} {on}");
    }
}

#[tokio::test]
async fn a_turn_and_a_refused_message_with_a_proxy_error_show_the_explanation_and_omp_stays() {
    for (text, on, expected) in proxy_cases() {
        let f = fixture().await;
        f.host.settings.lock().unwrap().system_certificates.enabled = on;
        f.host
            .script(json!({ "prompts": [[agent("Working"), { "error": text }]] }));
        f.send("Hello");
        until("the failure", LIMIT, || {
            f.entries()
                .iter()
                .any(|e| matches!(e, Entry::TurnFailed { reason } if *reason == expected))
        })
        .await;
        assert!(f.general.identity(f.id()).is_some(), "omp stays");

        let f = fixture().await;
        f.host.settings.lock().unwrap().system_certificates.enabled = on;
        f.host.script(json!({ "log": f.host.received_log(), "steering": [format!("failed:{text}")], "prompts": [[{ "wait_steer": 1 }, agent("Still here")]] }));
        f.send("Hello");
        f.received("the first prompt", |m| m["method"] == "session/prompt")
            .await;
        f.send("Meanwhile");
        until("the refusal", LIMIT, || {
            f.entries().iter().any(|e| matches!(e, Entry::Developer { text, delivery: Delivery::NotDelivered { reason }, .. } if text == "Meanwhile" && *reason == expected))
        })
        .await;
    }
}

#[tokio::test]
async fn an_error_that_is_not_a_proxy_problem_keeps_its_reason_in_chat() {
    let f = fixture().await;
    f.host
        .script(json!({ "prompts": [[{ "error": "status 404" }]] }));
    f.send("Hello");
    until("the failure", LIMIT, || {
        f.entries()
            .iter()
            .any(|e| matches!(e, Entry::TurnFailed { reason } if reason == "status 404"))
    })
    .await;
}

#[tokio::test]
async fn every_start_of_omp_reads_the_certificate_setting_again_also_the_one_a_message_makes_after_a_stop(
) {
    let names = [
        "NODE_EXTRA_CA_CERTS",
        "SSL_CERT_FILE",
        "PI_REQUEST_COMPRESSION",
    ];
    let f = fixture().await;
    let bundle = f.host.dirs.system_certificates_file();
    std::fs::create_dir_all(bundle.parent().unwrap()).unwrap();
    std::fs::write(&bundle, "pem").unwrap();
    f.host.settings.lock().unwrap().system_certificates.enabled = true;
    f.host.script(json!({
        "log": f.host.received_log(),
        "logEnv": names,
        "prompts": [[{ "exit": 3 }]],
    }));
    let stops = || {
        f.entries()
            .iter()
            .filter(|e| matches!(e, Entry::OmpStopped { .. }))
            .count()
    };
    f.send("Hello");
    until("the first stop", LIMIT, || stops() == 1).await;

    // The setting is turned off while omp is stopped: the next start gets nothing.
    f.host.settings.lock().unwrap().system_certificates.enabled = false;
    f.send("Again");
    until("the second stop", LIMIT, || stops() == 2).await;

    // And on again: the third start gets the variables of that moment.
    f.host.settings.lock().unwrap().system_certificates.enabled = true;
    f.send("Once more");
    until("the third stop", LIMIT, || stops() == 3).await;

    let environments: Vec<Value> = f
        .host
        .received()
        .into_iter()
        .filter_map(|line| line.get("env").cloned())
        .collect();
    assert_eq!(environments.len(), 3);
    let bundle = json!(bundle.to_string_lossy());
    assert_eq!(environments[0]["NODE_EXTRA_CA_CERTS"], bundle);
    assert_eq!(environments[0]["SSL_CERT_FILE"], bundle);
    assert_eq!(environments[0]["PI_REQUEST_COMPRESSION"], "0");
    assert!(names.iter().all(|name| environments[1][name].is_null()));
    assert_eq!(environments[2]["SSL_CERT_FILE"], bundle);
    assert_eq!(environments[2]["PI_REQUEST_COMPRESSION"], "0");
}

#[tokio::test]
async fn clear_deletes_the_record_and_the_conversation_also_during_a_turn_and_a_new_one_starts_empty(
) {
    let f = fixture().await;
    f.host.script(json!({ "log": f.host.received_log(), "prompts": [[agent("One")], [agent("Two"), { "hold": true }]] }));
    f.send("First");
    until("the turn", LIMIT, || {
        f.entries().contains(&Entry::Agent {
            text: "One".into(),
            complete: true,
        })
    })
    .await;
    assert!(f.record().is_some());

    // Clear while idle.
    f.general.clear(f.id()).unwrap();
    assert!(f.entries().is_empty());
    assert!(f.record().is_none(), "the record is deleted");
    f.received("session/close", |m| m["method"] == "session/close")
        .await;
    assert!(f.events.entries.lock().unwrap().last().unwrap().reset);
    assert!(!f.general.snapshot(f.id()).unwrap().state.kept);

    // The next message starts an empty session, not a reload.
    f.host.script(
        json!({ "log": f.host.received_log(), "prompts": [[agent("Fresh"), { "hold": true }]] }),
    );
    f.send("Second");
    until("the fresh answer", LIMIT, || {
        f.entries().contains(&Entry::Agent {
            text: "Fresh".into(),
            complete: false,
        })
    })
    .await;
    assert!(!f.methods().contains(&"session/load".to_owned()));
    assert_eq!(
        f.methods().iter().filter(|m| *m == "session/new").count(),
        2
    );

    // Clear during a turn stops it.
    f.general.clear(f.id()).unwrap();
    assert!(f.entries().is_empty() && !f.general.working(f.id()));
    until("the second shutdown", LIMIT, || {
        f.methods().iter().filter(|m| *m == "session/close").count() == 2
    })
    .await;
    assert!(
        f.methods().contains(&"session/cancel".to_owned()),
        "the turn was cancelled"
    );
    assert!(f.record().is_none());
}

#[tokio::test]
async fn closing_the_window_during_a_turn_cancels_it_and_keeps_the_conversation() {
    let f = fixture().await;
    f.host.script(
        json!({ "log": f.host.received_log(), "prompts": [[agent("Working"), { "hold": true }]] }),
    );
    f.send("Long work");
    until("the agent to answer", LIMIT, || {
        f.entries().contains(&Entry::Agent {
            text: "Working".into(),
            complete: false,
        })
    })
    .await;
    f.general.window_closed(f.id()).await;
    assert!(!f.general.working(f.id()));
    let methods = f.methods();
    assert!(
        methods.contains(&"session/cancel".to_owned())
            && methods.contains(&"session/close".to_owned()),
        "{methods:?}"
    );
    assert!(
        f.entries()
            .iter()
            .any(|e| matches!(e, Entry::Agent { text, .. } if text == "Working")),
        "the conversation is kept"
    );
    assert!(f.record().is_some());
}

#[tokio::test]
async fn clear_discards_no_draft_and_neither_do_stop_and_closing_the_window() {
    let f = fixture().await;
    let proposal = |id: &str| {
        f.project
            .root
            .join(format!(".weaver/proposals/{id}-p/proposal.md"))
    };
    for (id, marks) in [
        ("aaaaaaaa", "chat: true\nchatWriting: true\n"),
        ("bbbbbbbb", "chatWriting: true\n"),
    ] {
        std::fs::create_dir_all(proposal(id).parent().unwrap()).unwrap();
        std::fs::write(
            proposal(id),
            format!("---\nstatus: draft\nkind: change\ncreated: 2026-11-02T07:00:00.000Z\n{marks}---\n# P\n"),
        )
        .unwrap();
    }
    let status_of = |id: &str| std::fs::read_to_string(proposal(id)).unwrap();
    f.host.script(
        json!({ "log": f.host.received_log(), "prompts": [[agent("Writing"), { "hold": true }]] }),
    );
    f.send("Write it");
    f.working().await;

    // Stop and the closing of the window leave every Draft as it is.
    f.general.stop(f.id()).unwrap();
    until("the turn to stop", LIMIT, || !f.general.working(f.id())).await;
    f.general.window_closed(f.id()).await;
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    assert!(status_of("aaaaaaaa").contains("status: draft"));
    assert!(status_of("bbbbbbbb").contains("status: draft"));

    // Clear empties the conversation and nothing else: no Draft is discarded.
    f.general.clear(f.id()).unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    assert!(f.entries().is_empty());
    assert!(status_of("aaaaaaaa").contains("status: draft"));
    assert!(status_of("bbbbbbbb").contains("status: draft"));
}

#[tokio::test]
async fn a_permission_waits_for_the_developer_in_working_and_a_form_is_declined_with_a_text_that_asks_in_the_reply(
) {
    let f = fixture().await;
    f.host.permissions(Permissions::Ask);
    f.host.script(json!({
        "log": f.host.received_log(),
        "prompts": [[
            { "elicit": true },
            { "permission": { "toolCall": { "toolCallId": "t1", "title": "$ ls", "kind": "execute", "rawInput": { "command": "ls" } }, "options": [
                { "optionId": "allow", "name": "Allow once", "kind": "allow_once" },
                { "optionId": "no", "name": "Reject", "kind": "reject_once" },
            ] } },
            agent("Listed"),
        ]],
    }));
    f.send("List the files");
    until("the permission card", LIMIT, || {
        f.entries().iter().any(|e| {
            matches!(
                e,
                Entry::Permission {
                    state: PermissionState::Asking,
                    ..
                }
            )
        })
    })
    .await;
    assert!(f.general.working(f.id()), "the permission waits in Working");
    let declined = f
        .host
        .received()
        .into_iter()
        .find(|m| m["answerTo"] == "elicitation/create")
        .unwrap();
    assert_eq!(declined["result"]["action"], "decline");
    assert!(declined["result"]["_meta"]["message"]
        .as_str()
        .unwrap()
        .contains("ask the developer in your reply"));

    let request = f.entries().iter().find_map(|e| match e {
        Entry::Permission { request, .. } => Some(*request),
        _ => None,
    });
    assert!(f
        .general
        .answer_permission(f.id(), request.unwrap(), "missing")
        .is_err());
    f.general
        .answer_permission(f.id(), request.unwrap(), "allow")
        .unwrap();
    assert!(
        f.general
            .answer_permission(f.id(), request.unwrap(), "allow")
            .is_err(),
        "answered once"
    );
    until("the turn to end", LIMIT, || {
        f.entries().contains(&Entry::Agent {
            text: "Listed".into(),
            complete: true,
        })
    })
    .await;
    let answer = f
        .host
        .received()
        .into_iter()
        .find(|m| m["answerTo"] == "session/request_permission")
        .unwrap();
    assert_eq!(
        answer["result"]["outcome"],
        json!({ "outcome": "selected", "optionId": "allow" })
    );
    assert!(f.entries().iter().any(|e| matches!(e, Entry::Permission { state: PermissionState::Chosen { name }, .. } if name == "Allow once")));
}

// -----------------------------------------------------------------------------------------------
// `weaver job new` from General
// -----------------------------------------------------------------------------------------------

mod job_new {
    use weaver_cli::exec::{execute, Authority};
    use weaver_cli::spec::Caller;
    use weaver_core::command::{code, CommandRequest};

    use super::*;
    use crate::jobs::authority::AppAuthority;

    fn request(
        object: &str,
        verb: &str,
        root: &Path,
        input: Value,
        token: Option<&str>,
    ) -> CommandRequest {
        CommandRequest {
            object: object.into(),
            verb: verb.into(),
            card: None,
            state: None,
            root: root.to_string_lossy().into_owned(),
            input: input.as_object().unwrap().clone(),
            general: token.map(str::to_owned),
            ask: None,
        }
    }

    struct Setup {
        f: Fixture,
        jobs: Arc<Jobs>,
        authority: AppAuthority,
    }

    async fn setup() -> Setup {
        let host = TestHost::new();
        let events = Arc::new(Recorder::default());
        let general = General::new(host.clone(), events.clone());
        let jobs = Jobs::new(host.clone());
        jobs.set_general(general.clone());
        let (dir, project) = host
            .project(&[("tasks/aaaaaaaa-first-task.md", TASK)])
            .await;
        general.open(&project.id, &project.root).await;
        let authority = AppAuthority { jobs: jobs.clone() };
        Setup {
            f: Fixture {
                host,
                general,
                events,
                _dir: dir,
                project,
            },
            jobs,
            authority,
        }
    }

    fn run(
        s: &Setup,
        input: Value,
        token: Option<&str>,
    ) -> Result<weaver_core::command::CommandReply, weaver_cli::spec::Failure> {
        let caller = Caller {
            general: token.map(str::to_owned),
            ..Caller::default()
        };
        execute(
            &request("job", "new", &s.f.project.root, input, token),
            &caller,
            &s.authority,
        )
    }

    #[tokio::test]
    async fn the_working_general_starts_a_job_with_its_own_conversation_and_the_developers_refusals_are_shown(
    ) {
        let s = setup().await;
        s.f.host
            .script(json!({ "prompts": [[agent("Starting the scan"), { "hold": true }]] }));
        s.f.send("Please scan the System");
        s.f.working().await;
        until("omp to open", LIMIT, || {
            s.f.general.identity(s.f.id()).is_some()
        })
        .await;
        until("the agent to answer", LIMIT, || {
            s.f.entries()
                .iter()
                .any(|e| matches!(e, Entry::Agent { .. }))
        })
        .await;
        let token = s.f.general.identity(s.f.id()).unwrap();

        let started = run(&s, json!({ "kind": "run", "task": "t1" }), Some(&token)).unwrap();
        let job = started.json["id"].as_str().unwrap().to_owned();
        assert_eq!(started.json["kind"], "run");
        // The job is a normal job with a conversation of its own.
        assert_eq!(s.jobs.record(&job).unwrap().name, "Run T1");
        assert!(s.jobs.conversation(s.f.id(), &job).is_ok());
        assert!(
            s.f.entries().contains(&Entry::JobStarted {
                job: job.clone(),
                name: "Run T1".into()
            }),
            "{:?}",
            s.f.entries()
        );
        assert!(s.f.general.working(s.f.id()), "START_JOB stays in Working");

        // A second Run is refused as it is for the developer, and General says so.
        let refused = run(&s, json!({ "kind": "run" }), Some(&token)).unwrap_err();
        assert_eq!(refused.code, code::REFUSED);
        let shown = s.f.entries().into_iter().find_map(|e| match e {
            Entry::JobRefused { job_kind, reason } => Some((job_kind, reason)),
            _ => None,
        });
        let (kind, reason) = shown.unwrap();
        assert_eq!(kind, "run");
        assert!(
            reason.contains("A run is in progress")
                && !reason.ends_with('.')
                && !reason.starts_with("Could not start"),
            "{reason}"
        );
        // A scan always starts, and an unknown card is refused with the card's own words.
        assert!(run(&s, json!({ "kind": "scan" }), Some(&token)).is_ok());
        assert!(run(
            &s,
            json!({ "kind": "resolve", "drift": "d9" }),
            Some(&token)
        )
        .is_err());
    }

    #[tokio::test]
    async fn job_new_is_refused_to_every_other_caller_and_in_every_other_state() {
        let s = setup().await;
        s.f.host
            .script(json!({ "log": s.f.host.received_log(), "prompts": [[{ "hold": true }]] }));
        // No General has started omp yet.
        assert_eq!(
            run(&s, json!({ "kind": "scan" }), Some("gggggggg"))
                .unwrap_err()
                .code,
            code::FORBIDDEN
        );
        s.f.send("Hello");
        until("omp to open", LIMIT, || {
            s.f.general.identity(s.f.id()).is_some() && s.f.general.working(s.f.id())
        })
        .await;
        let token = s.f.general.identity(s.f.id()).unwrap();
        until("the turn to start", LIMIT, || {
            s.f.methods().contains(&"session/prompt".to_owned())
        })
        .await;

        assert_eq!(
            run(&s, json!({ "kind": "scan" }), None).unwrap_err().code,
            code::FORBIDDEN,
            "no identity"
        );
        assert_eq!(
            run(&s, json!({ "kind": "scan" }), Some("another1"))
                .unwrap_err()
                .code,
            code::FORBIDDEN,
            "not this General"
        );
        let a_job = Caller {
            job: Some(token.clone()),
            ..Caller::default()
        };
        let as_job = execute(
            &request(
                "job",
                "new",
                &s.f.project.root,
                json!({ "kind": "scan" }),
                None,
            ),
            &a_job,
            &s.authority,
        );
        assert_eq!(
            as_job.unwrap_err().code,
            code::FORBIDDEN,
            "a job's identity is not General's"
        );
        assert!(
            s.jobs.list_for(&s.f.project.root, true).is_empty(),
            "a refusal starts nothing"
        );
        assert!(!s
            .f
            .entries()
            .iter()
            .any(|e| matches!(e, Entry::JobStarted { .. } | Entry::JobRefused { .. })));

        // The commands tied to a job are refused in General, with the same identity.
        for (object, verb, input) in [
            ("job", "edit", json!({ "note": "x" })),
            (
                "announcement",
                "new",
                json!({ "kind": "task", "title": "t" }),
            ),
        ] {
            let caller = Caller {
                general: Some(token.clone()),
                ..Caller::default()
            };
            let error = execute(
                &request(object, verb, &s.f.project.root, input, Some(&token)),
                &caller,
                &s.authority,
            )
            .unwrap_err();
            assert_eq!(error.code, code::FORBIDDEN, "{object} {verb}");
        }
        let caller = Caller {
            general: Some(token.clone()),
            ..Caller::default()
        };
        let mut moved = request("task", "move", &s.f.project.root, json!({}), Some(&token));
        moved.card = Some("aaaaaaaa".into());
        moved.state = Some("doing".into());
        assert_eq!(
            execute(&moved, &caller, &s.authority).unwrap_err().code,
            code::FORBIDDEN
        );

        // Once the turn ends, General is Idle and its identity is not accepted any more.
        assert!(s.authority.accepts_general(&s.f.project.root, &token));
        s.f.general.stop(s.f.id()).unwrap();
        assert!(
            !s.authority.accepts_general(&s.f.project.root, &token),
            "only the working General"
        );
        assert_eq!(
            run(&s, json!({ "kind": "scan" }), Some(&token))
                .unwrap_err()
                .code,
            code::FORBIDDEN
        );
    }
}

/// Whether the process is alive (a zombie already reaped counts as gone).
#[cfg(unix)]
fn alive(pid: &str) -> bool {
    std::process::Command::new("kill")
        .args(["-0", pid])
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

#[cfg(unix)]
#[tokio::test]
async fn clear_right_after_the_window_opened_leaves_no_omp_alive_however_far_it_had_started() {
    let f = fixture().await;
    f.host.script(json!({ "prompts": [[agent("Answer")]] }));
    f.send("Question");
    until("the turn", LIMIT, || {
        f.entries().contains(&Entry::Agent {
            text: "Answer".into(),
            complete: true,
        })
    })
    .await;
    f.general.window_closed(f.id()).await;
    f.record().unwrap();

    for round in 0..5 {
        let pid_file = f
            .host
            .dirs
            .local_data_dir()
            .join(format!("omp-{round}.pid"));
        let gate = f.host.dirs.local_data_dir().join("never.gate");
        f.host
            .script(json!({ "loadGate": gate, "pidFile": pid_file }));
        let general = General::new(f.host.clone(), Arc::new(Recorder::default()));
        general.open(f.id(), &f.project.root).await;
        // Clear while omp may be waited for, being spawned, or loading.
        general.clear(f.id()).unwrap();
        // Every omp of that actor is ended when it is done.
        general.window_closed(f.id()).await;
        if let Ok(pid) = std::fs::read_to_string(&pid_file) {
            until("the omp to be gone", LIMIT, || !alive(pid.trim())).await;
        }
        // Clear deleted the record: put one back for the next round.
        f.general.flush();
        general.flush();
        weaver_core::jobs::general::GeneralStore::new(&f.host.dirs)
            .write(
                f.id(),
                &GeneralRecord {
                    project: f.project.root.to_string_lossy().into_owned(),
                    session_id: "fake-session".into(),
                    updated: "2026-11-02T07:00:01.000Z".into(),
                    cards: None,
                },
            )
            .unwrap();
    }
}

// ---------------------------------------------------------------------------------------------
// Sub-agents
// ---------------------------------------------------------------------------------------------

/// The folder of the sub-agents' transcripts of the fake agent's session, under the sessions
/// folder of the test host; its main session file is created.
fn agents_folder(host: &TestHost) -> PathBuf {
    let folder = host
        .dirs
        .local_data_dir()
        .join("omp-sessions")
        .join("--project--");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(
        folder.join("2026-11-02T07-00-00-000Z_fake-session.jsonl"),
        "{}\n",
    )
    .unwrap();
    folder.join("2026-11-02T07-00-00-000Z_fake-session")
}

fn user_line(text: &str) -> String {
    format!(
        "{}\n",
        json!({ "type": "message", "message": { "role": "user", "content": [{ "type": "text", "text": text }] } })
    )
}

fn assistant_line(text: &str) -> String {
    format!(
        "{}\n",
        json!({ "type": "message", "message": { "role": "assistant", "content": [{ "type": "text", "text": text }] } })
    )
}

fn exit_line(kind: &str) -> String {
    format!(
        "{}\n",
        json!({ "type": "custom", "customType": "session_exit", "data": { "kind": kind } })
    )
}

fn progress_step(call: &str, status: &str, progress: Value) -> Value {
    json!({ "update": { "sessionUpdate": "tool_call_update", "toolCallId": call, "status": status, "rawOutput": { "details": { "progress": progress } } } })
}

fn step(call: &str, id: &str) -> Step {
    Step {
        call: call.into(),
        id: id.into(),
    }
}

fn agent_states(general: &General, project: &str) -> Vec<(String, SubAgentState)> {
    general
        .sub_agents(project)
        .unwrap()
        .nodes
        .iter()
        .map(|node| (node.id.clone(), node.state))
        .collect()
}

#[tokio::test]
async fn the_sub_agents_of_the_chat_are_listed_read_while_shown_and_interrupted_when_omp_exits() {
    let f = fixture().await;
    let agents = agents_folder(&f.host);
    let gate = f.host.dirs.local_data_dir().join("gate");
    f.host.script(json!({
        "prompts": [[
            { "update": { "sessionUpdate": "tool_call", "toolCallId": "t1", "title": "Task", "status": "in_progress" } },
            progress_step("t1", "in_progress", json!([
                { "id": "0-A", "name": "A", "status": "running" },
                { "id": "1-B", "name": "B", "status": "running" },
                { "id": "__advisor.turn", "status": "running" },
            ])),
            { "write": { "path": agents.join("0-A.jsonl"), "text": user_line("Read the code.") + &assistant_line("Done reading.") + &exit_line("normal") } },
            { "write": { "path": agents.join("1-B.jsonl"), "text": user_line("Write the test.") } },
            // The call returns while they still work: they run in the background.
            progress_step("t1", "completed", json!([
                { "id": "0-A", "name": "A", "status": "pending" },
                { "id": "1-B", "name": "B", "status": "pending" },
            ])),
            { "wait_file": gate },
            { "stderr": "error: gone" },
            { "exit": 3 },
        ]],
    }));
    f.send("Start the work");
    until("the two sub-agents to be listed", LIMIT, || {
        agent_states(&f.general, f.id()).len() == 2
    })
    .await;
    assert!(
        f.general
            .sub_agent_conversation(f.id(), &[step("t1", "__advisor.turn")])
            .is_err(),
        "an advisor is never listed or shown"
    );

    // A's conversation is shown: its transcript is read, and its exit record (the fallback)
    // settles it while the task call says it still works.
    let path_a = vec![step("t1", "0-A")];
    f.general
        .sub_agents_view(f.id(), Some(path_a.clone()))
        .unwrap();
    until("A to be read and settled", LIMIT, || {
        agent_states(&f.general, f.id())
            .iter()
            .any(|(id, state)| id == "0-A" && *state == SubAgentState::Succeeded)
    })
    .await;
    let a = f.general.sub_agent_conversation(f.id(), &path_a).unwrap();
    assert_eq!(a.owner, Owner::General);
    assert_eq!(a.unavailable, None);
    assert_eq!(
        a.entries,
        [
            Entry::Assignment {
                from: "Chat".into(),
                text: "Read the code.".into()
            },
            Entry::Agent {
                text: "Done reading.".into(),
                complete: true
            },
        ]
    );
    assert!(f
        .host
        .lists
        .lock()
        .unwrap()
        .iter()
        .all(|list| list.owner == Owner::General && list.project_id == f.id()));
    assert_eq!(
        agent_states(&f.general, f.id())[1],
        ("1-B".to_owned(), SubAgentState::Working)
    );

    // omp exits with B still working: B is interrupted, in the list and under the task row.
    std::fs::write(&gate, "").unwrap();
    until("omp to stop", LIMIT, || {
        f.entries()
            .iter()
            .any(|e| matches!(e, Entry::OmpStopped { .. }))
    })
    .await;
    assert_eq!(
        agent_states(&f.general, f.id()),
        [
            ("0-A".to_owned(), SubAgentState::Succeeded),
            ("1-B".to_owned(), SubAgentState::Interrupted),
        ]
    );
    let row_states: Vec<SubAgentState> = f
        .entries()
        .iter()
        .filter_map(|entry| match entry {
            Entry::Tool(tool) if tool.id == "t1" => Some(tool.sub_agents.iter().map(|a| a.state)),
            _ => None,
        })
        .flatten()
        .collect();
    assert_eq!(
        row_states,
        [SubAgentState::Succeeded, SubAgentState::Interrupted]
    );
}

#[tokio::test]
async fn the_sub_agents_are_rebuilt_from_the_replayed_history_and_clear_forgets_them() {
    let f = fixture().await;
    f.host.script(json!({ "prompts": [[agent("Answer")]] }));
    f.send("Question");
    until("the turn", LIMIT, || {
        f.entries().contains(&Entry::Agent {
            text: "Answer".into(),
            complete: true,
        })
    })
    .await;
    f.general.window_closed(f.id()).await;
    f.record().unwrap();

    let agents = agents_folder(&f.host);
    std::fs::create_dir_all(&agents).unwrap();
    std::fs::write(
        agents.join("0-A.jsonl"),
        user_line("Read the code.") + &assistant_line("Read it.") + &exit_line("normal"),
    )
    .unwrap();

    // A new run of Weaver: the replay is what omp keeps; the sub-agents come from it.
    f.host.script(json!({
        "replay": [
            { "sessionUpdate": "user_message_chunk", "content": { "type": "text", "text": "Question" } },
            { "sessionUpdate": "tool_call", "toolCallId": "t1", "title": "Task", "status": "completed",
              "rawOutput": { "details": { "progress": [{ "id": "0-A", "name": "A", "status": "completed" }] } } },
        ],
    }));
    let general = General::new(f.host.clone(), Arc::new(Recorder::default()));
    general.open(f.id(), &f.project.root).await;
    until("the history to come back", LIMIT, || {
        let state = general.snapshot(f.id()).unwrap().state;
        !state.loading && !general.entries(f.id()).is_empty()
    })
    .await;
    assert_eq!(
        agent_states(&general, f.id()),
        [("0-A".to_owned(), SubAgentState::Succeeded)],
        "rebuilt from the history, not from the live updates"
    );

    // Its transcript is read from the start when it is shown.
    let path = vec![step("t1", "0-A")];
    general.sub_agents_view(f.id(), Some(path.clone())).unwrap();
    until("the transcript to be read", LIMIT, || {
        general
            .sub_agent_conversation(f.id(), &path)
            .is_ok_and(|c| c.entries.len() == 2)
    })
    .await;

    // Clear starts again empty: the sub-agents are gone with the conversation.
    general.clear(f.id()).unwrap();
    assert!(general.sub_agents(f.id()).unwrap().nodes.is_empty());
    assert!(general.sub_agent_conversation(f.id(), &path).is_err());
    general.window_closed(f.id()).await;
}

// -----------------------------------------------------------------------------------------------
// Ask more: conversations about one thing, not jobs (`requirements/tech#ask-conversation`)
// -----------------------------------------------------------------------------------------------

mod ask_more {
    use weaver_core::assets::tags;
    use weaver_core::jobs::ask;

    use super::*;
    use crate::general::service::{AskOpening, Rebuild};
    use crate::general::wire::{AskKind, AskStatus};

    const INSTRUCTION: &str = "Weaver: Ask more\n\nthe chosen thing\n";

    /// The instruction a lost session would be replaced with, changeable by the test.
    type Rebuilt = Arc<Mutex<Option<String>>>;

    fn rebuilt(text: Option<&str>) -> (Rebuild, Rebuilt) {
        let shared: Rebuilt = Arc::new(Mutex::new(text.map(str::to_owned)));
        let seen = shared.clone();
        (Arc::new(move || seen.lock().unwrap().clone()), shared)
    }

    fn open(f: &Fixture, rebuild: Rebuild) -> String {
        f.general.open_ask(AskOpening {
            project_id: f.id().to_owned(),
            root: f.project.root.clone(),
            name: "Ask T3".to_owned(),
            kind: AskKind::Task,
            round: None,
            card: None,
            instruction: INSTRUCTION.to_owned(),
            rebuild,
        })
    }

    fn prompts(f: &Fixture) -> Vec<String> {
        f.host
            .received()
            .iter()
            .filter(|m| m["method"] == "session/prompt")
            .map(|m| {
                m["params"]["prompt"][0]["text"]
                    .as_str()
                    .unwrap()
                    .to_owned()
            })
            .collect()
    }

    fn status(f: &Fixture, id: &str) -> AskStatus {
        f.general
            .asks(f.id())
            .into_iter()
            .find(|row| row.id == id)
            .expect("the conversation is listed")
            .status
    }

    async fn answered(f: &Fixture, id: &str, text: &str) {
        until("the turn to end", LIMIT, || {
            !f.general.working(id)
                && f.general.entries(id).contains(&Entry::Agent {
                    text: text.into(),
                    complete: true,
                })
        })
        .await;
    }

    #[tokio::test]
    async fn opening_sends_nothing_starts_no_omp_and_closing_before_the_first_message_starts_none()
    {
        let f = fixture().await;
        f.host.script(json!({
            "log": f.host.received_log(),
            "logEnv": ["WEAVER_ASK"],
            "prompts": [[agent("never")]],
        }));
        let (rebuild, _) = rebuilt(Some("again"));
        let id = open(&f, rebuild);

        let listed = f.general.asks(f.id());
        assert_eq!(listed.len(), 1);
        assert_eq!(
            (
                listed[0].id.as_str(),
                listed[0].name.as_str(),
                listed[0].status
            ),
            (id.as_str(), "Ask T3", AskStatus::Idle)
        );
        assert!(!listed[0].starting);
        assert!(f.general.ask_snapshot(&id).unwrap().entries.is_empty());
        assert_eq!(
            f.events
                .rows
                .lock()
                .unwrap()
                .last()
                .map(|row| row.id.clone()),
            Some(id.clone()),
            "the row is announced at opening"
        );
        // Not a job and not General: nothing of it is the General of the project.
        assert!(f.general.is_ask(&id));
        assert!(!f.general.is_ask(f.id()), "General's slot is not an Ask");
        assert!(f.general.identity(&id).is_none(), "no omp, no identity");

        let ended = f.general.close_ask(&id).unwrap();
        ended.await;
        assert!(f.general.asks(f.id()).is_empty());
        assert_eq!(
            f.events.closed.lock().unwrap().as_slice(),
            [AskClosed {
                project_id: f.id().to_owned(),
                id: id.clone()
            }]
        );
        assert!(f.general.ask_snapshot(&id).is_err());
        assert!(f.general.send_message(&id, "late").is_err());
        assert!(
            f.methods().is_empty(),
            "no omp ever started: {:?}",
            f.methods()
        );
        assert!(f.general.close_ask(&id).is_err(), "closed once");
    }

    #[tokio::test]
    async fn the_first_prompt_carries_the_instruction_and_the_message_the_next_ones_the_message_alone(
    ) {
        let f = fixture().await;
        f.host.script(json!({
            "log": f.host.received_log(),
            "logEnv": ["WEAVER_ASK", "WEAVER_GENERAL", "WEAVER_JOB", "WEAVER_ENDPOINT"],
            "prompts": [[agent("First")], [agent("Second")]],
        }));
        let (rebuild, _) = rebuilt(Some("not used"));
        let id = open(&f, rebuild);

        f.general.send_message(&id, "  What is this?  ").unwrap();
        assert_eq!(status(&f, &id), AskStatus::Running);
        assert!(f.general.asks(f.id())[0].starting, "omp is starting");
        answered(&f, &id, "First").await;
        assert_eq!(status(&f, &id), AskStatus::Idle);

        f.general.send_message(&id, "And then?").unwrap();
        answered(&f, &id, "Second").await;
        assert_eq!(
            prompts(&f),
            [
                ask::first_prompt(INSTRUCTION, "What is this?"),
                "And then?".to_owned()
            ]
        );

        // The conversation shows only the developer's messages, never the instruction.
        let entries = f.general.entries(&id);
        assert_eq!(
            developers(&entries),
            vec![
                ("What is this?", &Delivery::Delivered),
                ("And then?", &Delivery::Delivered)
            ]
        );
        assert!(!entries
            .iter()
            .any(|e| matches!(e, Entry::Instruction { .. })));

        // omp started once, on the analysis model with no instruction file, and knows the
        // conversation, never a job or General.
        let received = f.host.received();
        let start = &received[0];
        assert_eq!(start["env"]["WEAVER_ASK"], json!(id));
        assert_eq!(start["env"]["WEAVER_ENDPOINT"], "/tmp/weaver-test.sock");
        for name in ["WEAVER_GENERAL", "WEAVER_JOB"] {
            assert_eq!(start["env"][name], Value::Null, "{name}");
        }
        let args: Vec<&str> = start["args"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(Value::as_str)
            .collect();
        assert_eq!(args, ["--approval-mode", "yolo", "acp"]);
        assert!(
            !weaver_core::jobs::general::instruction_file(&f.host.dirs).exists(),
            "nothing of Ask more is written to disk"
        );
        assert_eq!(
            f.methods().iter().filter(|m| *m == "initialize").count(),
            1,
            "omp stays for the second message"
        );
        // Nothing of the conversation is kept: no record of it and no General record.
        f.general.flush();
        assert!(GeneralStore::new(&f.host.dirs)
            .read(f.id())
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn tagged_assets_follow_the_message_after_the_instruction_and_are_never_shown() {
        let f = fixture().await;
        *f.host.assets.lock().unwrap() = vec!["logo.png".to_owned(), "spec.pdf".to_owned()];
        f.host.script(json!({
            "log": f.host.received_log(),
            "prompts": [[agent("First")], [agent("Second")]],
        }));
        let (rebuild, _) = rebuilt(Some("not used"));
        let id = open(&f, rebuild);

        f.general
            .send_message(&id, "What is in @logo.png?")
            .unwrap();
        answered(&f, &id, "First").await;
        f.general
            .send_message(&id, "And @spec.pdf, not @other.pdf, and @logo.png again")
            .unwrap();
        answered(&f, &id, "Second").await;

        let block = |names: &[&str]| {
            tags::block(&names.iter().map(|n| (*n).to_owned()).collect::<Vec<_>>()).unwrap()
        };
        assert_eq!(
            prompts(&f),
            [
                ask::first_prompt(
                    INSTRUCTION,
                    &format!("What is in @logo.png?\n\n{}", block(&["logo.png"]))
                ),
                format!(
                    "And @spec.pdf, not @other.pdf, and @logo.png again\n\n{}",
                    block(&["spec.pdf", "logo.png"])
                ),
            ]
        );
        // The conversation shows what the developer wrote.
        assert_eq!(
            developers(&f.general.entries(&id)),
            vec![
                ("What is in @logo.png?", &Delivery::Delivered),
                (
                    "And @spec.pdf, not @other.pdf, and @logo.png again",
                    &Delivery::Delivered
                )
            ]
        );
    }

    #[tokio::test]
    async fn after_omp_stopped_the_next_message_reloads_the_session_and_a_failed_reload_sends_the_instruction_again(
    ) {
        let f = fixture().await;
        let (rebuild, rebuilt_now) = rebuilt(Some("REBUILT"));
        let id = open(&f, rebuild);
        let log = f.host.received_log();
        let stopped = |count: usize| {
            let (f, id) = (&f, &id);
            move || {
                f.general
                    .entries(id)
                    .iter()
                    .filter(|e| matches!(e, Entry::OmpStopped { .. }))
                    .count()
                    == count
            }
        };

        // 1. The first turn ends with omp exiting.
        f.host
            .script(json!({ "log": log, "logEnv": ["WEAVER_ASK"], "prompts": [[agent("one"), { "exit": 3 }]] }));
        f.general.send_message(&id, "first").unwrap();
        until("omp to stop", LIMIT, stopped(1)).await;
        assert_eq!(status(&f, &id), AskStatus::Idle);

        // 2. The next message starts omp and loads the session: it goes alone.
        f.host
            .script(json!({ "log": log, "logEnv": ["WEAVER_ASK"], "prompts": [[agent("two"), { "exit": 3 }]] }));
        f.general.send_message(&id, "second").unwrap();
        until("omp to stop again", LIMIT, stopped(2)).await;
        assert!(f.methods().contains(&"session/load".to_owned()));

        // 3. The session cannot be loaded: an empty one starts with the notice, and the message
        // carries the instruction read again from the files at that moment.
        f.host.script(json!({
            "log": log,
            "logEnv": ["WEAVER_ASK"],
            "loadError": "session not found",
            "sessionId": "fresh",
            "prompts": [[agent("three"), { "exit": 3 }]],
        }));
        *rebuilt_now.lock().unwrap() = Some("REBUILT AGAIN".to_owned());
        f.general.send_message(&id, "third").unwrap();
        until("omp to stop a third time", LIMIT, stopped(3)).await;
        assert!(f.general.entries(&id).iter().any(|e| matches!(e, Entry::Notice { text } if text == "The conversation could not be reloaded (session not found). A new one started.")));

        // 4. The chosen thing no longer exists: the message goes alone.
        f.host.script(json!({
            "log": log,
            "logEnv": ["WEAVER_ASK"],
            "loadError": "session not found",
            "sessionId": "fresher",
            "prompts": [[agent("four")]],
        }));
        *rebuilt_now.lock().unwrap() = None;
        f.general.send_message(&id, "fourth").unwrap();
        answered(&f, &id, "four").await;

        assert_eq!(
            prompts(&f),
            [
                ask::first_prompt(INSTRUCTION, "first"),
                "second".to_owned(),
                ask::first_prompt("REBUILT AGAIN", "third"),
                "fourth".to_owned(),
            ]
        );
        // Every start of omp is the same conversation, with the same identity: omp starts once
        // for each of the four messages, because the first three turns end with omp exiting and
        // the fourth is the message sent after the third exit.
        let starts: Vec<Value> = f
            .host
            .received()
            .into_iter()
            .filter(|m| !m["env"].is_null())
            .collect();
        assert_eq!(starts.len(), 4, "one start for each of the four messages");
        for start in &starts {
            assert_eq!(start["env"]["WEAVER_ASK"], json!(id));
        }
        f.general.close_ask(&id).unwrap().await;
    }

    #[tokio::test]
    async fn when_no_session_was_ever_opened_the_next_message_carries_the_instruction_prepared_at_opening(
    ) {
        let f = fixture().await;
        let (rebuild, _) = rebuilt(Some("not this one"));
        let id = open(&f, rebuild);
        f.host
            .script(json!({ "newError": "no provider is logged in" }));
        f.general.send_message(&id, "first").unwrap();
        until("omp to stop", LIMIT, || {
            f.general
                .entries(&id)
                .iter()
                .any(|e| matches!(e, Entry::OmpStopped { .. }))
        })
        .await;
        assert!(matches!(
            &f.general.entries(&id)[0],
            Entry::Developer {
                delivery: Delivery::NotDelivered { .. },
                ..
            }
        ));

        f.host
            .script(json!({ "log": f.host.received_log(), "prompts": [[agent("Fine")]] }));
        f.general.send_message(&id, "second").unwrap();
        answered(&f, &id, "Fine").await;
        assert_eq!(prompts(&f), [ask::first_prompt(INSTRUCTION, "second")]);
        assert!(
            !f.methods().contains(&"session/load".to_owned()),
            "there was no session to load"
        );
        f.general.close_ask(&id).unwrap().await;
    }

    #[tokio::test]
    async fn a_permission_waits_in_the_conversation_and_a_form_is_declined_naming_ask_more() {
        let f = fixture().await;
        f.host.permissions(Permissions::Ask);
        f.host.script(json!({
            "log": f.host.received_log(),
            "prompts": [[
                { "elicit": true },
                { "permission": { "toolCall": { "toolCallId": "t1", "title": "$ ls", "kind": "execute", "rawInput": { "command": "ls" } }, "options": [
                    { "optionId": "allow", "name": "Allow once", "kind": "allow_once" },
                ] } },
                agent("Listed"),
            ]],
        }));
        let (rebuild, _) = rebuilt(None);
        let id = open(&f, rebuild);
        f.general.send_message(&id, "List the files").unwrap();
        until("the permission card", LIMIT, || {
            f.general.entries(&id).iter().any(|e| {
                matches!(
                    e,
                    Entry::Permission {
                        state: PermissionState::Asking,
                        ..
                    }
                )
            })
        })
        .await;
        assert_eq!(status(&f, &id), AskStatus::Waiting);
        let declined = f
            .host
            .received()
            .into_iter()
            .find(|m| m["answerTo"] == "elicitation/create")
            .unwrap();
        let message = declined["result"]["_meta"]["message"].as_str().unwrap();
        assert!(
            message.contains("In Ask more") && !message.contains("General"),
            "{message}"
        );
        // A message sent meanwhile waits behind the permission.
        f.general.send_message(&id, "Meanwhile").unwrap();
        assert!(matches!(
            f.general.entries(&id).last(),
            Some(Entry::Developer {
                delivery: Delivery::Queued,
                ..
            })
        ));

        let request = f.general.entries(&id).iter().find_map(|e| match e {
            Entry::Permission { request, .. } => Some(*request),
            _ => None,
        });
        f.general
            .answer_permission(&id, request.unwrap(), "allow")
            .unwrap();
        answered(&f, &id, "Listed").await;
        assert_eq!(status(&f, &id), AskStatus::Idle);
        f.general.close_ask(&id).unwrap().await;
    }

    #[tokio::test]
    async fn stop_cancels_only_the_turn_and_the_conversation_stays() {
        let f = fixture().await;
        f.host.script(json!({
            "log": f.host.received_log(),
            "prompts": [[agent("Working"), { "wait_cancel": true }], [agent("Next")]],
        }));
        let (rebuild, _) = rebuilt(None);
        let id = open(&f, rebuild);
        f.general.send_message(&id, "Long thing").unwrap();
        until("the agent to answer", LIMIT, || {
            f.general.entries(&id).contains(&Entry::Agent {
                text: "Working".into(),
                complete: false,
            })
        })
        .await;
        f.general.stop(&id).unwrap();
        assert_eq!(status(&f, &id), AskStatus::Idle);
        f.received("session/cancel", |m| m["method"] == "session/cancel")
            .await;
        assert_eq!(f.general.asks(f.id()).len(), 1, "the conversation stays");
        f.general.send_message(&id, "Again").unwrap();
        answered(&f, &id, "Next").await;
        assert_eq!(
            f.methods().iter().filter(|m| *m == "initialize").count(),
            1,
            "omp stayed"
        );
        f.general.close_ask(&id).unwrap().await;
    }

    #[tokio::test]
    async fn closing_a_working_conversation_cancels_the_turn_and_ends_omp() {
        let f = fixture().await;
        f.host.script(json!({
            "log": f.host.received_log(),
            "prompts": [[agent("Working"), { "hold": true }]],
        }));
        let (rebuild, _) = rebuilt(None);
        let id = open(&f, rebuild);
        f.general.send_message(&id, "Long thing").unwrap();
        until("the turn", LIMIT, || f.general.working(&id)).await;
        until("the agent to answer", LIMIT, || {
            f.general
                .entries(&id)
                .iter()
                .any(|e| matches!(e, Entry::Agent { .. }))
        })
        .await;
        f.general.close_ask(&id).unwrap().await;
        f.received("session/close", |m| m["method"] == "session/close")
            .await;
        assert!(f.general.asks(f.id()).is_empty());
    }

    #[tokio::test]
    async fn the_window_closing_ends_every_conversation_of_its_project_and_keeps_general() {
        let f = fixture().await;
        f.host.script(json!({
            "log": f.host.received_log(),
            "prompts": [[agent("Hi")], [agent("General")]],
        }));
        let (rebuild, _) = rebuilt(None);
        let first = open(&f, rebuild.clone());
        let second = open(&f, rebuild);
        assert_ne!(first, second, "every request opens a new conversation");
        let names: Vec<String> = f.general.asks(f.id()).into_iter().map(|r| r.id).collect();
        assert_eq!(
            names,
            [second.clone(), first.clone()],
            "the most recent first"
        );

        f.general.send_message(&first, "Hello").unwrap();
        answered(&f, &first, "Hi").await;
        f.send("In General");
        f.general.window_closed(f.id()).await;

        assert!(f.general.asks(f.id()).is_empty());
        assert_eq!(f.events.closed.lock().unwrap().len(), 2);
        assert!(f.general.ask_snapshot(&first).is_err());
        assert!(f.general.ask_snapshot(&second).is_err());
        assert!(
            f.general.snapshot(f.id()).is_ok(),
            "General outlives its window"
        );
    }

    #[tokio::test]
    async fn the_sub_agents_of_a_conversation_belong_to_it() {
        let f = fixture().await;
        f.host.script(json!({
            "log": f.host.received_log(),
            "prompts": [[agent("Hi")]],
        }));
        let (rebuild, _) = rebuilt(None);
        let id = open(&f, rebuild);
        let list = f.general.sub_agents(&id).unwrap();
        assert_eq!(list.owner, Owner::Ask { id: id.clone() });
        assert_eq!(list.project_id, f.id());
        assert!(list.nodes.is_empty());
        assert!(f.general.sub_agents_interest(&id, true, None).is_ok());
        f.general.close_ask(&id).unwrap().await;
        assert!(f.general.sub_agents(&id).is_err());
    }
}

mod ask_job_new {
    use weaver_cli::exec::execute;
    use weaver_cli::spec::Caller;
    use weaver_core::command::{code, CommandRequest};

    use super::*;
    use crate::general::service::{AskOpening, Rebuild};
    use crate::general::wire::AskKind;
    use crate::jobs::authority::AppAuthority;

    fn run(
        f: &Fixture,
        authority: &AppAuthority,
        input: Value,
        token: Option<&str>,
        as_general: bool,
    ) -> Result<weaver_core::command::CommandReply, weaver_cli::spec::Failure> {
        let request = CommandRequest {
            object: "job".into(),
            verb: "new".into(),
            card: None,
            state: None,
            root: f.project.root.to_string_lossy().into_owned(),
            input: input.as_object().unwrap().clone(),
            general: token.filter(|_| as_general).map(str::to_owned),
            ask: token.filter(|_| !as_general).map(str::to_owned),
        };
        let caller = Caller {
            general: request.general.clone(),
            ask: request.ask.clone(),
            ..Caller::default()
        };
        execute(&request, &caller, authority)
    }

    #[tokio::test]
    async fn job_new_is_accepted_from_a_working_conversation_and_refused_from_an_idle_or_closed_one(
    ) {
        let host = TestHost::new();
        let events = Arc::new(Recorder::default());
        let general = General::new(host.clone(), events.clone());
        let jobs = Jobs::new(host.clone());
        jobs.set_general(general.clone());
        let (dir, project) = host
            .project(&[("tasks/aaaaaaaa-first-task.md", TASK)])
            .await;
        let f = Fixture {
            host,
            general,
            events,
            _dir: dir,
            project,
        };
        let authority = AppAuthority { jobs: jobs.clone() };
        f.host
            .script(json!({ "log": f.host.received_log(), "prompts": [[agent("Starting"), { "hold": true }]] }));
        let rebuild: Rebuild = Arc::new(|| None);
        let id = f.general.open_ask(AskOpening {
            project_id: f.id().to_owned(),
            root: f.project.root.clone(),
            name: "Ask T3".into(),
            kind: AskKind::Task,
            round: None,
            card: None,
            instruction: "Weaver: Ask more\n".into(),
            rebuild,
        });

        // Empty: no omp, nothing is accepted.
        let refused = run(&f, &authority, json!({ "kind": "scan" }), Some(&id), false);
        assert_eq!(refused.unwrap_err().code, code::FORBIDDEN, "idle");

        f.general.send_message(&id, "Please scan").unwrap();
        until("the agent to answer", LIMIT, || {
            f.general.working(&id)
                && f.general
                    .entries(&id)
                    .iter()
                    .any(|e| matches!(e, Entry::Agent { .. }))
        })
        .await;

        // The identity of another caller, or of the conversation given as General's, is refused.
        let wrong = run(
            &f,
            &authority,
            json!({ "kind": "scan" }),
            Some("another1"),
            false,
        );
        assert_eq!(wrong.unwrap_err().code, code::FORBIDDEN);
        let as_general = run(&f, &authority, json!({ "kind": "scan" }), Some(&id), true);
        assert_eq!(as_general.unwrap_err().code, code::FORBIDDEN);
        assert!(jobs.list_for(&f.project.root, true).is_empty());

        // Working: accepted, the job is shown in that conversation, and the developer's refusals
        // are shown too.
        let started = run(
            &f,
            &authority,
            json!({ "kind": "run", "task": "t1" }),
            Some(&id),
            false,
        )
        .unwrap();
        let job = started.json["id"].as_str().unwrap().to_owned();
        assert_eq!(jobs.record(&job).unwrap().name, "Run T1");
        assert!(f.general.entries(&id).contains(&Entry::JobStarted {
            job: job.clone(),
            name: "Run T1".into()
        }));
        let refused = run(&f, &authority, json!({ "kind": "run" }), Some(&id), false).unwrap_err();
        assert_eq!(refused.code, code::REFUSED);
        assert!(f
            .general
            .entries(&id)
            .iter()
            .any(|e| matches!(e, Entry::JobRefused { job_kind, .. } if job_kind == "run")));

        // Closed: refused.
        f.general.close_ask(&id).unwrap().await;
        let closed = run(&f, &authority, json!({ "kind": "scan" }), Some(&id), false);
        assert_eq!(closed.unwrap_err().code, code::FORBIDDEN, "closed");
    }
}

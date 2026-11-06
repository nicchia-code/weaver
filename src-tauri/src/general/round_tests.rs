//! The Chat in a turn that asks a round (`general-chat/Waiting for answers`), the Events it tells
//! and what Stop, Clear and the window do to it; the instruction written at each start. Against
//! the scripted fake ACP agent: no omp, no network.

use std::sync::{Arc, Mutex};

use serde_json::{json, Value};
use weaver_core::jobs::general::instruction_file;

use super::service::{ChatObserver, General};
use super::wire::{AskClosed, AskEntries, AskRow, GeneralEntries, GeneralEvents, GeneralState};
use crate::jobs::conversation::{Delivery, Entry};
use crate::jobs::testing::{text_update, until, Project, TestHost, LIMIT};

/// Drops what the UI would receive: these tests read the service.
struct Silent;

impl GeneralEvents for Silent {
    fn state(&self, _: &GeneralState) {}
    fn entries(&self, _: &GeneralEntries) {}
    fn ask_changed(&self, _: &AskRow) {}
    fn ask_entries(&self, _: &AskEntries) {}
    fn ask_closed(&self, _: &AskClosed) {}
}

/// What the Chat told, in order, as `<event> <project>`.
#[derive(Default)]
struct Told(Mutex<Vec<String>>);

impl Told {
    fn events(&self) -> Vec<String> {
        self.0.lock().unwrap().clone()
    }
}

impl ChatObserver for Told {
    fn chat_waiting(&self, project_id: &str) {
        self.0.lock().unwrap().push(format!("waiting {project_id}"));
    }

    fn chat_round_ended(&self, project_id: &str) {
        self.0.lock().unwrap().push(format!("ended {project_id}"));
    }

    fn chat_cleared(&self, project_id: &str) {
        self.0.lock().unwrap().push(format!("cleared {project_id}"));
    }
}

struct Fixture {
    host: Arc<TestHost>,
    general: Arc<General>,
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

    fn methods(&self) -> Vec<String> {
        self.host
            .received()
            .iter()
            .filter_map(|m| m["method"].as_str().map(str::to_owned))
            .collect()
    }

    /// The script, with the log of what omp receives.
    fn script(&self, mut script: Value) {
        script["log"] = json!(self.host.received_log());
        self.host.script(script);
    }

    async fn received(&self, what: &str, wanted: impl Fn(&Value) -> bool) {
        until(what, LIMIT, || self.host.received().iter().any(&wanted)).await;
    }
}

async fn fixture() -> Fixture {
    let host = TestHost::new();
    let general = General::new(host.clone(), Arc::new(Silent));
    let (dir, project) = host.project(&[]).await;
    general.open(&project.id, &project.root).await;
    Fixture {
        host,
        general,
        _dir: dir,
        project,
    }
}

fn agent(text: &str) -> Value {
    json!({ "update": text_update("agent_message_chunk", text) })
}

/// A Chat whose agent is in the middle of a turn that says `Asking`, with its identity known.
async fn asking(script: Value) -> (Fixture, Arc<Told>, String) {
    let f = fixture().await;
    let told = Arc::new(Told::default());
    f.general.observe(told.clone());
    f.script(script);
    f.send("Interview me");
    until("the agent to start its turn", LIMIT, || {
        f.entries().contains(&Entry::Agent {
            text: "Asking".into(),
            complete: false,
        })
    })
    .await;
    let token = f.general.identity(f.id()).expect("the live identity");
    (f, told, token)
}

#[tokio::test]
async fn the_instruction_of_each_start_carries_the_limit_and_the_language_of_the_settings_of_that_moment(
) {
    let f = fixture().await;
    {
        let mut settings = f.host.settings.lock().unwrap();
        settings.max_parallel_agents = 3;
        settings.language = "Italian".into();
    }
    f.script(json!({ "prompts": [[agent("One")]] }));
    let file = instruction_file(&f.host.dirs);
    f.send("Hi");
    until("the first answer", LIMIT, || {
        f.entries().contains(&Entry::Agent {
            text: "One".into(),
            complete: true,
        })
    })
    .await;
    let first = std::fs::read_to_string(&file).unwrap();
    assert!(first.starts_with("# Weaver Chat\n"), "{first}");
    assert!(
        first.contains("\nRun at most 3 sub-agents at once.\nPass each sub-agent what you have already read")
            && first.ends_with("Write the texts for the developer in Italian.\n"),
        "{first}"
    );

    // The settings changed: the next start of omp writes the instruction again with them.
    {
        let mut settings = f.host.settings.lock().unwrap();
        settings.max_parallel_agents = 5;
        settings.language = String::new();
    }
    f.general.clear(f.id()).unwrap();
    f.received("session/close", |m| m["method"] == "session/close")
        .await;
    f.send("Again");
    until("the second session", LIMIT, || {
        f.methods().iter().filter(|m| *m == "session/new").count() == 2
    })
    .await;
    let second = std::fs::read_to_string(&file).unwrap();
    assert!(
        second.contains("\nRun at most 5 sub-agents at once.\n"),
        "{second}"
    );
    assert!(!second.contains("Write the texts"), "{second}");
}

#[tokio::test]
async fn a_round_waits_for_the_answers_in_the_same_turn_and_the_message_sent_meanwhile_follows_its_close(
) {
    let (f, told, token) = asking(json!({
        "steering": ["injected"],
        "prompts": [[agent("Asking"), { "wait_steer": 1 }, agent("Got your answer")]],
    }))
    .await;
    assert_eq!(f.general.chat_of(&token).as_deref(), Some(f.id()));
    assert_eq!(f.general.chat_of("someone-else"), None);

    f.general.round_opened(f.id());
    f.general.flush();
    assert_eq!(told.events(), [format!("waiting {}", f.id())]);
    // The turn goes on: the Chat still counts as working, and the agent may still ask.
    assert!(f.general.working(f.id()));
    assert_eq!(f.general.chat_of(&token).as_deref(), Some(f.id()));
    // A second opening tells nothing more.
    f.general.round_opened(f.id());
    f.general.flush();
    assert_eq!(told.events().len(), 1);

    // A message sent meanwhile never answers the round: it waits, queued.
    f.send("One more thing");
    assert!(matches!(
        f.entries().last(),
        Some(Entry::Developer {
            delivery: Delivery::Queued,
            ..
        })
    ));
    assert!(!f.methods().contains(&"_session/steering".to_owned()));

    f.general.round_closed(f.id());
    f.received("the follow-up", |m| {
        m["method"] == "_session/steering"
            && m["params"]["_meta"]["steering"]["queue"] == "followUp"
    })
    .await;
    let steer = f
        .host
        .received()
        .into_iter()
        .find(|m| m["method"] == "_session/steering")
        .unwrap();
    assert_eq!(steer["params"]["prompt"][0]["text"], "One more thing");
    until("the agent to go on", LIMIT, || {
        f.entries().contains(&Entry::Agent {
            text: "Got your answer".into(),
            complete: true,
        }) && !f.general.working(f.id())
    })
    .await;
    assert!(f.entries().iter().any(|e| matches!(
        e,
        Entry::Developer { text, delivery: Delivery::Delivered, .. } if text == "One more thing"
    )));
    // A round that closes in the normal way ends nothing without answers.
    f.general.flush();
    assert_eq!(told.events(), [format!("waiting {}", f.id())]);
}

#[tokio::test]
async fn only_a_working_agent_opens_a_round_and_a_chat_with_no_turn_is_the_chat_of_no_token() {
    let f = fixture().await;
    let told = Arc::new(Told::default());
    f.general.observe(told.clone());
    // No omp yet: nothing to open a round.
    f.general.round_opened(f.id());
    f.general.round_closed(f.id());
    f.general.round_opened("0000000000000000");
    f.general.flush();
    assert!(told.events().is_empty());
    assert_eq!(f.general.identity(f.id()), None);

    f.script(json!({ "prompts": [[agent("Done")]] }));
    f.send("Hi");
    until("the turn to end", LIMIT, || {
        f.entries().contains(&Entry::Agent {
            text: "Done".into(),
            complete: true,
        }) && !f.general.working(f.id())
    })
    .await;
    let token = f
        .general
        .identity(f.id())
        .expect("omp stays for the next message");
    assert_eq!(
        f.general.chat_of(&token),
        None,
        "no turn is in progress any more"
    );
    f.general.round_opened(f.id());
    f.general.flush();
    assert!(told.events().is_empty());
}

#[tokio::test]
async fn stop_ends_the_round_without_answers_and_the_chat_goes_idle() {
    let (f, told, token) = asking(json!({
        "prompts": [[agent("Asking"), { "hold": true }]],
    }))
    .await;
    f.general.round_opened(f.id());
    f.send("Never mind");
    f.general.stop(f.id()).unwrap();
    f.general.flush();
    assert_eq!(
        told.events(),
        [format!("waiting {}", f.id()), format!("ended {}", f.id())]
    );
    assert!(!f.general.working(f.id()));
    assert_eq!(f.general.chat_of(&token), None);
    assert!(f.entries().iter().any(|e| matches!(
        e,
        Entry::Developer {
            delivery: Delivery::NotDelivered { .. },
            ..
        }
    )));
    f.received("session/cancel", |m| m["method"] == "session/cancel")
        .await;
}

#[tokio::test]
async fn clear_ends_the_round_and_tells_cleared_after_the_turn_stopped() {
    let (f, told, _token) = asking(json!({
        "prompts": [[agent("Asking"), { "hold": true }]],
    }))
    .await;
    f.general.round_opened(f.id());
    f.general.clear(f.id()).unwrap();
    until("the Chat to tell it was cleared", LIMIT, || {
        told.events().len() == 3
    })
    .await;
    assert_eq!(
        told.events(),
        [
            format!("waiting {}", f.id()),
            format!("ended {}", f.id()),
            format!("cleared {}", f.id())
        ]
    );
    // The turn had stopped when it was told: omp was cancelled and closed.
    let methods = f.methods();
    assert!(
        methods.contains(&"session/cancel".to_owned())
            && methods.contains(&"session/close".to_owned()),
        "{methods:?}"
    );
    assert!(f.entries().is_empty());
}

#[tokio::test]
async fn closing_the_window_ends_the_round_without_answers_and_clears_nothing() {
    let (f, told, _token) = asking(json!({
        "prompts": [[agent("Asking"), { "hold": true }]],
    }))
    .await;
    f.general.round_opened(f.id());
    f.general.window_closed(f.id()).await;
    f.general.flush();
    assert_eq!(
        told.events(),
        [format!("waiting {}", f.id()), format!("ended {}", f.id())]
    );
    assert!(!f.general.working(f.id()));
}

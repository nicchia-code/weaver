//! The instruction written at each start of the Chat, and which token is the Chat of a turn in
//! progress. Against the scripted fake ACP agent: no omp, no network.

use std::sync::Arc;

use serde_json::{json, Value};
use weaver_core::jobs::general::instruction_file;

use super::service::General;
use super::wire::{AskClosed, AskEntries, AskRow, GeneralEntries, GeneralEvents, GeneralState};
use crate::jobs::conversation::Entry;
use crate::jobs::testing::{text_update, until, Project, TestHost, LIMIT};
use crate::rounds::service::CardPublication;

/// Drops what the UI would receive: these tests read the service.
struct Silent;

impl GeneralEvents for Silent {
    fn cards(&self, _: &super::cards::ChatCards) {}
    fn state(&self, _: &GeneralState) {}
    fn entries(&self, _: &GeneralEntries) {}
    fn ask_changed(&self, _: &AskRow) {}
    fn ask_entries(&self, _: &AskEntries) {}
    fn ask_closed(&self, _: &AskClosed) {}
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
        first.contains(
            "\nRun at most 3 sub-agents at once.\nPass each sub-agent what you have already read"
        ) && first.ends_with("Write the texts for the developer in Italian.\n"),
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
async fn a_token_is_the_chat_of_a_project_only_while_its_agent_has_a_turn() {
    let f = fixture().await;
    assert_eq!(f.general.identity(f.id()), None);
    f.script(json!({ "prompts": [[agent("Asking"), { "hold": true }]] }));
    f.send("Hi");
    until("the agent to start its turn", LIMIT, || {
        f.entries().contains(&Entry::Agent {
            text: "Asking".into(),
            complete: false,
        })
    })
    .await;
    let token = f.general.identity(f.id()).expect("the live identity");
    let one_question = || {
        serde_json::from_value(json!({ "questions": [{
            "id": "a", "question": "Q",
            "options": [{"label": "x", "description": "d"}, {"label": "y", "description": "d"}],
            "recommended": 0, "multi": false
        }]}))
        .unwrap()
    };
    assert_eq!(
        f.general.publish_card("someone-else", one_question()),
        CardPublication::Unavailable
    );
    assert!(matches!(
        f.general.publish_card(&token, one_question()),
        CardPublication::Published(_)
    ));

    f.general.stop(f.id()).unwrap();
    assert!(!f.general.working(f.id()));
    assert_eq!(
        f.general.publish_card(&token, one_question()),
        CardPublication::Unavailable,
        "no turn is in progress any more"
    );
    f.received("session/cancel", |m| m["method"] == "session/cancel")
        .await;
}

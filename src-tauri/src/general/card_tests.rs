//! The cards of questions of the Chat in the service: publication during a turn (one per turn),
//! what a message or `Send to agent` does to the active card and what goes to omp, the record, and
//! `New chat`. Against the scripted fake ACP agent: no omp, no network.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use serde_json::{json, Value};
use weaver_core::jobs::general::{GeneralRecord, GeneralStore};
use weaver_core::question::{Action, CardRecord, CardState, Position, RecordedAnswer, RoundRequest};

use super::cards::{CardActionInput, ChatCardState, ChatCards};
use super::service::General;
use super::wire::{AskClosed, AskEntries, AskRow, GeneralEntries, GeneralEvents, GeneralState};
use crate::jobs::conversation::{Delivery, Entry};
use crate::jobs::testing::{text_update, until, Project, TestHost, LIMIT};
use crate::rounds::service::CardPublication;
use crate::rounds::view::PositionView;

#[derive(Default)]
struct Cards {
    told: Mutex<Vec<ChatCards>>,
}

impl GeneralEvents for Cards {
    fn state(&self, _: &GeneralState) {}
    fn entries(&self, _: &GeneralEntries) {}
    fn cards(&self, cards: &ChatCards) {
        self.told.lock().unwrap().push(cards.clone());
    }
    fn ask_changed(&self, _: &AskRow) {}
    fn ask_entries(&self, _: &AskEntries) {}
    fn ask_closed(&self, _: &AskClosed) {}
}

struct Fixture {
    host: Arc<TestHost>,
    general: Arc<General>,
    events: Arc<Cards>,
    _dir: tempfile::TempDir,
    project: Project,
}

impl Fixture {
    fn id(&self) -> &str {
        &self.project.id
    }

    fn entries(&self) -> Vec<Entry> {
        self.general.entries(self.id())
    }

    fn gate(&self) -> PathBuf {
        self.host.received_log().with_file_name("gate")
    }

    fn store(&self) -> GeneralStore {
        GeneralStore::new(&self.host.dirs)
    }

    fn record(&self) -> Option<GeneralRecord> {
        self.general.flush();
        self.store().read(self.id()).unwrap()
    }

    /// The texts of the prompts omp received, in order.
    fn prompts(&self) -> Vec<String> {
        self.host
            .received()
            .iter()
            .filter(|m| m["method"] == "session/prompt")
            .map(|m| m["params"]["prompt"][0]["text"].as_str().unwrap().to_owned())
            .collect()
    }

    /// The turn of the first prompt: the agent says something and waits for the gate.
    async fn start_turn(&self) -> String {
        self.host.script(json!({
            "log": self.host.received_log(),
            "prompts": [
                [agent("Asking"), { "wait_file": self.gate() }],
                [agent("Thanks"), { "wait_file": self.gate().with_file_name("gate2") }],
                [agent("And more")],
            ],
        }));
        self.general.send_message(self.id(), "Hi").unwrap();
        until("the agent to start its turn", LIMIT, || {
            self.entries().contains(&Entry::Agent {
                text: "Asking".into(),
                complete: false,
            })
        })
        .await;
        self.general.identity(self.id()).expect("the live identity")
    }

    async fn end_turn(&self) {
        std::fs::write(self.gate(), "open").unwrap();
        until("the turn to end", LIMIT, || !self.general.working(self.id())).await;
    }

    fn cards(&self) -> ChatCards {
        self.general.cards(self.id()).unwrap()
    }

    fn act(&self, card: &str, action: CardActionInput) -> Result<ChatCards, String> {
        self.general.card_act(self.id(), card, action.into())
    }
}

async fn fixture() -> Fixture {
    fixture_with(|_, _| {}).await
}

/// A project whose General finds what `before` put in the record when it opens.
async fn fixture_with(before: impl FnOnce(&GeneralStore, &Project)) -> Fixture {
    let host = TestHost::new();
    let events = Arc::new(Cards::default());
    let general = General::new(host.clone(), events.clone());
    let (dir, project) = host.project(&[]).await;
    before(&GeneralStore::new(&host.dirs), &project);
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

fn request() -> RoundRequest {
    serde_json::from_value(json!({ "questions": [
        { "id": "a", "question": "Which one?",
          "options": [{"label": "x", "description": "d"}, {"label": "y", "description": "d"}],
          "recommended": 1, "multi": false },
        { "id": "b", "question": "Which ones?",
          "options": [{"label": "p", "description": "d"}, {"label": "q", "description": "d"}],
          "recommended": 0, "multi": true },
    ]}))
    .unwrap()
}

fn published(publication: CardPublication) -> String {
    match publication {
        CardPublication::Published(id) => id,
        other => panic!("not published: {other:?}"),
    }
}

fn confirm(index: usize) -> CardActionInput {
    CardActionInput::Confirm { index }
}

fn skip(index: usize) -> CardActionInput {
    CardActionInput::Skip { index }
}

fn go(index: usize) -> CardActionInput {
    CardActionInput::Go {
        position: PositionView::Question { index },
    }
}

const ANSWERS: &str = "Answers to the card of questions";

#[tokio::test]
async fn the_agent_publishes_a_card_during_a_turn_and_only_one_per_turn() {
    let f = fixture().await;
    assert!(f.cards().cards.is_empty());
    let token = f.start_turn().await;

    let id = published(f.general.publish_card(&token, request()));
    assert_eq!(id.len(), 8);
    assert!(id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()));
    let cards = f.cards();
    assert_eq!(cards.cards.len(), 1);
    assert_eq!((cards.cards[0].id.as_str(), cards.cards[0].state), (id.as_str(), ChatCardState::Active));
    assert_eq!(cards.cards[0].position, PositionView::Question { index: 0 });
    assert_eq!(cards.cards[0].questions[0].draft.selected, [1], "the recommended option, preselected");
    // The UI was told, with the card in it.
    let told = f.events.told.lock().unwrap().clone();
    assert_eq!(told.last().unwrap().cards.len(), 1);

    // The card holds nothing: the turn goes on, the Chat is still working.
    assert!(f.general.working(f.id()));
    // A second card in the same turn is refused whole.
    assert_eq!(
        f.general.publish_card(&token, request()),
        CardPublication::AlreadyPublished
    );
    assert_eq!(f.cards().cards.len(), 1);

    // The turn ends: the card stays active.
    f.end_turn().await;
    assert_eq!(f.cards().cards[0].state, ChatCardState::Active);
    // No turn: nothing can be published.
    assert_eq!(
        f.general.publish_card(&token, request()),
        CardPublication::Unavailable
    );
}

#[tokio::test]
async fn the_turn_after_a_card_can_publish_another_and_only_the_latest_is_active() {
    let f = fixture().await;
    let token = f.start_turn().await;
    let first = published(f.general.publish_card(&token, request()));
    f.end_turn().await;

    // The developer answers the card; the agent's next turn publishes a new one.
    f.act(&first, confirm(0)).unwrap();
    f.act(&first, skip(1)).unwrap();
    f.general.send_answers(f.id(), &first).unwrap();
    until("the second prompt to reach omp", LIMIT, || f.prompts().len() == 2).await;
    let token = f.general.identity(f.id()).expect("the live identity");
    let second = published(f.general.publish_card(&token, request()));
    assert_ne!(first, second);
    let cards = f.cards().cards;
    assert_eq!(
        cards.iter().map(|c| (c.id.as_str(), c.state)).collect::<Vec<_>>(),
        [(first.as_str(), ChatCardState::Answered), (second.as_str(), ChatCardState::Active)]
    );
}

#[tokio::test]
async fn the_questions_of_the_card_are_read_for_ask_more_only_while_it_is_the_active_one() {
    let f = fixture().await;
    let token = f.start_turn().await;
    let first = published(f.general.publish_card(&token, request()));
    f.end_turn().await;
    let questions = |card: &str, index: usize| f.general.active_card_request(f.id(), card, index);

    // The questions only, whatever the developer did in the card.
    f.act(&first, confirm(0)).unwrap();
    assert_eq!(questions(&first, 1), Some(request()));
    assert_eq!(questions(&first, 2), None, "no such question");
    assert_eq!(questions("nothingx", 0), None, "no such card");
    assert_eq!(questions(&first, 0).map(|r| r.questions.len()), Some(2));
    assert_eq!(f.general.active_card_request("noproject", &first, 0), None);

    // Closed by a message of the developer: no longer active.
    f.general.send_message(f.id(), "Go on").unwrap();
    until("the second prompt to reach omp", LIMIT, || f.prompts().len() == 2).await;
    assert_eq!(questions(&first, 0), None);
}

#[tokio::test]
async fn a_message_written_while_a_card_is_active_closes_it_and_carries_the_confirmed_answers() {
    let f = fixture().await;
    let token = f.start_turn().await;
    let id = published(f.general.publish_card(&token, request()));
    f.end_turn().await;

    // Question 0 is confirmed; question 1 holds a draft nobody confirmed.
    f.act(&id, confirm(0)).unwrap();
    f.act(&id, go(1)).unwrap();
    f.act(
        &id,
        CardActionInput::Draft {
            index: 1,
            selected: vec![1],
            text: "not confirmed".into(),
        },
    )
    .unwrap();

    f.general.send_message(f.id(), "Go on").unwrap();
    until("the second prompt to reach omp", LIMIT, || f.prompts().len() == 2).await;
    let sent = f.prompts().remove(1);
    assert_eq!(
        sent,
        format!(
            "Go on\n\n{ANSWERS} {id} (added by Weaver, not written by the developer):\n{{\"status\":\"answered\",\"answers\":{{\"a\":{{\"status\":\"answered\",\"selected\":[\"y\"]}},\"b\":{{\"status\":\"not-answered\"}}}}}}"
        )
    );
    // The card is the read-only summary; the message is shown without the block, naming the card.
    assert_eq!(f.cards().cards[0].state, ChatCardState::AnsweredWithMessage);
    let shown: Vec<(String, Option<String>)> = f
        .entries()
        .iter()
        .filter_map(|e| match e {
            Entry::Developer { text, card, .. } => Some((text.clone(), card.clone())),
            _ => None,
        })
        .collect();
    assert_eq!(
        shown,
        [("Hi".to_owned(), None), ("Go on".to_owned(), Some(id.clone()))]
    );
    // A closed card takes no more answers.
    assert!(f.act(&id, go(0)).is_err());
    // The record keeps the state, the confirmed answer and nothing of the draft.
    let record = f.record().unwrap();
    let kept = &record.cards.as_ref().unwrap()[0];
    assert_eq!(kept.state, CardState::AnsweredWithMessage);
    assert_eq!(kept.answers.len(), 1);
    assert!(!std::fs::read_to_string(f.store().file(f.id())).unwrap().contains("not confirmed"));
}

#[tokio::test]
async fn send_answers_needs_every_question_answered_or_skipped_and_shows_no_message_of_the_developer() {
    let f = fixture().await;
    let token = f.start_turn().await;
    let id = published(f.general.publish_card(&token, request()));
    f.end_turn().await;

    // Whatever the interface does, the backend refuses a send with a question missing.
    f.act(&id, confirm(0)).unwrap();
    assert!(f.general.send_answers(f.id(), &id).is_err());
    assert_eq!(f.cards().cards[0].state, ChatCardState::Active);
    assert_eq!(f.prompts().len(), 1);
    assert!(f.general.send_answers(f.id(), "nosuchid").is_err());

    f.act(&id, skip(1)).unwrap();
    f.general.send_answers(f.id(), &id).unwrap();
    until("the answers to reach omp", LIMIT, || f.prompts().len() == 2).await;
    assert_eq!(
        f.prompts().remove(1),
        format!(
            "{ANSWERS} {id} (added by Weaver, not written by the developer):\n{{\"status\":\"answered\",\"answers\":{{\"a\":{{\"status\":\"answered\",\"selected\":[\"y\"]}},\"b\":{{\"status\":\"skipped\"}}}}}}"
        )
    );
    assert_eq!(f.cards().cards[0].state, ChatCardState::Answered);
    // The entry that carries the answers has no text: no message of the developer shows.
    assert!(f.entries().iter().any(|e| matches!(
        e,
        Entry::Developer { text, card: Some(card), .. } if text.is_empty() && *card == id
    )));
    // Sending again changes nothing: the card is not active.
    assert!(f.general.send_answers(f.id(), &id).is_err());
}

#[tokio::test]
async fn the_answers_of_a_card_sent_while_the_agent_works_are_a_steered_message_and_the_card_closes_at_once() {
    let f = fixture().await;
    let token = f.start_turn().await;
    let id = published(f.general.publish_card(&token, request()));
    f.act(&id, confirm(0)).unwrap();
    f.act(&id, skip(1)).unwrap();
    // The turn is still in progress.
    f.general.send_answers(f.id(), &id).unwrap();
    assert_eq!(f.cards().cards[0].state, ChatCardState::Answered);
    until("the steered message to reach omp", LIMIT, || {
        f.host.received().iter().any(|m| {
            m["method"] == "_session/steering"
                && m["params"]["prompt"][0]["text"].as_str().is_some_and(|t| t.contains(&id))
        })
    })
    .await;
    f.end_turn().await;
}

#[tokio::test]
async fn confirming_and_skipping_are_written_to_the_record_and_drafts_are_not() {
    let f = fixture().await;
    let token = f.start_turn().await;
    let id = published(f.general.publish_card(&token, request()));
    let recorded = || f.record().unwrap().cards.unwrap().remove(0);
    assert!(recorded().answers.is_empty());

    f.act(&id, go(1)).unwrap();
    f.act(
        &id,
        CardActionInput::Draft {
            index: 1,
            selected: vec![0],
            text: "draft".into(),
        },
    )
    .unwrap();
    assert!(recorded().answers.is_empty(), "a draft is never written");

    f.act(&id, confirm(1)).unwrap();
    f.act(&id, skip(0)).unwrap();
    let kept = recorded();
    assert_eq!(kept.answers["a"], RecordedAnswer::Skipped);
    assert_eq!(
        kept.answers["b"],
        RecordedAnswer::Answered {
            selected: vec!["p".into()],
            text: Some("draft".into())
        }
    );
    assert_eq!(kept.state, CardState::Active);
    f.end_turn().await;
}

#[tokio::test]
async fn new_chat_deletes_the_cards_in_memory_and_in_the_record() {
    let f = fixture().await;
    let token = f.start_turn().await;
    published(f.general.publish_card(&token, request()));
    assert!(f.record().unwrap().cards.is_some());

    f.general.clear(f.id()).unwrap();
    assert!(f.cards().cards.is_empty());
    assert!(f.record().is_none(), "the record goes with the conversation");
    assert!(f.events.told.lock().unwrap().last().unwrap().cards.is_empty());
}

fn kept_card(id: &str, answered: &[(&str, RecordedAnswer)], state: CardState) -> CardRecord {
    CardRecord {
        id: id.into(),
        questions: request(),
        answers: answered.iter().map(|(k, v)| ((*k).to_owned(), v.clone())).collect(),
        state,
    }
}

fn record_with(project: &Project, cards: Option<Vec<CardRecord>>) -> GeneralRecord {
    GeneralRecord {
        project: project.root.to_string_lossy().into_owned(),
        session_id: "fake-session".into(),
        updated: "2026-11-02T07:00:01.000Z".into(),
        cards,
    }
}

#[tokio::test]
async fn the_cards_come_back_with_the_record_and_the_active_one_restarts_from_its_first_unanswered_question() {
    let f = fixture_with(|store, project| {
        let answered = kept_card(
            "oldcard1",
            &[("a", RecordedAnswer::Skipped), ("b", RecordedAnswer::Skipped)],
            CardState::Answered,
        );
        let active = kept_card(
            "newcard2",
            &[("a", RecordedAnswer::Answered { selected: vec!["x".into()], text: None })],
            CardState::Active,
        );
        store
            .write(&project.id, &record_with(project, Some(vec![answered, active])))
            .unwrap();
    })
    .await;
    let cards = f.cards().cards;
    assert_eq!(
        cards.iter().map(|c| (c.id.as_str(), c.state)).collect::<Vec<_>>(),
        [("oldcard1", ChatCardState::Answered), ("newcard2", ChatCardState::Active)]
    );
    assert_eq!(cards[1].position, PositionView::Question { index: 1 });
    assert_eq!(cards[1].questions[0].draft.selected, [0], "what was confirmed is back");
    // The active card is still answerable and sendable after the reopening.
    f.act("newcard2", confirm(1)).unwrap();
    // The first card, read back as closed, takes nothing.
    assert!(f.act("oldcard1", go(0)).is_err());
}

#[tokio::test]
async fn two_active_cards_in_a_record_leave_only_the_newest_active() {
    let f = fixture_with(|store, project| {
        let cards = vec![
            kept_card("first111", &[], CardState::Active),
            kept_card("second22", &[], CardState::Active),
        ];
        store.write(&project.id, &record_with(project, Some(cards))).unwrap();
    })
    .await;
    let states: Vec<_> = f.cards().cards.iter().map(|c| c.state).collect();
    assert_eq!(states, [ChatCardState::AnsweredWithMessage, ChatCardState::Active]);
}

#[tokio::test]
async fn a_record_with_unreadable_cards_reads_as_no_cards() {
    let f = fixture_with(|store, project| {
        let record = record_with(project, None);
        store.write(&project.id, &record).unwrap();
        let file = store.file(&project.id);
        let mut text: Value = serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
        text["cards"] = json!([{ "id": "x" }]);
        std::fs::write(&file, text.to_string()).unwrap();
    })
    .await;
    assert!(f.cards().cards.is_empty());
    assert!(f.general.snapshot(f.id()).unwrap().state.kept, "the session itself is still kept");
}

#[tokio::test]
async fn a_card_whose_answers_do_not_fit_its_questions_is_not_restored() {
    let f = fixture_with(|store, project| {
        let wrong = kept_card(
            "wrong111",
            &[("zzz", RecordedAnswer::Skipped)],
            CardState::Active,
        );
        store.write(&project.id, &record_with(project, Some(vec![wrong]))).unwrap();
    })
    .await;
    assert!(f.cards().cards.is_empty());
}

#[tokio::test]
async fn a_kept_conversation_that_could_not_be_reloaded_loses_its_cards() {
    let f = fixture_with(|store, project| {
        let card = kept_card("keptcard", &[], CardState::Active);
        store.write(&project.id, &record_with(project, Some(vec![card]))).unwrap();
    })
    .await;
    f.host.script(json!({
        "log": f.host.received_log(),
        "loadError": "gone",
        "prompts": [[agent("A new start")]],
    }));
    assert_eq!(f.cards().cards.len(), 1);
    f.general.send_message(f.id(), "Hello again").unwrap();
    until("the cards to go with the conversation", LIMIT, || {
        f.cards().cards.is_empty()
    })
    .await;
    until("the record to be written again", LIMIT, || {
        f.record().is_some_and(|record| record.cards.is_none())
    })
    .await;
    // Nothing reached the agent from the card: the message goes alone.
    until("the prompt to reach omp", LIMIT, || {
        f.host.received().iter().any(|m| m["method"] == "session/prompt")
    })
    .await;
    let prompt = f
        .host
        .received()
        .into_iter()
        .find(|m| m["method"] == "session/prompt")
        .unwrap();
    assert!(!prompt["params"]["prompt"][0]["text"].as_str().unwrap().contains(ANSWERS));
}

#[tokio::test]
async fn the_question_of_the_active_card_is_read_for_ask_more_and_never_the_drafts() {
    let f = fixture().await;
    let token = f.start_turn().await;
    let id = published(f.general.publish_card(&token, request()));
    f.act(
        &id,
        CardActionInput::Draft {
            index: 0,
            selected: vec![0],
            text: "secret draft".into(),
        },
    )
    .unwrap();
    let seen = f.general.question_of_card(f.id(), &id, 1).unwrap();
    assert_eq!(seen.request, request());
    assert_eq!(seen.job, None);
    assert!(!format!("{seen:?}").contains("secret draft"));
    assert!(f.general.question_of_card(f.id(), &id, 2).is_none(), "no such question");
    assert!(f.general.question_of_card(f.id(), "nosuchid", 0).is_none());
    // The card is no longer active once a message closes it: nothing opens.
    f.end_turn().await;
    f.general.send_message(f.id(), "Meanwhile").unwrap();
    assert!(f.general.question_of_card(f.id(), &id, 0).is_none());
}

#[tokio::test]
async fn an_action_the_card_does_not_have_is_not_one() {
    let f = fixture().await;
    let token = f.start_turn().await;
    let id = published(f.general.publish_card(&token, request()));
    assert!(f
        .general
        .card_act(f.id(), &id, Action::Go(Position::Summary))
        .is_ok());
    for action in [Action::Minimize(true), Action::SkipAll, Action::Send] {
        assert!(f.general.card_act(f.id(), &id, action).is_err());
    }
    f.end_turn().await;
}

#[tokio::test]
async fn a_conversation_with_no_cards_writes_no_cards_and_its_messages_carry_no_block() {
    let f = fixture().await;
    f.host.script(json!({ "log": f.host.received_log(), "prompts": [[agent("Hello")]] }));
    f.general.send_message(f.id(), "Plain").unwrap();
    until("the turn to end", LIMIT, || {
        !f.general.working(f.id()) && f.prompts().len() == 1
    })
    .await;
    assert_eq!(f.prompts(), ["Plain"]);
    let record = f.record().unwrap();
    assert!(record.cards.is_none());
    assert!(!std::fs::read_to_string(f.store().file(f.id())).unwrap().contains("cards"));
    assert!(f.entries().iter().all(|e| !matches!(e, Entry::Developer { card: Some(_), .. })));
    assert!(matches!(
        &f.entries()[0],
        Entry::Developer { delivery: Delivery::Delivered, .. }
    ));
}

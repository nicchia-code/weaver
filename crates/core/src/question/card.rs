//! The cards of questions of the Chat (`requirements/rules#question-round`, **Cards of questions of
//! Chat**): what the agent of Chat publishes in its conversation instead of a round.
//!
//! A card never enters the queue of the rounds and never holds the agent. It reuses the model of a
//! round ([`Round`]: visit, preselection on the first visit, confirm, skip, drafts) without
//! minimize and without skip all, and adds what a conversation needs: an identity, the turn that
//! published it, a state, and the answers block that carries what the developer sent back.
//!
//! Pure: no clock, no I/O. The app keeps the cards of a project in memory and writes the confirmed
//! answers, the skips and the state to the record of the Chat ([`CardRecord`]); drafts and
//! position never leave memory.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::request::RoundRequest;
use super::round::{Action, ActionError, Draft, Origin, Position, Round, Slot, Status};

/// The line that opens the answers block: the card's identity goes between its two halves.
const HEAD_PREFIX: &str = "Answers to the card of questions ";
const HEAD_SUFFIX: &str = " (added by Weaver, not written by the developer):";

/// Where a card stands. Only one card of a project is active at a time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CardState {
    /// Published and waiting: the developer answers it, or ignores it and goes on writing.
    Active,
    /// `developer.SEND_ANSWERS`: every question was answered or skipped and the answers went to
    /// the agent as a message of the developer.
    Answered,
    /// `developer.SEND_MESSAGE`: a message written while the card was active closed it, carrying
    /// the confirmed answers and the skips.
    AnsweredWithMessage,
}

/// What the developer sent for one question of a card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CardAnswer {
    /// The labels chosen, in the order of the options, and the free text when there is some.
    Answered {
        selected: Vec<String>,
        text: Option<String>,
    },
    Skipped,
    /// A message closed the card while the question had neither a confirmed answer nor a skip: it
    /// stays open for the agent, which uses no recommendation for it.
    NotAnswered,
}

/// What a record keeps of a question: only confirmed answers and skips, never drafts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "kebab-case")]
pub enum RecordedAnswer {
    Answered {
        selected: Vec<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        text: Option<String>,
    },
    Skipped,
}

/// A card as the record of the Chat keeps it: `{id, questions, answers, state}`. `questions` is
/// the input of `weaver question new`; `answers` is keyed by question id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CardRecord {
    pub id: String,
    pub questions: RoundRequest,
    pub answers: BTreeMap<String, RecordedAnswer>,
    pub state: CardState,
}

/// A card of questions in the conversation of a project's Chat.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Card {
    /// Eight characters from `[a-z0-9]`, unique in the conversation.
    pub id: String,
    /// The turn of the Chat that published it; `0` for a card read back from the record, which
    /// no turn of this run equals.
    pub turn: u64,
    pub state: CardState,
    round: Round,
}

impl Card {
    /// A card the agent of the Chat of `project` publishes now: active, the first question
    /// visited.
    pub fn publish(id: String, turn: u64, project: &str, request: RoundRequest) -> Self {
        let mut round = Round::new(
            0,
            Origin::Chat {
                project: project.to_owned(),
            },
            request,
        );
        round.present();
        Self {
            id,
            turn,
            state: CardState::Active,
            round,
        }
    }

    /// A card read back from the record. It restarts from its first question without an answer
    /// (the summary when every question has one). `None` when the record does not fit its own
    /// questions: an answer for an unknown question or option, or a single choice with several.
    pub fn restore(project: &str, record: CardRecord) -> Option<Self> {
        let request = serde_json::to_value(&record.questions).ok()?;
        if !super::problems(&request).is_empty() {
            return None;
        }
        let mut round = Round::new(
            0,
            Origin::Chat {
                project: project.to_owned(),
            },
            record.questions,
        );
        if record
            .answers
            .keys()
            .any(|id| !round.request.questions.iter().any(|q| &q.id == id))
        {
            return None;
        }
        for (question, slot) in round.request.questions.iter().zip(&mut round.slots) {
            match record.answers.get(&question.id) {
                Some(RecordedAnswer::Answered { selected, text }) => {
                    let mut chosen = selected
                        .iter()
                        .map(|label| question.options.iter().position(|o| &o.label == label))
                        .collect::<Option<Vec<_>>>()?;
                    chosen.sort_unstable();
                    chosen.dedup();
                    if !question.multi && chosen.len() > 1 {
                        return None;
                    }
                    let draft = Draft {
                        selected: chosen,
                        text: text.clone().unwrap_or_default(),
                    };
                    if !draft.is_answer() {
                        return None;
                    }
                    slot.draft = draft.clone();
                    slot.confirmed = Some(draft);
                    slot.visited = true;
                }
                Some(RecordedAnswer::Skipped) => {
                    slot.skipped = true;
                    slot.visited = true;
                }
                None => {}
            }
        }
        match round.slots.iter().position(|s| s.status() == Status::Missing) {
            Some(index) => {
                round.apply(Action::Go(Position::Question { index })).ok()?;
            }
            None => round.position = Position::Summary,
        }
        Some(Self {
            id: record.id,
            turn: 0,
            state: record.state,
            round,
        })
    }

    pub fn is_active(&self) -> bool {
        self.state == CardState::Active
    }

    pub fn request(&self) -> &RoundRequest {
        &self.round.request
    }

    pub fn slots(&self) -> &[Slot] {
        &self.round.slots
    }

    pub fn position(&self) -> Position {
        self.round.position
    }

    /// What the developer does in the card: show a question or the summary, change what is on
    /// screen, confirm, skip. Only an active card takes it; `Minimize`, `Skip all` and `Send`
    /// do not exist in a card (sending is [`Card::answers`] once every question has one, then
    /// [`Card::close`]).
    pub fn act(&mut self, action: Action) -> Result<(), ActionError> {
        if !self.is_active() {
            return Err(ActionError::NotActive);
        }
        match action {
            Action::Minimize(_) | Action::SkipAll | Action::Send => Err(ActionError::NotInCard),
            action => self.round.apply(action).map(drop),
        }
    }

    /// How many questions have neither a confirmed answer nor a skip.
    pub fn missing(&self) -> usize {
        self.round.missing()
    }

    /// What goes to the agent as it is now: confirmed answers and skips, and `NotAnswered` for
    /// every other question, whatever its draft holds (a choice or a text not confirmed, and the
    /// preselected recommendation never touched, included).
    pub fn answers(&self) -> Vec<CardAnswer> {
        self.round
            .request
            .questions
            .iter()
            .zip(&self.round.slots)
            .map(|(question, slot)| match (&slot.confirmed, slot.skipped) {
                (Some(confirmed), _) => CardAnswer::Answered {
                    selected: confirmed
                        .selected
                        .iter()
                        .map(|&i| question.options[i].label.clone())
                        .collect(),
                    text: Some(confirmed.text.trim().to_owned()).filter(|text| !text.is_empty()),
                },
                (None, true) => CardAnswer::Skipped,
                (None, false) => CardAnswer::NotAnswered,
            })
            .collect()
    }

    /// The answers of `developer.SEND_ANSWERS`: the same, but only when every question has a
    /// confirmed answer or a skip.
    pub fn answers_to_send(&self) -> Result<Vec<CardAnswer>, ActionError> {
        match self.missing() {
            0 => Ok(self.answers()),
            missing => Err(ActionError::Missing(missing)),
        }
    }

    /// Closes the card: it stays in the conversation as the read-only summary of what was sent.
    pub fn close(&mut self, state: CardState) {
        self.state = state;
        self.round.position = Position::Summary;
    }

    /// The text of the answers block for what [`Card::answers`] says now.
    pub fn answers_block(&self) -> String {
        answers_block(&self.id, &self.round.request, &self.answers())
    }

    /// What the record keeps: confirmed answers and skips, never drafts or the position.
    pub fn record(&self) -> CardRecord {
        let answers = self
            .round
            .request
            .questions
            .iter()
            .zip(self.answers())
            .filter_map(|(question, answer)| {
                let recorded = match answer {
                    CardAnswer::Answered { selected, text } => {
                        RecordedAnswer::Answered { selected, text }
                    }
                    CardAnswer::Skipped => RecordedAnswer::Skipped,
                    CardAnswer::NotAnswered => return None,
                };
                Some((question.id.clone(), recorded))
            })
            .collect();
        CardRecord {
            id: self.id.clone(),
            questions: self.round.request.clone(),
            answers,
            state: self.state,
        }
    }
}

/// `{"status":"answered","answers":{<id>:...}}` as text, in that order: one entry per question id,
/// as the result of a round, with `{"status":"not-answered"}` for a question that stays open.
pub fn answers_json(request: &RoundRequest, answers: &[CardAnswer]) -> String {
    let text = |value: &str| json!(value).to_string();
    let entries: Vec<String> = request
        .questions
        .iter()
        .zip(answers)
        .map(|(question, answer)| {
            let entry = match answer {
                CardAnswer::Answered { selected, text: free } => {
                    let mut entry = format!(
                        "{{\"status\":\"answered\",\"selected\":{}",
                        json!(selected)
                    );
                    if let Some(free) = free {
                        entry.push_str(&format!(",\"text\":{}", text(free)));
                    }
                    entry.push('}');
                    entry
                }
                CardAnswer::Skipped => "{\"status\":\"skipped\"}".to_owned(),
                CardAnswer::NotAnswered => "{\"status\":\"not-answered\"}".to_owned(),
            };
            format!("{}:{entry}", text(&question.id))
        })
        .collect();
    format!(
        "{{\"status\":\"answered\",\"answers\":{{{}}}}}",
        entries.join(",")
    )
}

/// The answers block: the line that names the card, then the JSON on the next line.
pub fn answers_block(card: &str, request: &RoundRequest, answers: &[CardAnswer]) -> String {
    format!(
        "{HEAD_PREFIX}{card}{HEAD_SUFFIX}\n{}",
        answers_json(request, answers)
    )
}

/// The text that goes to omp: the developer's `text` (with the block of its tagged assets, when it
/// has one), an empty line (none when the text is empty) and the answers block.
pub fn with_answers_block(text: &str, block: &str) -> String {
    if text.is_empty() {
        block.to_owned()
    } else {
        format!("{text}\n\n{block}")
    }
}

/// A text that went to omp, cut at its answers block: what comes before it (without the empty
/// line that separates them) and the identity of the card it answers. `None` when the text holds
/// no such block, which is its last two lines: the head line and one line of JSON.
pub fn split_answers_block(text: &str) -> Option<(&str, &str)> {
    let mut start = 0;
    let mut found = None;
    for line in text.split_inclusive('\n') {
        let head = line.trim_end_matches('\n');
        if let Some(id) = head
            .strip_prefix(HEAD_PREFIX)
            .and_then(|rest| rest.strip_suffix(HEAD_SUFFIX))
            .filter(|id| !id.is_empty() && !id.contains(char::is_whitespace))
        {
            found = Some((start, start + line.len(), id));
        }
        start += line.len();
    }
    let (at, after, id) = found?;
    let json = text[after..].trim_end_matches('\n');
    if json.is_empty() || json.contains('\n') {
        return None;
    }
    let status = serde_json::from_str::<Value>(json).ok()?;
    if status.get("status").and_then(Value::as_str) != Some("answered") {
        return None;
    }
    let before = &text[..at];
    Some((before.strip_suffix("\n\n").unwrap_or(before), id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::question::request::{Question, QuestionOption};

    fn option(label: &str) -> QuestionOption {
        QuestionOption {
            label: label.into(),
            description: "d".into(),
            details: None,
        }
    }

    fn request() -> RoundRequest {
        RoundRequest {
            questions: vec![
                Question {
                    id: "a".into(),
                    question: "Which one?".into(),
                    context: None,
                    options: vec![option("x"), option("y")],
                    recommended: 1,
                    multi: false,
                },
                Question {
                    id: "b".into(),
                    question: "Which ones?".into(),
                    context: None,
                    options: vec![option("p"), option("q"), option("r")],
                    recommended: 0,
                    multi: true,
                },
            ],
        }
    }

    fn card() -> Card {
        Card::publish("abcd1234".into(), 3, "p1", request())
    }

    fn go(index: usize) -> Action {
        Action::Go(Position::Question { index })
    }

    fn pick(index: usize, selected: &[usize], text: &str) -> Action {
        Action::Draft {
            index,
            selected: selected.to_vec(),
            text: text.into(),
        }
    }

    #[test]
    fn a_published_card_is_active_and_preselects_the_recommended_option_of_the_first_visit_only() {
        let mut card = card();
        assert!(card.is_active() && card.turn == 3);
        assert_eq!(card.slots()[0].draft.selected, [1]);
        assert!(card.slots()[1].draft.selected.is_empty());
        card.act(go(1)).unwrap();
        assert_eq!(card.slots()[1].draft.selected, [0]);
        card.act(pick(0, &[], "")).unwrap();
        card.act(go(1)).unwrap();
        card.act(go(0)).unwrap();
        assert!(card.slots()[0].draft.selected.is_empty(), "a later visit preselects nothing");
    }

    #[test]
    fn a_card_has_no_minimize_no_skip_all_and_no_send_and_a_closed_one_takes_nothing() {
        let mut card = card();
        for action in [Action::Minimize(true), Action::SkipAll, Action::Send] {
            assert_eq!(card.act(action), Err(ActionError::NotInCard));
        }
        card.close(CardState::Answered);
        assert_eq!(card.act(go(1)), Err(ActionError::NotActive));
        assert_eq!(card.position(), Position::Summary);
    }

    #[test]
    fn a_send_needs_every_question_answered_or_skipped() {
        let mut card = card();
        card.act(Action::Confirm { index: 0 }).unwrap();
        assert_eq!(card.answers_to_send(), Err(ActionError::Missing(1)));
        card.act(Action::Skip { index: 1 }).unwrap();
        assert_eq!(
            card.answers_to_send().unwrap(),
            [
                CardAnswer::Answered {
                    selected: vec!["y".into()],
                    text: None
                },
                CardAnswer::Skipped
            ]
        );
    }

    #[test]
    fn a_message_sends_confirmed_answers_and_skips_and_everything_else_as_not_answered() {
        let mut card = card();
        // Question 0 holds a preselection nobody confirmed; question 1 a confirmed multi answer.
        card.act(go(1)).unwrap();
        card.act(pick(1, &[0, 2], "  why  ")).unwrap();
        card.act(Action::Confirm { index: 1 }).unwrap();
        card.act(pick(1, &[1], "changed after confirming")).unwrap();
        assert_eq!(
            card.answers(),
            [
                CardAnswer::NotAnswered,
                CardAnswer::Answered {
                    selected: vec!["p".into(), "r".into()],
                    text: Some("why".into())
                }
            ]
        );
        card.act(Action::Skip { index: 0 }).unwrap();
        assert_eq!(card.answers()[0], CardAnswer::Skipped);
        // A skip carries nothing, whatever the draft says.
        card.act(pick(0, &[0], "ignored")).unwrap();
        assert_eq!(card.answers()[0], CardAnswer::Skipped);
    }

    #[test]
    fn the_answers_block_names_the_card_and_has_one_entry_per_question() {
        let mut card = card();
        card.act(Action::Confirm { index: 0 }).unwrap();
        let block = card.answers_block();
        assert_eq!(
            block,
            "Answers to the card of questions abcd1234 (added by Weaver, not written by the developer):\n\
             {\"status\":\"answered\",\"answers\":{\"a\":{\"status\":\"answered\",\"selected\":[\"y\"]},\"b\":{\"status\":\"not-answered\"}}}"
        );
    }

    #[test]
    fn the_text_that_goes_to_omp_has_an_empty_line_before_the_block_unless_the_text_is_empty() {
        assert_eq!(with_answers_block("", "B"), "B");
        assert_eq!(with_answers_block("Hello", "B"), "Hello\n\nB");
    }

    #[test]
    fn a_text_is_cut_at_its_answers_block() {
        let mut card = card();
        card.act(Action::Skip { index: 0 }).unwrap();
        let block = card.answers_block();
        let alone = split_answers_block(&block).unwrap();
        assert_eq!(alone, ("", "abcd1234"));
        let sent = with_answers_block("My message", &block);
        assert_eq!(split_answers_block(&sent), Some(("My message", "abcd1234")));
        // What a text that only looks like a block is: shown as it is.
        for text in [
            "Answers to the card of questions abcd1234 (added by Weaver, not written by the developer):",
            "Answers to the card of questions abcd1234 (added by Weaver, not written by the developer):\nnot json",
            "Answers to the card of questions abcd1234 (added by Weaver, not written by the developer):\n{\"status\":\"other\"}",
            "Answers to the card of questions abcd1234 (added by Weaver, not written by the developer):\n{\"status\":\"answered\"}\nmore",
            "nothing",
        ] {
            assert_eq!(split_answers_block(text), None, "{text}");
        }
    }

    #[test]
    fn the_record_keeps_confirmed_answers_and_skips_and_never_drafts() {
        let mut card = card();
        card.act(Action::Confirm { index: 0 }).unwrap();
        card.act(go(1)).unwrap();
        card.act(pick(1, &[2], "draft")).unwrap();
        let record = card.record();
        assert_eq!(
            serde_json::to_value(&record).unwrap(),
            json!({
                "id": "abcd1234",
                "questions": serde_json::to_value(request()).unwrap(),
                "answers": { "a": { "status": "answered", "selected": ["y"] } },
                "state": "active"
            })
        );
        card.act(Action::Skip { index: 1 }).unwrap();
        card.close(CardState::AnsweredWithMessage);
        let record = card.record();
        assert_eq!(record.answers["b"], RecordedAnswer::Skipped);
        assert_eq!(record.state, CardState::AnsweredWithMessage);
    }

    #[test]
    fn a_restored_card_restarts_from_its_first_question_without_an_answer() {
        let mut card = card();
        card.act(Action::Confirm { index: 0 }).unwrap();
        let restored = Card::restore("p1", card.record()).unwrap();
        assert!(restored.is_active() && restored.turn == 0);
        assert_eq!(restored.position(), Position::Question { index: 1 });
        assert_eq!(restored.answers(), card.answers());
        // The preselection of the question it lands on is the first visit's.
        assert_eq!(restored.slots()[1].draft.selected, [0]);
        // Every question answered: the summary.
        card.act(Action::Skip { index: 1 }).unwrap();
        let restored = Card::restore("p1", card.record()).unwrap();
        assert_eq!(restored.position(), Position::Summary);
        assert_eq!(restored.missing(), 0);
    }

    #[test]
    fn a_record_that_does_not_fit_its_questions_is_not_restored() {
        let good = card().record();
        let mut unknown_question = good.clone();
        unknown_question
            .answers
            .insert("zzz".into(), RecordedAnswer::Skipped);
        let mut unknown_option = good.clone();
        unknown_option.answers.insert(
            "a".into(),
            RecordedAnswer::Answered {
                selected: vec!["nope".into()],
                text: None,
            },
        );
        let mut two_in_single = good.clone();
        two_in_single.answers.insert(
            "a".into(),
            RecordedAnswer::Answered {
                selected: vec!["x".into(), "y".into()],
                text: None,
            },
        );
        let mut nothing = good.clone();
        nothing.answers.insert(
            "a".into(),
            RecordedAnswer::Answered {
                selected: vec![],
                text: None,
            },
        );
        let mut no_questions = good;
        no_questions.questions.questions.clear();
        for record in [unknown_question, unknown_option, two_in_single, nothing, no_questions] {
            assert!(Card::restore("p1", record).is_none());
        }
    }
}

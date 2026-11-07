//! One round of questions as the developer answers it: the answers he confirmed, the ones he has
//! not confirmed yet, where he is, and what is sent.
//!
//! Pure: no clock, no I/O. The queue ([`super::queue`]) decides which round is presented; the app
//! keeps the state of a round only in memory (`requirements/tech#question-round`).

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use super::request::RoundRequest;

/// The identity of a round inside the app: assigned in the order rounds are submitted.
pub type RoundId = u64;

/// Who asks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Origin {
    /// The agent of a job, through `weaver question new`.
    Agent { job: String },
    /// The agent of the project's Chat, through `weaver question new`: it publishes a card of
    /// questions in the conversation, which never enters the queue (`card::Card` holds the round
    /// of a card). `project` is the project hash.
    Chat { project: String },
    /// Weaver itself: the prepared question of a Drift. `id` is the Drift's permanent identity,
    /// `drift` how the developer names it (its short number) and `title` its title.
    Drift {
        id: String,
        drift: String,
        title: String,
    },
}

impl Origin {
    /// The job the round belongs to, when the agent of a job asks.
    pub fn job(&self) -> Option<&str> {
        match self {
            Origin::Agent { job } => Some(job),
            Origin::Chat { .. } | Origin::Drift { .. } => None,
        }
    }

    /// The project whose Chat the round belongs to, when the agent of the Chat asks.
    pub fn chat(&self) -> Option<&str> {
        match self {
            Origin::Chat { project } => Some(project),
            Origin::Agent { .. } | Origin::Drift { .. } => None,
        }
    }
}

/// Where the developer is in a round.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Position {
    Question { index: usize },
    Summary,
}

/// What the developer has chosen and written in a question, confirmed or not.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Draft {
    /// Indexes of the chosen options, ascending.
    pub selected: Vec<usize>,
    pub text: String,
}

impl Draft {
    /// An answer needs an option or text with something in it.
    pub fn is_answer(&self) -> bool {
        !self.selected.is_empty() || !self.text.trim().is_empty()
    }
}

/// The state of one question.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Slot {
    /// What is on screen: kept by Weaver so that moving around loses nothing.
    pub draft: Draft,
    /// What the developer confirmed; only this is ever sent.
    pub confirmed: Option<Draft>,
    pub skipped: bool,
    /// The question has been shown; the recommended option is preselected on the first visit only.
    pub visited: bool,
}

/// What the index and the summary say about a question.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Status {
    Answered,
    Skipped,
    Missing,
}

impl Slot {
    pub fn status(&self) -> Status {
        if self.confirmed.is_some() {
            Status::Answered
        } else if self.skipped {
            Status::Skipped
        } else {
            Status::Missing
        }
    }
}

/// What one question came to in the result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Answer {
    /// The labels chosen, in the order of the options, and the free text when there is some.
    Answered {
        selected: Vec<String>,
        text: Option<String>,
    },
    /// Nothing is carried: no text, no option, no made-up answer.
    Skipped,
}

/// What the developer does in a presented round.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// `developer.GO_TO`: shows a question or the summary.
    Go(Position),
    /// Changes what is on screen in a question; nothing is confirmed.
    Draft {
        index: usize,
        selected: Vec<usize>,
        text: String,
    },
    /// `Next` / `Review`: confirms the question as it is on screen.
    Confirm { index: usize },
    /// `Skip`.
    Skip { index: usize },
    /// The bar at the bottom and the overlay.
    Minimize(bool),
    /// `developer.SKIP_ALL_QUESTIONS`.
    SkipAll,
    /// `developer.SEND_ANSWERS`.
    Send,
}

/// Why an action is refused; the round is left as it was.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ActionError {
    #[error("the round has no question {0}")]
    NoSuchQuestion(usize),
    #[error("the round has no option {option} in question {question}")]
    NoSuchOption { question: usize, option: usize },
    #[error("question {0} takes one option at most")]
    SingleChoice(usize),
    #[error("choose an option or write an answer")]
    NothingToConfirm,
    #[error("{0} question(s) have neither an answer nor a skip")]
    Missing(usize),
    #[error("the round is not presented")]
    NotPresented,
    #[error("the round does not exist")]
    NoSuchRound,
    #[error("the card is not active")]
    NotActive,
    #[error("a card of questions has no such action")]
    NotInCard,
}

/// What an action did to a round.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Applied {
    Changed,
    /// The round is complete: these are the answers, one per question in order.
    Finished(Vec<Answer>),
}

/// A round of questions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Round {
    pub id: RoundId,
    pub origin: Origin,
    pub request: RoundRequest,
    pub slots: Vec<Slot>,
    pub position: Position,
    pub minimized: bool,
}

impl Round {
    /// A round not shown yet. Presenting it is the queue's business ([`Round::present`]).
    pub fn new(id: RoundId, origin: Origin, request: RoundRequest) -> Self {
        let slots = vec![Slot::default(); request.questions.len()];
        Self {
            id,
            origin,
            request,
            slots,
            position: Position::Question { index: 0 },
            minimized: false,
        }
    }

    /// The round comes to the developer: the first question is visited.
    pub fn present(&mut self) {
        self.visit(0);
    }

    fn visit(&mut self, index: usize) {
        let recommended = self.request.questions[index].recommended;
        let slot = &mut self.slots[index];
        if !slot.visited {
            slot.visited = true;
            if slot.draft.selected.is_empty() && slot.confirmed.is_none() && !slot.skipped {
                slot.draft.selected = vec![recommended];
            }
        }
    }

    /// How many questions have neither a confirmed answer nor a skip.
    pub fn missing(&self) -> usize {
        self.slots
            .iter()
            .filter(|slot| slot.status() == Status::Missing)
            .count()
    }

    fn check_index(&self, index: usize) -> Result<(), ActionError> {
        if index < self.slots.len() {
            Ok(())
        } else {
            Err(ActionError::NoSuchQuestion(index))
        }
    }

    /// Applies what the developer did.
    pub fn apply(&mut self, action: Action) -> Result<Applied, ActionError> {
        match action {
            Action::Go(Position::Question { index }) => {
                self.check_index(index)?;
                self.visit(index);
                self.position = Position::Question { index };
            }
            Action::Go(Position::Summary) => self.position = Position::Summary,
            Action::Draft {
                index,
                mut selected,
                text,
            } => {
                self.check_index(index)?;
                let question = &self.request.questions[index];
                selected.sort_unstable();
                selected.dedup();
                if let Some(&option) = selected.iter().find(|&&o| o >= question.options.len()) {
                    return Err(ActionError::NoSuchOption {
                        question: index,
                        option,
                    });
                }
                if !question.multi && selected.len() > 1 {
                    return Err(ActionError::SingleChoice(index));
                }
                self.slots[index].draft = Draft { selected, text };
            }
            Action::Confirm { index } => {
                self.check_index(index)?;
                let slot = &mut self.slots[index];
                if !slot.draft.is_answer() {
                    return Err(ActionError::NothingToConfirm);
                }
                slot.confirmed = Some(slot.draft.clone());
                slot.skipped = false;
            }
            Action::Skip { index } => {
                self.check_index(index)?;
                let slot = &mut self.slots[index];
                slot.confirmed = None;
                slot.skipped = true;
            }
            Action::Minimize(minimized) => self.minimized = minimized,
            Action::SkipAll => {
                for slot in self
                    .slots
                    .iter_mut()
                    .filter(|slot| slot.status() == Status::Missing)
                {
                    slot.skipped = true;
                }
                return Ok(Applied::Finished(self.answers()));
            }
            Action::Send => {
                let missing = self.missing();
                if missing > 0 {
                    return Err(ActionError::Missing(missing));
                }
                return Ok(Applied::Finished(self.answers()));
            }
        }
        Ok(Applied::Changed)
    }

    /// The answers as they are now: confirmed ones, and skips for the rest. A skipped question
    /// carries nothing, whatever is in its draft.
    pub fn answers(&self) -> Vec<Answer> {
        self.request
            .questions
            .iter()
            .zip(&self.slots)
            .map(|(question, slot)| match &slot.confirmed {
                Some(confirmed) => Answer::Answered {
                    selected: confirmed
                        .selected
                        .iter()
                        .map(|&i| question.options[i].label.clone())
                        .collect(),
                    text: Some(confirmed.text.trim().to_owned()).filter(|text| !text.is_empty()),
                },
                None => Answer::Skipped,
            })
            .collect()
    }
}

/// The result `weaver question new` prints when the round was answered: one entry per question id.
pub fn answered_json(request: &RoundRequest, answers: &[Answer]) -> Value {
    let mut map = Map::new();
    for (question, answer) in request.questions.iter().zip(answers) {
        let entry = match answer {
            Answer::Answered { selected, text } => {
                let mut entry = json!({ "status": "answered", "selected": selected });
                if let Some(text) = text {
                    entry["text"] = json!(text);
                }
                entry
            }
            Answer::Skipped => json!({ "status": "skipped" }),
        };
        map.insert(question.id.clone(), entry);
    }
    json!({ "status": "answered", "answers": Value::Object(map) })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::question::request::{Question, QuestionOption};

    pub(crate) fn question(id: &str, multi: bool) -> Question {
        let option = |label: &str| QuestionOption {
            label: label.into(),
            description: "d".into(),
            details: None,
        };
        Question {
            id: id.into(),
            question: "Q".into(),
            context: None,
            options: vec![option("a"), option("b"), option("c")],
            recommended: 1,
            multi,
        }
    }

    pub(crate) fn round(questions: Vec<Question>) -> Round {
        let mut round = Round::new(
            1,
            Origin::Agent {
                job: "job00001".into(),
            },
            RoundRequest { questions },
        );
        round.present();
        round
    }

    fn draft(index: usize, selected: &[usize], text: &str) -> Action {
        Action::Draft {
            index,
            selected: selected.to_vec(),
            text: text.into(),
        }
    }

    #[test]
    fn the_recommended_option_is_preselected_on_the_first_visit_only() {
        let mut round = round(vec![question("x", false), question("y", false)]);
        assert_eq!(round.slots[0].draft.selected, [1]);
        assert!(round.slots[1].draft.selected.is_empty(), "not visited yet");

        round
            .apply(Action::Go(Position::Question { index: 1 }))
            .unwrap();
        assert_eq!(round.slots[1].draft.selected, [1]);

        // Cleared by the developer: coming back does not preselect it again.
        round.apply(draft(1, &[], "")).unwrap();
        round
            .apply(Action::Go(Position::Question { index: 0 }))
            .unwrap();
        round
            .apply(Action::Go(Position::Question { index: 1 }))
            .unwrap();
        assert!(round.slots[1].draft.selected.is_empty());
    }

    #[test]
    fn drafts_survive_moving_around_and_only_confirmed_answers_are_sent() {
        let mut round = round(vec![question("x", true), question("y", false)]);
        round.apply(draft(0, &[2, 0, 2], "a note")).unwrap();
        assert_eq!(round.slots[0].draft.selected, [0, 2], "ordered, no repeats");
        round.apply(Action::Go(Position::Summary)).unwrap();
        round
            .apply(Action::Go(Position::Question { index: 0 }))
            .unwrap();
        assert_eq!(round.slots[0].draft.text, "a note");
        assert_eq!(round.slots[0].status(), Status::Missing);

        round.apply(Action::Confirm { index: 0 }).unwrap();
        round
            .apply(draft(0, &[1], "changed, not confirmed"))
            .unwrap();
        round.apply(Action::Skip { index: 1 }).unwrap();
        let Applied::Finished(answers) = round.apply(Action::Send).unwrap() else {
            panic!("sent")
        };
        assert_eq!(
            answers,
            [
                Answer::Answered {
                    selected: vec!["a".into(), "c".into()],
                    text: Some("a note".into())
                },
                Answer::Skipped
            ]
        );
    }

    #[test]
    fn confirming_needs_an_option_or_text_and_single_choice_takes_one() {
        let mut round = round(vec![question("x", false)]);
        round.apply(draft(0, &[], "  \n ")).unwrap();
        assert_eq!(
            round.apply(Action::Confirm { index: 0 }),
            Err(ActionError::NothingToConfirm)
        );
        assert_eq!(
            round.apply(draft(0, &[0, 1], "")),
            Err(ActionError::SingleChoice(0))
        );
        assert_eq!(
            round.apply(draft(0, &[3], "")),
            Err(ActionError::NoSuchOption {
                question: 0,
                option: 3
            })
        );
        assert_eq!(
            round.apply(draft(5, &[], "")),
            Err(ActionError::NoSuchQuestion(5))
        );
        round.apply(draft(0, &[], "free")).unwrap();
        round.apply(Action::Confirm { index: 0 }).unwrap();
        assert_eq!(round.slots[0].status(), Status::Answered);
    }

    #[test]
    fn the_backend_refuses_a_send_with_a_question_missing() {
        let mut round = round(vec![
            question("x", false),
            question("y", false),
            question("z", false),
        ]);
        round.apply(Action::Confirm { index: 0 }).unwrap();
        assert_eq!(round.apply(Action::Send), Err(ActionError::Missing(2)));
        round.apply(Action::Skip { index: 1 }).unwrap();
        assert_eq!(round.apply(Action::Send), Err(ActionError::Missing(1)));
        assert_eq!(
            round.slots[0].status(),
            Status::Answered,
            "a refused send changes nothing"
        );
    }

    #[test]
    fn skip_all_skips_what_has_no_confirmed_answer_and_keeps_confirmed_answers() {
        let mut round = round(vec![question("x", false), question("y", false)]);
        round.apply(Action::Confirm { index: 0 }).unwrap();
        round
            .apply(draft(1, &[0], "typed, never confirmed"))
            .unwrap();
        let Applied::Finished(answers) = round.apply(Action::SkipAll).unwrap() else {
            panic!("finished")
        };
        assert_eq!(
            answers[0],
            Answer::Answered {
                selected: vec!["b".into()],
                text: None
            }
        );
        assert_eq!(answers[1], Answer::Skipped);
    }

    #[test]
    fn a_skip_carries_nothing_and_an_answer_confirmed_later_replaces_it() {
        let mut round = round(vec![question("x", false)]);
        round.apply(draft(0, &[0], "text")).unwrap();
        round.apply(Action::Skip { index: 0 }).unwrap();
        assert_eq!(round.answers(), [Answer::Skipped]);
        round.apply(Action::Confirm { index: 0 }).unwrap();
        assert_eq!(round.slots[0].status(), Status::Answered);
    }

    #[test]
    fn the_result_has_one_entry_per_question_id_with_labels_and_no_blank_text() {
        let request = RoundRequest {
            questions: vec![question("first", true), question("second", false)],
        };
        let answers = [
            Answer::Answered {
                selected: vec!["a".into(), "c".into()],
                text: None,
            },
            Answer::Skipped,
        ];
        assert_eq!(
            answered_json(&request, &answers),
            json!({ "status": "answered", "answers": {
                "first": { "status": "answered", "selected": ["a", "c"] },
                "second": { "status": "skipped" },
            } })
        );
        let with_text = [
            Answer::Answered {
                selected: vec![],
                text: Some("free".into()),
            },
            Answer::Skipped,
        ];
        assert_eq!(
            answered_json(&request, &with_text)["answers"]["first"],
            json!({ "status": "answered", "selected": [], "text": "free" })
        );
    }
}

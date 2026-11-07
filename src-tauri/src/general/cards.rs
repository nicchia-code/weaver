//! The cards of questions of the Chat as the UI gets them (`requirements/ui#question-round`,
//! **Active in General**).
//!
//! The cards of a project live in its Chat slot ([`super::service`]): in memory, with their drafts
//! and their position; their confirmed answers, skips and state are also in the record of the
//! Chat. The UI asks for them when its window loads and after each `chat-cards-changed`, and its
//! actions go through the commands of [`super::commands`]; a card never goes through the rounds'
//! queue, `question-round-changed` or the overlay.
//!
//! Where a card sits in the conversation is told by the conversation itself: a card that was
//! closed shows right before the entry of the developer whose `card` names it (an entry with no
//! text shows no message), the active card and any card no entry names show at the end.

use serde::{Deserialize, Serialize};
use ts_rs::TS;
use weaver_core::question::{Action, Card, CardState};

use crate::rounds::view::{question_views, PositionView, QuestionView};

/// Where a card stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "kebab-case")]
pub enum ChatCardState {
    /// Published and waiting: it takes answers, and is the only card that does.
    Active,
    /// Its answers were sent with `Send to agent`: the read-only summary.
    Answered,
    /// A message of the developer closed it, carrying what was confirmed and skipped.
    AnsweredWithMessage,
}

impl From<CardState> for ChatCardState {
    fn from(state: CardState) -> Self {
        match state {
            CardState::Active => Self::Active,
            CardState::Answered => Self::Answered,
            CardState::AnsweredWithMessage => Self::AnsweredWithMessage,
        }
    }
}

/// One card of questions, with what the developer did in it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ChatCardView {
    /// Eight characters, unique in the conversation.
    pub id: String,
    pub state: ChatCardState,
    pub questions: Vec<QuestionView>,
    pub position: PositionView,
}

/// The cards of the Chat of a project, in publication order, at one revision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ChatCards {
    /// The project hash.
    pub project_id: String,
    /// Grows with every change; an answer older than one already shown is dropped.
    #[ts(type = "number")]
    pub revision: u64,
    pub cards: Vec<ChatCardView>,
}

/// What the developer does in the active card. There is no minimize and no skip all in a card, and
/// sending is its own command.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum CardActionInput {
    /// Shows a question or the summary.
    Go { position: PositionView },
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
}

impl From<CardActionInput> for Action {
    fn from(input: CardActionInput) -> Self {
        match input {
            CardActionInput::Go { position } => Action::Go(position.into()),
            CardActionInput::Draft {
                index,
                selected,
                text,
            } => Action::Draft {
                index,
                selected,
                text,
            },
            CardActionInput::Confirm { index } => Action::Confirm { index },
            CardActionInput::Skip { index } => Action::Skip { index },
        }
    }
}

/// The view of `card`.
pub fn view_of(card: &Card) -> ChatCardView {
    ChatCardView {
        id: card.id.clone(),
        state: card.state.into(),
        questions: question_views(card.request(), card.slots()),
        position: card.position().into(),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use weaver_core::question::{Position, RoundRequest};

    use super::*;

    fn card() -> Card {
        let request: RoundRequest = serde_json::from_value(json!({ "questions": [{
            "id": "a", "question": "Q",
            "options": [{"label": "x", "description": "d"}, {"label": "y", "description": "d"}],
            "recommended": 1, "multi": false
        }]}))
        .unwrap();
        Card::publish("abcd1234".into(), 1, "p1", request)
    }

    #[test]
    fn a_card_is_seen_with_its_questions_its_preselection_and_its_position() {
        let view = view_of(&card());
        assert_eq!((view.id.as_str(), view.state), ("abcd1234", ChatCardState::Active));
        assert_eq!(view.position, PositionView::Question { index: 0 });
        assert_eq!(view.questions[0].draft.selected, [1]);
        assert!(view.questions[0].visited);
    }

    #[test]
    fn the_actions_of_a_card_are_the_ones_of_a_round_without_minimize_skip_all_and_send() {
        let go: CardActionInput = serde_json::from_value(json!({ "kind": "go", "position": { "kind": "summary" } })).unwrap();
        assert_eq!(Action::from(go), Action::Go(Position::Summary));
        let draft: CardActionInput = serde_json::from_value(json!({ "kind": "draft", "index": 0, "selected": [1], "text": "t" })).unwrap();
        assert_eq!(Action::from(draft), Action::Draft { index: 0, selected: vec![1], text: "t".into() });
        for refused in ["minimize", "skip-all", "send"] {
            assert!(serde_json::from_value::<CardActionInput>(json!({ "kind": refused })).is_err());
        }
    }
}

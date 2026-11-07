//! Question rounds (Machine `question-round`): a group of decisions the agent of a job asks the
//! developer at once, or the prepared question of a Drift that Weaver asks itself; and the cards of
//! questions the agent of the project's Chat publishes in its conversation, which never hold it.
//!
//! The logic is pure and in memory; nothing of a round is written to the project or to a log. A
//! card of Chat keeps its confirmed answers, its skips and its state in the record of the Chat
//! (`requirements/data#general-chat`), never its drafts.
//!
//! | Module      | Responsibility |
//! |-------------|----------------|
//! | [`request`] | The request of a round and its validation, every problem with its path |
//! | [`round`]   | One round as the developer answers it: drafts, confirmed answers, skips, send |
//! | [`queue`]   | The rounds of a project: one presented, the others first in, first out |
//! | [`card`]    | A card of questions of the Chat: state, answers sent back, the answers block, the record |
//!
//! The wire: `weaver question new` validates its input, then sends one request of kind [`KIND`] whose
//! payload is the input object and which carries the identities of the caller (`job` for the
//! agent of a job, `general` for the agent of the Chat; the job decides when both are there); the
//! app answers with a reply carrying the result (`answered`, for a job, after a connection that
//! stays open with no time limit until a result exists; closing it is `agent.STOP_WAITING`; or
//! `published`, for the Chat, at once) or a failure whose code is one of [`code`].

pub mod card;
pub mod queue;
pub mod request;
pub mod round;

pub use card::{
    answers_block, answers_json, split_answers_block, with_answers_block, Card, CardAnswer,
    CardRecord, CardState, RecordedAnswer,
};

pub use queue::{Effect, Ending, Outcome, Queue};
pub use request::{describe, parse, problems, Problem, Question, QuestionOption, RoundRequest};
pub use round::{
    answered_json, Action, ActionError, Answer, Applied, Draft, Origin, Position, Round, RoundId,
    Slot, Status,
};

/// `Request::kind` of a round of questions.
pub const KIND: &str = "question.new";

/// The text of `unavailable`: the agent must not ask in any other way.
pub const UNAVAILABLE_REASON: &str =
    "Weaver cannot show questions for this command. Do not ask in any other way: stop and tell the developer.";

/// Codes of the failures of a round, in `Response::error` and in `{error: {code, message}}`.
pub mod code {
    /// No round can be shown: the caller is neither a working job nor the Chat during a turn, or
    /// Weaver is not at hand.
    pub const UNAVAILABLE: &str = "unavailable";
    /// The round is invalid: the message names every bad field by path.
    pub const INVALID_INPUT: &str = crate::command::code::INVALID_INPUT;
    /// The job ended or the window closed before the round was answered; the connection was lost
    /// after submission counts too. Never happens to a card of Chat once it is published.
    pub const INTERRUPTED: &str = "interrupted";
    /// The CLI was stopped (SIGINT, SIGTERM, closed stdout): the round is withdrawn. Never
    /// happens to a card of Chat once it is published.
    pub const CANCELLED: &str = "cancelled";
    /// The Chat already published a card in this turn: nothing is published.
    pub const CARD_ALREADY_PUBLISHED: &str = "card-already-published";
}

/// The message of an interrupted round.
pub const INTERRUPTED_MESSAGE: &str =
    "The round was interrupted: its job ended or its window closed before the developer answered. No answer exists. Stop the work that waited for it and do not use any recommendation.";

/// The message of a withdrawn round.
pub const CANCELLED_MESSAGE: &str = "The command was stopped before the developer answered; the round is withdrawn and no answer exists.";

/// The message of `published`: what the agent of the Chat does next.
pub const PUBLISHED_MESSAGE: &str =
    "The card of questions is published in the conversation. End your turn now: do not repeat the questions in your reply and do not wait. The answers will come as a later message of the developer, with an answers block.";

/// The message of `card-already-published`.
pub const CARD_ALREADY_PUBLISHED_MESSAGE: &str =
    "A card of questions was already published in this turn and nothing was published now. Put every question of the turn in one card, and end your turn.";

/// The result `weaver question new` prints in Chat when the card was published.
pub fn published_json(card: &str) -> serde_json::Value {
    serde_json::json!({ "status": "published", "card": card, "message": PUBLISHED_MESSAGE })
}

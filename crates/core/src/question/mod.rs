//! Question rounds (Machine `question-round`): a group of decisions the agent of a job, or of the
//! project's Chat when the developer asked to be asked that way, asks the developer at once, or the
//! prepared question of a Drift that Weaver asks itself.
//!
//! The logic is pure and in memory; nothing of a round is written to the project or to a log.
//!
//! | Module      | Responsibility |
//! |-------------|----------------|
//! | [`request`] | The request of a round and its validation, every problem with its path |
//! | [`round`]   | One round as the developer answers it: drafts, confirmed answers, skips, send |
//! | [`queue`]   | The rounds of a project: one presented, the others first in, first out |
//!
//! The wire: `weaver question new` validates its input, then sends one request of kind [`KIND`] whose
//! payload is the input object and which carries the identities of the caller (`job` for the
//! agent of a job, `general` for the agent of the Chat; the job decides when both are there); the
//! app answers with a reply carrying the result (`answered`) or a
//! failure whose code is one of [`code`]. The connection stays open, with no time limit, until a
//! result exists; closing it is `agent.STOP_WAITING`.

pub mod queue;
pub mod request;
pub mod round;

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
    /// after submission counts too.
    pub const INTERRUPTED: &str = "interrupted";
    /// The CLI was stopped (SIGINT, SIGTERM, closed stdout): the round is withdrawn.
    pub const CANCELLED: &str = "cancelled";
}

/// The message of an interrupted round.
pub const INTERRUPTED_MESSAGE: &str =
    "The round was interrupted: its job ended or its window closed before the developer answered. No answer exists. Stop the work that waited for it and do not use any recommendation.";

/// The message of a withdrawn round.
pub const CANCELLED_MESSAGE: &str = "The command was stopped before the developer answered; the round is withdrawn and no answer exists.";

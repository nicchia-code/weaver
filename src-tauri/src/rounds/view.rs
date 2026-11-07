//! What a question round tells the UI and what the UI does to it. The state itself, drafts
//! included, lives in [`weaver_core::question`]; these are its shapes on the wire.

use serde::{Deserialize, Serialize};
use ts_rs::TS;
use weaver_core::question::{Action, Draft, Origin, Position, Round, RoundRequest, Slot, Status};

/// Who asks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum OriginView {
    /// The agent of a job: the header says `Questions from the agent · <job name>`.
    Agent {
        #[serde(rename = "jobName")]
        job_name: String,
    },
    /// The agent of the project's Chat: the header says `Questions from the agent · Chat`.
    Chat,
    /// Weaver itself, for the prepared question of a Drift: `Question about <Drift id> · <title>`.
    Drift { drift: String, title: String },
}

/// What the index and the summary say of a question.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "kebab-case")]
pub enum StatusView {
    Answered,
    Skipped,
    Missing,
}

impl From<Status> for StatusView {
    fn from(status: Status) -> Self {
        match status {
            Status::Answered => StatusView::Answered,
            Status::Skipped => StatusView::Skipped,
            Status::Missing => StatusView::Missing,
        }
    }
}

/// The options chosen and the free text of a question.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct DraftView {
    /// Indexes of the chosen options, ascending.
    pub selected: Vec<usize>,
    pub text: String,
}

impl From<&Draft> for DraftView {
    fn from(draft: &Draft) -> Self {
        Self {
            selected: draft.selected.clone(),
            text: draft.text.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct OptionView {
    /// Plain text.
    pub label: String,
    /// Markdown.
    pub description: String,
    /// Markdown.
    pub details: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct QuestionView {
    pub id: String,
    /// Markdown.
    pub question: String,
    /// Markdown.
    pub context: Option<String>,
    pub options: Vec<OptionView>,
    pub recommended: usize,
    pub multi: bool,
    pub status: StatusView,
    /// What is on screen: kept by Weaver so that moving around loses nothing.
    pub draft: DraftView,
    /// What the developer confirmed, when he did; only this is ever sent.
    pub confirmed: Option<DraftView>,
    /// The question has been shown: the recommended option is preselected on the first visit only.
    pub visited: bool,
}

/// Where the developer is in a round.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum PositionView {
    Question { index: usize },
    Summary,
}

impl From<Position> for PositionView {
    fn from(position: Position) -> Self {
        match position {
            Position::Question { index } => PositionView::Question { index },
            Position::Summary => PositionView::Summary,
        }
    }
}

impl From<PositionView> for Position {
    fn from(position: PositionView) -> Self {
        match position {
            PositionView::Question { index } => Position::Question { index },
            PositionView::Summary => Position::Summary,
        }
    }
}

/// The round on screen in the window of a project.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct QuestionRoundView {
    #[ts(type = "number")]
    pub id: u64,
    /// The project hash.
    pub project_id: String,
    pub origin: OriginView,
    pub questions: Vec<QuestionView>,
    pub position: PositionView,
    pub minimized: bool,
    /// Rounds of the project that wait behind this one: `+<k> more round(s) waiting`.
    pub waiting: usize,
}

/// `question-round-changed`: the round on screen in a project changed because one opened, ended
/// or queued behind another. `round` is `None` when none is presented.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct QuestionRoundChanged {
    pub project_id: String,
    pub round: Option<QuestionRoundView>,
}

/// What the developer does in the presented round.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ActionInput {
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
    /// The bar at the bottom and the overlay.
    Minimize { minimized: bool },
    /// `Skip all`, after its confirmation.
    SkipAll,
    /// `Send to agent`.
    Send,
}

impl From<ActionInput> for Action {
    fn from(input: ActionInput) -> Self {
        match input {
            ActionInput::Go { position } => Action::Go(position.into()),
            ActionInput::Draft {
                index,
                selected,
                text,
            } => Action::Draft {
                index,
                selected,
                text,
            },
            ActionInput::Confirm { index } => Action::Confirm { index },
            ActionInput::Skip { index } => Action::Skip { index },
            ActionInput::Minimize { minimized } => Action::Minimize(minimized),
            ActionInput::SkipAll => Action::SkipAll,
            ActionInput::Send => Action::Send,
        }
    }
}

/// The questions of a round or of a card of Chat, each with what the developer did in it.
pub fn question_views(request: &RoundRequest, slots: &[Slot]) -> Vec<QuestionView> {
    request
        .questions
        .iter()
        .zip(slots)
        .map(|(question, slot)| QuestionView {
            id: question.id.clone(),
            question: question.question.clone(),
            context: question.context.clone(),
            options: question
                .options
                .iter()
                .map(|option| OptionView {
                    label: option.label.clone(),
                    description: option.description.clone(),
                    details: option.details.clone(),
                })
                .collect(),
            recommended: question.recommended,
            multi: question.multi,
            status: slot.status().into(),
            draft: (&slot.draft).into(),
            confirmed: slot.confirmed.as_ref().map(Into::into),
            visited: slot.visited,
        })
        .collect()
}

/// The view of `round`, with the name of its job when an agent asks.
pub fn view_of(
    project_id: &str,
    round: &Round,
    job_name: Option<String>,
    waiting: usize,
) -> QuestionRoundView {
    let origin = match &round.origin {
        Origin::Agent { job } => OriginView::Agent {
            job_name: job_name.unwrap_or_else(|| job.clone()),
        },
        Origin::Chat { .. } => OriginView::Chat,
        Origin::Drift { drift, title, .. } => OriginView::Drift {
            drift: drift.clone(),
            title: title.clone(),
        },
    };
    let questions = question_views(&round.request, &round.slots);
    QuestionRoundView {
        id: round.id,
        project_id: project_id.to_owned(),
        origin,
        questions,
        position: round.position.into(),
        minimized: round.minimized,
        waiting,
    }
}

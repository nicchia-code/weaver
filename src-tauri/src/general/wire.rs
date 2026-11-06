//! What General and the Ask more conversations tell the UI: their state, their conversation and the
//! changes of it.

use serde::Serialize;
use ts_rs::TS;

use crate::jobs::conversation::{Changed, Entry};

/// The state of the General of a project, as the view needs it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct GeneralState {
    /// The project hash.
    pub project_id: String,
    /// A turn is in progress (the Machine is in Working or Waiting for answers): `Stop` shows.
    pub working: bool,
    /// omp is starting or reloading its session (or is waited for): the message that waits says
    /// `Starting omp…`.
    pub starting: bool,
    /// The window just opened and the kept conversation is coming back from omp (the Machine is
    /// in Loading and its history is not in the conversation yet): the view says
    /// `Loading conversation…` above the messages sent meanwhile.
    pub loading: bool,
    /// A kept session has not come back yet, so `Clear` stays usable although the conversation
    /// shows nothing yet.
    pub kept: bool,
    /// The Chat waits for the answers of its round (the Machine is in Waiting for answers): the
    /// sidebar's Chat entry shows `1`, and a message sent meanwhile shows `Queued`.
    pub waiting: bool,
}

/// The conversation as it is now, with the count of change batches it holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct GeneralSnapshot {
    pub state: GeneralState,
    /// The Draft Proposals the agent of Chat created that `Clear` discards (`chat: true`, not
    /// refined): the Clear dialog says how many.
    pub chat_drafts: u32,
    #[ts(type = "number")]
    pub seq: u64,
    pub entries: Vec<Entry>,
}

/// A batch of changes of the conversation. `reset` says it was rebuilt (a session reloaded, or
/// cleared): the UI fetches the snapshot again.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct GeneralEntries {
    pub project_id: String,
    #[ts(type = "number")]
    pub seq: u64,
    pub reset: bool,
    pub changes: Vec<Changed>,
}

/// What an Ask more conversation was opened about; the placeholder of its empty message box.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "kebab-case")]
pub enum AskKind {
    Proposal,
    Task,
    Drift,
    History,
    Requirements,
    Question,
}

/// What an Ask more conversation does: `running` during a turn, `waiting` while a permission
/// waits for the developer, `idle` before the first message and between turns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "kebab-case")]
pub enum AskStatus {
    Idle,
    Running,
    Waiting,
}

/// A row of the Jobs view that is an Ask more conversation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AskRow {
    /// The identity of the conversation, generated at opening.
    pub id: String,
    /// The project hash.
    pub project_id: String,
    /// `Ask T3`, `Ask Requirements`, `Ask question`...
    pub name: String,
    pub kind: AskKind,
    /// The instant it was opened (UTC ISO 8601).
    pub opened: String,
    pub status: AskStatus,
    /// omp is starting or reloading its session: the message that waits says `Starting omp…`.
    pub starting: bool,
    /// The question round the conversation was opened from, for `Back to questions`.
    #[ts(type = "number | null")]
    pub round: Option<u64>,
}

/// The Ask more conversations of a project, newest first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AsksSnapshot {
    pub project_id: String,
    pub asks: Vec<AskRow>,
}

/// One Ask more conversation as it is now, with the count of change batches it holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AskSnapshot {
    pub row: AskRow,
    #[ts(type = "number")]
    pub seq: u64,
    pub entries: Vec<Entry>,
}

/// A batch of changes of an Ask more conversation (`reset`: the UI fetches the snapshot again).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AskEntries {
    pub project_id: String,
    pub id: String,
    #[ts(type = "number")]
    pub seq: u64,
    pub reset: bool,
    pub changes: Vec<Changed>,
}

/// An Ask more conversation closed: it leaves the Jobs view.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AskClosed {
    pub project_id: String,
    pub id: String,
}

/// Where the events of General and of the Ask more conversations go: the windows (the real one)
/// or a test.
pub trait GeneralEvents: Send + Sync + 'static {
    fn state(&self, state: &GeneralState);
    fn entries(&self, entries: &GeneralEntries);
    /// An Ask more conversation opened or changed its row.
    fn ask_changed(&self, row: &AskRow);
    fn ask_entries(&self, entries: &AskEntries);
    fn ask_closed(&self, closed: &AskClosed);
}

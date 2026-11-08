//! What the sub-agents of an owner (a job, or General) tell the UI: the list, the conversation of
//! one of them and the changes of it.
//!
//! A sub-agent is addressed by its path: the `task` call and the `id` of each sub-agent from the
//! owner down to it (`requirements/tech#job`, "Sub-agents").

use serde::{Deserialize, Serialize};
use ts_rs::TS;
use weaver_core::jobs::subagents::SubAgentState;

use crate::jobs::conversation::{Changed, Entry};

/// Whose sub-agents these are: a job, the General of the project, or an Ask more conversation.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Owner {
    Job { id: String },
    General,
    Ask { id: String },
}

/// One step of the path to a sub-agent: its `task` call and its `id`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Step {
    pub call: String,
    pub id: String,
}

/// A sub-agent of the list, with the ones it started.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AgentNode {
    /// The `task` call that launched it.
    pub call: String,
    pub id: String,
    /// omp's name for it.
    pub name: String,
    pub state: SubAgentState,
    /// UTC ISO 8601 instant it started, when known.
    pub started: Option<String>,
    /// The sub-agents it started, in launch order.
    pub children: Vec<AgentNode>,
}

/// The list of the sub-agents of an owner, as it is now. It is also the event that carries a
/// change of it: the whole list, so the UI never merges.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SubAgentsEvent {
    pub project_id: String,
    pub owner: Owner,
    /// The direct sub-agents of the owner, in launch order.
    pub nodes: Vec<AgentNode>,
}

/// The conversation of one sub-agent as it is now, with the count of change batches it holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SubAgentConversation {
    pub project_id: String,
    pub owner: Owner,
    pub path: Vec<Step>,
    #[ts(type = "number")]
    pub seq: u64,
    pub entries: Vec<Entry>,
    /// The instant each entry arrived (the instant its transcript record was written), in
    /// milliseconds since the Unix epoch, as `entries` is indexed.
    #[ts(type = "Array<number>")]
    pub arrivals: Vec<i64>,
    /// Why the transcript cannot be read (`Conversation not available: <reason>.`).
    pub unavailable: Option<String>,
}

/// A batch of changes of the conversation of a sub-agent. `reset` says it was rebuilt (a session
/// reloaded): the UI fetches the snapshot again.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SubAgentEntries {
    pub project_id: String,
    pub owner: Owner,
    pub path: Vec<Step>,
    #[ts(type = "number")]
    pub seq: u64,
    pub reset: bool,
    pub unavailable: Option<String>,
    pub changes: Vec<Changed>,
}

/// What an owner has to tell the UI after a change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outgoing {
    List(SubAgentsEvent),
    Entries(SubAgentEntries),
}

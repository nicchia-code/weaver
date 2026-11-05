//! What the Board needs to know about the agent's work, and cannot read from the project files:
//! the ghosts of the cards a job announced and has not filed yet, and a few facts (which jobs
//! work, whether a Run works).
//!
//! The jobs of the agent (task "Lavori dell'agente") implement [`AgentWork`] over their registry
//! and install it with [`AgentWorkSlot::set`]; until then [`NoAgentWork`] answers, truthfully, that
//! no job exists. The Board only reads it.
//!
//! Contract for the implementor, so that the Board stays right:
//! * `ghosts` lists the announcements of the project in announcement order, and a ghost leaves
//!   the list in the same step in which the card it announced is filed (the Board shows the ghost
//!   and the card of one snapshot together, never both and never neither). A setup or import job
//!   that has no announced ghost on the Board has one ghost of its own, made by Weaver, with the
//!   identity `job:<job id>`; it is listed in the place it would have if it had been announced
//!   when the job was created, and it leaves in the same step in which the first announced ghost
//!   of the job comes;
//! * after any change of what these methods answer, the jobs call
//!   [`crate::board::publish_agent_change`] so that the Board is sent again.

use std::sync::{Arc, RwLock};

use serde::Serialize;
use ts_rs::TS;
use weaver_core::project::board::AgentFacts;

/// The kind of card a job is preparing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "kebab-case")]
pub enum GhostKind {
    Proposal,
    Task,
    Drift,
}

/// How far the job is: the UI words it (`Preparing`, `Writing`, `Waiting for your answers`,
/// `Being refined`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "kebab-case")]
pub enum GhostPhase {
    /// The job announced the card; its Draft does not exist yet.
    Preparing,
    /// The Draft exists.
    Writing,
    /// The job waits for a round of questions or a permission.
    WaitingForAnswers,
    /// A refinement of an existing Proposal.
    BeingRefined,
}

/// A card an agent job announced and has not filed yet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Ghost {
    /// The identity of the announcement.
    pub id: String,
    /// The job whose conversation the ghost opens.
    pub job_id: String,
    pub kind: GhostKind,
    pub title: String,
    pub phase: GhostPhase,
    /// The job's latest note, when there is one.
    pub note: Option<String>,
}

pub trait AgentWork: Send + Sync {
    /// The ghosts of a project, in announcement order.
    fn ghosts(&self, project_id: &str) -> Vec<Ghost>;
    /// What is known about the work of the agent in a project.
    fn facts(&self, project_id: &str) -> AgentFacts;
}

/// No job exists: nothing is announced, nothing works.
pub struct NoAgentWork;

impl AgentWork for NoAgentWork {
    fn ghosts(&self, _project_id: &str) -> Vec<Ghost> {
        Vec::new()
    }

    fn facts(&self, _project_id: &str) -> AgentFacts {
        AgentFacts::default()
    }
}

/// The installed [`AgentWork`], replaceable at runtime by whoever owns the jobs.
pub struct AgentWorkSlot {
    current: RwLock<Arc<dyn AgentWork>>,
}

impl Default for AgentWorkSlot {
    fn default() -> Self {
        Self {
            current: RwLock::new(Arc::new(NoAgentWork)),
        }
    }
}

impl AgentWorkSlot {
    pub fn get(&self) -> Arc<dyn AgentWork> {
        self.current
            .read()
            .expect("agent work slot poisoned")
            .clone()
    }

    pub fn set(&self, work: Arc<dyn AgentWork>) {
        *self.current.write().expect("agent work slot poisoned") = work;
    }
}

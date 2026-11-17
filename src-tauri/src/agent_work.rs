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
//!   of the job comes. A Run that took a Task and holds none has, in the same way, a cleanup
//!   ghost (kind Task, identity `job:<job id>`) whenever the picture it is worked out against
//!   shows no Task in To do or In progress and the jobs have no announced ghost of a Task;
//! * after any change of what these methods answer, the jobs call
//!   [`crate::board::publish_agent_change`] so that the Board is sent again.

use std::sync::{Arc, RwLock};

use serde::Serialize;
use ts_rs::TS;
use weaver_core::project::board::AgentFacts;
use weaver_core::project::image::ProjectImage;

/// The kind of card a job is preparing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "kebab-case")]
pub enum GhostKind {
    Proposal,
    Task,
    Drift,
}

/// How far the job is: the UI words it (`Preparing`, `Writing`, `Waiting for your answers`,
/// `Being refined`, `Finishing up`).
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
    /// A Run that took its Tasks and holds none now: it finishes the work that follows them.
    FinishingUp,
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
    /// For the cleanup ghost of a Run (`requirements/ui#work`): the name of the Run, which the
    /// accessible name of the ghost carries. `None` for every other ghost.
    pub cleanup_of: Option<String>,
}

pub trait AgentWork: Send + Sync {
    /// The ghosts of a project in order, worked out against `image`, the picture the Board is
    /// made of: the cleanup ghost of a Run depends on which Tasks it shows.
    fn ghosts(&self, project_id: &str, image: &ProjectImage) -> Vec<Ghost>;
    /// What is known about the work of the agent in a project.
    fn facts(&self, project_id: &str) -> AgentFacts;
}

/// No job exists: nothing is announced, nothing works.
pub struct NoAgentWork;

impl AgentWork for NoAgentWork {
    fn ghosts(&self, _project_id: &str, _image: &ProjectImage) -> Vec<Ghost> {
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

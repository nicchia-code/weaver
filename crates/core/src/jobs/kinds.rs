//! The kinds and the states of a job (`requirements/data#job`, `requirements/ui#job`).

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// What a job is for. The serialised form is the one of the registry record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum JobKind {
    Setup,
    Design,
    Import,
    Scan,
    Run,
    Resolve,
    Rework,
    Refinement,
    /// The resolution of the conflicts of a merge in progress (`solo-merge`).
    Merge,
}

/// The model role a job runs on (`requirements/tech#job`): the role is set by the developer once
/// for all projects. An empty role leaves the choice to omp.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Analysis,
    Scan,
    Execution,
}

impl JobKind {
    pub const ALL: [JobKind; 9] = [
        JobKind::Setup,
        JobKind::Design,
        JobKind::Import,
        JobKind::Scan,
        JobKind::Run,
        JobKind::Resolve,
        JobKind::Rework,
        JobKind::Refinement,
        JobKind::Merge,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            JobKind::Setup => "setup",
            JobKind::Design => "design",
            JobKind::Import => "import",
            JobKind::Scan => "scan",
            JobKind::Run => "run",
            JobKind::Resolve => "resolve",
            JobKind::Rework => "rework",
            JobKind::Refinement => "refinement",
            JobKind::Merge => "merge",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.as_str() == text)
    }

    /// The model role of the kind: analysis for everything that writes cards, the scan model for
    /// scans, the execution model for Runs.
    pub fn role(self) -> Role {
        match self {
            JobKind::Scan => Role::Scan,
            JobKind::Run => Role::Execution,
            _ => Role::Analysis,
        }
    }

    /// Whether the job's skill hands work to `weaver-analysis` sub-agents, which run on omp's slow
    /// role: only the scan does.
    pub fn uses_analysis_agents(self) -> bool {
        self == JobKind::Scan
    }

    /// Whether the job is a Run (it carries `WEAVER_RUN`, has the advisor, excludes a resolve).
    pub fn is_run(self) -> bool {
        self == JobKind::Run
    }

    /// Whether Weaver measures the job when it ends (`requirements/data#job`): design, rework,
    /// refinement and Run.
    pub fn is_measured(self) -> bool {
        matches!(
            self,
            JobKind::Design | JobKind::Rework | JobKind::Refinement | JobKind::Run
        )
    }
}

/// The state of a job (the Machine `job`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum JobState {
    /// Created, waiting for omp to be ready and to open its session.
    Queued,
    Running,
    /// Waiting for the developer: a round of questions or a permission.
    Waiting,
    /// The window closed or Weaver quit while the job worked; omp was ended.
    Suspended,
    Done,
    Failed,
    Stopped,
}

impl JobState {
    pub fn as_str(self) -> &'static str {
        match self {
            JobState::Queued => "queued",
            JobState::Running => "running",
            JobState::Waiting => "waiting",
            JobState::Suspended => "suspended",
            JobState::Done => "done",
            JobState::Failed => "failed",
            JobState::Stopped => "stopped",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        [
            JobState::Queued,
            JobState::Running,
            JobState::Waiting,
            JobState::Suspended,
            JobState::Done,
            JobState::Failed,
            JobState::Stopped,
        ]
        .into_iter()
        .find(|state| state.as_str() == text)
    }

    /// Done, Failed and Stopped.
    pub fn is_final(self) -> bool {
        matches!(self, JobState::Done | JobState::Failed | JobState::Stopped)
    }

    /// The job still owns what it holds: every state but the final ones. A card it handed over
    /// can be decided or run only when this is false.
    pub fn is_active(self) -> bool {
        !self.is_final()
    }

    /// The jobs that count as working for the confirmation of closing a window: the agent works or
    /// waits for the developer.
    pub fn is_working(self) -> bool {
        matches!(self, JobState::Running | JobState::Waiting)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_and_states_round_trip_through_their_names() {
        for kind in JobKind::ALL {
            assert_eq!(JobKind::parse(kind.as_str()), Some(kind));
            assert_eq!(
                serde_json::to_value(kind).unwrap(),
                serde_json::json!(kind.as_str())
            );
        }
        assert_eq!(JobKind::parse("refine"), None);
        assert_eq!(JobState::parse("waiting"), Some(JobState::Waiting));
        assert_eq!(
            serde_json::to_value(JobState::Suspended).unwrap(),
            serde_json::json!("suspended")
        );
    }

    #[test]
    fn roles_follow_the_kind_and_only_the_scan_uses_analysis_agents() {
        assert_eq!(JobKind::Scan.role(), Role::Scan);
        assert_eq!(JobKind::Run.role(), Role::Execution);
        for kind in [
            JobKind::Setup,
            JobKind::Design,
            JobKind::Import,
            JobKind::Resolve,
            JobKind::Rework,
            JobKind::Refinement,
            JobKind::Merge,
        ] {
            assert_eq!(kind.role(), Role::Analysis);
        }
        assert!(JobKind::Scan.uses_analysis_agents() && !JobKind::Run.uses_analysis_agents());
        assert!(!JobKind::Merge.uses_analysis_agents() && !JobKind::Merge.is_run());
    }

    #[test]
    fn which_states_own_holdings_and_which_count_as_working() {
        assert!(JobState::Suspended.is_active() && JobState::Queued.is_active());
        assert!(
            JobState::Done.is_final()
                && JobState::Failed.is_final()
                && JobState::Stopped.is_final()
        );
        assert!(JobState::Waiting.is_working() && JobState::Running.is_working());
        assert!(!JobState::Suspended.is_working() && !JobState::Queued.is_working());
    }
}

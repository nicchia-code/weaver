//! What the jobs tell the rest of the app and the UI: the rows of the Jobs list, the changes of a
//! conversation, and the hooks other Tasks plug into.

use std::path::PathBuf;
use std::sync::Arc;

use serde::Serialize;
use ts_rs::TS;
use weaver_core::jobs::notes::Note;
use weaver_core::jobs::{AnnouncementRecord, JobKind, JobState};

use super::conversation::{Changed, Entry};
use super::registry::Jobs;

/// A row of the Jobs list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct JobRow {
    pub id: String,
    pub kind: JobKind,
    /// The fixed name set at creation.
    pub name: String,
    pub state: JobState,
    /// Why a Failed job failed.
    pub reason: Option<String>,
    /// UTC ISO 8601 instants: when the job was created, when its session started (none while
    /// queued) and when it ended.
    pub created: String,
    pub started: Option<String>,
    pub ended: Option<String>,
    /// The latest note.
    pub note: Option<String>,
    pub target: Option<String>,
    /// For a setup or design job, the request it received.
    pub request: Option<String>,
    /// That request exists and is still sent to this job (`View request` shows only then).
    pub request_sent: bool,
}

/// The Jobs list of a project, in the order the view shows it: waiting, running, queued,
/// suspended (each newest start first), then the finished jobs, latest end first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct JobsSnapshot {
    pub project_id: String,
    pub jobs: Vec<JobRow>,
    /// Whether omp is ready: a Queued job says `Waiting for omp to be ready.` when it is not.
    pub omp_ready: bool,
}

/// The conversation of a job as it is now, with the count of change batches it holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ConversationSnapshot {
    pub job_id: String,
    #[ts(type = "number")]
    pub seq: u64,
    pub entries: Vec<Entry>,
    /// The notes of the job, with their instants, for the Jobs list tooltip.
    pub notes: Vec<Note>,
}

/// A batch of changes of a conversation. `reset` says the conversation was rebuilt (a session
/// reloaded): the UI fetches the snapshot again.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct EntriesEvent {
    pub project_id: String,
    pub job_id: String,
    #[ts(type = "number")]
    pub seq: u64,
    pub reset: bool,
    pub changes: Vec<Changed>,
}

/// A job, as the other parts of the app see it when it waits or ends.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobInfo {
    pub id: String,
    /// The project hash.
    pub project_id: String,
    pub root: PathBuf,
    pub kind: JobKind,
    pub name: String,
    pub state: JobState,
    pub target: Option<String>,
    pub reason: Option<String>,
    /// The cards it announced and that never arrived.
    pub announcements: Vec<AnnouncementRecord>,
    /// A setup or an import job that found the Model empty when it started working.
    pub empty_model_at_start: bool,
}

/// Who wants to know when a job waits or ends (`job.WAITING`, `job.ENDED`). Installed with
/// [`super::Jobs::observe`]. The methods are called outside every lock and must not block.
///
/// `job.ENDED` is received by: the question rounds of the job (Interrupted), the notifications
/// (Shown, Withdrawn), the design request the job worked on (Sent → Writing), the Proposal it
/// refines (Refining → Draft); the Tasks it held go back to To do in the same step, by the jobs.
pub trait JobObserver: Send + Sync {
    fn job_waiting(&self, _job: &JobInfo) {}
    fn job_ended(&self, _job: &JobInfo) {}
    /// `question-round.OPENED`: a round of this job is on screen and the job waits for it.
    fn round_opened(&self, _job: &JobInfo) {}
    /// `question-round.CLOSED`: the round of this job on screen ended.
    fn round_closed(&self, _job: &JobInfo) {}
    /// The job asks the developer for a permission (`request` is the id of the request in its conversation).
    fn permission_asked(&self, _job: &JobInfo, _request: u64) {}
    /// `developer.ANSWER_PERMISSION`: the permission was answered.
    fn permission_answered(&self, _job: &JobInfo, _request: u64) {}
}

impl Jobs {
    /// Tells the observers about one job, outside every lock: `tell` is called with each of them.
    /// With `only_waiting`, nothing is told when the job is not waiting for the developer (the round
    /// or the permission did not make it wait: it ended, or was never working).
    pub(crate) fn tell(
        &self,
        job_id: &str,
        only_waiting: bool,
        tell: impl Fn(&dyn JobObserver, &JobInfo),
    ) {
        let Some(info) = self.info(job_id) else {
            return;
        };
        if only_waiting && info.state != JobState::Waiting {
            return;
        }
        let observers: Vec<Arc<dyn JobObserver>> =
            self.observers.read().expect("observers poisoned").clone();
        for observer in &observers {
            tell(observer.as_ref(), &info);
        }
    }
}

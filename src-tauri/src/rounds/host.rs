//! The real [`RoundsHost`]: the jobs of the agent, the window manager and the events to the UI.

use std::sync::Arc;

use tauri::{AppHandle, Emitter, Manager};

use super::service::{JobFacts, RoundsHost};
use super::view::QuestionRoundChanged;
use crate::events::EventName;
use crate::general::General;
use crate::jobs::Jobs;
use crate::{log_warn, windows};

pub struct TauriRoundsHost {
    app: AppHandle,
    jobs: Arc<Jobs>,
}

impl TauriRoundsHost {
    pub fn new(app: AppHandle, jobs: Arc<Jobs>) -> Self {
        Self { app, jobs }
    }

    /// The Chat of the projects: installed after the rounds, so it is looked up when needed.
    fn general(&self) -> Option<Arc<General>> {
        self.app
            .try_state::<Arc<General>>()
            .map(|general| general.inner().clone())
    }
}

impl RoundsHost for TauriRoundsHost {
    fn job(&self, job: &str) -> Option<JobFacts> {
        let info = self.jobs.info(job)?;
        let accepting = self.jobs.accepts_job(&info.root, job);
        Some(JobFacts {
            project_id: info.project_id,
            name: info.name,
            accepting,
        })
    }

    fn round_opened(&self, job: &str) {
        self.jobs.round_opened(job);
        // A round on screen makes the job wait: the notifications hear of it before the window comes forward.
        self.jobs
            .tell(job, true, |observer, info| observer.round_opened(info));
    }

    fn round_closed(&self, job: &str) {
        self.jobs.round_closed(job);
        self.jobs
            .tell(job, false, |observer, info| observer.round_closed(info));
    }

    fn chat(&self, token: &str) -> Option<String> {
        self.general()?.chat_of(token)
    }

    fn chat_round_opened(&self, project_id: &str) {
        if let Some(general) = self.general() {
            general.round_opened(project_id);
        }
    }

    fn chat_round_closed(&self, project_id: &str) {
        if let Some(general) = self.general() {
            general.round_closed(project_id);
        }
    }

    fn focus_project(&self, project_id: &str) {
        if let Some(manager) = windows::manager_of(&self.app) {
            manager.focus_project(project_id, None);
        }
    }

    fn changed(&self, change: &QuestionRoundChanged) {
        if let Err(error) = self
            .app
            .emit(EventName::QuestionRoundChanged.as_str(), change)
        {
            // The round itself is unaffected; the window shows it at its next fetch.
            log_warn!("the change of a question round could not be sent to the window: {error}");
        }
    }
}

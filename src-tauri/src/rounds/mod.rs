//! Question rounds (Machine `question-round`): the app side.
//!
//! A group of decisions the agent of a job asks the developer at once, or the prepared question of
//! a Drift that Weaver asks itself. The decisions are in `weaver_core::question`; here live the
//! queues of the open projects, the agents waiting for their results, and what a round does to the
//! jobs and the window. The agent of the project's Chat asks no round: its card of questions is
//! published in its conversation and kept there ([`crate::general::cards`]), never queued, never
//! shown in the overlay and never held by anyone.
//!
//! | Module      | Responsibility |
//! |-------------|----------------|
//! | [`service`] | [`Rounds`]: the queue of each project, the waiters, the effects |
//! | [`view`]    | What the UI sees of a round and what it does to it |
//! | [`commands`]| The Tauri commands: the round on screen, an action of the developer |
//! | `host`      | The jobs, the windows and the events behind [`service::RoundsHost`] |
//!
//! Hooks for the other parts of the app: `Rounds::handle` answers the request kind `question.new`
//! ([`crate::cli_handler`]); `Rounds::present_prepared` asks the prepared question of a Drift.
//! Round state, drafts included, lives only in memory and is never logged.

pub mod commands;
mod host;
pub mod service;
pub mod view;

use std::sync::Arc;

use tauri::{AppHandle, Manager};

pub use service::Rounds;

use crate::jobs::wire::{JobInfo, JobObserver};
use crate::jobs::Jobs;
use crate::windows::jobs::{BoxFuture, CloseReason, WindowJobs, WindowOpened};

/// `job.ENDED`: the rounds of the job are interrupted.
struct RoundsObserver(Arc<Rounds>);

impl JobObserver for RoundsObserver {
    fn job_ended(&self, job: &JobInfo) {
        self.0.job_ended(&job.project_id, &job.id);
    }
}

/// The rounds plugged into the window hook: a queue is created when a project opens, and its
/// rounds are interrupted when the window closes, before the jobs are suspended.
struct RoundsWindows {
    inner: Arc<dyn WindowJobs>,
    rounds: Arc<Rounds>,
}

impl WindowJobs for RoundsWindows {
    fn working_jobs(&self, project_id: &str) -> usize {
        self.inner.working_jobs(project_id)
    }

    fn window_opened(&self, event: &WindowOpened) {
        self.rounds.window_opened(&event.project_id);
        self.inner.window_opened(event);
    }

    fn window_closed(&self, project_id: &str, reason: CloseReason) -> BoxFuture<()> {
        self.rounds.window_closed(project_id);
        self.inner.window_closed(project_id, reason)
    }
}

/// The window hook with the rounds added to `inner`.
pub fn with_windows(inner: Arc<dyn WindowJobs>, rounds: Arc<Rounds>) -> Arc<dyn WindowJobs> {
    Arc::new(RoundsWindows { inner, rounds })
}

/// Creates the rounds and plugs them into the jobs. Call from the installation of the jobs.
pub fn install(app: &AppHandle, jobs: &Arc<Jobs>) -> Arc<Rounds> {
    let rounds = Rounds::new(Arc::new(host::TauriRoundsHost::new(
        app.clone(),
        jobs.clone(),
    )));
    app.manage(rounds.clone());
    jobs.observe(Arc::new(RoundsObserver(rounds.clone())));
    rounds
}

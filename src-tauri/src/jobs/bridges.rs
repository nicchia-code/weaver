//! The jobs plugged into the rest of the app: the Board's [`AgentWork`], the windows' hook, the
//! Proposals' hook, the follow-ups of `job.ENDED` that belong to the Proposals, and [`install`].

use std::path::Path;
use std::sync::Arc;

use tauri::{AppHandle, Manager};
use weaver_cli::spec::{Call, Failure};
use weaver_core::command::CommandReply;
use weaver_core::jobs::JobKind;
use weaver_core::project::board::AgentFacts;
use weaver_core::project::clock::{now_instant, SystemClock};
use weaver_core::project::image::ProjectImage;
use weaver_core::project::proposal::transitions::{self, SubmitContext};
use weaver_core::project::random::OsRandom;

use super::authority::{AppAuthority, CommandExtension};
use super::host::{MutationResult, TauriHost};
use super::registry::Jobs;
use super::starts::Ctx;
use super::wire::{JobInfo, JobObserver};
use crate::agent_work::{AgentWork, Ghost};
use crate::proposals::ProposalJobs;
use crate::state::AppState;
use crate::windows::jobs::{BoxFuture, CloseReason, WindowJobs, WindowOpened};
use crate::windows::WindowManager;
use crate::{log_info, log_warn};

/// What the Board knows about the agent's work: the ghosts and the facts.
pub struct JobsWork(pub Arc<Jobs>);

impl AgentWork for JobsWork {
    fn ghosts(&self, project_id: &str) -> Vec<Ghost> {
        self.0.ghosts_of(project_id)
    }

    fn facts(&self, project_id: &str) -> AgentFacts {
        self.0.facts_of(project_id)
    }
}

/// What the window manager asks of the jobs.
pub struct JobsWindows(pub Arc<Jobs>);

impl WindowJobs for JobsWindows {
    fn working_jobs(&self, project_id: &str) -> usize {
        self.0.working_count(project_id)
    }

    fn window_opened(&self, event: &WindowOpened) {
        self.0.window_opened(&event.project_id, &event.root);
    }

    fn window_closed(&self, project_id: &str, _reason: CloseReason) -> BoxFuture<()> {
        Box::pin(self.0.window_closed(project_id))
    }
}

/// Runs `start` in the project's queue, on a fresh picture; a refusal is logged (the decision that
/// asked has been applied already and has no one to tell).
fn start_later(
    jobs: &Arc<Jobs>,
    project_id: &str,
    what: &'static str,
    start: impl FnOnce(&Jobs, &Ctx) -> Result<(), String> + Send + 'static,
) {
    let jobs = jobs.clone();
    let project = project_id.to_owned();
    tauri::async_runtime::spawn(async move {
        let worker = jobs.clone();
        let id = project.clone();
        let result = jobs
            .host()
            .mutate(
                &project,
                Box::new(move |root| -> MutationResult {
                    let image = ProjectImage::load(root);
                    start(
                        &worker,
                        &Ctx {
                            project_id: &id,
                            root,
                            image: &image,
                        },
                    )?;
                    Ok(None)
                }),
            )
            .await;
        if let Err(message) = result {
            log_warn!("{what} did not start: {message}");
        }
    });
}

/// What a decision on a Proposal starts.
pub struct JobsProposals(pub Arc<Jobs>);

impl ProposalJobs for JobsProposals {
    fn can_start(&self, _project_id: &str) -> Result<(), String> {
        // Reworks and refinements always start: nothing excludes them.
        Ok(())
    }

    fn start_refinement(&self, project_id: &str, proposal_id: &str) {
        let proposal = proposal_id.to_owned();
        start_later(&self.0, project_id, "a refinement", move |jobs, ctx| {
            jobs.start_refinement(ctx, &proposal)
                .map(|_| ())
                .map_err(|refusal| refusal.to_string())
        });
    }

    fn start_rework(&self, project_id: &str, proposal_id: &str) {
        let proposal = proposal_id.to_owned();
        start_later(&self.0, project_id, "a rework", move |jobs, ctx| {
            jobs.start_rework(ctx, &proposal)
                .map(|_| ())
                .map_err(|refusal| refusal.to_string())
        });
    }

    fn run_all_after_setup(&self, project_id: &str) {
        start_later(&self.0, project_id, "Run all", |jobs, ctx| {
            jobs.start_run_all_after_setup(ctx);
            Ok(())
        });
    }

    fn request_consumed(&self, project_id: &str) {
        self.0.rows_changed(project_id);
    }
}

/// What `job.ENDED` means for the Proposals: a refinement that did not hand its Proposal over
/// sends it back to Draft, and when the last Run ends the Proposals refined during it are handed
/// over again.
struct ProposalFollowUp {
    jobs: Arc<Jobs>,
}

impl JobObserver for ProposalFollowUp {
    fn job_ended(&self, job: &JobInfo) {
        match job.kind {
            JobKind::Refinement => {
                let Some(proposal) = job.target.clone() else {
                    return;
                };
                self.mutate(job, "the end of a refinement", move |root| {
                    let image = ProjectImage::load(root);
                    let changes = transitions::plan_refinement_ended(&image, &proposal);
                    if changes.is_empty() {
                        return Ok(None);
                    }
                    weaver_core::project::changes::apply(root, &changes, |_| Ok::<(), ()>(()))
                        .map(Some)
                        .map_err(|error| error.to_string())
                });
            }
            JobKind::Run => {
                let jobs = self.jobs.clone();
                let project = job.project_id.clone();
                self.mutate(job, "the hand-over of the refined Proposals", move |root| {
                    if jobs.facts_of(&project).run_in_progress {
                        return Ok(None);
                    }
                    let image = ProjectImage::load(root);
                    let working = |proposal: &str| {
                        jobs.active_ids(&project).iter().any(|id| {
                            jobs.info(id).is_some_and(|info| {
                                info.kind == JobKind::Refinement
                                    && info.target.as_deref() == Some(proposal)
                            })
                        })
                    };
                    let waiting: Vec<String> = image
                        .cards_of(weaver_core::project::cards::CardKind::Proposal)
                        .filter(|card| {
                            card.str_key("status") == Some("refining") && !working(&card.id)
                        })
                        .map(|card| card.id.clone())
                        .collect();
                    let mut published = None;
                    for id in waiting {
                        let now = now_instant(&SystemClock).map_err(|error| error.to_string())?;
                        let ctx = SubmitContext {
                            now: &now,
                            job_found_empty_model: false,
                            run_in_progress: false,
                        };
                        match transitions::submit(root, &id, &ctx, &mut OsRandom) {
                            Ok((image, _)) => published = Some(image),
                            Err(error) => log_warn!(
                                "the refined Proposal {id} could not be handed over again: {error}"
                            ),
                        }
                    }
                    Ok(published)
                });
            }
            _ => {}
        }
    }
}

impl ProposalFollowUp {
    fn mutate(
        &self,
        job: &JobInfo,
        what: &'static str,
        change: impl FnOnce(&Path) -> MutationResult + Send + 'static,
    ) {
        let host = self.jobs.host().clone();
        let project = job.project_id.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(message) = host.mutate(&project, Box::new(change)).await {
                log_warn!("{what} could not be applied: {message}");
            }
        });
    }
}

/// The commands of the Proposals that need the app (`weaver proposal move <id> approved|rejected|open`).
struct ProposalCli {
    app: AppHandle,
}

impl CommandExtension for ProposalCli {
    fn run(&self, call: &Call) -> Option<Result<CommandReply, Failure>> {
        let state = self.app.state::<AppState>();
        crate::proposals::answer_cli(call, state.proposal_jobs.get().as_ref())
    }
}

/// Creates the jobs, plugs them into the app, and returns them. Call from the setup of the app,
/// before the windows and the CLI socket start.
pub fn install(app: &AppHandle) -> Arc<Jobs> {
    let host = Arc::new(TauriHost::new(app.clone()));
    let jobs = Jobs::new(host);
    app.manage(jobs.clone());
    let state = app.state::<AppState>();
    state.agent_work.set(Arc::new(JobsWork(jobs.clone())));
    state
        .proposal_jobs
        .set(Arc::new(JobsProposals(jobs.clone())));
    jobs.observe(Arc::new(ProposalFollowUp { jobs: jobs.clone() }));
    jobs.add_extension(Arc::new(ProposalCli { app: app.clone() }));
    crate::rounds::install(app, &jobs);

    // What omp's readiness changes: a Queued job says `Waiting for omp to be ready.` or not.
    let watcher = jobs.clone();
    let mut ready = app.state::<crate::omp::OmpSetup>().subscribe();
    tauri::async_runtime::spawn(async move {
        while ready.changed().await.is_ok() {
            watcher.omp_changed();
        }
    });
    crate::general::install(app, &jobs);
    log_info!("the jobs of the agent are ready");
    jobs
}

/// Plugs the jobs into the window manager. Call right after the manager is created, before it
/// restores the projects.
pub fn install_windows(app: &AppHandle, manager: &Arc<WindowManager>) {
    if let Some(jobs) = app.try_state::<Arc<Jobs>>() {
        let hook: Arc<dyn WindowJobs> = Arc::new(JobsWindows(jobs.inner().clone()));
        let hook = match app.try_state::<Arc<crate::general::General>>() {
            Some(general) => crate::general::with_windows(hook, general.inner().clone()),
            None => hook,
        };
        // The rounds of questions come first: they are interrupted before the jobs are suspended.
        let hook = match app.try_state::<Arc<crate::rounds::Rounds>>() {
            Some(rounds) => crate::rounds::with_windows(hook, rounds.inner().clone()),
            None => hook,
        };
        manager.set_jobs(hook);
    }
}

/// The authority of the CLI socket.
pub fn app_authority(app: &AppHandle) -> AppAuthority {
    AppAuthority {
        jobs: app.state::<Arc<Jobs>>().inner().clone(),
    }
}

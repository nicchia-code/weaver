//! Merge of Solo mode in the app (`requirements/tech#solo-merge`): the state `ProjectStatus.merge`
//! carries, its recomputation in the project's background work, and the commands the Board calls.
//!
//! The rules and every git step are in `weaver_core::solo_merge`; this module runs them in the
//! project's mutation queue, under the project lock, off the UI and IPC threads, and tells the
//! windows what changed:
//!
//! | Event of the Machine | Where |
//! |----------------------|-------|
//! | the Board is shown | `merge_refresh` ([`ProjectSetup::merge_refresh`]) |
//! | `window.OPENED` | the opening run of [`ProjectSetup`] |
//! | `desktop.WINDOW_FOCUSED` | [`ProjectSetup::window_focused`] (`WindowEvent::Focused(true)`) |
//! | `job.ENDED` | [`JobEnded`] |
//! | `developer.MERGE_BRANCH` | `merge_start` ([`ProjectSetup::merge_start`]) |
//! | `developer.ABORT_MERGE` | `merge_abort` ([`ProjectSetup::merge_abort`]) |
//! | `developer.RESOLVE_MERGE_CONFLICTS` | `resolve_merge_conflicts` ([`ProjectSetup::resolve_merge_conflicts`]) |
//! | `workspace.PROJECT_FOLDER_REMOVED` | the memory of the merge goes with `.weaver/`: the next batch clears the status; git's merge stays and no scan starts |
//!
//! The step that finds a merge concluded (`solo-merge.CONCLUDED`) has `merge.json` deleted by the
//! core and, in the same step, starts the scan of the whole System through the jobs.

use std::path::PathBuf;
use std::sync::Arc;

use serde::Serialize;
use tauri::State;
use ts_rs::TS;
use weaver_core::solo_merge::{
    self, Aborted, Concluded, Ended, InProgress, Refreshed, Refusal, Started,
};
use weaver_core::solo_mode::{self, Git};

use super::{locked, Environment, ProjectSetup};
use crate::error::{AppError, ErrorCode};
use crate::jobs::wire::{JobInfo, JobObserver};
use crate::{log_info, log_warn};

/// A merge with conflicts in progress, as the Board shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct MergeProgress {
    /// The branch being merged, as the developer chose it.
    pub branch: String,
    /// The branch it goes into.
    pub current: String,
    /// How many files still have conflicts.
    pub conflicts: u32,
}

/// Merge as the window knows it (`requirements/tech#solo-merge`, **State in the app**).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct MergeStatus {
    /// Merge can start now.
    pub available: bool,
    /// Why it cannot, as the sentence the Board shows (`requirements/ui#solo-merge`).
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub reason: Option<String>,
    /// The short name of the current branch, whenever the checks got as far as HEAD (always when
    /// `available`).
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub current: Option<String>,
    /// The merge with conflicts that Weaver follows, while `merge.json` exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub in_progress: Option<MergeProgress>,
}

/// Something to say at the top of the Board about a merge command: `text` is the whole sentence
/// and `paths` the files it names, shown after it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct MergeNotice {
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub paths: Option<Vec<String>>,
}

impl MergeNotice {
    fn text(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            paths: None,
        }
    }
}

/// What `merge_start` and `merge_abort` answer: the new state and, when there is something to say,
/// the notice.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct MergeAnswer {
    pub merge: MergeStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub notice: Option<MergeNotice>,
}

/// What the Dialog of Merge offers, when it opens.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct MergeBranches {
    /// The current branch.
    pub current: String,
    /// The local branches but the current one, sorted by name.
    pub local: Vec<String>,
    /// The remote branches git already knows (never fetched), sorted by name.
    pub remote: Vec<String>,
    /// Weaver's files git still tracks, relative to the project folder: their changes go into the
    /// merge commit. The reminder shows when the list is not empty.
    pub tracked: Vec<String>,
}

/// What the project's jobs say now.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AgentWork {
    /// A job of the project is Queued, Running, Waiting for the developer or Suspended.
    pub busy: bool,
    /// A `merge` job of the project has not ended.
    pub merge_job_unended: bool,
}

/// What one recomputation of Merge's state found.
pub(super) struct MergeWork {
    /// `None` when Solo mode is off: the project has no Merge.
    pub status: Option<MergeStatus>,
}

/// What the work of a merge step needs, inside the project's queue.
pub(super) struct Ctx {
    pub env: Arc<dyn Environment>,
    pub git: Git,
    pub project_id: String,
    pub root: PathBuf,
    /// The state the window had: the count of conflicts it showed stays when git cannot say.
    pub previous: Option<MergeStatus>,
}

fn count(number: usize) -> u32 {
    u32::try_from(number).unwrap_or(u32::MAX)
}

/// The first line of a git error as a sentence part: no final `.` or `:` of its own.
fn clause(line: &str) -> &str {
    line.trim().trim_end_matches(['.', ':', ' '])
}

fn lower_first(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_lowercase().chain(chars).collect(),
        None => String::new(),
    }
}

impl Ctx {
    fn progress(&self, found: &InProgress) -> MergeProgress {
        let before = self
            .previous
            .as_ref()
            .and_then(|status| status.in_progress.as_ref())
            .map(|progress| progress.conflicts);
        MergeProgress {
            branch: found.branch.clone(),
            current: found.current.clone(),
            conflicts: found.conflicts.map(count).or(before).unwrap_or(0),
        }
    }

    fn status_of(&self, refreshed: &Refreshed) -> MergeStatus {
        MergeStatus {
            available: refreshed.unavailable.is_none(),
            reason: refreshed.unavailable.as_ref().map(ToString::to_string),
            current: refreshed.current.clone(),
            in_progress: refreshed
                .in_progress
                .as_ref()
                .map(|found| self.progress(found)),
        }
    }

    /// Merge needs Solo mode: without it the project has no Merge.
    fn solo_is_on(&self) -> bool {
        solo_mode::read_state(&self.root).mode.is_on()
    }

    /// `solo-merge.CONCLUDED`: the merge ended and `merge.json` is deleted; the scan of the whole
    /// System starts in the same step, without a confirmation. A scan that cannot start is a notice.
    fn concluded(&self, concluded: &Concluded) {
        log_info!(
            "a merge was concluded with {} files: the scan starts",
            concluded.files.len()
        );
        if let Err(reason) =
            self.env
                .start_scan_after_merge(&self.project_id, &self.root, concluded.files.clone())
        {
            log_warn!("the scan after a merge did not start: {reason}");
            self.env.notice(
                &self.project_id,
                &format!(
                    "Could not scan the System after the merge of {}: {reason}.",
                    concluded.branch
                ),
            );
        }
    }

    /// A merge that ended without a commit: forgotten, no scan, said on the Board.
    fn forgotten(
        &self,
        branch: Option<&str>,
        diagnostic: Option<&weaver_core::project::diagnostic::Diagnostic>,
    ) {
        if let Some(diagnostic) = diagnostic {
            log_warn!(
                "{} cannot be followed: {}",
                diagnostic.path,
                diagnostic.message
            );
        }
        log_info!("a merge ended without a commit: forgotten, no scan");
        let text = match branch {
            Some(branch) => format!(
                "The merge of {branch} ended without a commit: Weaver forgot it and did not scan."
            ),
            None => "A merge ended without a commit: Weaver forgot it and did not scan.".to_owned(),
        };
        self.env.notice(&self.project_id, &text);
    }

    /// The check of a merge in progress and the availability, then what follows from them: the scan
    /// of a concluded merge, the notice of a forgotten one, and the Board told when `merge.json`
    /// appeared or went.
    pub(super) fn recompute(&self) -> MergeWork {
        if !self.solo_is_on() {
            return MergeWork { status: None };
        }
        let before = solo_merge::in_progress(&self.root);
        let busy = self.env.agent_work(&self.project_id).busy;
        let refreshed = solo_merge::refresh(&self.git, &self.root, busy);
        if let Some(reason) = &refreshed.failure {
            log_warn!("the merge in progress could not be checked: {reason}");
        }
        let mut status = self.status_of(&refreshed);
        match &refreshed.ended {
            Some(Ended::Concluded(concluded)) => {
                self.concluded(concluded);
                status = self.after_scan(status);
            }
            Some(Ended::Forgotten { branch, diagnostic }) => {
                self.forgotten(branch.as_deref(), diagnostic.as_ref());
            }
            None => {}
        }
        if before != solo_merge::in_progress(&self.root) {
            self.env.board_changed(&self.project_id);
        }
        MergeWork {
            status: Some(status),
        }
    }

    /// A scan started: Merge is not available while the agent works. No git is called: the
    /// availability stops at the jobs.
    fn after_scan(&self, mut status: MergeStatus) -> MergeStatus {
        let availability = solo_merge::availability(
            &self.git,
            &self.root,
            self.env.agent_work(&self.project_id).busy,
        );
        status.available = availability.unavailable.is_none();
        status.reason = availability.unavailable.map(|reason| reason.to_string());
        status.current = availability.current.or(status.current);
        status
    }

    /// `developer.MERGE_BRANCH`.
    fn start(&self, branch: &str) -> Result<MergeAnswer, String> {
        if !self.solo_is_on() {
            return Err(format!(
                "Could not merge {branch}: Merge is available only while Solo mode is on."
            ));
        }
        let busy = self.env.agent_work(&self.project_id).busy;
        let started = solo_merge::start(&self.git, &self.root, branch, busy);
        let mut notice = None;
        match started {
            Started::UpToDate { current, branch } => {
                notice = Some(MergeNotice::text(format!(
                    "{current} is already up to date with {branch}."
                )));
            }
            Started::Concluded(concluded) => {
                self.concluded(&concluded);
                self.env.board_changed(&self.project_id);
            }
            Started::Merging(_) => {
                log_info!("a merge stopped on conflicts: Weaver follows it");
                self.env.board_changed(&self.project_id);
            }
            Started::Refused(refusal) => {
                log_info!("a merge was refused: {refusal:?}");
                notice = Some(refusal_notice(branch, &refusal));
            }
        }
        let status = self.recompute().status.ok_or_else(|| {
            format!("Could not merge {branch}: Merge is available only while Solo mode is on.")
        })?;
        Ok(MergeAnswer {
            merge: status,
            notice,
        })
    }

    /// `developer.ABORT_MERGE`.
    fn abort(&self) -> Result<MergeAnswer, String> {
        if !self.solo_is_on() {
            return Err(
                "Could not abort the merge: Merge is available only while Solo mode is on."
                    .to_owned(),
            );
        }
        let work = self.env.agent_work(&self.project_id);
        let mut notice = None;
        match solo_merge::abort(&self.git, &self.root, work.merge_job_unended) {
            Aborted::Done => {
                log_info!("a merge was aborted: forgotten, no scan");
                self.env.board_changed(&self.project_id);
            }
            Aborted::NoMerge => {
                notice = Some(MergeNotice::text(
                    "Could not abort the merge: no merge is in progress.",
                ));
            }
            Aborted::JobNotEnded => {
                notice = Some(MergeNotice::text(
                    "Could not abort the merge: the agent is resolving the conflicts.",
                ));
            }
            Aborted::Failed(line) => {
                log_warn!("the merge could not be aborted: {line}");
                notice = Some(MergeNotice::text(format!(
                    "Could not abort the merge: {}.",
                    clause(&line)
                )));
            }
        }
        let status = self
            .recompute()
            .status
            .ok_or_else(|| "Could not abort the merge: Solo mode is off.".to_owned())?;
        Ok(MergeAnswer {
            merge: status,
            notice,
        })
    }

    /// `developer.RESOLVE_MERGE_CONFLICTS`: only while some file still has conflicts and no `merge`
    /// job is unended; the job itself is the jobs' to start.
    fn resolve(&self) -> Result<MergeStatus, String> {
        let refuse = |reason: &str| format!("Could not resolve the conflicts: {reason}.");
        if !self.solo_is_on() || !solo_merge::in_progress(&self.root) {
            return Err(refuse("no merge is in progress"));
        }
        let conflicts = solo_merge::conflicts(&self.git, &self.root)
            .map_err(|reason| refuse(&reason.to_string()))?;
        self.env
            .start_merge_job(&self.project_id, &self.root, count(conflicts.len()))
            .map_err(|reason| refuse(&lower_first(&reason)))?;
        self.recompute()
            .status
            .ok_or_else(|| refuse("Solo mode is off"))
    }
}

/// The notice of a merge that git or Weaver refused (`requirements/ui#solo-merge`, **Notices**).
fn refusal_notice(branch: &str, refusal: &Refusal) -> MergeNotice {
    match refusal {
        Refusal::Unavailable(reason) => MergeNotice::text(format!(
            "Could not merge {branch}: {}.",
            lower_first(&reason.to_string())
        )),
        Refusal::Git(line) => {
            MergeNotice::text(format!("Could not merge {branch}: {}.", clause(line)))
        }
        Refusal::KeptOutOfGit(paths) => MergeNotice {
            text: format!(
                "Could not merge {branch}: it would overwrite files that Solo mode keeps out of git."
            ),
            paths: Some(paths.clone()),
        },
        Refusal::TimedOut => MergeNotice::text(format!(
            "Could not merge {branch}: git did not finish the merge within 2 minutes."
        )),
    }
}

impl ProjectSetup {
    fn not_in_solo(reason: String) -> AppError {
        AppError::new(ErrorCode::Refused, reason)
    }

    /// Runs `work` in the project's queue with the merge context of the moment, and records and
    /// announces the status it leaves.
    async fn in_merge_queue<T: Send + 'static>(
        &self,
        project_id: &str,
        work: impl FnOnce(&Ctx) -> T + Send + 'static,
    ) -> Result<T, AppError> {
        let (root, _) = self
            .env
            .picture(project_id)
            .ok_or_else(|| Self::not_open(project_id))?;
        let ctx = Ctx {
            env: self.env.clone(),
            git: self.git.clone(),
            project_id: project_id.to_owned(),
            root,
            previous: self
                .entries()
                .get(project_id)
                .and_then(|entry| entry.merge.clone()),
        };
        locked(self.env.as_ref(), project_id, move || work(&ctx))
            .await
            .map_err(|reason| AppError::new(ErrorCode::ProjectBusy, reason))
    }

    /// The Board is shown: Merge's state is worked out again, in the project's background work,
    /// coalesced with the other runs. `None` while Solo mode is off.
    pub async fn merge_refresh(&self, project_id: &str) -> Result<Option<MergeStatus>, AppError> {
        let receive = self
            .request(
                project_id,
                weaver_core::project_files::schedule::Run::merge_only(),
                true,
            )
            .ok_or_else(|| Self::not_open(project_id))?;
        receive
            .await
            .map_err(|_| AppError::new(ErrorCode::InvalidInput, "the window closed"))?
            .ok();
        Ok(self
            .entries()
            .get(project_id)
            .and_then(|entry| entry.merge.clone()))
    }

    /// The Dialog opens: the branches to merge from and the tracked files of Weaver.
    pub async fn merge_branches(&self, project_id: &str) -> Result<MergeBranches, AppError> {
        let outcome = self
            .in_merge_queue(project_id, |ctx| {
                if !ctx.solo_is_on() {
                    return Err("Merge is available only while Solo mode is on.".to_owned());
                }
                solo_merge::branches(&ctx.git, &ctx.root).map_err(|reason| reason.to_string())
            })
            .await?;
        let found = outcome.map_err(Self::not_in_solo)?;
        let current = found.current.ok_or_else(|| {
            AppError::new(
                ErrorCode::Refused,
                "Merge needs a branch: HEAD is detached.",
            )
        })?;
        Ok(MergeBranches {
            current,
            local: found.local,
            remote: found.remote,
            tracked: found.tracked,
        })
    }

    /// `developer.MERGE_BRANCH`: the new state, with the notice when there is something to say.
    pub async fn merge_start(
        &self,
        project_id: &str,
        branch: String,
    ) -> Result<MergeAnswer, AppError> {
        let outcome = self
            .in_merge_queue(project_id, move |ctx| ctx.start(&branch))
            .await?;
        let answer = outcome.map_err(Self::not_in_solo)?;
        self.follow_merge(project_id, Some(answer.merge.clone()));
        Ok(answer)
    }

    /// `developer.ABORT_MERGE`, once the developer confirmed.
    pub async fn merge_abort(&self, project_id: &str) -> Result<MergeAnswer, AppError> {
        let outcome = self.in_merge_queue(project_id, |ctx| ctx.abort()).await?;
        let answer = outcome.map_err(Self::not_in_solo)?;
        self.follow_merge(project_id, Some(answer.merge.clone()));
        Ok(answer)
    }

    /// `developer.RESOLVE_MERGE_CONFLICTS`: starts the job `merge`.
    pub async fn resolve_merge_conflicts(&self, project_id: &str) -> Result<MergeStatus, AppError> {
        let outcome = self.in_merge_queue(project_id, |ctx| ctx.resolve()).await?;
        let status = outcome.map_err(Self::not_in_solo)?;
        self.follow_merge(project_id, Some(status.clone()));
        Ok(status)
    }

    /// Records Merge's status after a command and tells the window when it differs.
    pub(super) fn follow_merge(&self, project_id: &str, status: Option<MergeStatus>) {
        let announced = {
            let mut entries = self.entries();
            let Some(entry) = entries.get_mut(project_id) else {
                return;
            };
            if !entry.set_merge(status) {
                return;
            }
            entry.revision += 1;
            entry.status(project_id)
        };
        self.env.emit(&announced);
    }

    /// The window came back to the foreground, or a job ended: with Solo mode on, Merge's state is
    /// worked out again in the project's background work, never otherwise.
    pub(super) fn merge_may_have_changed(&self, project_id: &str) {
        let on = self
            .entries()
            .get(project_id)
            .is_some_and(|entry| entry.solo.mode.is_on());
        if on {
            self.request(
                project_id,
                weaver_core::project_files::schedule::Run::merge_only(),
                false,
            );
        }
    }
}

/// `job.ENDED`: Merge's state is worked out again, also after a `merge` job that ended with
/// conflicts left (the merge stays in progress and the count is updated).
pub struct JobEnded(pub Arc<ProjectSetup>);

impl JobObserver for JobEnded {
    fn job_ended(&self, job: &JobInfo) {
        self.0.merge_may_have_changed(&job.project_id);
    }
}

// --- the commands ------------------------------------------------------------------------------------

/// The Board is shown: Merge's state, worked out again (`null` while Solo mode is off).
#[tauri::command]
pub async fn merge_refresh(
    setup: State<'_, Arc<ProjectSetup>>,
    project_id: String,
) -> Result<Option<MergeStatus>, AppError> {
    let setup = setup.inner().clone();
    setup.merge_refresh(&project_id).await
}

/// The Dialog of Merge opens: the branches and Weaver's tracked files.
#[tauri::command]
pub async fn merge_branches(
    setup: State<'_, Arc<ProjectSetup>>,
    project_id: String,
) -> Result<MergeBranches, AppError> {
    let setup = setup.inner().clone();
    setup.merge_branches(&project_id).await
}

/// `developer.MERGE_BRANCH`.
#[tauri::command]
pub async fn merge_start(
    setup: State<'_, Arc<ProjectSetup>>,
    project_id: String,
    branch: String,
) -> Result<MergeAnswer, AppError> {
    let setup = setup.inner().clone();
    setup.merge_start(&project_id, branch).await
}

/// `developer.ABORT_MERGE`.
#[tauri::command]
pub async fn merge_abort(
    setup: State<'_, Arc<ProjectSetup>>,
    project_id: String,
) -> Result<MergeAnswer, AppError> {
    let setup = setup.inner().clone();
    setup.merge_abort(&project_id).await
}

/// `developer.RESOLVE_MERGE_CONFLICTS`.
#[tauri::command]
pub async fn resolve_merge_conflicts(
    setup: State<'_, Arc<ProjectSetup>>,
    project_id: String,
) -> Result<MergeStatus, AppError> {
    let setup = setup.inner().clone();
    setup.resolve_merge_conflicts(&project_id).await
}

#[cfg(test)]
mod tests;

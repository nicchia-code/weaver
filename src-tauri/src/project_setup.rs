//! The Machine `project` in the app (`requirements/tech#project`, `requirements/ui#project`): whether
//! a folder is a project and whether its Model is empty, the files Weaver installs for the agent
//! and the commands of Get Started, `Import from Code` and `Update skills`.
//!
//! The rules are in `weaver_core::project_files` (what is written, the one report, at most one check
//! at a time); this module keeps one small record per open project, runs the work in the project's
//! queue and tells the window with one `project-state` event per change:
//!
//! | Event of the Machine | Where |
//! |----------------------|-------|
//! | `developer.INITIALISE_PROJECT` | [`ProjectSetup::initialise`] |
//! | `developer.UPDATE_SKILLS` | [`ProjectSetup::update_skills`] |
//! | window opens, with or without a Model | [`ProjectHooks::window_opened`]: the repair of Solo mode's block runs in the background, with the check of the files when there is a Model |
//! | `developer.CHANGE_SOLO_MODE` | [`ProjectSetup::set_solo_mode`] |
//! | `workspace.FILES_CHANGED`, `PROJECT_FOLDER_APPEARED` | [`ProjectHooks::batch_applied`] |
//! | `workspace.PROJECT_FOLDER_REMOVED` | the watcher removes Solo mode's block first (`windows::watch`), then [`ProjectHooks::batch_applied`] |
//! | `proposal.APPROVED` | the approval regenerates the Requirement skills itself (`transitions::approve_with`) |
//!
//! The state is never stored apart from the files: it is worked out from whether `.weaver/` exists
//! and from what the project's picture holds, after every batch of changes.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, Weak};
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::oneshot;
use ts_rs::TS;
use weaver_core::atomic_commits;
use weaver_core::project::image::ProjectImage;
use weaver_core::project_files::folder_tree::{self, TreeEntry};
use weaver_core::project_files::schedule::{Run, Runs, Want};
use weaver_core::project_files::{
    self, has_weaver, InitialiseError, ProjectState, SyncOptions, SyncReport,
};
use weaver_core::solo_mode::{self, Git, Mode, Opening};

pub mod merge;
use merge::{AgentWork, MergeStatus};

use crate::blocking;
use crate::error::{AppError, ErrorCode};
use crate::events::EventName;
use crate::jobs::starts::Ctx;
use crate::jobs::Jobs;
use crate::pictures::{Rebuild, Revision, SLICE_REQUIREMENTS};
use crate::state::AppState;
use crate::windows::jobs::BoxFuture;
use crate::windows::project::ProjectHooks;
use crate::windows::WindowManager;
use crate::{log_info, log_warn};

/// How long after the last change under `.weaver/requirements/` the Requirement skills are generated again.
pub const REGENERATION_DELAY: Duration = Duration::from_secs(1);

/// What the check of the installed files did when the window opened, as the window shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum CheckOutcome {
    /// Nothing to say: everything was in place, or the failure is gone.
    None,
    /// The check wrote files: the window says so once.
    Updated { files: u32 },
    /// The check failed: the window shows `Retry` at every opening until it succeeds.
    Failed { reason: String },
}

/// The opening check of a project window.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SkillsCheck {
    /// How many opening checks ended; the window tells an outcome once per number.
    #[ts(type = "number")]
    pub opening: u64,
    pub outcome: CheckOutcome,
}

impl SkillsCheck {
    fn none() -> Self {
        Self {
            opening: 0,
            outcome: CheckOutcome::None,
        }
    }

    /// What the check says after a run of `want` ended with `result`.
    fn after(&self, want: Want, result: &Result<SyncReport, String>) -> SkillsCheck {
        match want {
            // The repair of Solo mode alone never changes what the check says.
            Want::NoFiles => self.clone(),
            Want::OpeningCheck => SkillsCheck {
                opening: self.opening + 1,
                outcome: match result {
                    Ok(report) if report.is_unchanged() => CheckOutcome::None,
                    Ok(report) => CheckOutcome::Updated {
                        files: u32::try_from(report.changes.len()).unwrap_or(u32::MAX),
                    },
                    Err(reason) => CheckOutcome::Failed {
                        reason: reason.clone(),
                    },
                },
            },
            // `Update skills` that succeeds ends a failure of the opening check.
            Want::UpdateSkills if result.is_ok() => SkillsCheck {
                opening: self.opening,
                outcome: CheckOutcome::None,
            },
            Want::UpdateSkills | Want::RequirementSkills => self.clone(),
        }
    }
}

/// What the window knows of the project: the state, the files to install, the opening check and
/// Solo mode.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProjectStatus {
    pub project_id: String,
    /// Grows with every change of this status; a window drops an older one.
    #[ts(type = "number")]
    pub revision: u64,
    pub state: ProjectState,
    /// The agent files `Initialise` installs, relative to the project folder.
    pub files: Vec<String>,
    /// The folder holds names not starting with a dot (the warning of `Start from Scratch`).
    pub folder_has_files: bool,
    pub check: SkillsCheck,
    pub solo: SoloStatus,
    /// Atomic commits of the working copy (`requirements/data#atomic-commits`).
    pub atomic_commits: AtomicCommitsStatus,
    /// Merge of Solo mode; absent while Solo mode is off (`requirements/tech#solo-merge`).
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub merge: Option<MergeStatus>,
}

/// Solo mode of the working copy as the window shows it (`requirements/tech#solo-mode`). The tracked
/// files are not here: they come from `solo_tracked_files` and from the answer of every change.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SoloStatus {
    pub mode: Mode,
    /// The reason Solo mode cannot be changed; absent when it can.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub unavailable: Option<String>,
    /// Why the block of Solo mode was not put in place at the last opening (or after the last
    /// change); absent otherwise.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub problem: Option<String>,
}

impl SoloStatus {
    /// Takes what an opening found about git and the block; whether the status changed. The mode
    /// stays: it is the one remembered in the folder.
    fn follow(&mut self, opening: &Opening) -> bool {
        let unavailable = opening.unavailable.as_ref().map(ToString::to_string);
        let problem = opening.problem.as_ref().map(ToString::to_string);
        let changed = self.unavailable != unavailable || self.problem != problem;
        self.unavailable = unavailable;
        self.problem = problem;
        changed
    }
}

/// Atomic commits of the working copy as the window shows it. The choice lives only in the working
/// copy (`.weaver/commits/commits.json`), never in Weaver's settings.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AtomicCommitsStatus {
    pub enabled: bool,
    /// The reason it cannot be turned on (no git, or the folder is not in a repository); absent
    /// when it can. Worked out when the project opens and each time Settings is shown.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub unavailable: Option<String>,
    /// The state file cannot be read or holds another value, so Atomic commits counts as off:
    /// `<path>: <reason>`, the path relative to the project folder. Absent otherwise.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub diagnostic: Option<String>,
}

impl AtomicCommitsStatus {
    fn read(state: &atomic_commits::State) -> Self {
        Self {
            enabled: state.enabled,
            unavailable: None,
            diagnostic: state.diagnostic.as_ref().map(shown_diagnostic),
        }
    }

    fn inspected(inspection: &atomic_commits::Inspection) -> Self {
        Self {
            enabled: inspection.enabled,
            unavailable: inspection.unavailable.as_ref().map(ToString::to_string),
            diagnostic: inspection.diagnostic.as_ref().map(shown_diagnostic),
        }
    }
}

fn shown_diagnostic(diagnostic: &weaver_core::project::diagnostic::Diagnostic) -> String {
    format!(".weaver/{}: {}", diagnostic.path, diagnostic.message)
}

/// What a change of Solo mode answers: the new state and the tracked files the block excludes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SoloChange {
    pub solo: SoloStatus,
    /// Relative to the project folder, with `/`, sorted; empty when Solo mode is off.
    pub tracked: Vec<String>,
}

/// What `Initialise` answered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct InitialiseResult {
    /// The agent files that exist with other content, when overwriting them was not confirmed:
    /// nothing was created. Empty when the project was initialised.
    pub conflicts: Vec<String>,
}

/// What the module needs of the app: kept behind a trait so that tests drive it with a folder and no window.
pub trait Environment: Send + Sync + 'static {
    /// The root and the current picture of an open project.
    fn picture(&self, project_id: &str) -> Option<(PathBuf, Arc<ProjectImage>)>;
    /// Runs `work` in the project's queue, under the project lock.
    fn locked(
        &self,
        project_id: &str,
        work: Box<dyn FnOnce() + Send>,
    ) -> BoxFuture<Result<(), String>>;
    /// Reads the project again and tells the windows what changed (after `.weaver/` appeared).
    fn rebuilt(&self, project_id: &str) -> BoxFuture<()>;
    fn emit(&self, status: &ProjectStatus);
    /// What the project's jobs say now: whether any has not ended, and whether a `merge` job has not.
    fn agent_work(&self, project_id: &str) -> AgentWork;
    /// `solo-merge.CONCLUDED`: starts the scan of the whole System with the files the merge brought,
    /// through the path of Scan System and without its confirmation. Called inside the project's
    /// queue, after `merge.json` was deleted.
    fn start_scan_after_merge(
        &self,
        project_id: &str,
        root: &Path,
        merged: Vec<String>,
    ) -> Result<(), String>;
    /// `developer.RESOLVE_MERGE_CONFLICTS`: starts the job `merge`, `conflicts` being the number of
    /// files git says still have them. Called inside the project's queue.
    fn start_merge_job(&self, project_id: &str, root: &Path, conflicts: u32) -> Result<(), String>;
    /// What the Board shows changed without a file changing (`merge.json` appeared or went).
    fn board_changed(&self, project_id: &str);
    /// A notice at the top of the Board.
    fn notice(&self, project_id: &str, text: &str);
}

/// Runs `work` in the project's queue and returns what it returned.
async fn locked<T: Send + 'static>(
    env: &dyn Environment,
    project_id: &str,
    work: impl FnOnce() -> T + Send + 'static,
) -> Result<T, String> {
    let (send, receive) = oneshot::channel();
    env.locked(
        project_id,
        Box::new(move || {
            let _ = send.send(work());
        }),
    )
    .await?;
    receive.await.map_err(|_| "the work was dropped".to_owned())
}

type Waiter = oneshot::Sender<Result<SyncReport, String>>;

struct Entry {
    root: PathBuf,
    revision: u64,
    state: ProjectState,
    folder_has_files: bool,
    check: SkillsCheck,
    solo: SoloStatus,
    atomic: AtomicCommitsStatus,
    /// Merge of Solo mode; absent while Solo mode is off.
    merge: Option<MergeStatus>,
    runs: Runs,
    /// Requests waiting for the next run, and those the running one serves.
    waiting: Vec<Waiter>,
    serving: Vec<Waiter>,
    /// Grows with every change under `.weaver/requirements/`: only the last one regenerates.
    regeneration: u64,
}

impl Entry {
    fn status(&self, project_id: &str) -> ProjectStatus {
        ProjectStatus {
            project_id: project_id.to_owned(),
            revision: self.revision,
            state: self.state,
            files: project_files::initialise::installed_paths(),
            folder_has_files: self.folder_has_files,
            check: self.check.clone(),
            solo: self.solo.clone(),
            atomic_commits: self.atomic.clone(),
            merge: self.merge.clone(),
        }
    }

    /// Takes the status of Solo mode; with Solo mode off the project has no Merge.
    fn set_solo(&mut self, solo: SoloStatus) {
        if !solo.mode.is_on() {
            self.merge = None;
        }
        self.solo = solo;
    }

    /// Takes Merge's status; whether it changed.
    fn set_merge(&mut self, merge: Option<MergeStatus>) -> bool {
        if self.merge == merge {
            return false;
        }
        self.merge = merge;
        true
    }
}

/// The folder holds a name not starting with a dot.
fn folder_has_files(root: &Path) -> bool {
    std::fs::read_dir(root).is_ok_and(|listing| {
        listing
            .filter_map(Result::ok)
            .any(|item| !item.file_name().to_string_lossy().starts_with('.'))
    })
}

/// What the disk and the picture say of a folder: its state, whether it holds files of its own and
/// the mode of Solo mode remembered in `.weaver/solo/solo.json` (no git is called).
#[derive(Debug, Clone, PartialEq, Eq)]
struct Measured {
    state: ProjectState,
    folder_has_files: bool,
    solo: Mode,
    atomic: AtomicCommitsStatus,
}

fn measure(root: &Path, image: &ProjectImage) -> Measured {
    let state = ProjectState::of(has_weaver(root), image.model());
    let solo = read_solo_mode(root, state);
    Measured {
        state,
        folder_has_files: folder_has_files(root),
        solo,
        atomic: read_atomic_commits(root, state),
    }
}

/// The mode remembered in the folder; a file that cannot be read counts as off and is logged.
fn read_solo_mode(root: &Path, state: ProjectState) -> Mode {
    if !state.is_initialised() {
        return Mode::Off;
    }
    let read = solo_mode::read_state(root);
    if let Some(diagnostic) = &read.diagnostic {
        log_warn!(
            "{} counts as Solo mode off: {}",
            diagnostic.path,
            diagnostic.message
        );
    }
    read.mode
}

/// Atomic commits as the folder remembers it (no git is called); a file that cannot be read counts
/// as off, is logged and comes back as the `diagnostic` of the status.
fn read_atomic_commits(root: &Path, state: ProjectState) -> AtomicCommitsStatus {
    if !state.is_initialised() {
        return AtomicCommitsStatus::default();
    }
    let read = atomic_commits::read_state(root);
    if let Some(diagnostic) = &read.diagnostic {
        log_warn!(
            "{} counts as Atomic commits off: {}",
            diagnostic.path,
            diagnostic.message
        );
    }
    AtomicCommitsStatus::read(&read)
}

/// What one run did: the installed files (`None` when the run only repairs Solo mode) and the
/// repair of Solo mode's block (`None` when the run does not repair it).
struct Worked {
    files: Option<Result<SyncReport, String>>,
    solo: Option<Opening>,
    /// Merge's state worked out again (`None` when the run does not).
    merge: Option<merge::MergeWork>,
}

/// What `run` does to the project at `root`, in the project's queue. The repair of Solo mode never
/// changes the outcome for the installed files: they are separate results. `merge` is the context
/// of Merge's recomputation, given when the run does it.
fn run_work(root: &Path, run: Run, git: &Git, merge: Option<&merge::Ctx>) -> Worked {
    if !has_weaver(root) {
        return Worked {
            files: Some(Err(
                "the folder is not a project yet: initialise it first".to_owned()
            )),
            solo: None,
            merge: None,
        };
    }
    let options = match run.want {
        Want::NoFiles => None,
        Want::RequirementSkills => Some(SyncOptions::requirement_skills()),
        Want::OpeningCheck => Some(SyncOptions::everything()),
        Want::UpdateSkills => Some(SyncOptions::update_skills()),
    };
    let files = options.map(|options| project_files::sync(root, options).map_err(|error| error.0));
    let solo = run.solo.then(|| solo_mode::open(git, root));
    // After the repair: the block is what Solo mode's availability reads.
    let merge = merge.map(merge::Ctx::recompute);
    Worked { files, solo, merge }
}

/// The record of each open project.
pub struct ProjectSetup {
    env: Arc<dyn Environment>,
    delay: Duration,
    git: Git,
    entries: Mutex<std::collections::HashMap<String, Entry>>,
    me: Weak<ProjectSetup>,
}

impl ProjectSetup {
    pub fn new(env: Arc<dyn Environment>) -> Arc<Self> {
        Self::with_delay(env, REGENERATION_DELAY)
    }

    /// [`ProjectSetup::new`] with another delay for the regeneration after a change of the Requirements (tests).
    pub fn with_delay(env: Arc<dyn Environment>, delay: Duration) -> Arc<Self> {
        Self::with_git(env, delay, Git::system())
    }

    /// [`ProjectSetup::with_delay`] with another way to start git (tests).
    pub fn with_git(env: Arc<dyn Environment>, delay: Duration, git: Git) -> Arc<Self> {
        Arc::new_cyclic(|me| Self {
            env,
            delay,
            git,
            entries: Mutex::new(Default::default()),
            me: me.clone(),
        })
    }

    fn entries(&self) -> MutexGuard<'_, std::collections::HashMap<String, Entry>> {
        self.entries.lock().expect("project records poisoned")
    }

    fn arc(&self) -> Option<Arc<ProjectSetup>> {
        self.me.upgrade()
    }

    fn not_open(project_id: &str) -> AppError {
        AppError::new(
            ErrorCode::InvalidInput,
            format!("the project `{project_id}` is not open"),
        )
    }

    /// The status of an open project, for a window that just loaded.
    pub async fn status(&self, project_id: &str) -> Result<ProjectStatus, AppError> {
        if let Some(entry) = self.entries().get(project_id) {
            return Ok(entry.status(project_id));
        }
        // The window asked before the record was made: the answer is worked out without keeping it.
        let (root, image) = self
            .env
            .picture(project_id)
            .ok_or_else(|| Self::not_open(project_id))?;
        let measured = blocking::disk(move || measure(&root, &image)).await?;
        let entry = Entry {
            root: PathBuf::new(),
            revision: 0,
            state: measured.state,
            folder_has_files: measured.folder_has_files,
            check: SkillsCheck::none(),
            solo: SoloStatus {
                mode: measured.solo,
                ..SoloStatus::default()
            },
            atomic: measured.atomic,
            runs: Runs::default(),
            waiting: Vec::new(),
            serving: Vec::new(),
            regeneration: 0,
            merge: None,
        };
        Ok(entry.status(project_id))
    }

    /// The state, the folder and Solo mode's mode look again after a batch; the window is told when
    /// they differ. A mode that changed (the command line, an initialisation) was changed by a
    /// step that worked: what was said of git and of the block before no longer holds.
    async fn refresh(&self, project_id: &str) {
        let Some((root, image)) = self.env.picture(project_id) else {
            return;
        };
        let Ok(measured) = blocking::disk(move || measure(&root, &image)).await else {
            return;
        };
        let (status, solo_turned_on) = {
            let mut entries = self.entries();
            let Some(entry) = entries.get_mut(project_id) else {
                return;
            };
            if entry.state == measured.state
                && entry.folder_has_files == measured.folder_has_files
                && entry.solo.mode == measured.solo
                && entry.atomic.enabled == measured.atomic.enabled
                && entry.atomic.diagnostic == measured.atomic.diagnostic
            {
                return;
            }
            entry.state = measured.state;
            entry.folder_has_files = measured.folder_has_files;
            let mut turned_on = false;
            if entry.solo.mode != measured.solo {
                turned_on = measured.solo.is_on();
                entry.set_solo(SoloStatus {
                    mode: measured.solo,
                    ..SoloStatus::default()
                });
            }
            // What was said of git stays: only the file was read again.
            entry.atomic = AtomicCommitsStatus {
                unavailable: entry.atomic.unavailable.take(),
                ..measured.atomic
            };
            entry.revision += 1;
            (entry.status(project_id), turned_on)
        };
        self.env.emit(&status);
        // Merge appears with Solo mode: its state is worked out in the background work.
        if solo_turned_on {
            self.merge_may_have_changed(project_id);
        }
    }

    /// Asks for a run. The returned receiver hears the report of the run that serves the request
    /// (`waited` only); `None` when the project has no record.
    fn request(
        &self,
        project_id: &str,
        wanted: Run,
        waited: bool,
    ) -> Option<oneshot::Receiver<Result<SyncReport, String>>> {
        let (send, receive) = oneshot::channel();
        let start = {
            let mut entries = self.entries();
            let entry = entries.get_mut(project_id)?;
            if waited {
                entry.waiting.push(send);
            }
            let start = entry.runs.request(wanted);
            if start.is_some() {
                entry.serving = std::mem::take(&mut entry.waiting);
            }
            start
        };
        if let (Some(first), Some(me)) = (start, self.arc()) {
            let id = project_id.to_owned();
            tauri::async_runtime::spawn(async move { me.serve(id, first).await });
        }
        Some(receive)
    }

    /// Runs the check, then the one more run that the requests made meanwhile coalesced into.
    async fn serve(self: Arc<Self>, project_id: String, first: Run) {
        let mut current = Some(first);
        while let Some(run) = current.take() {
            let Some((root, previous)) = self
                .entries()
                .get(&project_id)
                .map(|entry| (entry.root.clone(), entry.merge.clone()))
            else {
                return;
            };
            let git = self.git.clone();
            let merge = run.merge.then(|| merge::Ctx {
                env: self.env.clone(),
                git: git.clone(),
                project_id: project_id.clone(),
                root: root.clone(),
                previous,
            });
            let worked = match locked(self.env.as_ref(), &project_id, move || {
                run_work(&root, run, &git, merge.as_ref())
            })
            .await
            {
                Ok(worked) => worked,
                Err(reason) => Worked {
                    files: (run.want != Want::NoFiles).then_some(Err(reason)),
                    solo: None,
                    merge: None,
                },
            };
            match &worked.files {
                Some(Ok(report)) if !report.is_unchanged() => {
                    log_info!("{} installed files written", report.changes.len())
                }
                Some(Err(reason)) => {
                    log_warn!("the installed files could not be written: {reason}")
                }
                _ => {}
            }
            if let Some(opening) = &worked.solo {
                if opening.rewritten {
                    log_info!("the block of Solo mode was written again");
                }
                if let Some(problem) = &opening.problem {
                    log_warn!("Solo mode is on but its block is not in place: {problem}");
                }
            }
            let (waiters, status, next) = {
                let mut entries = self.entries();
                // The window closed meanwhile: the waiters are dropped and hear that.
                let Some(entry) = entries.get_mut(&project_id) else {
                    return;
                };
                let waiters = std::mem::take(&mut entry.serving);
                let mut changed = false;
                if let Some(files) = &worked.files {
                    let check = entry.check.after(run.want, files);
                    if check != entry.check {
                        entry.check = check;
                        changed = true;
                    }
                }
                if let Some(opening) = &worked.solo {
                    changed |= entry.solo.follow(opening);
                }
                if let Some(merge) = &worked.merge {
                    changed |= entry.set_merge(merge.status.clone());
                }
                let status = changed.then(|| {
                    entry.revision += 1;
                    entry.status(&project_id)
                });
                let next = entry.runs.finished();
                if next.is_some() {
                    entry.serving = std::mem::take(&mut entry.waiting);
                }
                (waiters, status, next)
            };
            let answer = worked
                .files
                .unwrap_or_else(|| Err("no installed file was checked".to_owned()));
            for waiter in waiters {
                let _ = waiter.send(answer.clone());
            }
            if let Some(status) = status {
                self.env.emit(&status);
            }
            current = next;
        }
    }

    /// `developer.UPDATE_SKILLS`: writes again every installed file that differs, creates the
    /// starting Areas when the Requirements are missing, and reports what it did.
    pub async fn update_skills(&self, project_id: &str) -> Result<SyncReport, AppError> {
        let receive = self
            .request(project_id, Run::new(Want::UpdateSkills, false), true)
            .ok_or_else(|| Self::not_open(project_id))?;
        match receive.await {
            Ok(Ok(report)) => Ok(report),
            Ok(Err(reason)) => Err(AppError::new(ErrorCode::Internal, reason)),
            Err(_) => Err(AppError::new(ErrorCode::InvalidInput, "the window closed")),
        }
    }

    /// `developer.INITIALISE_PROJECT`, with the mode of Solo mode the developer chose (`Off`: git is
    /// not called). Conflicts are an answer, not an error: nothing was created. A Solo mode that
    /// cannot be turned on refuses the initialisation with its reason: nothing was created either.
    pub async fn initialise(
        &self,
        project_id: &str,
        force: bool,
        solo: Mode,
    ) -> Result<InitialiseResult, AppError> {
        let (root, _) = self
            .env
            .picture(project_id)
            .ok_or_else(|| Self::not_open(project_id))?;
        let git = self.git.clone();
        let outcome = locked(self.env.as_ref(), project_id, move || {
            project_files::initialise(&root, force, solo, &git)
        })
        .await
        .map_err(|reason| AppError::new(ErrorCode::ProjectBusy, reason))?;
        match outcome {
            Ok(done) => {
                log_info!(
                    "a project was initialised with {} agent files",
                    done.agent_files.len()
                );
                self.env.rebuilt(project_id).await;
                self.refresh(project_id).await;
                Ok(InitialiseResult {
                    conflicts: Vec::new(),
                })
            }
            Err(InitialiseError::Conflicts(conflicts)) => Ok(InitialiseResult { conflicts }),
            Err(InitialiseError::Exists) => {
                self.env.rebuilt(project_id).await;
                self.refresh(project_id).await;
                Err(AppError::new(
                    ErrorCode::Refused,
                    "the folder is already a project",
                ))
            }
            Err(InitialiseError::Solo(reason)) => Err(AppError::new(ErrorCode::Refused, reason)),
            Err(InitialiseError::Failed(reason)) => Err(AppError::new(ErrorCode::Internal, reason)),
        }
    }

    /// `developer.CHANGE_SOLO_MODE`: in the project's queue, under the project lock, like the other
    /// writes. A change that fails changes nothing: the reason comes back as the error, and when it
    /// says git cannot do its part it also becomes the `unavailable` of the status.
    pub async fn set_solo_mode(
        &self,
        project_id: &str,
        mode: Mode,
    ) -> Result<SoloChange, AppError> {
        let (root, _) = self
            .env
            .picture(project_id)
            .ok_or_else(|| Self::not_open(project_id))?;
        let git = self.git.clone();
        let outcome = locked(self.env.as_ref(), project_id, move || {
            solo_mode::set_mode(&git, &root, mode)
        })
        .await
        .map_err(|reason| AppError::new(ErrorCode::ProjectBusy, reason))?;
        match outcome {
            Ok(inspection) => {
                let solo = SoloStatus {
                    mode: inspection.mode,
                    unavailable: None,
                    problem: inspection.problem.as_ref().map(ToString::to_string),
                };
                self.follow_solo(project_id, &solo);
                // Merge appears with Solo mode and goes with it.
                self.merge_may_have_changed(project_id);
                log_info!("Solo mode is now {}", inspection.mode.as_str());
                Ok(SoloChange {
                    solo,
                    tracked: inspection.tracked,
                })
            }
            Err(reason) => {
                if reason.is_unavailable() {
                    let mut solo = self
                        .entries()
                        .get(project_id)
                        .map(|entry| entry.solo.clone())
                        .unwrap_or_default();
                    solo.unavailable = Some(reason.to_string());
                    self.follow_solo(project_id, &solo);
                }
                Err(AppError::new(ErrorCode::Refused, reason.to_string()))
            }
        }
    }

    /// The files git tracks that the block excludes, computed now (the Settings view is shown); never
    /// otherwise. Empty, with no git call, when Solo mode is off.
    pub async fn solo_tracked_files(&self, project_id: &str) -> Result<Vec<String>, AppError> {
        let (root, _) = self
            .env
            .picture(project_id)
            .ok_or_else(|| Self::not_open(project_id))?;
        let git = self.git.clone();
        blocking::disk(move || solo_mode::tracked_files(&git, &root))
            .await?
            .map_err(|reason| AppError::new(ErrorCode::Refused, reason.to_string()))
    }

    /// Records Solo mode's status after a change and tells the window when it differs.
    fn follow_solo(&self, project_id: &str, solo: &SoloStatus) {
        let status = {
            let mut entries = self.entries();
            let Some(entry) = entries.get_mut(project_id) else {
                return;
            };
            if &entry.solo == solo {
                return;
            }
            entry.set_solo(solo.clone());
            entry.revision += 1;
            entry.status(project_id)
        };
        self.env.emit(&status);
    }

    /// `developer.CHANGE_ATOMIC_COMMITS`: in the project's queue, under the project lock, like the
    /// other writes. A change that fails changes nothing: the reason comes back as the error, and
    /// when it says git cannot do its part it also becomes the `unavailable` of the status.
    pub async fn set_atomic_commits(
        &self,
        project_id: &str,
        enabled: bool,
    ) -> Result<AtomicCommitsStatus, AppError> {
        let (root, _) = self
            .env
            .picture(project_id)
            .ok_or_else(|| Self::not_open(project_id))?;
        let git = self.git.clone();
        let outcome = locked(self.env.as_ref(), project_id, move || {
            atomic_commits::set_enabled(&git, &root, enabled)
        })
        .await
        .map_err(|reason| AppError::new(ErrorCode::ProjectBusy, reason))?;
        match outcome {
            Ok(inspection) => {
                let atomic = AtomicCommitsStatus::inspected(&inspection);
                self.follow_atomic(project_id, &atomic);
                log_info!(
                    "Atomic commits is now {}",
                    if atomic.enabled { "on" } else { "off" }
                );
                Ok(atomic)
            }
            Err(reason) => {
                if reason.is_unavailable() {
                    let mut atomic = self
                        .entries()
                        .get(project_id)
                        .map(|entry| entry.atomic.clone())
                        .unwrap_or_default();
                    atomic.unavailable = Some(reason.to_string());
                    self.follow_atomic(project_id, &atomic);
                }
                Err(AppError::new(ErrorCode::Refused, reason.to_string()))
            }
        }
    }

    /// Atomic commits as it stands now, with git asked whether it can be turned on: the Settings view
    /// calls it each time it is shown, and the opening of the project does. Never on the thread of the
    /// window; git is not asked anywhere else.
    pub async fn check_atomic_commits(
        &self,
        project_id: &str,
    ) -> Result<AtomicCommitsStatus, AppError> {
        let (root, _) = self
            .env
            .picture(project_id)
            .ok_or_else(|| Self::not_open(project_id))?;
        let git = self.git.clone();
        let inspection = blocking::disk(move || atomic_commits::inspect(&git, &root)).await?;
        let atomic = AtomicCommitsStatus::inspected(&inspection);
        self.follow_atomic(project_id, &atomic);
        Ok(atomic)
    }

    /// Records Atomic commits' status and tells the window when it differs.
    fn follow_atomic(&self, project_id: &str, atomic: &AtomicCommitsStatus) {
        let status = {
            let mut entries = self.entries();
            let Some(entry) = entries.get_mut(project_id) else {
                return;
            };
            if &entry.atomic == atomic {
                return;
            }
            entry.atomic = atomic.clone();
            entry.revision += 1;
            entry.status(project_id)
        };
        self.env.emit(&status);
    }

    /// The Requirements changed: the Requirement skills are generated again 1 s after the last change.
    fn regenerate_later(&self, project_id: &str) {
        let generation = {
            let mut entries = self.entries();
            let Some(entry) = entries.get_mut(project_id) else {
                return;
            };
            entry.regeneration += 1;
            entry.regeneration
        };
        let Some(me) = self.arc() else { return };
        let id = project_id.to_owned();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(me.delay).await;
            let current = me.entries().get(&id).is_some_and(|entry| {
                entry.regeneration == generation && entry.state != ProjectState::NotInitialised
            });
            if current {
                me.request(&id, Run::new(Want::RequirementSkills, false), false);
            }
        });
    }

    async fn opened(self: Arc<Self>, project_id: String, root: PathBuf) {
        let Some((_, image)) = self.env.picture(&project_id) else {
            return;
        };
        let measuring = root.clone();
        let Ok(measured) = blocking::disk(move || measure(&measuring, &image)).await else {
            return;
        };
        let status = {
            let mut entries = self.entries();
            let entry = Entry {
                root: root.clone(),
                revision: 1,
                state: measured.state,
                folder_has_files: measured.folder_has_files,
                check: SkillsCheck::none(),
                solo: SoloStatus {
                    mode: measured.solo,
                    ..SoloStatus::default()
                },
                atomic: measured.atomic,
                runs: Runs::default(),
                waiting: Vec::new(),
                serving: Vec::new(),
                regeneration: 0,
                merge: None,
            };
            let status = entry.status(&project_id);
            entries.insert(project_id.clone(), entry);
            status
        };
        self.env.emit(&status);
        // Whether Atomic commits can be turned on: asked once at the opening, in the background.
        if let Some(me) = self.arc() {
            let id = project_id.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(error) = me.check_atomic_commits(&id).await {
                    log_warn!("Atomic commits could not be checked: {}", error.message);
                }
            });
        }
        match measured.state {
            // The check of the installed files and the repair of Solo mode's block, in one run.
            ProjectState::HasModel => {
                self.request(
                    &project_id,
                    Run::new(Want::OpeningCheck, true).with_merge(),
                    false,
                );
            }
            // An Empty Model is not checked, but its block is repaired all the same.
            ProjectState::EmptyModel => {
                self.request(
                    &project_id,
                    Run::new(Want::NoFiles, true).with_merge(),
                    false,
                );
            }
            // No project yet: only whether Solo mode can be chosen, for Get Started.
            ProjectState::NotInitialised => {
                let git = self.git.clone();
                let Ok(opening) = blocking::disk(move || solo_mode::open(&git, &root)).await else {
                    return;
                };
                let status = {
                    let mut entries = self.entries();
                    let Some(entry) = entries.get_mut(&project_id) else {
                        return;
                    };
                    entry.solo.follow(&opening).then(|| {
                        entry.revision += 1;
                        entry.status(&project_id)
                    })
                };
                if let Some(status) = status {
                    self.env.emit(&status);
                }
            }
        }
    }

    async fn batch(self: Arc<Self>, project_id: String, requirements_changed: bool) {
        self.refresh(&project_id).await;
        if requirements_changed {
            self.regenerate_later(&project_id);
        }
    }
}

impl ProjectHooks for ProjectSetup {
    fn window_opened(&self, project_id: &str, root: &Path) {
        if let Some(me) = self.arc() {
            let (id, root) = (project_id.to_owned(), root.to_path_buf());
            tauri::async_runtime::spawn(async move { me.opened(id, root).await });
        }
    }

    fn batch_applied(&self, project_id: &str, revision: Option<&Revision>) {
        if let Some(me) = self.arc() {
            let requirements =
                revision.is_some_and(|r| r.slices.iter().any(|slice| slice == SLICE_REQUIREMENTS));
            let id = project_id.to_owned();
            tauri::async_runtime::spawn(async move { me.batch(id, requirements).await });
        }
    }

    fn window_focused(&self, project_id: &str) {
        self.merge_may_have_changed(project_id);
    }

    fn window_closed(&self, project_id: &str) {
        self.entries().remove(project_id);
    }
}

// --- the real environment -----------------------------------------------------------------------

/// The app: its pictures, its queues and its windows.
pub struct TauriEnvironment {
    app: AppHandle,
}

impl TauriEnvironment {
    pub fn new(app: AppHandle) -> Self {
        Self { app }
    }
}

impl Environment for TauriEnvironment {
    fn picture(&self, project_id: &str) -> Option<(PathBuf, Arc<ProjectImage>)> {
        self.app
            .state::<AppState>()
            .pictures
            .get(project_id)
            .ok()
            .map(|picture| (picture.root, picture.image))
    }

    fn locked(
        &self,
        project_id: &str,
        work: Box<dyn FnOnce() + Send>,
    ) -> BoxFuture<Result<(), String>> {
        let app = self.app.clone();
        let id = project_id.to_owned();
        Box::pin(async move {
            app.state::<AppState>()
                .mutations
                .run(&id, move || {
                    work();
                    Ok(())
                })
                .await
                .map_err(|error| error.message)
        })
    }

    fn rebuilt(&self, project_id: &str) -> BoxFuture<()> {
        let app = self.app.clone();
        let id = project_id.to_owned();
        Box::pin(async move {
            match app
                .state::<AppState>()
                .pictures
                .rebuild(&id, Rebuild::All)
                .await
            {
                Ok(Some(revision)) => {
                    let _ = crate::events::emit_project_changed(&app, &revision.event());
                }
                Ok(None) => {}
                Err(error) => log_warn!("the picture of a project was not rebuilt: {error}"),
            }
        })
    }

    fn emit(&self, status: &ProjectStatus) {
        if let Err(error) = self.app.emit(EventName::ProjectState.as_str(), status) {
            log_warn!("the windows were not told about a project: {error}");
        }
    }

    fn agent_work(&self, project_id: &str) -> AgentWork {
        let facts = self.app.state::<Arc<Jobs>>().facts_of(project_id);
        AgentWork {
            busy: !facts.working_jobs.is_empty(),
            merge_job_unended: facts.merge_job_unended,
        }
    }

    fn start_scan_after_merge(
        &self,
        project_id: &str,
        root: &Path,
        merged: Vec<String>,
    ) -> Result<(), String> {
        let image = ProjectImage::load(root);
        self.app
            .state::<Arc<Jobs>>()
            .start_scan_after_merge(
                &Ctx {
                    project_id,
                    root,
                    image: &image,
                },
                merged,
            )
            .map(|_| ())
            .map_err(|refusal| refusal.to_string())
    }

    fn start_merge_job(&self, project_id: &str, root: &Path, conflicts: u32) -> Result<(), String> {
        let image = ProjectImage::load(root);
        self.app
            .state::<Arc<Jobs>>()
            .start_merge(
                &Ctx {
                    project_id,
                    root,
                    image: &image,
                },
                conflicts,
            )
            .map(|_| ())
            .map_err(|refusal| refusal.to_string())
    }

    fn board_changed(&self, project_id: &str) {
        // A project whose window is closed has no picture: nothing to tell.
        let _ = crate::board::publish_agent_change(
            &self.app,
            &self.app.state::<AppState>().pictures,
            project_id,
        );
    }

    fn notice(&self, project_id: &str, text: &str) {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let id = format!(
            "merge-{}",
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        );
        let notice = crate::events::Notice {
            project_id: project_id.to_owned(),
            id,
            text: text.to_owned(),
        };
        if let Err(error) = crate::events::emit_notice(&self.app, &notice) {
            log_warn!("a notice about a merge was not sent: {error}");
        }
    }
}

/// Creates the record keeper and manages it, for the commands.
pub fn install(app: &AppHandle) -> Arc<ProjectSetup> {
    let setup = ProjectSetup::new(Arc::new(TauriEnvironment::new(app.clone())));
    app.manage(setup.clone());
    setup
}

/// Creates the record keeper and hooks it to the windows: it hears of every window that opens or
/// closes, of every window that comes back to the foreground and of every batch of changes, from
/// the first window, and of every job that ends. Call right after [`crate::jobs::install_windows`],
/// before the projects are restored.
pub fn install_windows(app: &AppHandle, manager: &Arc<WindowManager>) {
    let setup = install(app);
    if let Some(jobs) = app.try_state::<Arc<Jobs>>() {
        jobs.observe(Arc::new(merge::JobEnded(setup.clone())));
    }
    manager.set_project(setup);
}

// --- commands ----------------------------------------------------------------------------------------

/// The state of the project and what its window needs for Get Started and the opening check;
/// later changes arrive as `project-state`.
#[tauri::command]
pub async fn project_status(
    setup: State<'_, Arc<ProjectSetup>>,
    project_id: String,
) -> Result<ProjectStatus, AppError> {
    setup.status(&project_id).await
}

/// `developer.INITIALISE_PROJECT`; `force` confirms the overwrite of agent files that differ; `solo`
/// is the mode of Solo mode chosen in Get Started (`off`: git is not called).
#[tauri::command]
pub async fn initialise_project(
    setup: State<'_, Arc<ProjectSetup>>,
    project_id: String,
    force: bool,
    solo: Mode,
) -> Result<InitialiseResult, AppError> {
    let setup = setup.inner().clone();
    setup.initialise(&project_id, force, solo).await
}

/// `developer.CHANGE_SOLO_MODE`: the new state with the tracked files the block excludes, or the
/// reason it could not change.
#[tauri::command]
pub async fn set_solo_mode(
    setup: State<'_, Arc<ProjectSetup>>,
    project_id: String,
    mode: Mode,
) -> Result<SoloChange, AppError> {
    let setup = setup.inner().clone();
    setup.set_solo_mode(&project_id, mode).await
}

/// The tracked files Solo mode's block excludes, computed now: the Settings view calls it each time
/// it is shown.
#[tauri::command]
pub async fn solo_tracked_files(
    setup: State<'_, Arc<ProjectSetup>>,
    project_id: String,
) -> Result<Vec<String>, AppError> {
    let setup = setup.inner().clone();
    setup.solo_tracked_files(&project_id).await
}

/// `developer.CHANGE_ATOMIC_COMMITS`: the new state, or the reason it could not change. The only way
/// to change it: no command of the command line does.
#[tauri::command]
pub async fn set_atomic_commits(
    setup: State<'_, Arc<ProjectSetup>>,
    project_id: String,
    enabled: bool,
) -> Result<AtomicCommitsStatus, AppError> {
    let setup = setup.inner().clone();
    setup.set_atomic_commits(&project_id, enabled).await
}

/// Atomic commits with git asked whether it can be turned on: the Settings view calls it each time
/// it is shown.
#[tauri::command]
pub async fn check_atomic_commits(
    setup: State<'_, Arc<ProjectSetup>>,
    project_id: String,
) -> Result<AtomicCommitsStatus, AppError> {
    let setup = setup.inner().clone();
    setup.check_atomic_commits(&project_id).await
}

/// `developer.UPDATE_SKILLS`: the same structured report the command line prints.
#[tauri::command]
pub async fn update_skills(
    setup: State<'_, Arc<ProjectSetup>>,
    project_id: String,
) -> Result<SyncReport, AppError> {
    let setup = setup.inner().clone();
    setup.update_skills(&project_id).await
}

/// The entries of one folder of the project for the tree of `Import from Code`: `path` is relative
/// to the project folder, empty for the project folder itself.
#[tauri::command]
pub async fn project_folder(
    state: State<'_, AppState>,
    project_id: String,
    path: String,
) -> Result<Vec<TreeEntry>, AppError> {
    let root = state.pictures.get(&project_id)?.root;
    blocking::disk(move || {
        folder_tree::list(&root, &path)
            .map_err(|reason| AppError::new(ErrorCode::InvalidInput, reason))
    })
    .await?
}

/// `developer.IMPORT_CODE`: starts the import job, leaving out `excluded` (paths relative to the
/// project folder, with `/`). Refused unless the Model is empty.
#[tauri::command]
pub async fn import_code(
    jobs: State<'_, Arc<Jobs>>,
    project_id: String,
    excluded: Vec<String>,
) -> Result<(), AppError> {
    let jobs = jobs.inner().clone();
    let worker = jobs.clone();
    let project = project_id.clone();
    jobs.host()
        .mutate(
            &project_id,
            Box::new(move |root| {
                let image = ProjectImage::load(root);
                if !image.model().is_empty() {
                    return Err("Could not start the import: the Model already holds a Feature or an Actor.".to_owned());
                }
                worker
                    .start_import(&Ctx { project_id: &project, root, image: &image }, excluded)
                    .map(|_| None)
                    .map_err(|refusal| format!("Could not start the import: {refusal}."))
            }),
        )
        .await
        .map_err(|message| AppError::new(ErrorCode::Refused, message))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::Duration;

    use tokio::sync::Notify;
    use weaver_core::project_files::initialise;

    use super::*;

    pub(super) const ID: &str = "0123456789abcdef";

    /// A folder with its picture read from disk at each question, and the statuses it announced.
    /// The jobs are told by hand: what they say, and what was started in them.
    pub(super) struct FakeEnvironment {
        root: PathBuf,
        pub(super) emitted: Mutex<Vec<ProjectStatus>>,
        pub(super) changed: Arc<Notify>,
        /// What the jobs say now.
        pub(super) work: Mutex<AgentWork>,
        /// The merged files of each scan started after a merge.
        pub(super) scans: Mutex<Vec<Vec<String>>>,
        /// The number of conflicts of each `merge` job started.
        pub(super) merge_jobs: Mutex<Vec<u32>>,
        /// How many times the Board was told it changed.
        pub(super) boards: Mutex<u32>,
        /// The notices sent to the Board.
        pub(super) notices: Mutex<Vec<String>>,
        /// The reason the jobs refuse the scan after a merge, when set.
        pub(super) scan_refusal: Mutex<Option<String>>,
    }

    impl FakeEnvironment {
        pub(super) fn new(root: &Path) -> Arc<Self> {
            Arc::new(Self {
                root: root.to_path_buf(),
                emitted: Mutex::new(Vec::new()),
                changed: Arc::new(Notify::new()),
                work: Mutex::new(AgentWork::default()),
                scans: Mutex::new(Vec::new()),
                merge_jobs: Mutex::new(Vec::new()),
                boards: Mutex::new(0),
                notices: Mutex::new(Vec::new()),
                scan_refusal: Mutex::new(None),
            })
        }

        pub(super) fn last(&self) -> Option<ProjectStatus> {
            self.emitted.lock().unwrap().last().cloned()
        }
    }

    impl Environment for FakeEnvironment {
        fn picture(&self, _project_id: &str) -> Option<(PathBuf, Arc<ProjectImage>)> {
            Some((self.root.clone(), Arc::new(ProjectImage::load(&self.root))))
        }

        fn locked(
            &self,
            _project_id: &str,
            work: Box<dyn FnOnce() + Send>,
        ) -> BoxFuture<Result<(), String>> {
            let changed = self.changed.clone();
            Box::pin(async move {
                let done = blocking::disk(work).await.map_err(|error| error.message);
                // Whoever waits on the files looks again.
                changed.notify_waiters();
                done
            })
        }

        fn rebuilt(&self, _project_id: &str) -> BoxFuture<()> {
            Box::pin(async {})
        }

        fn emit(&self, status: &ProjectStatus) {
            self.emitted.lock().unwrap().push(status.clone());
            self.changed.notify_waiters();
        }

        fn agent_work(&self, _project_id: &str) -> AgentWork {
            *self.work.lock().unwrap()
        }

        fn start_scan_after_merge(
            &self,
            _project_id: &str,
            _root: &Path,
            merged: Vec<String>,
        ) -> Result<(), String> {
            if let Some(reason) = self.scan_refusal.lock().unwrap().clone() {
                return Err(reason);
            }
            self.scans.lock().unwrap().push(merged);
            self.work.lock().unwrap().busy = true;
            Ok(())
        }

        fn start_merge_job(
            &self,
            _project_id: &str,
            _root: &Path,
            conflicts: u32,
        ) -> Result<(), String> {
            let mut work = self.work.lock().unwrap();
            if work.merge_job_unended {
                return Err("A merge conflicts job has not ended".to_owned());
            }
            self.merge_jobs.lock().unwrap().push(conflicts);
            *work = AgentWork {
                busy: true,
                merge_job_unended: true,
            };
            Ok(())
        }

        fn board_changed(&self, _project_id: &str) {
            *self.boards.lock().unwrap() += 1;
        }

        fn notice(&self, _project_id: &str, text: &str) {
            self.notices.lock().unwrap().push(text.to_owned());
        }
    }

    /// Waits, without a timer of its own, until `condition` holds for what was announced.
    pub(super) async fn until(env: &FakeEnvironment, condition: impl Fn(&[ProjectStatus]) -> bool) {
        let waiting = async {
            loop {
                let notified = env.changed.notified();
                tokio::pin!(notified);
                notified.as_mut().enable();
                if condition(&env.emitted.lock().unwrap()) {
                    return;
                }
                notified.await;
            }
        };
        tokio::time::timeout(Duration::from_secs(20), waiting)
            .await
            .expect("the status never came");
    }

    fn feature(root: &Path) {
        let path = root.join(".weaver/model/shop/feature.md");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, "# Shop\n\nSells things.\n").unwrap();
    }

    fn project_with_a_model(root: &Path) {
        initialise(root, false, Mode::Off, &Git::system()).unwrap();
        feature(root);
    }

    pub(super) fn setup(env: &Arc<FakeEnvironment>) -> Arc<ProjectSetup> {
        ProjectSetup::with_delay(env.clone(), Duration::from_millis(20))
    }

    pub(super) fn open(setup: &ProjectSetup, root: &Path) {
        setup.window_opened(ID, root);
    }

    #[tokio::test]
    async fn a_folder_without_weaver_offers_the_files_and_initialising_it_makes_an_empty_model() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("main.c"), "int main;").unwrap();
        let env = FakeEnvironment::new(dir.path());
        let setup = setup(&env);
        open(&setup, dir.path());
        until(&env, |all| !all.is_empty()).await;
        let first = env.last().unwrap();
        assert_eq!(
            (first.state, first.folder_has_files),
            (ProjectState::NotInitialised, true)
        );
        assert_eq!(first.files, initialise::installed_paths());
        assert!(first.files.contains(&".omp/WATCHDOG.yml".to_owned()));

        let done = setup.initialise(ID, false, Mode::Off).await.unwrap();
        assert!(done.conflicts.is_empty());
        until(&env, |all| {
            all.last()
                .is_some_and(|s| s.state == ProjectState::EmptyModel)
        })
        .await;
        assert!(dir.path().join(".weaver/model/system.md").is_file());
        assert!(dir
            .path()
            .join(".agents/skills/weaver-design/SKILL.md")
            .is_file());
        // An Empty Model is not checked when the window opens, and nothing is reported.
        assert_eq!(env.last().unwrap().check, SkillsCheck::none());
    }

    #[tokio::test]
    async fn an_old_trama_folder_means_nothing_to_the_window() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join(".trama/model")).unwrap();
        fs::write(dir.path().join(".trama/model/system.md"), "# Old\n").unwrap();
        let env = FakeEnvironment::new(dir.path());
        let setup = setup(&env);
        open(&setup, dir.path());
        until(&env, |all| !all.is_empty()).await;
        // Not initialised, and the folder holds no file of its own: `.trama` is a name with a dot.
        let first = env.last().unwrap();
        assert_eq!(
            (first.state, first.folder_has_files),
            (ProjectState::NotInitialised, false)
        );
        assert!(setup.update_skills(ID).await.is_err());
        assert!(!dir.path().join(".weaver").exists());

        setup.initialise(ID, false, Mode::Off).await.unwrap();
        until(&env, |all| {
            all.last()
                .is_some_and(|s| s.state == ProjectState::EmptyModel)
        })
        .await;
        assert!(dir.path().join(".weaver/model/system.md").is_file());
        assert_eq!(
            fs::read_to_string(dir.path().join(".trama/model/system.md")).unwrap(),
            "# Old\n"
        );
    }

    #[test]
    fn a_folder_with_both_is_the_project_in_weaver_and_the_trama_folder_adds_no_warning() {
        let dir = tempfile::tempdir().unwrap();
        project_with_a_model(dir.path());
        fs::create_dir_all(dir.path().join(".trama/model")).unwrap();
        fs::write(dir.path().join(".trama/model/system.md"), "no title here\n").unwrap();
        let image = ProjectImage::load(dir.path());
        assert!(image.diagnostics().is_empty(), "{:?}", image.diagnostics());
        let measured = measure(dir.path(), &image);
        assert_eq!(
            (measured.state, measured.folder_has_files),
            (ProjectState::HasModel, false)
        );
    }

    #[tokio::test]
    async fn differing_agent_files_are_listed_and_nothing_is_created_until_the_overwrite_is_confirmed(
    ) {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join(".omp")).unwrap();
        fs::write(dir.path().join(".omp/WATCHDOG.yml"), "mine").unwrap();
        let env = FakeEnvironment::new(dir.path());
        let setup = setup(&env);
        open(&setup, dir.path());
        until(&env, |all| !all.is_empty()).await;

        let answer = setup.initialise(ID, false, Mode::Off).await.unwrap();
        assert_eq!(answer.conflicts, [".omp/WATCHDOG.yml"]);
        assert!(!dir.path().join(".weaver").exists());
        assert_eq!(
            fs::read_to_string(dir.path().join(".omp/WATCHDOG.yml")).unwrap(),
            "mine"
        );
        assert_eq!(env.last().unwrap().state, ProjectState::NotInitialised);

        assert!(setup
            .initialise(ID, true, Mode::Off)
            .await
            .unwrap()
            .conflicts
            .is_empty());
        until(&env, |all| {
            all.last()
                .is_some_and(|s| s.state == ProjectState::EmptyModel)
        })
        .await;
        assert_ne!(
            fs::read_to_string(dir.path().join(".omp/WATCHDOG.yml")).unwrap(),
            "mine"
        );
    }

    #[tokio::test]
    async fn a_file_edited_by_hand_is_written_again_when_the_window_opens_and_the_window_says_so_once(
    ) {
        let dir = tempfile::tempdir().unwrap();
        project_with_a_model(dir.path());
        fs::write(dir.path().join(".omp/WATCHDOG.yml"), "edited").unwrap();
        let env = FakeEnvironment::new(dir.path());
        let setup = setup(&env);
        open(&setup, dir.path());
        until(&env, |all| all.last().is_some_and(|s| s.check.opening == 1)).await;
        let status = env.last().unwrap();
        assert_eq!(
            (status.state, status.check.outcome.clone()),
            (ProjectState::HasModel, CheckOutcome::Updated { files: 1 })
        );
        assert_ne!(
            fs::read_to_string(dir.path().join(".omp/WATCHDOG.yml")).unwrap(),
            "edited"
        );

        // Opened again with everything in place: the check ran and has nothing to say.
        setup.window_closed(ID);
        let env = FakeEnvironment::new(dir.path());
        let setup = self::setup(&env);
        open(&setup, dir.path());
        until(&env, |all| all.last().is_some_and(|s| s.check.opening == 1)).await;
        assert_eq!(env.last().unwrap().check.outcome, CheckOutcome::None);
    }

    #[tokio::test]
    async fn a_failed_check_is_shown_until_update_skills_succeeds() {
        let dir = tempfile::tempdir().unwrap();
        project_with_a_model(dir.path());
        fs::remove_dir_all(dir.path().join(".omp")).unwrap();
        // `.omp` is a file: nothing can be written below it.
        fs::write(dir.path().join(".omp"), "in the way").unwrap();
        let env = FakeEnvironment::new(dir.path());
        let setup = setup(&env);
        open(&setup, dir.path());
        until(&env, |all| all.last().is_some_and(|s| s.check.opening == 1)).await;
        let CheckOutcome::Failed { reason } = env.last().unwrap().check.outcome else {
            panic!("{:?}", env.last())
        };
        assert!(reason.contains(".omp"), "{reason}");

        let error = setup.update_skills(ID).await.unwrap_err();
        assert!(error.message.contains(".omp"), "{}", error.message);
        assert!(matches!(
            env.last().unwrap().check.outcome,
            CheckOutcome::Failed { .. }
        ));

        fs::remove_file(dir.path().join(".omp")).unwrap();
        let report = setup.update_skills(ID).await.unwrap();
        assert!(report.changes.iter().any(|c| c.path == ".omp/WATCHDOG.yml"));
        until(&env, |all| {
            all.last()
                .is_some_and(|s| s.check.outcome == CheckOutcome::None)
        })
        .await;
    }

    #[tokio::test]
    async fn deleting_and_creating_weaver_goes_to_get_started_and_back() {
        let dir = tempfile::tempdir().unwrap();
        project_with_a_model(dir.path());
        let env = FakeEnvironment::new(dir.path());
        let setup = setup(&env);
        open(&setup, dir.path());
        until(&env, |all| all.last().is_some_and(|s| s.check.opening == 1)).await;
        assert_eq!(env.last().unwrap().state, ProjectState::HasModel);

        fs::remove_dir_all(dir.path().join(".weaver")).unwrap();
        setup.batch_applied(ID, None);
        until(&env, |all| {
            all.last()
                .is_some_and(|s| s.state == ProjectState::NotInitialised)
        })
        .await;

        fs::create_dir_all(dir.path().join(".weaver/model")).unwrap();
        fs::write(
            dir.path().join(".weaver/model/system.md"),
            "# Shop\n\nA shop.\n",
        )
        .unwrap();
        setup.batch_applied(ID, None);
        until(&env, |all| {
            all.last()
                .is_some_and(|s| s.state == ProjectState::EmptyModel)
        })
        .await;

        feature(dir.path());
        setup.batch_applied(ID, None);
        until(&env, |all| {
            all.last()
                .is_some_and(|s| s.state == ProjectState::HasModel)
        })
        .await;
    }

    #[tokio::test]
    async fn the_requirement_skills_follow_a_change_of_the_requirements_once_after_the_last_change()
    {
        let dir = tempfile::tempdir().unwrap();
        project_with_a_model(dir.path());
        let env = FakeEnvironment::new(dir.path());
        let setup = setup(&env);
        open(&setup, dir.path());
        until(&env, |all| all.last().is_some_and(|s| s.check.opening == 1)).await;
        let skill = dir
            .path()
            .join(".agents/skills/weaver-req-ui-main/SKILL.md");
        assert!(!skill.exists());

        fs::write(
            dir.path().join(".weaver/requirements/ui/main.md"),
            "---\ndescription: Use when building screens.\n---\n# Main\n\n## General\n- A line.\n",
        )
        .unwrap();
        let revision = Revision {
            project_id: ID.to_owned(),
            revision: 2,
            slices: vec![SLICE_REQUIREMENTS.to_owned()],
        };
        setup.batch_applied(ID, Some(&revision));
        setup.batch_applied(ID, Some(&revision));
        // The skill appears; the status does not change (nothing to say), so wait on the files.
        until(&env, |_| skill.exists()).await;
        assert!(fs::read_to_string(&skill).unwrap().contains("A line."));
        assert!(env.last().unwrap().check.outcome == CheckOutcome::None);
    }

    #[tokio::test]
    async fn update_skills_in_a_folder_that_is_not_a_project_is_refused_and_writes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let env = FakeEnvironment::new(dir.path());
        let setup = setup(&env);
        open(&setup, dir.path());
        until(&env, |all| !all.is_empty()).await;
        let error = setup.update_skills(ID).await.unwrap_err();
        assert!(error.message.contains("initialise"), "{}", error.message);
        assert!(fs::read_dir(dir.path()).unwrap().next().is_none());
    }

    #[tokio::test]
    async fn the_status_of_a_window_that_asks_early_is_worked_out_and_a_closed_window_has_none() {
        let dir = tempfile::tempdir().unwrap();
        let env = FakeEnvironment::new(dir.path());
        let setup = setup(&env);
        let status = setup.status(ID).await.unwrap();
        assert_eq!(
            (status.state, status.revision),
            (ProjectState::NotInitialised, 0)
        );
        assert!(env.last().is_none());
    }

    #[test]
    fn the_check_reports_what_the_opening_run_did_and_update_skills_ends_a_failure() {
        use weaver_core::project_files::{FileAction, FileChange};
        let changed = SyncReport {
            changes: vec![FileChange {
                path: "a".into(),
                action: FileAction::Rewritten,
            }],
            areas: vec![],
            generated: project_files::sync::Generated {
                mode: project_files::SkillMode::Fragment,
                budget: 0,
                limit: 4000,
                switched: None,
            },
        };
        let none = SkillsCheck::none();
        let opened = none.after(Want::OpeningCheck, &Ok(changed.clone()));
        assert_eq!(
            opened,
            SkillsCheck {
                opening: 1,
                outcome: CheckOutcome::Updated { files: 1 }
            }
        );
        let failed = opened.after(Want::OpeningCheck, &Err("no".into()));
        assert_eq!(
            failed,
            SkillsCheck {
                opening: 2,
                outcome: CheckOutcome::Failed {
                    reason: "no".into()
                }
            }
        );
        // A failed `Update skills` and an automatic regeneration leave it alone; a good one ends it.
        assert_eq!(
            failed.after(Want::UpdateSkills, &Err("still".into())),
            failed
        );
        assert_eq!(
            failed.after(Want::RequirementSkills, &Ok(changed.clone())),
            failed
        );
        assert_eq!(
            failed.after(Want::UpdateSkills, &Ok(changed)),
            SkillsCheck {
                opening: 2,
                outcome: CheckOutcome::None
            }
        );
    }

    // --- Solo mode ---------------------------------------------------------------------------

    use weaver_core::solo_mode::{self, locate, read_state};

    /// A temporary git repository (real git, no global or system config); its root is canonical.
    fn git_repo() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let root = weaver_core::paths::canonical_root(dir.path()).unwrap();
        let status = std::process::Command::new("git")
            .args(["init", "-q"])
            .current_dir(&root)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_SYSTEM", "/dev/null")
            .status()
            .unwrap();
        assert!(status.success());
        (dir, root)
    }

    fn with_git(env: &Arc<FakeEnvironment>, git: Git) -> Arc<ProjectSetup> {
        ProjectSetup::with_git(env.clone(), Duration::from_millis(20), git)
    }

    fn no_git() -> Git {
        Git::system().with_search_path("")
    }

    fn block_in_place(root: &Path, mode: Mode) -> bool {
        locate(&Git::system(), root)
            .unwrap()
            .block_in_place(mode)
            .unwrap()
    }

    #[tokio::test]
    async fn a_folder_that_is_not_a_project_says_whether_solo_mode_can_be_chosen() {
        let (_dir, root) = git_repo();
        let env = FakeEnvironment::new(&root);
        let setup = setup(&env);
        open(&setup, &root);
        until(&env, |all| !all.is_empty()).await;
        // A repository: nothing to say, and the mode is off.
        tokio::time::sleep(Duration::from_millis(300)).await;
        let status = env.last().unwrap();
        assert_eq!(status.solo, SoloStatus::default());
        assert_eq!(status.state, ProjectState::NotInitialised);

        // Outside a repository, or without git, it says why.
        let plain = tempfile::tempdir().unwrap();
        let env = FakeEnvironment::new(plain.path());
        let setup = setup_with_open(&env, plain.path(), Git::system());
        until(&env, |all| {
            all.last().is_some_and(|s| s.solo.unavailable.is_some())
        })
        .await;
        assert_eq!(
            env.last().unwrap().solo,
            SoloStatus {
                mode: Mode::Off,
                unavailable: Some("this folder is not in a git repository".to_owned()),
                problem: None
            }
        );
        drop(setup);

        let env = FakeEnvironment::new(&root);
        let _setup = setup_with_open(&env, &root, no_git());
        until(&env, |all| {
            all.last().is_some_and(|s| s.solo.unavailable.is_some())
        })
        .await;
        assert_eq!(
            env.last().unwrap().solo.unavailable.as_deref(),
            Some("git was not found")
        );
    }

    fn setup_with_open(env: &Arc<FakeEnvironment>, root: &Path, git: Git) -> Arc<ProjectSetup> {
        let setup = with_git(env, git);
        open(&setup, root);
        setup
    }

    #[tokio::test]
    async fn initialising_with_a_mode_turns_solo_mode_on_before_the_project_exists() {
        let (_dir, root) = git_repo();
        let env = FakeEnvironment::new(&root);
        let setup = setup(&env);
        open(&setup, &root);
        until(&env, |all| !all.is_empty()).await;

        let done = setup.initialise(ID, false, Mode::Files).await.unwrap();
        assert!(done.conflicts.is_empty());
        until(&env, |all| {
            all.last()
                .is_some_and(|s| s.state == ProjectState::EmptyModel && s.solo.mode == Mode::Files)
        })
        .await;
        assert_eq!(read_state(&root).mode, Mode::Files);
        assert!(block_in_place(&root, Mode::Files));
        assert_eq!(env.last().unwrap().solo.problem, None);
    }

    #[tokio::test]
    async fn a_solo_mode_that_cannot_be_turned_on_refuses_the_initialisation_and_creates_nothing() {
        let plain = tempfile::tempdir().unwrap();
        let env = FakeEnvironment::new(plain.path());
        let setup = setup(&env);
        open(&setup, plain.path());
        until(&env, |all| !all.is_empty()).await;

        let error = setup
            .initialise(ID, false, Mode::Folders)
            .await
            .unwrap_err();
        assert_eq!(error.message, "this folder is not in a git repository");
        assert!(fs::read_dir(plain.path()).unwrap().next().is_none());
        assert_eq!(env.last().unwrap().state, ProjectState::NotInitialised);
    }

    #[tokio::test]
    async fn opening_with_an_empty_model_repairs_the_block_silently_and_checks_no_file() {
        let (_dir, root) = git_repo();
        initialise(&root, false, Mode::Folders, &Git::system()).unwrap();
        // A file of the agent edited by hand: an Empty Model is not checked, so it stays.
        fs::write(root.join(".omp/WATCHDOG.yml"), "edited").unwrap();
        locate(&Git::system(), &root)
            .unwrap()
            .remove_block()
            .unwrap();
        assert!(!block_in_place(&root, Mode::Folders));

        let env = FakeEnvironment::new(&root);
        let setup = setup(&env);
        open(&setup, &root);
        until(&env, |_| block_in_place(&root, Mode::Folders)).await;

        let status = env.last().unwrap();
        assert_eq!(status.state, ProjectState::EmptyModel);
        assert_eq!(status.check, SkillsCheck::none());
        assert_eq!(
            status.solo,
            SoloStatus {
                mode: Mode::Folders,
                ..SoloStatus::default()
            }
        );
        assert_eq!(
            fs::read_to_string(root.join(".omp/WATCHDOG.yml")).unwrap(),
            "edited"
        );
    }

    #[tokio::test]
    async fn opening_with_a_model_repairs_the_block_in_the_run_of_the_check_without_changing_its_outcome(
    ) {
        let (_dir, root) = git_repo();
        initialise(&root, false, Mode::Files, &Git::system()).unwrap();
        feature(&root);
        fs::write(root.join(".omp/WATCHDOG.yml"), "edited").unwrap();
        locate(&Git::system(), &root)
            .unwrap()
            .remove_block()
            .unwrap();

        let env = FakeEnvironment::new(&root);
        let setup = setup(&env);
        open(&setup, &root);
        until(&env, |all| all.last().is_some_and(|s| s.check.opening == 1)).await;

        let status = env.last().unwrap();
        assert_eq!(
            (status.state, status.check.outcome.clone()),
            (ProjectState::HasModel, CheckOutcome::Updated { files: 1 })
        );
        assert!(block_in_place(&root, Mode::Files));
        assert_eq!(status.solo.problem, None);
        // The repair wrote only what was missing: another opening finds nothing to do.
        let before = fs::read_to_string(root.join(".git/info/exclude")).unwrap();
        setup.window_closed(ID);
        let env = FakeEnvironment::new(&root);
        let setup = self::setup(&env);
        open(&setup, &root);
        until(&env, |all| all.last().is_some_and(|s| s.check.opening == 1)).await;
        assert_eq!(env.last().unwrap().check.outcome, CheckOutcome::None);
        assert_eq!(
            fs::read_to_string(root.join(".git/info/exclude")).unwrap(),
            before
        );
    }

    #[tokio::test]
    async fn without_git_solo_mode_stays_on_with_its_problem_and_the_check_is_untouched() {
        let (_dir, root) = git_repo();
        initialise(&root, false, Mode::Folders, &Git::system()).unwrap();
        feature(&root);
        let env = FakeEnvironment::new(&root);
        let _setup = setup_with_open(&env, &root, no_git());
        until(&env, |all| all.last().is_some_and(|s| s.check.opening == 1)).await;

        let status = env.last().unwrap();
        assert_eq!(status.check.outcome, CheckOutcome::None);
        assert_eq!(
            status.solo,
            SoloStatus {
                mode: Mode::Folders,
                unavailable: Some("git was not found".to_owned()),
                problem: Some("git was not found".to_owned()),
            }
        );
        assert_eq!(read_state(&root).mode, Mode::Folders);
    }

    #[tokio::test]
    async fn a_block_that_cannot_be_written_again_is_a_problem_until_the_next_opening_writes_it() {
        let (_dir, root) = git_repo();
        initialise(&root, false, Mode::Folders, &Git::system()).unwrap();
        let exclude = root.join(".git/info/exclude");
        let text = fs::read_to_string(&exclude).unwrap();
        fs::remove_file(&exclude).unwrap();
        fs::create_dir(&exclude).unwrap();

        let env = FakeEnvironment::new(&root);
        let setup = setup(&env);
        open(&setup, &root);
        until(&env, |all| {
            all.last().is_some_and(|s| s.solo.problem.is_some())
        })
        .await;
        let solo = env.last().unwrap().solo;
        assert_eq!(solo.mode, Mode::Folders);
        assert_eq!(solo.unavailable, None);
        assert!(solo.problem.unwrap().starts_with("could not write "));

        // Opened again with the file writable: written, the problem is gone.
        setup.window_closed(ID);
        fs::remove_dir(&exclude).unwrap();
        fs::write(&exclude, &text).unwrap();
        locate(&Git::system(), &root)
            .unwrap()
            .remove_block()
            .unwrap();
        let env = FakeEnvironment::new(&root);
        let setup = self::setup(&env);
        open(&setup, &root);
        until(&env, |_| block_in_place(&root, Mode::Folders)).await;
        assert_eq!(env.last().unwrap().solo.problem, None);
        drop(setup);
    }

    #[tokio::test]
    async fn a_change_of_solo_mode_runs_in_the_queue_and_answers_with_the_state_and_the_tracked_files(
    ) {
        let (_dir, root) = git_repo();
        initialise(&root, false, Mode::Off, &Git::system()).unwrap();
        let tracked = ".weaver/model/system.md";
        let added = std::process::Command::new("git")
            .args(["add", "-f", tracked])
            .current_dir(&root)
            .status()
            .unwrap();
        assert!(added.success());
        let env = FakeEnvironment::new(&root);
        let setup = setup(&env);
        open(&setup, &root);
        until(&env, |all| !all.is_empty()).await;

        let on = setup.set_solo_mode(ID, Mode::Folders).await.unwrap();
        assert_eq!(
            on.solo,
            SoloStatus {
                mode: Mode::Folders,
                ..SoloStatus::default()
            }
        );
        assert_eq!(on.tracked, [tracked]);
        assert_eq!(env.last().unwrap().solo.mode, Mode::Folders);
        assert_eq!(setup.solo_tracked_files(ID).await.unwrap(), [tracked]);

        let other = setup.set_solo_mode(ID, Mode::Files).await.unwrap();
        assert_eq!(other.solo.mode, Mode::Files);
        assert_eq!(env.last().unwrap().solo.mode, Mode::Files);

        let off = setup.set_solo_mode(ID, Mode::Off).await.unwrap();
        assert_eq!((off.solo.mode, off.tracked.clone()), (Mode::Off, vec![]));
        assert!(!root.join(".weaver/solo").exists());
        assert_eq!(env.last().unwrap().solo.mode, Mode::Off);
        assert_eq!(
            setup.solo_tracked_files(ID).await.unwrap(),
            Vec::<String>::new()
        );
    }

    #[tokio::test]
    async fn a_change_that_fails_changes_nothing_says_why_and_marks_solo_mode_unavailable() {
        let (_dir, root) = git_repo();
        initialise(&root, false, Mode::Off, &Git::system()).unwrap();
        let env = FakeEnvironment::new(&root);
        let setup = with_git(&env, no_git());
        open(&setup, &root);
        until(&env, |all| {
            all.last().is_some_and(|s| s.solo.unavailable.is_some())
        })
        .await;

        let error = setup.set_solo_mode(ID, Mode::Folders).await.unwrap_err();
        assert_eq!(error.message, "git was not found");
        assert!(!root.join(".weaver/solo").exists());
        assert_eq!(
            env.last().unwrap().solo,
            SoloStatus {
                mode: Mode::Off,
                unavailable: Some("git was not found".to_owned()),
                problem: None
            }
        );
        // Off: the tracked files need no git at all.
        assert_eq!(
            setup.solo_tracked_files(ID).await.unwrap(),
            Vec::<String>::new()
        );
    }

    #[tokio::test]
    async fn a_change_made_elsewhere_shows_in_the_status_after_the_next_batch() {
        let (_dir, root) = git_repo();
        initialise(&root, false, Mode::Off, &Git::system()).unwrap();
        let env = FakeEnvironment::new(&root);
        let setup = setup(&env);
        open(&setup, &root);
        until(&env, |all| !all.is_empty()).await;
        assert_eq!(env.last().unwrap().solo.mode, Mode::Off);

        // The command line turned it on while the window was open.
        solo_mode::set_mode(&Git::system(), &root, Mode::Files).unwrap();
        setup.batch_applied(ID, None);
        until(&env, |all| {
            all.last().is_some_and(|s| s.solo.mode == Mode::Files)
        })
        .await;
        assert_eq!(
            env.last().unwrap().solo,
            SoloStatus {
                mode: Mode::Files,
                ..SoloStatus::default()
            }
        );
    }

    #[test]
    fn the_status_names_its_solo_fields_in_camel_case_and_leaves_absent_ones_out() {
        let status = SoloStatus {
            mode: Mode::Files,
            unavailable: None,
            problem: Some("because".to_owned()),
        };
        assert_eq!(
            serde_json::to_value(&status).unwrap(),
            serde_json::json!({ "mode": "files", "problem": "because" })
        );
        assert_eq!(
            serde_json::to_value(SoloStatus::default()).unwrap(),
            serde_json::json!({ "mode": "off" })
        );
    }

    #[tokio::test]
    async fn atomic_commits_turns_on_in_a_repository_writes_its_state_and_turns_off_deleting_the_folder() {
        let (_dir, root) = git_repo();
        initialise(&root, false, Mode::Off, &Git::system()).unwrap();
        let env = FakeEnvironment::new(&root);
        let setup = setup(&env);
        open(&setup, &root);
        until(&env, |all| !all.is_empty()).await;
        assert_eq!(env.last().unwrap().atomic_commits, AtomicCommitsStatus::default());
        assert!(!root.join(".weaver/commits").exists());

        let on = setup.set_atomic_commits(ID, true).await.unwrap();
        assert_eq!(on, AtomicCommitsStatus { enabled: true, unavailable: None, diagnostic: None });
        assert_eq!(
            fs::read_to_string(root.join(".weaver/commits/commits.json")).unwrap(),
            "{\"enabled\": true}\n"
        );
        assert_eq!(fs::read(root.join(".weaver/commits/.gitignore")).unwrap(), b"*\n");
        assert!(env.last().unwrap().atomic_commits.enabled);
        assert!(setup.check_atomic_commits(ID).await.unwrap().enabled);

        let off = setup.set_atomic_commits(ID, false).await.unwrap();
        assert_eq!(off, AtomicCommitsStatus::default());
        assert!(!root.join(".weaver/commits").exists());
        assert!(!env.last().unwrap().atomic_commits.enabled);
    }

    #[tokio::test]
    async fn atomic_commits_is_refused_without_git_with_the_reason_and_nothing_written() {
        let (_dir, root) = git_repo();
        initialise(&root, false, Mode::Off, &Git::system()).unwrap();
        let env = FakeEnvironment::new(&root);
        let setup = with_git(&env, no_git());
        open(&setup, &root);
        until(&env, |all| {
            all.last().is_some_and(|s| s.atomic_commits.unavailable.is_some())
        })
        .await;
        assert_eq!(
            env.last().unwrap().atomic_commits,
            AtomicCommitsStatus {
                enabled: false,
                unavailable: Some("git was not found".to_owned()),
                diagnostic: None
            }
        );

        let error = setup.set_atomic_commits(ID, true).await.unwrap_err();
        assert_eq!(error.message, "git was not found");
        assert!(!root.join(".weaver/commits").exists());
        assert!(!env.last().unwrap().atomic_commits.enabled);
    }

    #[tokio::test]
    async fn atomic_commits_is_refused_outside_a_repository_and_checked_when_asked() {
        let dir = tempfile::tempdir().unwrap();
        let root = weaver_core::paths::canonical_root(dir.path()).unwrap();
        initialise(&root, false, Mode::Off, &Git::system()).unwrap();
        let env = FakeEnvironment::new(&root);
        let setup = setup(&env);
        open(&setup, &root);
        until(&env, |all| !all.is_empty()).await;

        let error = setup.set_atomic_commits(ID, true).await.unwrap_err();
        assert_eq!(error.message, "this folder is not in a git repository");
        assert!(!root.join(".weaver/commits").exists());
        let checked = setup.check_atomic_commits(ID).await.unwrap();
        assert_eq!(
            checked,
            AtomicCommitsStatus {
                enabled: false,
                unavailable: Some("this folder is not in a git repository".to_owned()),
                diagnostic: None
            }
        );
        until(&env, |all| {
            all.last().is_some_and(|s| s.atomic_commits.unavailable.is_some())
        })
        .await;
    }

    #[tokio::test]
    async fn atomic_commits_turns_off_without_git_and_the_state_is_read_when_the_project_opens() {
        let (_dir, root) = git_repo();
        initialise(&root, false, Mode::Off, &Git::system()).unwrap();
        atomic_commits::set_enabled(&Git::system(), &root, true).unwrap();
        let env = FakeEnvironment::new(&root);
        let setup = with_git(&env, no_git());
        open(&setup, &root);
        until(&env, |all| {
            all.last().is_some_and(|s| s.atomic_commits.enabled)
        })
        .await;

        let off = setup.set_atomic_commits(ID, false).await.unwrap();
        assert!(!off.enabled);
        assert!(!root.join(".weaver/commits").exists());
        assert!(!env.last().unwrap().atomic_commits.enabled);
    }

    #[tokio::test]
    async fn an_unreadable_atomic_commits_file_counts_as_off_and_is_not_rewritten() {
        let (_dir, root) = git_repo();
        initialise(&root, false, Mode::Off, &Git::system()).unwrap();
        let folder = root.join(".weaver/commits");
        fs::create_dir_all(&folder).unwrap();
        fs::write(folder.join("commits.json"), "{\"enabled\": 3}\n").unwrap();
        let env = FakeEnvironment::new(&root);
        let setup = setup(&env);
        open(&setup, &root);
        until(&env, |all| !all.is_empty()).await;
        let atomic = env.last().unwrap().atomic_commits;
        assert!(!atomic.enabled);
        // The window is told which file counts as off and why.
        assert_eq!(
            atomic.diagnostic.as_deref(),
            Some(".weaver/commits/commits.json: `enabled` must be `true`")
        );
        assert_eq!(
            fs::read_to_string(folder.join("commits.json")).unwrap(),
            "{\"enabled\": 3}\n"
        );

        // Only the next change rewrites it.
        let on = setup.set_atomic_commits(ID, true).await.unwrap();
        assert!(on.enabled);
        assert_eq!(on.diagnostic, None);
        assert_eq!(env.last().unwrap().atomic_commits.diagnostic, None);
        assert_eq!(
            fs::read_to_string(folder.join("commits.json")).unwrap(),
            "{\"enabled\": true}\n"
        );
    }

    #[test]
    fn the_status_names_its_atomic_commits_fields_in_camel_case_and_leaves_an_absent_reason_out() {
        assert_eq!(
            serde_json::to_value(AtomicCommitsStatus::default()).unwrap(),
            serde_json::json!({ "enabled": false })
        );
        assert_eq!(
            serde_json::to_value(AtomicCommitsStatus {
                enabled: false,
                unavailable: Some("git was not found".to_owned()),
                diagnostic: None
            })
            .unwrap(),
            serde_json::json!({ "enabled": false, "unavailable": "git was not found" })
        );
    }
}

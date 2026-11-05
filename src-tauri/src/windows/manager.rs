//! The window manager: machine `window` (`app` feature).
//!
//! One process serves every open project, with one window per project (`p-<project hash>`) and,
//! when no project window is open, the `welcome` window. This module holds the rules and their
//! order; everything that touches the system (creating a window, a dialog, exiting) goes through
//! [`Shell`], so the tests drive the whole machine with a fake.
//!
//! | Event | Where |
//! |-------|-------|
//! | `developer.OPEN_PROJECT` | [`WindowManager::open_project`]: opens the project, or brings its window forward (back from the background when it was there) |
//! | `desktop.APP_STARTED` | [`WindowManager::restore`]: reopens the projects of the last session |
//! | `notification.CLICKED` | [`WindowManager::focus_project`]: the window comes forward showing [`ShowRequest`] |
//! | `developer.REOPEN_APP` | [`WindowManager::reopen_app`]: every window in the background comes back |
//! | `developer.CLOSE_WINDOW` | [`WindowManager::close_window`]; with `Keep working in background` the window goes to the background ([`WindowManager::send_to_background`]) |
//! | `developer.QUIT_APP` | [`WindowManager::exit_requested`] and [`WindowManager::quit`] |
//! | `window.OPENED` / `window.CLOSED` | [`WindowJobs::window_opened`] / [`WindowJobs::window_closed`] |
//!
//! Closing a project window or quitting with jobs working, or with unsaved changes in a File view
//! ([`WindowManager::set_unsaved`], told by the UI), asks a native confirmation first
//! ([`Shell::confirm`], `tauri-plugin-dialog`, which works with a hung webview); with unsaved
//! changes it offers `Save all` ([`WindowManager::save_all`]), `Discard all` and `Cancel`, with
//! working jobs only it asks to confirm or cancel; closing a window with working jobs only also
//! offers `Keep working in background`, the only way to the background: the window is hidden
//! ([`WindowManager::send_to_background`]), nothing is suspended and the window does not close;
//! when it cannot be hidden it stays shown and its UI is told why ([`Shell::emit_hide_failed`]).
//! Cancelling changes nothing. After the
//! confirmation and the saving the jobs are suspended (the future of [`WindowJobs::window_closed`]
//! completes), then the window closes or the app exits.
//!
//! A window in the background is open: it stays in the registry, so its jobs count for the quit
//! confirmation, the restore list written at quit holds it and the last-window rule sees it.
//!
//! The picture of a project ([`Pictures`]) is opened with its window and fed by the project's
//! watcher ([`Watch`]), which calls [`Pictures::rebuild`] once per batch of changes.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, RwLock, Weak};
use std::time::Duration;

use serde::Serialize;
use ts_rs::TS;
use weaver_core::paths;
use weaver_core::project::clock::{now_instant, Clock};

use super::jobs::{BoxFuture, CloseReason, NoJobs, OpenReason, WindowJobs, WindowOpened};
use super::launch;
use super::local_state::{LocalState, StateStore};
use super::project::{NoProject, ProjectHooks};
use super::registry::{decide_exit, label_of, ExitDecision, OpenProject, OpenStart, Registry};
use super::unsaved::{FilesSaveFailed, SaveFailure, SaveReport, SavedFile, TabText};
use super::watch::{BatchSink, Watch, WatchGuard};
use crate::blocking;
use crate::error::{AppError, ErrorCode};
use crate::events::ProjectChanged;
use crate::notes::changes::NotesBatch;
use crate::pictures::{Pictures, Rebuild};
use crate::{log_info, log_warn};

/// What a window shows when it comes forward because of a notification (`notification.CLICKED`).
/// The UI switches to `view`, and to the conversation of `jobId` when there is one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ShowRequest {
    /// The project hash (the window's label is `p-<hash>`).
    pub project_id: String,
    pub view: ShowView,
    /// The job whose conversation to open, in the Jobs view.
    pub job_id: Option<String>,
}

/// A view of the project window, named as the sidebar's `ViewId`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum ShowView {
    Board,
    Requirements,
    Specs,
    History,
    Jobs,
    Files,
    General,
    Settings,
}

/// The text of a native confirmation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfirmPrompt {
    pub title: String,
    pub message: String,
    /// The label of the button that goes on; with `discard` or `background` the other buttons are
    /// that one and `Cancel`, else only `Cancel`.
    pub accept: String,
    /// The label of the button that goes on without saving, when there is something to save.
    pub discard: Option<String>,
    /// The label of the button that keeps the jobs working with the window in the background
    /// (closing a window with working jobs and nothing to save).
    pub background: Option<String>,
}

/// The button pressed in a [`ConfirmPrompt`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Confirmation {
    /// `accept`: go on (`Save all` when there are unsaved changes).
    Accept,
    /// `discard`: go on without saving.
    Discard,
    /// `background`: nothing is suspended, the window goes to the background instead of closing.
    Background,
    Cancel,
}

/// The count phrase, the pronoun for the jobs as an object, and the pronoun with its verb.
fn jobs_phrase(working: usize) -> (String, &'static str, &'static str) {
    if working == 1 {
        ("1 job is".to_owned(), "it", "it resumes")
    } else {
        (format!("{working} jobs are"), "them", "they resume")
    }
}

/// The files with unsaved changes, one per line.
fn unsaved_lines(unsaved: &[String]) -> String {
    let mut lines = String::from("Unsaved changes in:");
    for path in unsaved {
        lines.push_str("\n• ");
        lines.push_str(path);
    }
    lines
}

/// Closing the window of `project` while `working` jobs work and `unsaved` files have unsaved
/// changes. With unsaved changes the buttons are `Save all`, `Discard all` and `Cancel`.
pub fn close_prompt(project: &str, working: usize, unsaved: &[String]) -> ConfirmPrompt {
    let jobs = (working > 0).then(|| {
        let (count, them, _) = jobs_phrase(working);
        format!("{count} working in {project}. Closing the window suspends {them}.")
    });
    if unsaved.is_empty() {
        return ConfirmPrompt {
            title: format!("Close {project}?"),
            message: jobs.unwrap_or_default(),
            accept: "Suspend and Close".to_owned(),
            discard: None,
            background: Some("Keep working in background".to_owned()),
        };
    }
    let files = unsaved_lines(unsaved);
    ConfirmPrompt {
        title: format!("Close {project}?"),
        message: match jobs {
            Some(jobs) => format!("{files}\n\n{jobs}"),
            None => files,
        },
        accept: "Save all".to_owned(),
        discard: Some("Discard all".to_owned()),
        background: None,
    }
}

/// Quitting Weaver while `working` jobs work, in any project, and no File view has unsaved changes.
pub fn quit_prompt(working: usize) -> ConfirmPrompt {
    let (count, them, they) = jobs_phrase(working);
    ConfirmPrompt {
        title: "Quit Weaver?".to_owned(),
        message: format!(
            "{count} working. Quitting suspends {them}; {they} when Weaver starts again."
        ),
        accept: "Suspend and Quit".to_owned(),
        discard: None,
        background: None,
    }
}

/// Quitting Weaver, asked of the window of `project` whose File view has `unsaved` files with
/// unsaved changes; `working` counts the jobs of every project.
pub fn quit_unsaved_prompt(project: &str, working: usize, unsaved: &[String]) -> ConfirmPrompt {
    let files = format!("{project}: {}", unsaved_lines(unsaved));
    let message = if working == 0 {
        files
    } else {
        let (count, them, they) = jobs_phrase(working);
        format!("{files}\n\n{count} working. Quitting suspends {them}; {they} when Weaver starts again.")
    };
    ConfirmPrompt {
        title: "Quit Weaver?".to_owned(),
        message,
        accept: "Save all".to_owned(),
        discard: Some("Discard all".to_owned()),
        background: None,
    }
}

/// Everything the manager asks of the system.
pub trait Shell: Send + Sync + 'static {
    /// Creates and shows the window `label` for a project (title: the folder name).
    fn create_project_window(&self, label: &str, title: &str) -> Result<(), String>;
    /// Shows the welcome window: creates it, or brings the open one forward.
    fn show_welcome(&self) -> Result<(), String>;
    fn close_welcome(&self);
    /// Brings a window forward (shown, restored, focused). `false` when it does not exist.
    fn focus(&self, label: &str) -> bool;
    /// Brings any window forward; its label. `None` when no window exists.
    fn focus_any(&self) -> Option<String>;
    /// Takes a window off the screen and, on Windows and Linux, off the taskbar, keeping the
    /// window and its webview alive (`WebviewWindow::hide`).
    fn hide(&self, label: &str) -> Result<(), String>;
    /// Shows a window again and restores it when minimised, without taking the focus
    /// (`show()`, `unminimize()`). `false` when it does not exist.
    fn reveal(&self, label: &str) -> bool;
    /// Closes a window for good, with no further event.
    fn destroy(&self, label: &str);
    /// How many windows exist (project windows and the welcome window).
    fn window_count(&self) -> usize;
    /// A native confirmation: the button the developer pressed; `Cancel` also when dismissed.
    fn confirm(&self, prompt: ConfirmPrompt) -> BoxFuture<Confirmation>;
    /// Asks the File view of a project window for the texts of its tabs with unsaved changes
    /// (`files-flush`, answered through [`WindowManager::tabs_flushed`]); `false` when the window
    /// cannot be reached.
    fn request_tab_texts(&self, project_id: &str) -> bool;
    /// Saves the tab of `path` (relative to the project) with `text`, as `developer.SAVE_FILE`;
    /// the answer is the hash of what was written, or why it was not saved.
    fn save_tab(
        &self,
        project_id: &str,
        path: &str,
        text: &str,
    ) -> BoxFuture<Result<String, String>>;
    /// Tells a window which files `Save all` did not save, and why.
    fn save_failed(&self, failed: FilesSaveFailed);
    /// The window of the project is gone: its File view's tabs, their watches and the agent
    /// locks they tracked go with it.
    fn drop_tabs(&self, project_id: &str);
    /// Asks the app to exit with `code` (raising `RunEvent::ExitRequested` again).
    fn exit(&self, code: i32);
    fn emit_show(&self, label: &str, request: &ShowRequest);
    /// Tells a window why it could not be hidden after `Keep working in background`.
    fn emit_hide_failed(&self, label: &str, failure: &HideFailed);
    fn publish(&self, change: ProjectChanged);
    /// The recent projects changed (an opening, a removal, `Clear Recent`): the File menu's
    /// `Open Recent` and the welcome window, when it is open, follow the new list.
    fn recent_changed(&self) {}
    /// Writes the size and position of the open windows now.
    fn save_window_state(&self);
    /// A batch of file changes of a project, before its picture is rebuilt (the request editor
    /// follows `.weaver/requests/`).
    fn files_changed(&self, _project_id: &str, _root: &Path, _batch: &Rebuild) {}
    /// The assets of a project may have changed (`.weaver/assets/`): the project just opened, or
    /// a batch of its watcher named a path under it. The list is read again and, when it
    /// differs, the window is told (`workspace.ASSETS_CHANGED`).
    fn assets_changed(&self, _project_id: &str, _root: &Path) {}
    /// A batch of a project's watcher named paths under `.weaver/notes/`: the sheet, an image or
    /// the folder. The window is told when something changed that Weaver did not write itself
    /// (`workspace.NOTES_CHANGED`). Never part of what rebuilds the picture.
    fn notes_changed(&self, _project_id: &str, _root: &Path, _batch: NotesBatch) {}
    /// Waits until the request editor and the Notes view of a project window saved what is pending (a window is closing).
    fn flush_before_close(&self, _project_id: &str) -> BoxFuture<()> {
        Box::pin(async {})
    }
}

/// A project that could not be opened without anyone asking at that moment (the restore at
/// start, a second launch): it is reported as `<project> could not open: <reason>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenFailure {
    pub path: PathBuf,
    /// The canonical path's project hash, or the one of the path as given when it cannot be
    /// resolved.
    pub project_id: String,
    pub name: String,
    pub reason: String,
}

pub type FailureSink = Arc<dyn Fn(OpenFailure) + Send + Sync>;

/// What [`WindowManager::open_project`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenOutcome {
    Opened,
    /// The project had its window (or was being opened): it came to the front.
    Focused,
}

/// Why a window could not go to the background: the UI of that window shows it in an error toast.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct HideFailed {
    /// What the system said when the window could not be hidden.
    pub reason: String,
}

/// Why an opening started from the File menu failed: the UI of the window it was chosen from shows
/// it (an error toast in a project window, the red `Alert` of the welcome window).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct MenuOpenFailed {
    /// The folder's name; `None` when the folder picker itself failed.
    pub name: Option<String>,
    /// The reason the developer reads.
    pub reason: String,
}

/// What [`WindowManager::close_window`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CloseOutcome {
    Closed,
    /// The developer cancelled the confirmation: nothing changed.
    Cancelled,
    /// The developer chose to keep the working jobs going in the background: nothing was
    /// suspended and the window is hidden, not closed.
    Background,
    /// The developer chose to keep the working jobs going in the background and the window could
    /// not be hidden (the reason): it stays shown and open, nothing was suspended, and its UI was
    /// told why.
    HideFailed(String),
    /// The developer chose `Save all` and a file could not be saved: the window stays open on
    /// its tab, the other files are saved and nothing is suspended.
    NotSaved,
    /// There is no such window, or it is already closing.
    Ignored,
}

/// What [`WindowManager::send_to_background`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackgroundOutcome {
    /// The window left the screen and is in the background.
    Hidden,
    /// The project has no window, or its window already is in the background: nothing changed.
    Unchanged,
    /// The window could not be hidden (the reason): it stays shown and is not in the background.
    Failed(String),
}

pub struct WindowManager {
    shell: Arc<dyn Shell>,
    pictures: Arc<Pictures>,
    store: Arc<StateStore>,
    clock: Arc<dyn Clock>,
    watch: Arc<dyn Watch>,
    failures: FailureSink,
    jobs: RwLock<Arc<dyn WindowJobs>>,
    project: RwLock<Arc<dyn ProjectHooks>>,
    registry: Mutex<Registry>,
    watchers: Mutex<HashMap<String, WatchGuard>>,
    /// Per project, the files with unsaved changes in its File view, as the UI last said.
    unsaved: Mutex<HashMap<String, Vec<String>>>,
    /// The projects whose File view has a tab: **Close Window** closes it instead of the window.
    close_key: Mutex<HashSet<String>>,
    /// Per project, the `Save all` waiting for the File view's texts.
    flushing: Mutex<HashMap<String, tokio::sync::oneshot::Sender<Vec<TabText>>>>,
    flush_limit: Mutex<Duration>,
    /// The File menu opening that failed with no window to say so, until the welcome window asks.
    welcome_failure: Mutex<Option<MenuOpenFailed>>,
    me: Weak<WindowManager>,
}

/// The name of a folder: its last component, or the whole path for a root (`/`).
pub fn folder_name(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.to_string_lossy().into_owned(),
        |name| name.to_string_lossy().into_owned(),
    )
}

/// The canonical folder of a project. The reason is the sentence the developer reads.
pub fn canonical_folder(root: &Path) -> Result<PathBuf, AppError> {
    let canonical = paths::canonical_root(root).map_err(|error| {
        let why = if error.kind() == std::io::ErrorKind::NotFound {
            "the folder does not exist".to_owned()
        } else {
            error.to_string()
        };
        AppError::new(ErrorCode::InvalidInput, why)
    })?;
    if !canonical.is_dir() {
        return Err(AppError::new(ErrorCode::InvalidInput, "it is not a folder"));
    }
    Ok(canonical)
}

impl WindowManager {
    pub fn new(
        shell: Arc<dyn Shell>,
        pictures: Arc<Pictures>,
        store: Arc<StateStore>,
        clock: Arc<dyn Clock>,
        watch: Arc<dyn Watch>,
        failures: FailureSink,
    ) -> Arc<Self> {
        Arc::new_cyclic(|me| Self {
            shell,
            pictures,
            store,
            clock,
            watch,
            failures,
            jobs: RwLock::new(Arc::new(NoJobs)),
            project: RwLock::new(Arc::new(NoProject)),
            registry: Mutex::new(Registry::new()),
            watchers: Mutex::new(HashMap::new()),
            unsaved: Mutex::new(HashMap::new()),
            close_key: Mutex::new(HashSet::new()),
            flushing: Mutex::new(HashMap::new()),
            flush_limit: Mutex::new(crate::requests::FLUSH_LIMIT),
            me: me.clone(),
            welcome_failure: Mutex::new(None),
        })
    }

    /// Installs the jobs of the agent (see [`super::jobs`]).
    pub fn set_jobs(&self, jobs: Arc<dyn WindowJobs>) {
        *self.jobs.write().expect("jobs slot poisoned") = jobs;
    }

    /// Installs the project hooks (see [`super::project`]).
    pub fn set_project(&self, project: Arc<dyn ProjectHooks>) {
        *self.project.write().expect("project slot poisoned") = project;
    }

    fn project(&self) -> Arc<dyn ProjectHooks> {
        self.project.read().expect("project slot poisoned").clone()
    }

    fn jobs(&self) -> Arc<dyn WindowJobs> {
        self.jobs.read().expect("jobs slot poisoned").clone()
    }

    fn registry(&self) -> MutexGuard<'_, Registry> {
        self.registry.lock().expect("window registry poisoned")
    }

    fn arc(&self) -> Arc<WindowManager> {
        self.me
            .upgrade()
            .expect("the manager is alive while it is used")
    }

    /// The projects whose window is open, in opening order.
    pub fn open_projects(&self) -> Vec<OpenProject> {
        self.registry().projects().to_vec()
    }

    /// How many jobs work, in every project (what a quit confirms).
    pub fn working_jobs(&self) -> usize {
        let jobs = self.jobs();
        self.open_projects()
            .iter()
            .map(|project| jobs.working_jobs(&project.id))
            .sum()
    }

    /// A window got the focus (the most recent one comes forward for a second launch); a project
    /// window that came back to the foreground is `desktop.WINDOW_FOCUSED`.
    pub fn on_focused(&self, label: &str) {
        self.registry().focused(label);
        if let Some(project_id) = crate::pictures::project_of_label(label) {
            self.project().window_focused(project_id);
        }
    }

    fn report(&self, failure: OpenFailure) {
        log_warn!("{} could not open: {}", failure.name, failure.reason);
        (self.failures)(failure);
    }

    // --- developer.OPEN_PROJECT ---------------------------------------------------------------

    /// `developer.OPEN_PROJECT`: opens the project in its window, or brings the window it has
    /// forward; a folder never gets a second window. The welcome window closes once the project
    /// window is open.
    pub async fn open_project(
        &self,
        root: PathBuf,
        reason: OpenReason,
    ) -> Result<OpenOutcome, AppError> {
        let given = root.clone();
        let canonical = blocking::disk(move || canonical_folder(&root)).await??;
        let id = paths::project_hash_of_canonical(&canonical);
        let start = self.registry().begin_open(&id);
        match start {
            OpenStart::AlreadyOpen => {
                self.focus_project(&id, None);
                return Ok(OpenOutcome::Focused);
            }
            OpenStart::InFlight => return Ok(OpenOutcome::Focused),
            OpenStart::Reserved => {}
        }
        let outcome = self.open_reserved(&id, &canonical, reason).await;
        if let Err(error) = &outcome {
            self.registry().abort_open(&id);
            log_warn!("{} could not open: {}", folder_name(&given), error.message);
            // While the last session is restored the welcome window waits for the end of it.
            if !self.registry().is_restoring() {
                self.ensure_a_window();
            }
        }
        outcome.map(|()| OpenOutcome::Opened)
    }

    async fn open_reserved(
        &self,
        id: &str,
        canonical: &Path,
        reason: OpenReason,
    ) -> Result<(), AppError> {
        let name = folder_name(canonical);
        self.pictures.open(canonical.to_path_buf()).await?;

        let guard = match self.watch.start(canonical, self.sink_for(id)) {
            Ok(guard) => guard,
            Err(why) => {
                self.pictures.close(id);
                return Err(AppError::internal(why));
            }
        };
        if let Err(why) = self.shell.create_project_window(&label_of(id), &name) {
            drop(guard);
            self.pictures.close(id);
            return Err(AppError::internal(format!(
                "the window could not be opened: {why}"
            )));
        }
        self.watchers
            .lock()
            .expect("watchers poisoned")
            .insert(id.to_owned(), guard);
        self.registry().finish_open(OpenProject {
            id: id.to_owned(),
            root: canonical.to_path_buf(),
            name: name.clone(),
        });
        // The assets are listed from the start, whether or not the Assets view is ever shown.
        self.shell.assets_changed(id, canonical);

        let path_text = canonical.to_string_lossy().into_owned();
        match now_instant(self.clock.as_ref()) {
            Ok(opened) => {
                let store = self.store.clone();
                let stored = blocking::disk(move || {
                    store.update(|state| state.record_open(&path_text, &name, &opened))
                })
                .await;
                match stored {
                    Ok(Ok(())) => {}
                    Ok(Err(error)) => log_warn!("the recent projects were not stored: {error}"),
                    Err(error) => log_warn!("the recent projects were not stored: {error}"),
                }
            }
            Err(error) => log_warn!("the recent projects were not stored: {error}"),
        }
        // The File menu's `Open Recent` and the welcome window follow the new list.
        self.shell.recent_changed();
        log_info!("opened {}", folder_name(canonical));
        self.jobs().window_opened(&WindowOpened {
            project_id: id.to_owned(),
            root: canonical.to_path_buf(),
            reason,
        });
        self.project().window_opened(id, canonical);
        self.shell.close_welcome();
        Ok(())
    }

    /// One batch of changes of a project: what is under `.weaver/assets/` only refreshes the list
    /// of the assets and what is under `.weaver/notes/` only goes to the Notes view; neither
    /// reaches the picture. The rest rebuilds the picture, and the UI is told once.
    fn sink_for(&self, id: &str) -> BatchSink {
        let manager = self.me.clone();
        let id = id.to_owned();
        Arc::new(move |batch: Rebuild| {
            let Some(manager) = manager.upgrade() else {
                return;
            };
            let id = id.clone();
            tauri::async_runtime::spawn(async move {
                let root = manager.pictures.get(&id).ok().map(|picture| picture.root);
                let batch = match &root {
                    Some(root) => {
                        let split = crate::assets::split_batch(root, batch);
                        if split.assets {
                            manager.shell.assets_changed(&id, root);
                        }
                        let Some(rest) = split.rest else {
                            return;
                        };
                        let split = crate::notes::changes::split_batch(root, rest);
                        if let Some(notes) = split.notes {
                            manager.shell.notes_changed(&id, root, notes);
                        }
                        let Some(rest) = split.rest else {
                            return;
                        };
                        manager.shell.files_changed(&id, root, &rest);
                        rest
                    }
                    None => batch,
                };
                let published = manager.pictures.rebuild(&id, batch).await;
                match &published {
                    Ok(Some(revision)) => manager.shell.publish(revision.event()),
                    Ok(None) => {}
                    // The window closed while the batch waited: nothing to update.
                    Err(error) if error.code == ErrorCode::InvalidInput => {}
                    Err(error) => log_warn!("the picture of a project was not rebuilt: {error}"),
                }
                if let Ok(revision) = &published {
                    manager.project().batch_applied(&id, revision.as_ref());
                }
            });
        })
    }

    /// The welcome window is the window of Weaver when no project window is open.
    fn ensure_a_window(&self) {
        if self.registry().projects().is_empty() {
            if let Err(why) = self.shell.show_welcome() {
                log_warn!("the welcome window could not be shown: {why}");
            }
        }
    }

    // --- notification.CLICKED, a second launch, the Dock, the background -------------------------

    /// Brings a window forward; a project window that was in the background is back on the screen.
    /// `false` when it does not exist.
    fn bring_forward(&self, label: &str) -> bool {
        if !self.shell.focus(label) {
            return false;
        }
        if let Some(id) = crate::pictures::project_of_label(label) {
            self.registry().shown(id);
        }
        true
    }

    /// The window of the project comes to the front, back from the background when it was there,
    /// showing `show` when given (what the notification was about). `false` when the project has
    /// no window.
    pub fn focus_project(&self, project_id: &str, show: Option<ShowRequest>) -> bool {
        let label = label_of(project_id);
        if !self.bring_forward(&label) {
            return false;
        }
        if let Some(request) = show {
            self.shell.emit_show(&label, &request);
        }
        true
    }

    /// The window labels, the most recently focused first.
    pub fn focus_order(&self) -> Vec<String> {
        self.registry().focus_order().map(str::to_owned).collect()
    }

    /// The welcome window comes to the front; it is created when it is not there.
    pub fn show_welcome(&self) {
        if let Err(why) = self.shell.show_welcome() {
            log_warn!("the welcome window could not be shown: {why}");
        }
    }

    /// The window of `order` used most recently comes to the front (`order` lists labels, the
    /// most recent first); the welcome window shows when there is none.
    fn focus_first_of(&self, order: &[String]) {
        if order.iter().any(|label| self.bring_forward(label)) {
            return;
        }
        match self.shell.focus_any() {
            Some(label) => {
                if let Some(id) = crate::pictures::project_of_label(&label) {
                    self.registry().shown(id);
                }
            }
            None => self.show_welcome(),
        }
    }

    /// The most recently focused window comes to the front; the welcome window shows when there
    /// is none.
    pub fn focus_recent(&self) {
        self.focus_first_of(&self.focus_order());
    }

    /// The window of the project leaves the screen (and, on Windows and Linux, the taskbar), after
    /// `Keep working in background` of the close confirmation. Nothing is suspended and
    /// `window.CLOSED` is not emitted: the window and its webview stay alive and the project stays
    /// open. When the window cannot be hidden the failure is logged as a warning, the background
    /// flag is not left set and the reason is returned.
    pub fn send_to_background(&self, project_id: &str) -> BackgroundOutcome {
        if !self.registry().send_to_background(project_id) {
            return BackgroundOutcome::Unchanged;
        }
        if let Err(why) = self.shell.hide(&label_of(project_id)) {
            log_warn!("the window of a project could not be sent to the background: {why}");
            self.registry().shown(project_id);
            return BackgroundOutcome::Failed(why);
        }
        BackgroundOutcome::Hidden
    }

    /// Whether the window of the project is in the background.
    pub fn is_background(&self, project_id: &str) -> bool {
        self.registry().is_background(project_id)
    }

    /// `developer.REOPEN_APP`: every project window in the background comes back on the screen, the
    /// least recently used first, then the window used most recently gets the focus and ends on
    /// top. With no window in the background the window used most recently comes to the front,
    /// and the welcome window shows when no window is open.
    pub fn reopen_app(&self) {
        // Taken first: showing a window can move the focus, and with it the order.
        let order = self.focus_order();
        let hidden = self.registry().background_projects();
        for id in hidden {
            if !self.shell.reveal(&label_of(&id)) {
                log_warn!("a window in the background could not be shown");
            }
            self.registry().shown(&id);
        }
        self.focus_first_of(&order);
    }

    /// A second launch handed its arguments over: a folder is `developer.OPEN_PROJECT` (the window
    /// of that project alone comes back from the background), anything else is
    /// `developer.REOPEN_APP`.
    pub fn second_launch(&self, args: Vec<String>, cwd: PathBuf) {
        match launch::folder_argument(&args, &cwd) {
            Some(folder) => self.open_in_background(folder, OpenReason::Developer),
            None => self.reopen_app(),
        }
    }

    /// Opens a project without a caller waiting for the result: a failure is reported.
    pub fn open_in_background(&self, folder: PathBuf, reason: OpenReason) {
        let manager = self.arc();
        tauri::async_runtime::spawn(async move {
            if let Err(error) = manager.open_project(folder.clone(), reason).await {
                manager.report(OpenFailure {
                    project_id: paths::project_hash_of_canonical(&folder),
                    name: folder_name(&folder),
                    path: folder,
                    reason: error.message,
                });
            }
        });
    }

    /// A click on the Dock icon (macOS): `developer.REOPEN_APP` whenever a project window is in the
    /// background, whatever `has_visible_windows` says; otherwise the welcome window shows only
    /// when no window is visible.
    pub fn reopen(&self, has_visible_windows: bool) {
        if self.registry().has_background() {
            self.reopen_app();
        } else if !has_visible_windows {
            self.focus_recent();
        }
    }

    // --- desktop.APP_STARTED --------------------------------------------------------------------

    /// `desktop.APP_STARTED`: opens the projects whose window was open when Weaver last quit, in
    /// their opening order (their jobs that were working resume), then `folder` when the launch
    /// named one. Without any project window the welcome window shows, with the recent projects.
    /// A project that cannot open is reported, and the others go on.
    pub async fn restore(&self, folder: Option<PathBuf>) {
        self.registry().set_restoring(true);
        let store = self.store.clone();
        let session = match blocking::disk(move || store.snapshot()).await {
            Ok(state) => state.last_session,
            Err(error) => {
                log_warn!("the last session could not be read: {error}");
                Vec::new()
            }
        };
        let wanted = session
            .into_iter()
            .map(|path| (PathBuf::from(path), OpenReason::AppStart))
            .chain(folder.map(|f| (f, OpenReason::Developer)));
        for (path, reason) in wanted {
            if let Err(error) = self.open_project(path.clone(), reason).await {
                self.report(OpenFailure {
                    project_id: paths::project_hash_of_canonical(&path),
                    name: folder_name(&path),
                    path,
                    reason: error.message,
                });
            }
        }
        self.registry().set_restoring(false);
        self.ensure_a_window();
    }

    // --- developer.CLOSE_WINDOW ------------------------------------------------------------------

    /// `developer.CLOSE_WINDOW`: with jobs working in the project, or unsaved changes in its File
    /// view, a native confirmation comes first; cancelling changes nothing. With unsaved changes the
    /// developer chooses `Save all` or `Discard all`: a file that cannot be saved leaves the window
    /// open on its tab, the other files saved and nothing suspended. With working jobs only the
    /// developer can also choose to keep them working in the background: nothing is suspended and
    /// the window does not close; when it cannot be hidden it stays shown and its UI is told why.
    /// Else the
    /// jobs are suspended and the window closes; the project does not reopen at the next start.
    /// Closing the last project window opens the welcome window, unless another project window is
    /// in the background.
    pub async fn close_window(&self, project_id: &str) -> CloseOutcome {
        let Some(project) = self.registry().begin_close(project_id) else {
            return CloseOutcome::Ignored;
        };
        let working = self.jobs().working_jobs(project_id);
        let unsaved = self.unsaved(project_id);
        if working > 0 || !unsaved.is_empty() {
            let answer = self
                .shell
                .confirm(close_prompt(&project.name, working, &unsaved))
                .await;
            if answer == Confirmation::Background && unsaved.is_empty() {
                self.registry().cancel_close(project_id);
                return match self.send_to_background(project_id) {
                    BackgroundOutcome::Hidden => CloseOutcome::Background,
                    BackgroundOutcome::Unchanged => CloseOutcome::Cancelled,
                    BackgroundOutcome::Failed(reason) => {
                        self.shell.emit_hide_failed(
                            &label_of(project_id),
                            &HideFailed {
                                reason: reason.clone(),
                            },
                        );
                        CloseOutcome::HideFailed(reason)
                    }
                };
            }
            let saved = match answer {
                Confirmation::Cancel | Confirmation::Background => false,
                Confirmation::Accept if !unsaved.is_empty() => self.save_tabs(project_id).await,
                Confirmation::Accept | Confirmation::Discard => true,
            };
            if !saved {
                self.registry().cancel_close(project_id);
                return if matches!(answer, Confirmation::Cancel | Confirmation::Background) {
                    CloseOutcome::Cancelled
                } else {
                    CloseOutcome::NotSaved
                };
            }
        }
        // The welcome window opens before the last project window goes: the app never has no window.
        if self.registry().is_last_project(project_id) {
            if let Err(why) = self.shell.show_welcome() {
                log_warn!("the welcome window could not be shown: {why}");
            }
        }
        // The request being written is saved before the jobs end and the window goes: closing never sends it.
        self.shell.flush_before_close(project_id).await;
        self.jobs()
            .window_closed(project_id, CloseReason::WindowClosed)
            .await;
        self.shell.save_window_state();
        self.forget_project(project_id);
        self.remember_closed(&project).await;
        self.shell.destroy(&label_of(project_id));
        log_info!("closed {}", project.name);
        CloseOutcome::Closed
    }

    /// `WindowEvent::CloseRequested` of a project window (already prevented by the caller): the
    /// close goes on in the background, since the confirmation and the suspension of the jobs
    /// take time.
    pub fn close_requested(&self, project_id: &str) {
        let manager = self.arc();
        let id = project_id.to_owned();
        tauri::async_runtime::spawn(async move {
            manager.close_window(&id).await;
        });
    }

    /// Stops what the open window held: the watcher, the picture, what its File view said and its
    /// place in the registry.
    fn forget_project(&self, project_id: &str) {
        self.set_unsaved(project_id, Vec::new());
        self.set_close_key(project_id, false);
        self.shell.drop_tabs(project_id);
        self.project().window_closed(project_id);
        self.watchers
            .lock()
            .expect("watchers poisoned")
            .remove(project_id);
        self.pictures.close(project_id);
        self.registry().remove(project_id);
    }

    /// The project leaves the session: it will not reopen at the next start.
    async fn remember_closed(&self, project: &OpenProject) {
        let store = self.store.clone();
        let path = project.root.to_string_lossy().into_owned();
        match blocking::disk(move || store.update(|state| state.record_close(&path))).await {
            Ok(Ok(())) => {}
            Ok(Err(error)) => log_warn!("the session was not stored: {error}"),
            Err(error) => log_warn!("the session was not stored: {error}"),
        }
    }

    /// A project window was destroyed without going through [`Self::close_window`] (the system
    /// closed it): it is closed like the developer closing it, minus the confirmation.
    pub fn on_destroyed(&self, label: &str) {
        self.registry().forget_label(label);
        let Some(id) = crate::pictures::project_of_label(label) else {
            return;
        };
        let Some(project) = self.registry().project(id).cloned() else {
            return;
        };
        let manager = self.arc();
        tauri::async_runtime::spawn(async move {
            let quitting = manager.registry().quit_state() != super::registry::QuitState::Idle;
            let reason = if quitting {
                CloseReason::AppQuit
            } else {
                CloseReason::WindowClosed
            };
            manager.jobs().window_closed(&project.id, reason).await;
            manager.forget_project(&project.id);
            if !quitting {
                manager.remember_closed(&project).await;
                manager.ensure_a_window();
            }
        });
    }

    // --- developer.QUIT_APP -----------------------------------------------------------------------

    /// `RunEvent::ExitRequested`: decides, and for a quit starts the quit path, which exits
    /// again when it is done. `windows_left` is how many windows still exist.
    pub fn exit_requested(&self, code: Option<i32>, windows_left: usize) -> ExitDecision {
        let state = self.registry().quit_state();
        let decision = decide_exit(state, code, windows_left, cfg!(target_os = "macos"));
        if let ExitDecision::Quit { code } = decision {
            let manager = self.arc();
            tauri::async_runtime::spawn(async move { manager.quit(code, true).await });
        }
        decision
    }

    /// `developer.QUIT_APP`: with jobs working in any project, or a File view with unsaved changes,
    /// a native confirmation comes first (`confirm`); cancelling changes nothing. Every window with
    /// unsaved changes asks in turn, its dialog counting the jobs of every project, and `Cancel` in
    /// any of them stops the quit before anything is saved or suspended. Then the windows that
    /// chose `Save all` are saved; when a file cannot be saved the others still are, nothing is
    /// suspended and Weaver does not quit. Then the session is written (the projects open now
    /// reopen at the next start), the jobs of every project are suspended (`window.CLOSED`) and the
    /// app exits with `code`. A shutdown that allows no confirmation passes `confirm = false`: the
    /// jobs are suspended without asking.
    pub async fn quit(&self, code: i32, confirm: bool) {
        if !self.registry().begin_quit() {
            return;
        }
        if confirm && !self.confirm_quit().await {
            self.registry().cancel_quit();
            return;
        }
        let (projects, restoring) = {
            let registry = self.registry();
            (registry.projects().to_vec(), registry.is_restoring())
        };
        // Written first: a crash while the jobs are suspended still keeps the session.
        if !restoring {
            let store = self.store.clone();
            let open_now: Vec<String> = projects
                .iter()
                .map(|p| p.root.to_string_lossy().into_owned())
                .collect();
            match blocking::disk(move || store.update(|state| state.set_last_session(open_now)))
                .await
            {
                Ok(Ok(())) => {}
                Ok(Err(error)) => log_warn!("the session was not stored: {error}"),
                Err(error) => log_warn!("the session was not stored: {error}"),
            }
        }
        let jobs = self.jobs();
        for project in &projects {
            self.shell.flush_before_close(&project.id).await;
        }
        let ending: Vec<_> = projects
            .iter()
            .map(|project| {
                tauri::async_runtime::spawn(jobs.window_closed(&project.id, CloseReason::AppQuit))
            })
            .collect();
        for handle in ending {
            if let Err(error) = handle.await {
                log_warn!("the jobs of a project were not suspended: {error}");
            }
        }
        self.watchers.lock().expect("watchers poisoned").clear();
        self.shell.save_window_state();
        self.registry().quit_ready();
        log_info!("quitting");
        self.shell.exit(code);
    }

    /// What a quit asks before it goes on; `true` when it does. Without unsaved changes anywhere
    /// one dialog asks about the working jobs; otherwise each window with unsaved changes asks in
    /// turn. Nothing is saved before every dialog is answered.
    async fn confirm_quit(&self) -> bool {
        let working = self.working_jobs();
        let mut asking = Vec::new();
        for project in self.open_projects() {
            let unsaved = self.unsaved(&project.id);
            if !unsaved.is_empty() {
                asking.push((project, unsaved));
            }
        }
        if asking.is_empty() {
            return working == 0
                || self.shell.confirm(quit_prompt(working)).await != Confirmation::Cancel;
        }
        let mut saving = Vec::new();
        for (project, unsaved) in &asking {
            self.bring_forward(&label_of(&project.id));
            match self
                .shell
                .confirm(quit_unsaved_prompt(&project.name, working, unsaved))
                .await
            {
                // The quit confirmation never offers the background: any other answer is a cancel.
                Confirmation::Cancel | Confirmation::Background => return false,
                Confirmation::Accept => saving.push(project.id.clone()),
                Confirmation::Discard => {}
            }
        }
        let mut saved = true;
        for id in saving {
            saved &= self.save_tabs(&id).await;
        }
        saved
    }

    // --- the unsaved changes of the File view ------------------------------------------------------

    /// The File view of a window says in which files it has unsaved changes (all of them, each time).
    pub fn set_unsaved(&self, project_id: &str, paths: Vec<String>) {
        let mut unsaved = self.unsaved.lock().expect("unsaved poisoned");
        if paths.is_empty() {
            unsaved.remove(project_id);
        } else {
            unsaved.insert(project_id.to_owned(), paths);
        }
    }

    /// The File view of a window has a tab (or has none): whether **Close Window** closes the tab.
    pub fn set_close_key(&self, project_id: &str, claimed: bool) {
        let mut keys = self.close_key.lock().expect("close key poisoned");
        if claimed {
            keys.insert(project_id.to_owned());
        } else {
            keys.remove(project_id);
        }
    }

    /// **Close Window** closes a tab of the File view of this window, not the window.
    pub fn close_key_claimed(&self, project_id: &str) -> bool {
        self.close_key
            .lock()
            .expect("close key poisoned")
            .contains(project_id)
    }

    /// The files with unsaved changes in the File view of a window, as it last said.
    pub fn unsaved(&self, project_id: &str) -> Vec<String> {
        self.unsaved
            .lock()
            .expect("unsaved poisoned")
            .get(project_id)
            .cloned()
            .unwrap_or_default()
    }

    /// The answer of a window's File view to `files-flush`.
    pub fn tabs_flushed(&self, project_id: &str, tabs: Vec<TabText>) {
        if let Some(waiting) = self
            .flushing
            .lock()
            .expect("flushing poisoned")
            .remove(project_id)
        {
            let _ = waiting.send(tabs);
        }
    }

    /// How long `Save all` waits for the window's File view to send its texts.
    pub fn set_flush_limit(&self, limit: Duration) {
        *self.flush_limit.lock().expect("flush limit poisoned") = limit;
    }

    /// `Save all` of a window: asks its File view for the texts of its tabs, waits at most the
    /// flush limit and saves each one as `developer.SAVE_FILE`. The files that were not saved
    /// (the window did not answer in time, or the file could not be written) are returned with
    /// the reason; the others are saved anyway.
    pub async fn save_all(&self, project_id: &str) -> SaveReport {
        let wanted = self.unsaved(project_id);
        let mut report = SaveReport::default();
        if wanted.is_empty() {
            return report;
        }
        let limit = *self.flush_limit.lock().expect("flush limit poisoned");
        let (answer, receiver) = tokio::sync::oneshot::channel();
        self.flushing
            .lock()
            .expect("flushing poisoned")
            .insert(project_id.to_owned(), answer);
        let reached = self.shell.request_tab_texts(project_id);
        let tabs = if reached {
            tokio::time::timeout(limit, receiver)
                .await
                .ok()
                .and_then(Result::ok)
        } else {
            None
        };
        self.flushing
            .lock()
            .expect("flushing poisoned")
            .remove(project_id);
        let Some(tabs) = tabs else {
            report.failures = wanted
                .into_iter()
                .map(|path| SaveFailure {
                    path,
                    reason: "the window did not send its text in time".to_owned(),
                })
                .collect();
            return report;
        };
        for tab in &tabs {
            match self.shell.save_tab(project_id, &tab.path, &tab.text).await {
                Ok(hash) => report.saved.push(SavedFile {
                    path: tab.path.clone(),
                    hash,
                }),
                Err(reason) => {
                    log_warn!("{} was not saved: {reason}", tab.path);
                    report.failures.push(SaveFailure {
                        path: tab.path.clone(),
                        reason,
                    });
                }
            }
        }
        for path in wanted {
            if !tabs.iter().any(|tab| tab.path == path) {
                report.failures.push(SaveFailure {
                    path,
                    reason: "the window did not send its text".to_owned(),
                });
            }
        }
        report
    }

    /// [`Self::save_all`], and when a file was not saved, the window is told which and why (its
    /// File view shows the tab of the first one, and marks the saved ones saved) and comes
    /// forward. `true` when everything was saved.
    async fn save_tabs(&self, project_id: &str) -> bool {
        let report = self.save_all(project_id).await;
        if report.failures.is_empty() {
            return true;
        }
        self.bring_forward(&label_of(project_id));
        self.shell.save_failed(FilesSaveFailed {
            project_id: project_id.to_owned(),
            saved: report.saved,
            failures: report.failures,
        });
        false
    }

    // --- the welcome window's data ----------------------------------------------------------------

    /// The recent projects, most recent first (blocking: call from the blocking pool).
    pub fn recent(&self) -> LocalState {
        self.store.snapshot()
    }

    /// Removes a project from the recent list; returns the list that remains.
    pub fn remove_recent(&self, path: &str) -> Result<LocalState, AppError> {
        let stored = self.store.update(|state| state.remove_recent(path));
        // What is in memory is what the menu and the welcome window show, written or not.
        self.shell.recent_changed();
        stored.map_err(AppError::internal)?;
        Ok(self.store.snapshot())
    }

    /// `Clear Recent` of the File menu: empties the list of recent projects, without touching the
    /// last session or the open windows; returns the list that remains (blocking: call from the
    /// blocking pool).
    pub fn clear_recent(&self) -> Result<LocalState, AppError> {
        let stored = self.store.update(LocalState::clear_recent);
        self.shell.recent_changed();
        stored.map_err(AppError::internal)?;
        Ok(self.store.snapshot())
    }

    /// A File menu opening failed with no window to say so: the welcome window, which may still be
    /// loading, shows it as soon as it asks for its state ([`WindowManager::take_welcome_failure`]).
    pub fn set_welcome_failure(&self, failure: MenuOpenFailed) {
        *self
            .welcome_failure
            .lock()
            .expect("welcome failure poisoned") = Some(failure);
    }

    /// The failure waiting for the welcome window, handed over once.
    pub fn take_welcome_failure(&self) -> Option<MenuOpenFailed> {
        self.welcome_failure
            .lock()
            .expect("welcome failure poisoned")
            .take()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    use weaver_core::project::clock::ManualClock;

    use super::*;
    use crate::windows::watch::{Watch, WatchGuard};

    /// Everything the manager did to the system, in order.
    #[derive(Default)]
    struct FakeShell {
        log: Mutex<Vec<String>>,
        windows: Mutex<Vec<String>>,
        answers: Mutex<Vec<Confirmation>>,
        /// What the File view of the window sends when asked for its texts (`None`: it never answers).
        texts: Mutex<Option<Vec<TabText>>>,
        /// The files whose save is refused, with the reason.
        refused: Mutex<HashMap<String, String>>,
        failed: Mutex<Vec<FilesSaveFailed>>,
        manager: Mutex<Option<Weak<WindowManager>>>,
        prompts: Mutex<Vec<ConfirmPrompt>>,
        published: Mutex<Vec<ProjectChanged>>,
        fail_windows: Mutex<bool>,
        /// The windows taken off the screen.
        hidden: Mutex<Vec<String>>,
        fail_hide: Mutex<bool>,
        /// The projects whose assets the manager asked to read again, in order.
        assets: Mutex<Vec<String>>,
        /// The batches under the notes folder the manager handed to the shell, in order.
        notes: Mutex<Vec<(String, NotesBatch)>>,
        /// How many times the manager announced a change of the recent projects.
        recent_changes: AtomicUsize,
    }

    impl FakeShell {
        fn note(&self, line: impl Into<String>) {
            self.log.lock().unwrap().push(line.into());
        }
        fn log(&self) -> Vec<String> {
            self.log.lock().unwrap().clone()
        }
        fn windows(&self) -> Vec<String> {
            self.windows.lock().unwrap().clone()
        }
        fn answer(&self, yes: bool) {
            self.answer_with(if yes {
                Confirmation::Accept
            } else {
                Confirmation::Cancel
            });
        }
        fn answer_with(&self, answer: Confirmation) {
            self.answers.lock().unwrap().push(answer);
        }
    }

    impl Shell for FakeShell {
        fn create_project_window(&self, label: &str, title: &str) -> Result<(), String> {
            if *self.fail_windows.lock().unwrap() {
                return Err("no display".into());
            }
            self.note(format!("create {label} {title}"));
            self.windows.lock().unwrap().push(label.to_owned());
            Ok(())
        }
        fn show_welcome(&self) -> Result<(), String> {
            let mut windows = self.windows.lock().unwrap();
            if !windows.iter().any(|w| w == "welcome") {
                windows.push("welcome".to_owned());
                drop(windows);
                self.note("create welcome");
            } else {
                drop(windows);
                self.note("focus welcome");
            }
            Ok(())
        }
        fn close_welcome(&self) {
            let mut windows = self.windows.lock().unwrap();
            if let Some(at) = windows.iter().position(|w| w == "welcome") {
                windows.remove(at);
                drop(windows);
                self.note("close welcome");
            }
        }
        fn focus(&self, label: &str) -> bool {
            let exists = self.windows.lock().unwrap().iter().any(|w| w == label);
            if exists {
                self.hidden.lock().unwrap().retain(|w| w != label);
                self.note(format!("focus {label}"));
            }
            exists
        }
        fn focus_any(&self) -> Option<String> {
            let first = self.windows.lock().unwrap().first().cloned();
            if let Some(label) = &first {
                self.hidden.lock().unwrap().retain(|w| w != label);
                self.note(format!("focus {label}"));
            }
            first
        }
        fn hide(&self, label: &str) -> Result<(), String> {
            if *self.fail_hide.lock().unwrap() {
                return Err("no window system".into());
            }
            self.hidden.lock().unwrap().push(label.to_owned());
            self.note(format!("hide {label}"));
            Ok(())
        }
        fn reveal(&self, label: &str) -> bool {
            let exists = self.windows.lock().unwrap().iter().any(|w| w == label);
            if exists {
                self.hidden.lock().unwrap().retain(|w| w != label);
                self.note(format!("reveal {label}"));
            }
            exists
        }
        fn destroy(&self, label: &str) {
            self.windows.lock().unwrap().retain(|w| w != label);
            self.note(format!("destroy {label}"));
        }
        fn window_count(&self) -> usize {
            self.windows.lock().unwrap().len()
        }
        fn confirm(&self, prompt: ConfirmPrompt) -> BoxFuture<Confirmation> {
            self.note(format!("confirm {}", prompt.title));
            self.prompts.lock().unwrap().push(prompt);
            let answer = self.answers.lock().unwrap().remove(0);
            Box::pin(async move { answer })
        }
        fn request_tab_texts(&self, project_id: &str) -> bool {
            self.note(format!("flush {project_id}"));
            let Some(texts) = self.texts.lock().unwrap().clone() else {
                return true;
            };
            let manager = self
                .manager
                .lock()
                .unwrap()
                .clone()
                .and_then(|m| m.upgrade());
            if let Some(manager) = manager {
                manager.tabs_flushed(project_id, texts);
            }
            true
        }
        fn save_tab(
            &self,
            _project_id: &str,
            path: &str,
            text: &str,
        ) -> BoxFuture<Result<String, String>> {
            self.note(format!("write {path} {text}"));
            let refused = self.refused.lock().unwrap().get(path).cloned();
            let hash = format!("hash-of-{path}");
            Box::pin(async move { refused.map_or(Ok(hash), Err) })
        }
        fn save_failed(&self, failed: FilesSaveFailed) {
            self.note(format!("save failed {}", failed.failures.len()));
            self.failed.lock().unwrap().push(failed);
        }
        fn drop_tabs(&self, project_id: &str) {
            self.note(format!("drop tabs {project_id}"));
        }
        fn exit(&self, code: i32) {
            self.note(format!("exit {code}"));
        }
        fn emit_show(&self, label: &str, request: &ShowRequest) {
            self.note(format!(
                "show {label} {:?} {:?}",
                request.view, request.job_id
            ));
        }
        fn emit_hide_failed(&self, label: &str, failure: &HideFailed) {
            self.note(format!("hide failed {label} {}", failure.reason));
        }
        fn assets_changed(&self, project_id: &str, _root: &Path) {
            self.assets.lock().unwrap().push(project_id.to_owned());
        }
        fn notes_changed(&self, project_id: &str, _root: &Path, batch: NotesBatch) {
            self.notes
                .lock()
                .unwrap()
                .push((project_id.to_owned(), batch));
        }
        fn publish(&self, change: ProjectChanged) {
            self.published.lock().unwrap().push(change);
        }
        fn recent_changed(&self) {
            self.recent_changes.fetch_add(1, Ordering::SeqCst);
        }
        fn save_window_state(&self) {
            self.note("save window state");
        }
    }

    /// Keeps the sink of each watcher so a test can feed a batch.
    #[derive(Default)]
    struct FakeWatch {
        sinks: Mutex<HashMap<PathBuf, BatchSink>>,
        stopped: Arc<AtomicUsize>,
        fail: Mutex<bool>,
    }

    struct CountOnDrop(Arc<AtomicUsize>);
    impl Drop for CountOnDrop {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    impl Watch for FakeWatch {
        fn start(&self, root: &Path, sink: BatchSink) -> Result<WatchGuard, String> {
            if *self.fail.lock().unwrap() {
                return Err("cannot watch".into());
            }
            self.sinks.lock().unwrap().insert(root.to_path_buf(), sink);
            Ok(WatchGuard::new(CountOnDrop(self.stopped.clone())))
        }
    }

    /// Records what the jobs were told, and reports a number of working jobs per project.
    #[derive(Default)]
    struct FakeJobs {
        working: Mutex<HashMap<String, usize>>,
        events: Mutex<Vec<String>>,
    }

    impl FakeJobs {
        fn set_working(&self, id: &str, n: usize) {
            self.working.lock().unwrap().insert(id.to_owned(), n);
        }
        fn events(&self) -> Vec<String> {
            self.events.lock().unwrap().clone()
        }
    }

    impl WindowJobs for FakeJobs {
        fn working_jobs(&self, project_id: &str) -> usize {
            self.working
                .lock()
                .unwrap()
                .get(project_id)
                .copied()
                .unwrap_or(0)
        }
        fn window_opened(&self, event: &WindowOpened) {
            self.events
                .lock()
                .unwrap()
                .push(format!("opened {} {:?}", event.project_id, event.reason));
        }
        fn window_closed(&self, project_id: &str, reason: CloseReason) -> BoxFuture<()> {
            self.events
                .lock()
                .unwrap()
                .push(format!("closed {project_id} {reason:?}"));
            Box::pin(async {})
        }
    }

    struct Fixture {
        manager: Arc<WindowManager>,
        shell: Arc<FakeShell>,
        watch: Arc<FakeWatch>,
        jobs: Arc<FakeJobs>,
        store: Arc<StateStore>,
        failures: Arc<Mutex<Vec<OpenFailure>>>,
        data: tempfile::TempDir,
        _clock: Arc<ManualClock>,
    }

    fn fixture() -> Fixture {
        let data = tempfile::tempdir().unwrap();
        let shell = Arc::new(FakeShell::default());
        let watch = Arc::new(FakeWatch::default());
        let jobs = Arc::new(FakeJobs::default());
        let store = Arc::new(StateStore::in_dir(data.path()));
        let clock = Arc::new(ManualClock::at("2026-11-02T07:00:00.000Z"));
        let failures: Arc<Mutex<Vec<OpenFailure>>> = Arc::default();
        let sink = failures.clone();
        let manager = WindowManager::new(
            shell.clone(),
            Arc::new(Pictures::new()),
            store.clone(),
            clock.clone(),
            watch.clone(),
            Arc::new(move |failure| sink.lock().unwrap().push(failure)),
        );
        manager.set_jobs(jobs.clone());
        *shell.manager.lock().unwrap() = Some(Arc::downgrade(&manager));
        Fixture {
            manager,
            shell,
            watch,
            jobs,
            store,
            failures,
            data,
            _clock: clock,
        }
    }

    /// A project folder in a temporary folder; returns its canonical root and its hash.
    fn project(name: &str) -> (tempfile::TempDir, PathBuf, String) {
        let parent = tempfile::tempdir().unwrap();
        let root = parent.path().join(name);
        std::fs::create_dir_all(root.join(".weaver")).unwrap();
        let canonical = paths::canonical_root(&root).unwrap();
        let id = paths::project_hash_of_canonical(&canonical);
        (parent, canonical, id)
    }

    #[tokio::test]
    async fn opening_a_folder_opens_its_window_once_and_a_second_open_brings_it_forward() {
        let f = fixture();
        let (_keep, root, id) = project("alpha");
        assert_eq!(
            f.manager
                .open_project(root.clone(), OpenReason::Developer)
                .await
                .unwrap(),
            OpenOutcome::Opened
        );
        assert_eq!(
            f.manager
                .open_project(root.clone(), OpenReason::Developer)
                .await
                .unwrap(),
            OpenOutcome::Focused
        );
        // A path that reaches the same folder another way is the same project.
        let roundabout = root.join(".weaver").join("..");
        assert_eq!(
            f.manager
                .open_project(roundabout, OpenReason::Developer)
                .await
                .unwrap(),
            OpenOutcome::Focused
        );

        let label = label_of(&id);
        let created = f
            .shell
            .log()
            .iter()
            .filter(|l| l.starts_with("create p-"))
            .count();
        assert_eq!(
            created,
            1,
            "never two windows for a project: {:?}",
            f.shell.log()
        );
        assert_eq!(f.shell.windows(), std::slice::from_ref(&label));
        assert!(
            f.shell.log().contains(&format!("create {label} alpha")),
            "the title is the folder name"
        );
        assert_eq!(
            f.shell
                .log()
                .iter()
                .filter(|l| **l == format!("focus {label}"))
                .count(),
            2
        );
        assert_eq!(f.jobs.events(), [format!("opened {id} Developer")]);
        assert_eq!(f.watch.sinks.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn opening_a_project_closes_the_welcome_window_after_the_project_window_exists() {
        let f = fixture();
        f.shell.show_welcome().unwrap();
        let (_keep, root, id) = project("alpha");
        f.manager
            .open_project(root, OpenReason::Developer)
            .await
            .unwrap();
        let log = f.shell.log();
        let created = log.iter().position(|l| l.starts_with("create p-")).unwrap();
        let closed = log.iter().position(|l| l == "close welcome").unwrap();
        assert!(created < closed, "{log:?}");
        assert_eq!(f.shell.windows(), [label_of(&id)]);
    }

    #[tokio::test]
    async fn opening_records_the_recent_project_and_the_session() {
        let f = fixture();
        let (_keep, root, _id) = project("alpha");
        f.manager
            .open_project(root.clone(), OpenReason::Developer)
            .await
            .unwrap();
        let state = f.store.snapshot();
        let path = root.to_string_lossy().into_owned();
        assert_eq!(state.recent[0].path, path);
        assert_eq!(state.recent[0].name, "alpha");
        assert_eq!(state.recent[0].opened, "2026-11-02T07:00:00.000Z");
        assert_eq!(state.last_session, [path]);
        assert!(f.data.path().join("state.json").exists());
    }

    #[tokio::test]
    async fn every_change_of_the_recent_projects_is_announced_to_the_shell() {
        let f = fixture();
        let (_keep, root, _id) = project("alpha");
        f.manager
            .open_project(root.clone(), OpenReason::Developer)
            .await
            .unwrap();
        assert_eq!(
            f.shell.recent_changes.load(Ordering::SeqCst),
            1,
            "an opening"
        );
        f.manager.remove_recent(&root.to_string_lossy()).unwrap();
        assert_eq!(
            f.shell.recent_changes.load(Ordering::SeqCst),
            2,
            "a removal"
        );
        f.manager.clear_recent().unwrap();
        assert_eq!(
            f.shell.recent_changes.load(Ordering::SeqCst),
            3,
            "Clear Recent"
        );
    }

    #[tokio::test]
    async fn clear_recent_empties_the_list_and_keeps_the_session_and_the_windows() {
        let f = fixture();
        let (_a, root_a, id_a) = project("alpha");
        let (_b, root_b, _id_b) = project("beta");
        f.manager
            .open_project(root_a.clone(), OpenReason::Developer)
            .await
            .unwrap();
        f.manager
            .open_project(root_b.clone(), OpenReason::Developer)
            .await
            .unwrap();
        let session = f.store.snapshot().last_session;
        assert_eq!(f.store.snapshot().recent.len(), 2);

        let remaining = f.manager.clear_recent().unwrap();

        assert!(remaining.recent.is_empty());
        assert!(f.store.snapshot().recent.is_empty());
        assert_eq!(f.store.snapshot().last_session, session);
        assert_eq!(f.manager.open_projects().len(), 2);
        assert!(f.shell.windows().contains(&label_of(&id_a)));
        let on_disk = StateStore::in_dir(f.data.path()).snapshot();
        assert!(on_disk.recent.is_empty());
        assert_eq!(on_disk.last_session, session);
    }

    #[test]
    fn a_failure_waiting_for_the_welcome_window_is_handed_over_once() {
        let f = fixture();
        assert_eq!(f.manager.take_welcome_failure(), None);
        let failure = MenuOpenFailed {
            name: Some("alpha".to_owned()),
            reason: "the folder does not exist".to_owned(),
        };
        f.manager.set_welcome_failure(failure.clone());
        assert_eq!(f.manager.take_welcome_failure(), Some(failure));
        assert_eq!(f.manager.take_welcome_failure(), None);
    }

    #[tokio::test]
    async fn a_folder_that_does_not_exist_or_is_a_file_is_refused_and_leaves_the_welcome_window() {
        let f = fixture();
        f.manager.restore(None).await;
        let dir = tempfile::tempdir().unwrap();
        let error = f
            .manager
            .open_project(dir.path().join("missing"), OpenReason::Developer)
            .await
            .unwrap_err();
        assert_eq!(error.message, "the folder does not exist");
        let file = dir.path().join("a.txt");
        std::fs::write(&file, "x").unwrap();
        let error = f
            .manager
            .open_project(file, OpenReason::Developer)
            .await
            .unwrap_err();
        assert_eq!(error.message, "it is not a folder");
        assert_eq!(
            f.shell.windows(),
            ["welcome"],
            "with no project window the welcome window shows"
        );
        assert!(f.manager.open_projects().is_empty());
    }

    #[tokio::test]
    async fn a_window_that_cannot_be_created_leaves_no_picture_nor_watcher_behind() {
        let f = fixture();
        *f.shell.fail_windows.lock().unwrap() = true;
        let (_keep, root, id) = project("alpha");
        let error = f
            .manager
            .open_project(root.clone(), OpenReason::Developer)
            .await
            .unwrap_err();
        assert!(error.message.contains("no display"), "{}", error.message);
        assert!(f.manager.pictures.get(&id).is_err());
        assert_eq!(f.watch.stopped.load(Ordering::SeqCst), 1);
        // The project can be opened again once the window can be created.
        *f.shell.fail_windows.lock().unwrap() = false;
        assert_eq!(
            f.manager
                .open_project(root, OpenReason::Developer)
                .await
                .unwrap(),
            OpenOutcome::Opened
        );
    }

    #[tokio::test]
    async fn a_watcher_that_cannot_start_fails_the_open_with_its_reason() {
        let f = fixture();
        *f.watch.fail.lock().unwrap() = true;
        let (_keep, root, id) = project("alpha");
        let error = f
            .manager
            .open_project(root, OpenReason::Developer)
            .await
            .unwrap_err();
        assert_eq!(error.message, "cannot watch");
        assert!(f.manager.pictures.get(&id).is_err());
        assert!(!f.shell.log().iter().any(|l| l.starts_with("create p-")));
    }

    #[tokio::test]
    async fn a_batch_from_the_watcher_rebuilds_the_picture_and_tells_the_ui_once() {
        let f = fixture();
        let (_keep, root, id) = project("alpha");
        f.manager
            .open_project(root.clone(), OpenReason::Developer)
            .await
            .unwrap();
        let before = f.manager.pictures.get(&id).unwrap().revision;

        let file = root.join(".weaver").join("model").join("system.md");
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, "---\nname: Alpha\n---\n\n# Alpha\n").unwrap();
        let sink = f.watch.sinks.lock().unwrap().get(&root).unwrap().clone();
        sink(Rebuild::Paths(vec![file]));

        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        loop {
            let published = f.shell.published.lock().unwrap().clone();
            if let Some(change) = published.first() {
                assert_eq!(published.len(), 1, "one batch, one event");
                assert_eq!(change.project_id, id);
                assert!(change.revision > before);
                assert!(
                    change.slices.iter().any(|s| s == "model"),
                    "{:?}",
                    change.slices
                );
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "no project-changed event"
            );
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }

    /// How many times the assets of the shell were asked to be read, waiting for `count` (the
    /// batch is handled on the async runtime).
    async fn assets_reads(shell: &FakeShell, count: usize) -> Vec<String> {
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        loop {
            let reads = shell.assets.lock().unwrap().clone();
            if reads.len() >= count {
                return reads;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "the assets were not read again"
            );
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }

    #[tokio::test]
    async fn the_assets_are_read_when_the_project_opens_and_after_a_batch_under_their_folder_which_never_reaches_the_picture(
    ) {
        let f = fixture();
        let (_keep, root, id) = project("alpha");
        f.manager
            .open_project(root.clone(), OpenReason::Developer)
            .await
            .unwrap();
        assert_eq!(
            assets_reads(&f.shell, 1).await,
            std::slice::from_ref(&id),
            "read when the project opens"
        );
        let before = f.manager.pictures.get(&id).unwrap().revision;

        let assets = root.join(".weaver").join("assets");
        std::fs::create_dir_all(&assets).unwrap();
        std::fs::write(assets.join("logo.png"), "png").unwrap();
        let sink = f.watch.sinks.lock().unwrap().get(&root).unwrap().clone();
        sink(Rebuild::Paths(vec![
            assets.clone(),
            assets.join("logo.png"),
        ]));
        assert_eq!(assets_reads(&f.shell, 2).await.len(), 2);
        // Time for a rebuild that must not happen.
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert!(
            f.shell.published.lock().unwrap().is_empty(),
            "no project-changed for an asset"
        );
        assert_eq!(f.manager.pictures.get(&id).unwrap().revision, before);

        // A batch that also names a file of the Model still rebuilds the picture, once.
        let file = root.join(".weaver").join("model").join("system.md");
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, "---\nname: Alpha\n---\n\n# Alpha\n").unwrap();
        sink(Rebuild::Paths(vec![assets.join("logo.png"), file]));
        assert_eq!(assets_reads(&f.shell, 3).await.len(), 3);
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        while f.shell.published.lock().unwrap().is_empty() {
            assert!(
                std::time::Instant::now() < deadline,
                "no project-changed event"
            );
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert_eq!(f.shell.published.lock().unwrap().len(), 1);

        // A batch that names no path, events lost, reads the assets and rebuilds everything.
        sink(Rebuild::All);
        assert_eq!(assets_reads(&f.shell, 4).await.len(), 4);
    }

    /// The batches the shell was asked to tell the Notes view about, waiting for `count`.
    async fn notes_batches(shell: &FakeShell, count: usize) -> Vec<(String, NotesBatch)> {
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        loop {
            let batches = shell.notes.lock().unwrap().clone();
            if batches.len() >= count {
                return batches;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "the notes were not told of the batch"
            );
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }

    #[tokio::test]
    async fn a_batch_under_the_notes_folder_only_reaches_the_notes_and_never_the_picture() {
        let f = fixture();
        let (_keep, root, id) = project("alpha");
        f.manager
            .open_project(root.clone(), OpenReason::Developer)
            .await
            .unwrap();
        let before = f.manager.pictures.get(&id).unwrap().revision;

        let notes = root.join(".weaver").join("notes");
        std::fs::create_dir_all(&notes).unwrap();
        std::fs::write(notes.join("notes.md"), "my notes\n").unwrap();
        let sink = f.watch.sinks.lock().unwrap().get(&root).unwrap().clone();
        sink(Rebuild::Paths(vec![
            notes.clone(),
            notes.join("notes.md"),
            notes.join("shot.png"),
        ]));
        let batches = notes_batches(&f.shell, 1).await;
        assert_eq!(batches[0].0, id);
        assert!(batches[0].1.sheet);
        assert!(batches[0].1.candidates.contains("shot.png"));
        // Time for a rebuild that must not happen.
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert!(
            f.shell.published.lock().unwrap().is_empty(),
            "no project-changed for the notes"
        );
        assert_eq!(f.manager.pictures.get(&id).unwrap().revision, before);

        // Hidden files of the folder tell the Notes view nothing and reach no one else.
        sink(Rebuild::Paths(vec![notes.join(".gitignore")]));
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert_eq!(f.shell.notes.lock().unwrap().len(), 1);
        assert!(f.shell.published.lock().unwrap().is_empty());

        // A batch that also names a file of the Model still rebuilds the picture, once.
        let file = root.join(".weaver").join("model").join("system.md");
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, "---\nname: Alpha\n---\n\n# Alpha\n").unwrap();
        sink(Rebuild::Paths(vec![notes.join("notes.md"), file]));
        assert_eq!(notes_batches(&f.shell, 2).await.len(), 2);
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        while f.shell.published.lock().unwrap().is_empty() {
            assert!(
                std::time::Instant::now() < deadline,
                "no project-changed event"
            );
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert_eq!(f.shell.published.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn a_notification_click_brings_the_window_forward_showing_what_it_was_about() {
        let f = fixture();
        let (_keep, root, id) = project("alpha");
        f.manager
            .open_project(root, OpenReason::Developer)
            .await
            .unwrap();
        let request = ShowRequest {
            project_id: id.clone(),
            view: ShowView::Jobs,
            job_id: Some("abcd1234".into()),
        };
        assert!(f.manager.focus_project(&id, Some(request)));
        let log = f.shell.log();
        assert_eq!(
            &log[log.len() - 2..],
            [
                format!("focus {}", label_of(&id)),
                format!("show {} Jobs Some(\"abcd1234\")", label_of(&id))
            ]
        );
        assert!(
            !f.manager.focus_project("0000000000000000", None),
            "a project without a window cannot come forward"
        );
    }

    /// What the project hooks heard of the focus.
    #[derive(Default)]
    struct HeardFocus(Mutex<Vec<String>>);

    impl ProjectHooks for HeardFocus {
        fn window_opened(&self, _project_id: &str, _root: &Path) {}
        fn batch_applied(&self, _project_id: &str, _revision: Option<&crate::pictures::Revision>) {}
        fn window_focused(&self, project_id: &str) {
            self.0.lock().unwrap().push(project_id.to_owned());
        }
        fn window_closed(&self, _project_id: &str) {}
    }

    /// `desktop.WINDOW_FOCUSED`: a project window that comes back to the foreground is told to the
    /// project's hooks, with its project; the welcome window is not a project.
    #[tokio::test]
    async fn a_project_window_coming_back_to_the_foreground_is_told_to_the_project_hooks() {
        let f = fixture();
        let heard = Arc::new(HeardFocus::default());
        f.manager.set_project(heard.clone());
        let (_a, root_a, id_a) = project("alpha");
        f.manager
            .open_project(root_a, OpenReason::Developer)
            .await
            .unwrap();
        f.manager.on_focused(&label_of(&id_a));
        f.manager.on_focused("welcome");
        f.manager.on_focused(&label_of(&id_a));
        assert_eq!(*heard.0.lock().unwrap(), [id_a.clone(), id_a]);
    }

    #[tokio::test]
    async fn the_focus_order_lists_the_most_recently_focused_window_first_and_the_welcome_window_shows(
    ) {
        let f = fixture();
        let (_a, root_a, id_a) = project("alpha");
        let (_b, root_b, id_b) = project("beta");
        for root in [root_a, root_b] {
            f.manager
                .open_project(root, OpenReason::Developer)
                .await
                .unwrap();
        }
        assert_eq!(f.manager.focus_order(), [label_of(&id_b), label_of(&id_a)]);
        f.manager.on_focused(&label_of(&id_a));
        assert_eq!(f.manager.focus_order(), [label_of(&id_a), label_of(&id_b)]);

        let empty = fixture();
        empty.manager.show_welcome();
        assert_eq!(empty.shell.log().last().unwrap(), "create welcome");
        empty.manager.show_welcome();
        assert_eq!(empty.shell.log().last().unwrap(), "focus welcome");
    }

    #[tokio::test]
    async fn closing_a_window_without_jobs_working_closes_it_and_removes_it_from_the_session() {
        let f = fixture();
        let (_a, root_a, id_a) = project("alpha");
        let (_b, root_b, _id_b) = project("beta");
        f.manager
            .open_project(root_a.clone(), OpenReason::Developer)
            .await
            .unwrap();
        f.manager
            .open_project(root_b.clone(), OpenReason::Developer)
            .await
            .unwrap();

        assert_eq!(f.manager.close_window(&id_a).await, CloseOutcome::Closed);
        assert!(!f.shell.log().iter().any(|l| l.starts_with("confirm")));
        assert_eq!(
            f.store.snapshot().last_session,
            [root_b.to_string_lossy().into_owned()]
        );
        assert_eq!(
            f.store.snapshot().recent.len(),
            2,
            "closing a window keeps it in the recent projects"
        );
        assert!(f
            .jobs
            .events()
            .contains(&format!("closed {id_a} WindowClosed")));
        assert_eq!(
            f.shell.windows().len(),
            1,
            "no welcome window while another project window is open"
        );
        assert!(f.manager.pictures.get(&id_a).is_err());
        assert_eq!(f.watch.stopped.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn closing_the_last_project_window_opens_the_welcome_window_first() {
        let f = fixture();
        let (_a, root, id) = project("alpha");
        f.manager
            .open_project(root, OpenReason::Developer)
            .await
            .unwrap();
        assert_eq!(f.manager.close_window(&id).await, CloseOutcome::Closed);
        let log = f.shell.log();
        let welcome = log.iter().position(|l| l == "create welcome").unwrap();
        let destroyed = log
            .iter()
            .position(|l| l.starts_with("destroy p-"))
            .unwrap();
        assert!(welcome < destroyed, "{log:?}");
        assert_eq!(f.shell.windows(), ["welcome"]);
        assert!(f.store.snapshot().last_session.is_empty());
    }

    #[tokio::test]
    async fn closing_with_jobs_working_asks_the_number_and_cancel_changes_nothing() {
        let f = fixture();
        let (_a, root, id) = project("alpha");
        f.manager
            .open_project(root.clone(), OpenReason::Developer)
            .await
            .unwrap();
        f.jobs.set_working(&id, 2);
        f.shell.answer(false);
        assert_eq!(f.manager.close_window(&id).await, CloseOutcome::Cancelled);

        let prompt = f.shell.prompts.lock().unwrap()[0].clone();
        assert_eq!(prompt.title, "Close alpha?");
        assert!(
            prompt.message.starts_with("2 jobs are working in alpha."),
            "{}",
            prompt.message
        );
        assert_eq!(
            f.shell.windows(),
            [label_of(&id)],
            "the window is still there"
        );
        assert_eq!(
            f.jobs.events(),
            [format!("opened {id} Developer")],
            "no job was suspended"
        );
        assert_eq!(
            f.store.snapshot().last_session,
            [root.to_string_lossy().into_owned()]
        );
        assert!(f.manager.pictures.get(&id).is_ok());
        // A new request asks again, and confirming suspends the jobs, then closes.
        f.shell.answer(true);
        assert_eq!(f.manager.close_window(&id).await, CloseOutcome::Closed);
        let events = f.jobs.events();
        assert_eq!(events.last().unwrap(), &format!("closed {id} WindowClosed"));
        assert!(f.shell.windows().iter().all(|w| w != &label_of(&id)));
    }

    #[tokio::test]
    async fn one_job_is_worded_in_the_singular() {
        let close = close_prompt("alpha", 1, &[]);
        assert_eq!(
            close.message,
            "1 job is working in alpha. Closing the window suspends it."
        );
        let quit = quit_prompt(1);
        assert_eq!(
            quit.message,
            "1 job is working. Quitting suspends it; it resumes when Weaver starts again."
        );
        let quit = quit_prompt(3);
        assert_eq!(
            quit.message,
            "3 jobs are working. Quitting suspends them; they resume when Weaver starts again."
        );
        assert_eq!(quit.accept, "Suspend and Quit");
    }

    #[tokio::test]
    async fn quitting_writes_the_session_suspends_every_project_and_exits() {
        let f = fixture();
        let (_a, root_a, id_a) = project("alpha");
        let (_b, root_b, id_b) = project("beta");
        f.manager
            .open_project(root_a.clone(), OpenReason::Developer)
            .await
            .unwrap();
        f.manager
            .open_project(root_b.clone(), OpenReason::Developer)
            .await
            .unwrap();
        f.jobs.set_working(&id_a, 1);
        f.jobs.set_working(&id_b, 2);
        f.shell.answer(true);

        f.manager.quit(0, true).await;
        assert_eq!(
            f.shell.prompts.lock().unwrap()[0].message,
            "3 jobs are working. Quitting suspends them; they resume when Weaver starts again."
        );
        let events = f.jobs.events();
        assert!(
            events.contains(&format!("closed {id_a} AppQuit"))
                && events.contains(&format!("closed {id_b} AppQuit")),
            "{events:?}"
        );
        assert_eq!(
            f.store.snapshot().last_session,
            [
                root_a.to_string_lossy().into_owned(),
                root_b.to_string_lossy().into_owned()
            ],
            "quitting keeps the open windows in the session"
        );
        let log = f.shell.log();
        assert_eq!(log.last().unwrap(), "exit 0");
        assert!(
            f.shell.windows().len() == 2,
            "the windows are not destroyed one by one: the process exits"
        );
        // The exit that follows goes through.
        assert_eq!(f.manager.exit_requested(Some(0), 2), ExitDecision::Allow);
    }

    #[tokio::test]
    async fn cancelling_the_quit_changes_nothing_and_a_new_quit_asks_again() {
        let f = fixture();
        let (_a, root, id) = project("alpha");
        f.manager
            .open_project(root, OpenReason::Developer)
            .await
            .unwrap();
        f.jobs.set_working(&id, 1);
        f.shell.answer(false);
        f.manager.quit(0, true).await;
        assert!(!f.shell.log().iter().any(|l| l.starts_with("exit")));
        assert_eq!(f.jobs.events(), [format!("opened {id} Developer")]);
        f.shell.answer(true);
        f.manager.quit(0, true).await;
        assert_eq!(f.shell.log().last().unwrap(), "exit 0");
    }

    #[tokio::test]
    async fn a_shutdown_that_allows_no_confirmation_suspends_the_jobs_without_asking() {
        let f = fixture();
        let (_a, root, id) = project("alpha");
        f.manager
            .open_project(root, OpenReason::Developer)
            .await
            .unwrap();
        f.jobs.set_working(&id, 4);
        f.manager.quit(0, false).await;
        assert!(f.shell.prompts.lock().unwrap().is_empty());
        assert!(f.jobs.events().contains(&format!("closed {id} AppQuit")));
        assert_eq!(f.shell.log().last().unwrap(), "exit 0");
    }

    fn tab(path: &str, text: &str) -> TabText {
        TabText {
            path: path.to_owned(),
            text: text.to_owned(),
        }
    }

    async fn open(f: &Fixture, name: &str) -> (tempfile::TempDir, String) {
        let (keep, root, id) = project(name);
        f.manager
            .open_project(root, OpenReason::Developer)
            .await
            .unwrap();
        (keep, id)
    }

    #[tokio::test]
    async fn unsaved_changes_are_asked_with_save_all_discard_all_and_cancel_listing_the_files() {
        let f = fixture();
        let (_a, id) = open(&f, "alpha").await;
        f.jobs.set_working(&id, 2);
        f.manager
            .set_unsaved(&id, vec!["src/a.rs".into(), "docs/b.md".into()]);
        f.shell.answer(false);
        assert_eq!(f.manager.close_window(&id).await, CloseOutcome::Cancelled);

        let prompt = f.shell.prompts.lock().unwrap()[0].clone();
        assert_eq!(prompt.title, "Close alpha?");
        assert_eq!(prompt.accept, "Save all");
        assert_eq!(prompt.discard.as_deref(), Some("Discard all"));
        assert_eq!(
            prompt.message,
            "Unsaved changes in:\n• src/a.rs\n• docs/b.md\n\n2 jobs are working in alpha. Closing the window suspends them."
        );
        assert!(
            !f.shell.log().iter().any(|l| l.starts_with("flush")),
            "cancel saves nothing"
        );
        assert_eq!(f.jobs.events(), [format!("opened {id} Developer")]);
        assert_eq!(f.shell.windows(), [label_of(&id)]);
    }

    #[tokio::test]
    async fn jobs_alone_keep_the_two_button_confirmation() {
        let prompt = close_prompt("alpha", 3, &[]);
        assert_eq!(prompt.accept, "Suspend and Close");
        assert_eq!(prompt.discard, None);
        let only_files = close_prompt("alpha", 0, &["a.txt".to_owned()]);
        assert_eq!(only_files.message, "Unsaved changes in:\n• a.txt");
        let quit = quit_unsaved_prompt("beta", 1, &["b.txt".to_owned()]);
        assert_eq!(
            quit.message,
            "beta: Unsaved changes in:\n• b.txt\n\n1 job is working. Quitting suspends it; it resumes when Weaver starts again."
        );
        assert_eq!(quit.discard.as_deref(), Some("Discard all"));
    }

    #[tokio::test]
    async fn discard_all_closes_the_window_without_saving() {
        let f = fixture();
        let (_a, id) = open(&f, "alpha").await;
        f.manager.set_unsaved(&id, vec!["a.txt".into()]);
        f.shell.answer_with(Confirmation::Discard);
        assert_eq!(f.manager.close_window(&id).await, CloseOutcome::Closed);
        assert!(!f
            .shell
            .log()
            .iter()
            .any(|l| l.starts_with("flush") || l.starts_with("write ")));
        assert!(f.manager.unsaved(&id).is_empty());
        assert!(f.shell.windows().iter().all(|w| w != &label_of(&id)));
    }

    #[tokio::test]
    async fn save_all_saves_the_texts_the_window_sends_then_suspends_the_jobs_and_closes() {
        let f = fixture();
        let (_a, id) = open(&f, "alpha").await;
        f.jobs.set_working(&id, 1);
        f.manager
            .set_unsaved(&id, vec!["a.txt".into(), "b.txt".into()]);
        *f.shell.texts.lock().unwrap() = Some(vec![tab("a.txt", "one"), tab("b.txt", "two")]);
        f.shell.answer(true);
        assert_eq!(f.manager.close_window(&id).await, CloseOutcome::Closed);
        let log = f.shell.log();
        let at = |line: &str| {
            log.iter()
                .position(|l| l == line)
                .unwrap_or_else(|| panic!("{line} in {log:?}"))
        };
        assert!(at(&format!("flush {id}")) < at("write a.txt one"));
        assert!(at("write a.txt one") < at("write b.txt two"));
        assert!(f
            .jobs
            .events()
            .contains(&format!("closed {id} WindowClosed")));
        assert!(f.shell.windows().iter().all(|w| w != &label_of(&id)));
        assert!(f.shell.failed.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn a_file_that_cannot_be_saved_keeps_the_window_open_saves_the_others_and_suspends_nothing(
    ) {
        let f = fixture();
        let (_a, id) = open(&f, "alpha").await;
        f.jobs.set_working(&id, 1);
        f.manager
            .set_unsaved(&id, vec!["a.txt".into(), "b.txt".into()]);
        *f.shell.texts.lock().unwrap() = Some(vec![tab("a.txt", "one"), tab("b.txt", "two")]);
        f.shell.refused.lock().unwrap().insert(
            "a.txt".into(),
            "the file changed on disk since it was opened".into(),
        );
        f.shell.answer(true);
        assert_eq!(f.manager.close_window(&id).await, CloseOutcome::NotSaved);
        assert!(f.shell.log().contains(&"write b.txt two".to_owned()));
        assert_eq!(
            f.shell.failed.lock().unwrap().clone(),
            [FilesSaveFailed {
                project_id: id.clone(),
                saved: vec![SavedFile {
                    path: "b.txt".into(),
                    hash: "hash-of-b.txt".into(),
                }],
                failures: vec![SaveFailure {
                    path: "a.txt".into(),
                    reason: "the file changed on disk since it was opened".into(),
                }],
            }]
        );
        assert_eq!(f.jobs.events(), [format!("opened {id} Developer")]);
        assert_eq!(f.shell.windows(), [label_of(&id)]);
        // The window can be closed again afterwards.
        f.shell.refused.lock().unwrap().clear();
        f.shell.answer(true);
        assert_eq!(f.manager.close_window(&id).await, CloseOutcome::Closed);
    }

    #[tokio::test]
    async fn a_window_that_does_not_answer_in_time_stays_open_with_every_file_failed() {
        let f = fixture();
        let (_a, id) = open(&f, "alpha").await;
        f.manager.set_flush_limit(Duration::from_millis(30));
        f.manager
            .set_unsaved(&id, vec!["a.txt".into(), "b.txt".into()]);
        f.shell.answer(true);
        assert_eq!(f.manager.close_window(&id).await, CloseOutcome::NotSaved);
        let failed = f.shell.failed.lock().unwrap().clone();
        let paths: Vec<_> = failed[0].failures.iter().map(|x| x.path.as_str()).collect();
        assert_eq!(paths, ["a.txt", "b.txt"]);
        assert!(!f.shell.log().iter().any(|l| l.starts_with("write ")));
        assert!(f.jobs.events().iter().all(|e| !e.starts_with("closed")));
        assert_eq!(f.shell.windows(), [label_of(&id)]);
    }

    #[tokio::test]
    async fn quitting_asks_each_window_with_unsaved_changes_in_turn_counting_every_jobs() {
        let f = fixture();
        let (_a, id_a) = open(&f, "alpha").await;
        let (_b, id_b) = open(&f, "beta").await;
        let (_c, id_c) = open(&f, "gamma").await;
        f.jobs.set_working(&id_c, 2);
        f.manager.set_unsaved(&id_a, vec!["a.txt".into()]);
        f.manager.set_unsaved(&id_b, vec!["b.txt".into()]);
        *f.shell.texts.lock().unwrap() = Some(vec![tab("a.txt", "A")]);
        f.shell.answer(true);
        f.shell.answer_with(Confirmation::Discard);
        f.manager.quit(0, true).await;

        let prompts = f.shell.prompts.lock().unwrap().clone();
        assert_eq!(
            prompts.len(),
            2,
            "gamma has no unsaved change and asks nothing"
        );
        assert!(prompts[0]
            .message
            .starts_with("alpha: Unsaved changes in:\n• a.txt"));
        assert!(prompts[1]
            .message
            .starts_with("beta: Unsaved changes in:\n• b.txt"));
        for prompt in &prompts {
            assert!(prompt.message.ends_with(
                "2 jobs are working. Quitting suspends them; they resume when Weaver starts again."
            ));
        }
        let log = f.shell.log();
        assert!(log.contains(&"write a.txt A".to_owned()));
        assert!(!log.iter().any(|l| l.contains("write b.txt")));
        assert_eq!(log.last().unwrap(), "exit 0");
    }

    #[tokio::test]
    async fn cancel_in_any_window_stops_the_quit_before_anything_is_saved() {
        let f = fixture();
        let (_a, id_a) = open(&f, "alpha").await;
        let (_b, id_b) = open(&f, "beta").await;
        f.jobs.set_working(&id_a, 1);
        f.manager.set_unsaved(&id_a, vec!["a.txt".into()]);
        f.manager.set_unsaved(&id_b, vec!["b.txt".into()]);
        *f.shell.texts.lock().unwrap() = Some(vec![tab("a.txt", "A"), tab("b.txt", "B")]);
        f.shell.answer(true);
        f.shell.answer(false);
        f.manager.quit(0, true).await;
        let log = f.shell.log();
        assert!(!log
            .iter()
            .any(|l| l.starts_with("write ") || l.starts_with("exit")));
        assert!(f.jobs.events().iter().all(|e| !e.starts_with("closed")));
        // The quit can be asked again.
        f.shell.answer_with(Confirmation::Discard);
        f.shell.answer_with(Confirmation::Discard);
        f.manager.quit(0, true).await;
        assert_eq!(f.shell.log().last().unwrap(), "exit 0");
    }

    #[tokio::test]
    async fn a_failed_save_on_quit_keeps_weaver_running_and_suspends_nothing() {
        let f = fixture();
        let (_a, id_a) = open(&f, "alpha").await;
        f.jobs.set_working(&id_a, 1);
        f.manager
            .set_unsaved(&id_a, vec!["a.txt".into(), "b.txt".into()]);
        *f.shell.texts.lock().unwrap() = Some(vec![tab("a.txt", "A"), tab("b.txt", "B")]);
        f.shell
            .refused
            .lock()
            .unwrap()
            .insert("b.txt".into(), "disk full".into());
        f.shell.answer(true);
        f.manager.quit(0, true).await;
        let log = f.shell.log();
        assert!(
            log.contains(&"write a.txt A".to_owned()),
            "the others are saved"
        );
        assert!(!log.iter().any(|l| l.starts_with("exit")));
        assert!(f.jobs.events().iter().all(|e| !e.starts_with("closed")));
        assert_eq!(
            f.shell.failed.lock().unwrap()[0].failures[0].reason,
            "disk full"
        );
        // Weaver can be quit again afterwards.
        f.shell.refused.lock().unwrap().clear();
        f.shell.answer(true);
        f.manager.quit(0, true).await;
        assert_eq!(f.shell.log().last().unwrap(), "exit 0");
    }

    #[tokio::test]
    async fn a_shutdown_that_allows_no_confirmation_asks_nothing_about_unsaved_changes() {
        let f = fixture();
        let (_a, id) = open(&f, "alpha").await;
        f.manager.set_unsaved(&id, vec!["a.txt".into()]);
        f.manager.quit(0, false).await;
        assert!(f.shell.prompts.lock().unwrap().is_empty());
        assert_eq!(f.shell.log().last().unwrap(), "exit 0");
    }

    #[tokio::test]
    async fn a_quit_asked_with_a_code_exits_with_that_code() {
        let f = fixture();
        f.manager.quit(7, true).await;
        assert_eq!(f.shell.log().last().unwrap(), "exit 7");
    }

    #[tokio::test]
    async fn a_quit_while_the_session_is_being_restored_does_not_overwrite_it() {
        let f = fixture();
        let (_a, root_a, _) = project("alpha");
        let (_b, root_b, _) = project("beta");
        let (a, b) = (
            root_a.to_string_lossy().into_owned(),
            root_b.to_string_lossy().into_owned(),
        );
        f.store
            .update(|s| s.set_last_session(vec![a.clone(), b.clone()]))
            .unwrap();
        f.manager.registry().set_restoring(true);
        f.manager.quit(0, false).await;
        assert_eq!(f.store.snapshot().last_session, [a, b]);
    }

    #[tokio::test]
    async fn start_reopens_the_projects_of_the_last_session_in_order_and_resumes_their_jobs() {
        let f = fixture();
        let (_a, root_a, id_a) = project("alpha");
        let (_b, root_b, id_b) = project("beta");
        f.store
            .update(|s| {
                s.set_last_session(vec![
                    root_b.to_string_lossy().into_owned(),
                    root_a.to_string_lossy().into_owned(),
                ])
            })
            .unwrap();
        f.manager.restore(None).await;

        assert_eq!(f.shell.windows(), [label_of(&id_b), label_of(&id_a)]);
        assert_eq!(
            f.jobs.events(),
            [
                format!("opened {id_b} AppStart"),
                format!("opened {id_a} AppStart")
            ]
        );
        assert!(!f.shell.windows().iter().any(|w| w == "welcome"));
        assert!(f.failures.lock().unwrap().is_empty());
        assert!(!f.manager.registry().is_restoring());
    }

    #[tokio::test]
    async fn start_without_a_last_session_shows_the_welcome_window() {
        let f = fixture();
        f.manager.restore(None).await;
        assert_eq!(f.shell.windows(), ["welcome"]);
        assert!(f.jobs.events().is_empty());
    }

    #[tokio::test]
    async fn a_project_of_the_session_that_cannot_open_is_reported_and_the_others_open() {
        let f = fixture();
        let (_a, root_a, id_a) = project("alpha");
        let gone = root_a.parent().unwrap().join("gone");
        f.store
            .update(|s| {
                s.set_last_session(vec![
                    gone.to_string_lossy().into_owned(),
                    root_a.to_string_lossy().into_owned(),
                ])
            })
            .unwrap();
        f.manager.restore(None).await;
        assert_eq!(f.shell.windows(), [label_of(&id_a)]);
        let failures = f.failures.lock().unwrap().clone();
        assert_eq!(failures.len(), 1);
        assert_eq!(failures[0].name, "gone");
        assert_eq!(failures[0].reason, "the folder does not exist");
    }

    #[tokio::test]
    async fn when_no_project_of_the_session_opens_the_welcome_window_shows() {
        let f = fixture();
        let dir = tempfile::tempdir().unwrap();
        let gone = dir.path().join("gone").to_string_lossy().into_owned();
        f.store.update(|s| s.set_last_session(vec![gone])).unwrap();
        f.manager.restore(None).await;
        assert_eq!(f.shell.windows(), ["welcome"]);
        assert_eq!(f.failures.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn a_second_launch_with_a_folder_opens_or_focuses_and_without_one_focuses_the_recent_window(
    ) {
        let f = fixture();
        let (_a, root_a, id_a) = project("alpha");
        let (_b, root_b, id_b) = project("beta");
        let program = "weaver".to_owned();
        let launch = |root: &Path| vec![program.clone(), root.to_string_lossy().into_owned()];

        f.manager.second_launch(launch(&root_a), PathBuf::from("/"));
        f.manager.second_launch(launch(&root_b), PathBuf::from("/"));
        // The opens run in the background: wait for both windows.
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        while f.shell.windows().len() < 2 {
            assert!(
                std::time::Instant::now() < deadline,
                "the windows did not open"
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        f.manager.second_launch(launch(&root_a), PathBuf::from("/"));
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        while !f
            .shell
            .log()
            .iter()
            .any(|l| *l == format!("focus {}", label_of(&id_a)))
        {
            assert!(
                std::time::Instant::now() < deadline,
                "the window of alpha did not come forward"
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert_eq!(
            f.shell.windows().len(),
            2,
            "never two windows for a project"
        );

        // Without a folder the most recently focused window comes forward.
        f.manager.on_focused(&label_of(&id_b));
        f.manager.second_launch(vec![program], PathBuf::from("/"));
        assert_eq!(
            f.shell.log().last().unwrap(),
            &format!("focus {}", label_of(&id_b))
        );
    }

    #[tokio::test]
    async fn the_dock_icon_with_no_window_shows_the_welcome_window() {
        let f = fixture();
        f.manager.reopen(false);
        assert_eq!(f.shell.windows(), ["welcome"]);
        f.manager.reopen(true);
        assert_eq!(f.shell.windows(), ["welcome"]);
    }

    #[tokio::test]
    async fn a_window_destroyed_by_the_system_is_closed_like_the_developer_closing_it() {
        let f = fixture();
        let (_a, root, id) = project("alpha");
        f.manager
            .open_project(root.clone(), OpenReason::Developer)
            .await
            .unwrap();
        f.shell.destroy(&label_of(&id));
        f.manager.on_destroyed(&label_of(&id));
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        while !f.manager.open_projects().is_empty()
            || !f.shell.windows().iter().any(|w| w == "welcome")
        {
            assert!(
                std::time::Instant::now() < deadline,
                "the project was not closed"
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert!(f
            .jobs
            .events()
            .contains(&format!("closed {id} WindowClosed")));
        assert!(f.store.snapshot().last_session.is_empty());
    }

    async fn open_at(f: &Fixture, name: &str) -> (tempfile::TempDir, PathBuf, String) {
        let (keep, root, id) = project(name);
        f.manager
            .open_project(root.clone(), OpenReason::Developer)
            .await
            .unwrap();
        (keep, root, id)
    }

    /// What the shell did from the first line that starts with `from` on.
    fn log_from(f: &Fixture, from: &str) -> Vec<String> {
        let log = f.shell.log();
        let start = log.iter().position(|l| l.starts_with(from)).unwrap();
        log[start..].to_vec()
    }

    #[tokio::test]
    async fn sending_a_window_to_the_background_hides_it_and_suspends_nothing() {
        let f = fixture();
        let (_a, root, id) = open_at(&f, "alpha").await;
        f.jobs.set_working(&id, 2);

        assert_eq!(f.manager.send_to_background(&id), BackgroundOutcome::Hidden);
        assert!(f.manager.is_background(&id));
        assert_eq!(
            f.shell.log().last().unwrap(),
            &format!("hide {}", label_of(&id))
        );
        // No confirmation, and the project stays open: window, picture, jobs and session.
        assert!(f.shell.prompts.lock().unwrap().is_empty());
        assert_eq!(f.manager.open_projects().len(), 1);
        assert_eq!(f.shell.windows(), [label_of(&id)]);
        assert!(f.manager.pictures.get(&id).is_ok());
        assert_eq!(
            f.jobs.events(),
            [format!("opened {id} Developer")],
            "window.CLOSED is not emitted"
        );
        assert_eq!(
            f.store.snapshot().last_session,
            [root.to_string_lossy().into_owned()]
        );
        // A window already in the background, and one that is not open, change nothing.
        assert_eq!(
            f.manager.send_to_background(&id),
            BackgroundOutcome::Unchanged
        );
        assert_eq!(
            f.manager.send_to_background("missing"),
            BackgroundOutcome::Unchanged
        );
    }

    #[tokio::test]
    async fn a_window_that_cannot_be_hidden_stays_on_the_screen() {
        let f = fixture();
        let (_a, _root, id) = open_at(&f, "alpha").await;
        *f.shell.fail_hide.lock().unwrap() = true;
        assert_eq!(
            f.manager.send_to_background(&id),
            BackgroundOutcome::Failed("no window system".into())
        );
        assert!(!f.manager.is_background(&id));
    }

    #[tokio::test]
    async fn every_project_window_can_be_in_the_background_at_once() {
        let f = fixture();
        let (_a, _, id_a) = open_at(&f, "alpha").await;
        let (_b, _, id_b) = open_at(&f, "beta").await;
        assert_eq!(
            f.manager.send_to_background(&id_a),
            BackgroundOutcome::Hidden
        );
        assert_eq!(
            f.manager.send_to_background(&id_b),
            BackgroundOutcome::Hidden
        );
        assert_eq!(f.manager.open_projects().len(), 2);
        assert_eq!(f.shell.windows().len(), 2, "no welcome window opens");
    }

    #[test]
    fn closing_with_jobs_working_alone_offers_the_background_and_unsaved_changes_never_do() {
        let jobs = close_prompt("alpha", 3, &[]);
        assert_eq!(jobs.accept, "Suspend and Close");
        assert_eq!(jobs.discard, None);
        assert_eq!(
            jobs.background.as_deref(),
            Some("Keep working in background")
        );

        let unsaved = close_prompt("alpha", 3, &["a.txt".to_owned()]);
        assert_eq!(unsaved.accept, "Save all");
        assert_eq!(unsaved.discard.as_deref(), Some("Discard all"));
        assert_eq!(unsaved.background, None);

        assert_eq!(quit_prompt(2).background, None);
        assert_eq!(
            quit_unsaved_prompt("alpha", 2, &["a.txt".to_owned()]).background,
            None
        );
    }

    #[tokio::test]
    async fn keep_working_in_background_hides_the_window_instead_of_closing_it() {
        let f = fixture();
        let (_a, root, id) = open_at(&f, "alpha").await;
        f.jobs.set_working(&id, 2);
        f.shell.answer_with(Confirmation::Background);

        assert_eq!(f.manager.close_window(&id).await, CloseOutcome::Background);
        assert!(f.manager.is_background(&id));
        assert_eq!(
            f.shell.log().last().unwrap(),
            &format!("hide {}", label_of(&id))
        );
        assert_eq!(
            f.shell.windows(),
            [label_of(&id)],
            "the window is not closed, nor a welcome window opened"
        );
        assert_eq!(
            f.jobs.events(),
            [format!("opened {id} Developer")],
            "nothing is suspended"
        );
        assert_eq!(
            f.store.snapshot().last_session,
            [root.to_string_lossy().into_owned()]
        );
        // The close request is not stuck: a later one asks again.
        f.manager.on_focused(&label_of(&id));
        f.shell.answer(false);
        assert_eq!(f.manager.close_window(&id).await, CloseOutcome::Cancelled);
    }

    #[tokio::test]
    async fn keep_working_in_background_with_a_window_that_cannot_be_hidden_leaves_it_open_and_says_why(
    ) {
        let f = fixture();
        let (_a, root, id) = open_at(&f, "alpha").await;
        f.jobs.set_working(&id, 2);
        *f.shell.fail_hide.lock().unwrap() = true;
        f.shell.answer_with(Confirmation::Background);

        assert_eq!(
            f.manager.close_window(&id).await,
            CloseOutcome::HideFailed("no window system".into())
        );
        // The window stays shown and open, and nothing is suspended.
        assert!(!f.manager.is_background(&id));
        assert!(f.shell.hidden.lock().unwrap().is_empty());
        assert_eq!(f.shell.windows(), [label_of(&id)]);
        assert_eq!(f.manager.open_projects().len(), 1);
        assert_eq!(
            f.jobs.events(),
            [format!("opened {id} Developer")],
            "nothing is suspended"
        );
        assert_eq!(
            f.store.snapshot().last_session,
            [root.to_string_lossy().into_owned()]
        );
        // The reason goes to the UI of that window.
        assert_eq!(
            f.shell.log().last().unwrap(),
            &format!("hide failed {} no window system", label_of(&id))
        );
        // The close request is not stuck: a later one asks again.
        f.manager.on_focused(&label_of(&id));
        f.shell.answer(false);
        assert_eq!(f.manager.close_window(&id).await, CloseOutcome::Cancelled);
    }

    #[tokio::test]
    async fn suspend_and_close_does_what_closing_did() {
        let f = fixture();
        let (_a, id) = open(&f, "alpha").await;
        f.jobs.set_working(&id, 1);
        f.shell.answer(true);
        assert_eq!(f.manager.close_window(&id).await, CloseOutcome::Closed);
        assert!(f
            .jobs
            .events()
            .contains(&format!("closed {id} WindowClosed")));
        assert!(!f.manager.is_background(&id));
    }

    #[tokio::test]
    async fn closing_the_last_window_on_the_screen_opens_no_welcome_window_while_others_are_in_the_background(
    ) {
        let f = fixture();
        let (_a, root_a, id_a) = open_at(&f, "alpha").await;
        let (_b, _, id_b) = open_at(&f, "beta").await;
        f.manager.send_to_background(&id_a);

        assert_eq!(f.manager.close_window(&id_b).await, CloseOutcome::Closed);
        assert_eq!(f.shell.windows(), [label_of(&id_a)]);
        assert!(!f.shell.log().iter().any(|l| l.contains("welcome")));
        assert_eq!(
            f.store.snapshot().last_session,
            [root_a.to_string_lossy().into_owned()]
        );
        // The window in the background is still the project's: closing it is the last window.
        f.manager.reopen_app();
        assert_eq!(f.manager.close_window(&id_a).await, CloseOutcome::Closed);
        assert_eq!(f.shell.windows(), ["welcome"]);
    }

    #[tokio::test]
    async fn a_window_in_the_background_counts_for_the_quit_and_the_restore_list() {
        let f = fixture();
        let (_a, root_a, id_a) = open_at(&f, "alpha").await;
        let (_b, root_b, _id_b) = open_at(&f, "beta").await;
        f.jobs.set_working(&id_a, 1);
        f.manager.send_to_background(&id_a);
        assert_eq!(f.manager.working_jobs(), 1);

        f.shell.answer(true);
        f.manager.quit(0, true).await;
        let prompt = f.shell.prompts.lock().unwrap()[0].clone();
        assert_eq!(prompt.title, "Quit Weaver?");
        assert!(
            prompt.message.starts_with("1 job is working."),
            "{}",
            prompt.message
        );
        assert_eq!(
            prompt.background, None,
            "quitting never offers the background"
        );
        assert_eq!(
            f.store.snapshot().last_session,
            [
                root_a.to_string_lossy().into_owned(),
                root_b.to_string_lossy().into_owned()
            ]
        );
        assert!(f.jobs.events().contains(&format!("closed {id_a} AppQuit")));
        assert_eq!(f.shell.log().last().unwrap(), "exit 0");
    }

    #[tokio::test]
    async fn the_next_start_opens_every_window_of_the_restore_list_on_the_screen() {
        let f = fixture();
        let (_a, root_a, id_a) = project("alpha");
        let (_b, root_b, id_b) = project("beta");
        f.store
            .update(|state| {
                state.set_last_session(vec![
                    root_a.to_string_lossy().into_owned(),
                    root_b.to_string_lossy().into_owned(),
                ])
            })
            .unwrap();
        f.manager.restore(None).await;
        assert_eq!(f.shell.windows(), [label_of(&id_a), label_of(&id_b)]);
        assert!(!f.manager.is_background(&id_a));
        assert!(!f.manager.is_background(&id_b));
        assert!(f.shell.hidden.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn reopening_the_app_shows_every_window_in_the_background_least_recently_used_first_then_focuses_the_last_used(
    ) {
        let f = fixture();
        let (_a, _, id_a) = open_at(&f, "alpha").await;
        let (_b, _, id_b) = open_at(&f, "beta").await;
        let (_c, _, id_c) = open_at(&f, "gamma").await;
        // Used last: c, then b, then a.
        f.manager.on_focused(&label_of(&id_a));
        f.manager.on_focused(&label_of(&id_b));
        f.manager.on_focused(&label_of(&id_c));
        for id in [&id_c, &id_a, &id_b] {
            assert_eq!(f.manager.send_to_background(id), BackgroundOutcome::Hidden);
        }

        f.manager.reopen_app();
        assert_eq!(
            log_from(&f, "reveal"),
            [
                format!("reveal {}", label_of(&id_a)),
                format!("reveal {}", label_of(&id_b)),
                format!("reveal {}", label_of(&id_c)),
                format!("focus {}", label_of(&id_c)),
            ]
        );
        for id in [&id_a, &id_b, &id_c] {
            assert!(!f.manager.is_background(id));
        }
        assert!(f.shell.hidden.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn reopening_the_app_with_no_window_in_the_background_focuses_the_window_used_most_recently(
    ) {
        let f = fixture();
        let (_a, _, id_a) = open_at(&f, "alpha").await;
        let (_b, _, _id_b) = open_at(&f, "beta").await;
        f.manager.on_focused(&label_of(&id_a));
        f.manager.reopen_app();
        assert_eq!(
            f.shell.log().last().unwrap(),
            &format!("focus {}", label_of(&id_a))
        );
        assert!(!f.shell.log().iter().any(|l| l.starts_with("reveal")));
    }

    #[tokio::test]
    async fn reopening_the_app_with_no_project_window_shows_the_welcome_window() {
        let f = fixture();
        f.manager.reopen_app();
        assert_eq!(f.shell.windows(), ["welcome"]);
        f.manager.reopen_app();
        assert_eq!(f.shell.log().last().unwrap(), "focus welcome");
    }

    #[tokio::test]
    async fn the_dock_icon_brings_the_windows_back_whenever_one_is_in_the_background() {
        let f = fixture();
        let (_a, _, id_a) = open_at(&f, "alpha").await;
        let (_b, _, id_b) = open_at(&f, "beta").await;
        // No window in the background and one visible: nothing happens, as before.
        let before = f.shell.log();
        f.manager.reopen(true);
        assert_eq!(f.shell.log(), before);

        f.manager.send_to_background(&id_a);
        // The system says a window is visible (beta): the window in the background comes back anyway.
        f.manager.reopen(true);
        assert!(!f.manager.is_background(&id_a));
        assert!(f
            .shell
            .log()
            .contains(&format!("reveal {}", label_of(&id_a))));
        assert!(f.shell.hidden.lock().unwrap().is_empty());

        f.manager.send_to_background(&id_a);
        f.manager.send_to_background(&id_b);
        f.manager.reopen(false);
        assert!(!f.manager.is_background(&id_a) && !f.manager.is_background(&id_b));
    }

    #[tokio::test]
    async fn a_second_launch_without_a_folder_brings_every_window_back() {
        let f = fixture();
        let (_a, _, id_a) = open_at(&f, "alpha").await;
        let (_b, _, id_b) = open_at(&f, "beta").await;
        f.manager.send_to_background(&id_a);
        f.manager.send_to_background(&id_b);

        f.manager
            .second_launch(vec!["weaver".to_owned()], PathBuf::from("/"));
        assert!(f.shell.hidden.lock().unwrap().is_empty());
        assert!(!f.manager.is_background(&id_a) && !f.manager.is_background(&id_b));
        // beta was used most recently: it has the focus.
        assert_eq!(
            f.shell.log().last().unwrap(),
            &format!("focus {}", label_of(&id_b))
        );
    }

    #[tokio::test]
    async fn opening_a_project_or_clicking_its_notification_brings_back_only_its_window() {
        let f = fixture();
        let (_a, root_a, id_a) = open_at(&f, "alpha").await;
        let (_b, _, id_b) = open_at(&f, "beta").await;
        f.manager.send_to_background(&id_a);
        f.manager.send_to_background(&id_b);

        assert_eq!(
            f.manager
                .open_project(root_a, OpenReason::Developer)
                .await
                .unwrap(),
            OpenOutcome::Focused
        );
        assert!(!f.manager.is_background(&id_a));
        assert!(f.manager.is_background(&id_b));
        assert_eq!(*f.shell.hidden.lock().unwrap(), [label_of(&id_b)]);

        f.manager.send_to_background(&id_a);
        let request = ShowRequest {
            project_id: id_a.clone(),
            view: ShowView::Jobs,
            job_id: None,
        };
        assert!(f.manager.focus_project(&id_a, Some(request)));
        assert!(!f.manager.is_background(&id_a));
        assert!(f.manager.is_background(&id_b));
        assert!(!f.shell.log().iter().any(|l| l.starts_with("reveal")));
    }

    #[tokio::test]
    async fn a_save_all_that_fails_brings_back_only_its_window() {
        let f = fixture();
        let (_a, id_a) = open(&f, "alpha").await;
        let (_b, id_b) = open(&f, "beta").await;
        f.manager.set_unsaved(&id_a, vec!["a.txt".into()]);
        *f.shell.texts.lock().unwrap() = Some(vec![tab("a.txt", "A")]);
        f.shell
            .refused
            .lock()
            .unwrap()
            .insert("a.txt".into(), "disk full".into());
        f.manager.send_to_background(&id_a);
        f.manager.send_to_background(&id_b);

        // Quitting asks the window with unsaved changes, back from the background, and `Save all` fails.
        f.shell.answer(true);
        f.manager.quit(0, true).await;
        assert!(!f.manager.is_background(&id_a));
        assert!(f.manager.is_background(&id_b));
        assert!(!f.shell.log().iter().any(|l| l.starts_with("exit")));
        assert_eq!(f.shell.failed.lock().unwrap().len(), 1);
    }

    #[test]
    fn project_folders_are_named_by_their_last_component() {
        assert_eq!(folder_name(Path::new("/work/my project")), "my project");
        assert_eq!(folder_name(Path::new("/")), "/");
    }
}

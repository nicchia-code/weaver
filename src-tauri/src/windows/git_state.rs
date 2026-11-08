//! What a project window shows of git: the branch of its folder under the project switcher and the
//! developer's user name in the user row (`requirements/tech#window`).
//!
//! The reads ([`weaver_core::git_info`]) use the read-only git call of Solo mode, never block the
//! window and are tagged with the project and a revision. They run when the window opens, when it
//! gets the focus and when git's HEAD file of the working copy changes ([`super::head_watch`]); the
//! watch is outside the project's picture and is set again at the next focus when it could not be
//! set (the folder was not in a repository yet). A new value reaches only that project's window,
//! as the event `project-git-changed`; [`project_git`] answers the value at once, for a window
//! that just loaded.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State, WebviewWindow};
use ts_rs::TS;
use weaver_core::git_info;
use weaver_core::solo_mode::Git;

use super::head_watch::{self, HeadWatch};
use super::registry::{label_of, OpenProject};
use crate::blocking;
use crate::error::{AppError, ErrorCode};
use crate::events::EventName;
use crate::log_warn;

/// The branch and the user name of a project's folder, as the window shows them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProjectGit {
    /// The project hash: the window ignores a value for another project.
    pub project_id: String,
    /// The project folder's name: what the switcher shows before it knows its list.
    pub name: String,
    /// Increases with every read of the project; a window keeps the value with the highest one.
    #[ts(type = "number")]
    pub revision: u64,
    /// The branch git is on, the short hash of the commit when it is on no branch, `None` when
    /// the folder is not in a repository or git cannot be run.
    pub branch: Option<String>,
    /// git's `user.name` for the project, `None` when there is none.
    pub user_name: Option<String>,
}

/// What was read: the branch and the user name.
type Reading = (Option<String>, Option<String>);

/// What one project keeps: its reads, one at a time, and its watch of HEAD.
#[derive(Default)]
struct ProjectReads {
    state: Mutex<Reads>,
    watch: Mutex<Option<HeadWatch>>,
}

#[derive(Default)]
struct Reads {
    revision: u64,
    last: Option<Reading>,
}

impl Reads {
    /// A new reading: its payload, and whether it differs from the one before (the first always
    /// does).
    fn record(&mut self, project: &OpenProject, reading: Reading) -> (ProjectGit, bool) {
        self.revision += 1;
        let changed = self.last.as_ref() != Some(&reading);
        let payload = ProjectGit {
            project_id: project.id.clone(),
            name: project.name.clone(),
            revision: self.revision,
            branch: reading.0.clone(),
            user_name: reading.1.clone(),
        };
        self.last = Some(reading);
        (payload, changed)
    }
}

/// What the windows know of git, one entry per open project. Managed by Tauri.
#[derive(Default)]
pub struct GitState {
    projects: Mutex<HashMap<String, Arc<ProjectReads>>>,
}

impl GitState {
    fn project(&self, project_id: &str) -> Arc<ProjectReads> {
        self.projects
            .lock()
            .expect("git state poisoned")
            .entry(project_id.to_owned())
            .or_default()
            .clone()
    }

    /// Reads git for the project at `root` (blocking: up to three git calls) and answers the new
    /// value and whether it differs from the one read before. Reads of a project are one at a
    /// time, so that the highest revision is the latest read.
    fn read(&self, project: &OpenProject) -> (ProjectGit, bool) {
        let kept = self.project(&project.id);
        let mut reads = kept.state.lock().expect("git reads poisoned");
        let git = Git::system();
        let reading = (
            git_info::branch(&git, &project.root),
            git_info::user_name(&git, &project.root),
        );
        reads.record(project, reading)
    }

    /// The project's window is gone: its reads and its watch go with it.
    fn forget(&self, project_id: &str) {
        self.projects
            .lock()
            .expect("git state poisoned")
            .remove(project_id);
    }
}

/// The open project `project_id`.
fn open_project(app: &AppHandle, project_id: &str) -> Option<OpenProject> {
    super::manager_of(app)?
        .open_projects()
        .into_iter()
        .find(|project| project.id == project_id)
}

fn emit(app: &AppHandle, value: &ProjectGit) {
    let label = label_of(&value.project_id);
    // `emit_to`, never `emit`: only that project's window hears of it.
    if let Err(error) = app.emit_to(&label, EventName::ProjectGitChanged.as_str(), value) {
        log_warn!("the window {label} was not told its branch: {error}");
    }
}

/// Starts the watch of HEAD of the project when it has none and its folder is in a repository.
fn ensure_watch(app: &AppHandle, state: &GitState, project: &OpenProject) {
    let kept = state.project(&project.id);
    let mut watch = kept.watch.lock().expect("git watch poisoned");
    if watch.is_some() {
        return;
    }
    let Some(head) = git_info::head_path(&Git::system(), &project.root) else {
        return;
    };
    let handle = app.clone();
    let id = project.id.clone();
    match head_watch::start(&head, Arc::new(move || refresh(&handle, &id))) {
        Ok(started) => *watch = Some(started),
        Err(reason) => log_warn!("{reason}"),
    }
}

/// Reads the branch and the user name of the project again, on the blocking pool, tells its window
/// when they changed and sets the watch of git's HEAD file when there is none. Called when the
/// window opens, when it gets the focus and when HEAD changes.
pub fn refresh(app: &AppHandle, project_id: &str) {
    let app = app.clone();
    let project_id = project_id.to_owned();
    tauri::async_runtime::spawn_blocking(move || {
        let Some(project) = open_project(&app, &project_id) else {
            return;
        };
        let Some(state) = app.try_state::<GitState>() else {
            return;
        };
        let (value, changed) = state.read(&project);
        if changed {
            emit(&app, &value);
        }
        ensure_watch(&app, &state, &project);
        // The window closed while git was read: nothing of it is kept.
        if open_project(&app, &project_id).is_none() {
            state.forget(&project_id);
        }
    });
}

/// The window of the project is gone.
pub fn closed(app: &AppHandle, project_id: &str) {
    if let Some(state) = app.try_state::<GitState>() {
        state.forget(project_id);
    }
}

/// The branch and the user name of the project of the asking window, as they are now. The window
/// keeps the value with the highest revision, so an event that comes first is not overwritten.
#[tauri::command]
pub async fn project_git(
    app: AppHandle,
    window: WebviewWindow,
    manager: State<'_, Arc<super::WindowManager>>,
) -> Result<ProjectGit, AppError> {
    let project_id = crate::pictures::project_of_label(window.label())
        .ok_or_else(|| AppError::new(ErrorCode::InvalidInput, "this is not a project window"))?
        .to_owned();
    let project = manager
        .open_projects()
        .into_iter()
        .find(|project| project.id == project_id)
        .ok_or_else(|| AppError::new(ErrorCode::InvalidInput, "the project is not open"))?;
    blocking::disk_named("project_git", None, move || {
        app.state::<GitState>().read(&project).0
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(id: &str) -> OpenProject {
        OpenProject {
            id: id.to_owned(),
            root: std::path::PathBuf::from(format!("/work/{id}")),
            name: format!("name-{id}"),
        }
    }

    fn reading(branch: Option<&str>, user: Option<&str>) -> Reading {
        (branch.map(str::to_owned), user.map(str::to_owned))
    }

    #[test]
    fn the_first_reading_is_new_and_each_one_has_a_higher_revision() {
        let mut reads = Reads::default();
        let (first, changed) = reads.record(&project("p"), reading(Some("main"), Some("Luca")));
        assert!(changed);
        assert_eq!((first.project_id.as_str(), first.revision), ("p", 1));
        assert_eq!(first.name, "name-p");
        assert_eq!(first.branch.as_deref(), Some("main"));
        assert_eq!(first.user_name.as_deref(), Some("Luca"));

        let (second, changed) = reads.record(&project("p"), reading(Some("main"), Some("Luca")));
        assert!(!changed, "the same value is not announced again");
        assert_eq!(second.revision, 2);

        let (third, changed) = reads.record(&project("p"), reading(Some("feature"), Some("Luca")));
        assert!(changed);
        assert_eq!(third.branch.as_deref(), Some("feature"));
        assert_eq!(third.revision, 3);
    }

    #[test]
    fn a_new_user_name_or_losing_the_repository_is_a_change() {
        let mut reads = Reads::default();
        reads.record(&project("p"), reading(Some("main"), None));
        assert!(reads.record(&project("p"), reading(Some("main"), Some("Luca"))).1);
        assert!(reads.record(&project("p"), reading(None, Some("Luca"))).1);
        assert!(!reads.record(&project("p"), reading(None, Some("Luca"))).1);
    }

    #[test]
    fn the_reads_of_a_project_are_kept_apart_from_the_others_and_forgotten_with_its_window() {
        let state = GitState::default();
        let a = state.project("a");
        let b = state.project("b");
        a.state.lock().unwrap().record(&project("a"), reading(Some("x"), None));
        assert_eq!(a.state.lock().unwrap().revision, 1);
        assert_eq!(b.state.lock().unwrap().revision, 0);
        assert!(Arc::ptr_eq(&a, &state.project("a")));
        state.forget("a");
        assert_eq!(state.project("a").state.lock().unwrap().revision, 0);
    }

    #[test]
    fn a_read_of_a_real_repository_gives_its_branch_and_user_name() {
        let folder = tempfile::tempdir().unwrap();
        let root = weaver_core::paths::canonical_root(folder.path()).unwrap();
        let git = |args: &[&str]| {
            let status = std::process::Command::new("git")
                .args(args)
                .current_dir(&root)
                .status()
                .unwrap();
            assert!(status.success(), "git {args:?}");
        };
        git(&["init", "-q"]);
        git(&["checkout", "-q", "-b", "sidebar"]);
        git(&["config", "user.name", "Luca Soda"]);
        let state = GitState::default();
        let (value, changed) = state.read(&OpenProject {
            id: "p".to_owned(),
            root: root.clone(),
            name: "repo".to_owned(),
        });
        assert!(changed);
        assert_eq!(value.branch.as_deref(), Some("sidebar"));
        assert_eq!(value.user_name.as_deref(), Some("Luca Soda"));
        let json = serde_json::to_value(&value).unwrap();
        assert_eq!(json["projectId"], "p");
        assert_eq!(json["userName"], "Luca Soda");
    }
}

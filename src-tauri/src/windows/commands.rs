//! Commands of the windows: the welcome window's data and actions, and `Open in editor`.
//!
//! * `welcome_state`: the name and version of the app and the recent projects, each with whether
//!   its folder still exists (the welcome window marks the others `Not found`);
//! * `open_project`: `developer.OPEN_PROJECT` for a folder (a recent project);
//! * `open_folder`: the system folder picker, then `developer.OPEN_PROJECT` for the choice;
//! * `remove_recent`: removes a recent project (offered for a folder that no longer exists);
//! * `open_in_editor`: a file at a line in the editor of the settings.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, Manager, State, WebviewWindow};
use tauri_plugin_dialog::DialogExt;
use ts_rs::TS;

use super::editor::{self, ProgramSearch};
use super::jobs::OpenReason;
use super::local_state::LocalState;
use super::manager::{MenuOpenFailed, WindowManager};
use crate::blocking;
use crate::error::{AppError, ErrorCode};
use crate::state::AppState;

/// A recent project.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RecentEntry {
    /// The canonical path, shown in muted text under the name.
    pub path: String,
    /// The folder's name.
    pub name: String,
    /// When it was last opened (UTC ISO 8601).
    pub opened: String,
    /// `false` when the folder is gone: the entry is marked `Not found` and can be removed.
    pub exists: bool,
}

/// What the welcome window shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct WelcomeState {
    pub name: String,
    pub version: String,
    /// At most 10, the most recently opened first.
    pub recent: Vec<RecentEntry>,
    /// An opening from the File menu that failed while no window had the focus: the welcome window
    /// shows it in its `Alert`. Handed over once, by `welcome_state` only.
    pub failure: Option<MenuOpenFailed>,
}

/// The welcome state of `state`; `exists` says whether a folder is there (blocking).
pub fn welcome_state_of(state: &LocalState, exists: impl Fn(&Path) -> bool) -> WelcomeState {
    WelcomeState {
        name: "Weaver".to_owned(),
        version: env!("CARGO_PKG_VERSION").to_owned(),
        recent: recent_entries_of(state, exists),
        failure: None,
    }
}

/// The recent projects of `state`, each with whether its folder is still there (blocking).
pub fn recent_entries_of(state: &LocalState, exists: impl Fn(&Path) -> bool) -> Vec<RecentEntry> {
    state
        .recent
        .iter()
        .map(|project| RecentEntry {
            path: project.path.clone(),
            name: project.name.clone(),
            opened: project.opened.clone(),
            exists: exists(Path::new(&project.path)),
        })
        .collect()
}

#[tauri::command]
pub async fn welcome_state(
    manager: State<'_, Arc<WindowManager>>,
) -> Result<WelcomeState, AppError> {
    let manager = manager.inner().clone();
    blocking::disk_named("welcome_state", None, move || {
        let mut state = welcome_state_of(&manager.recent(), Path::is_dir);
        // A File menu opening that failed with no window to say so, waiting for this window.
        state.failure = manager.take_welcome_failure();
        state
    })
    .await
}

#[tauri::command]
pub async fn open_project(
    manager: State<'_, Arc<WindowManager>>,
    path: String,
) -> Result<(), AppError> {
    manager
        .open_project(PathBuf::from(path), OpenReason::Developer)
        .await
        .map(|_| ())
}

/// Shows the system folder picker, modal to `parent` when there is one (a sheet on macOS). `None`
/// when the developer chose no folder.
pub async fn pick_folder(
    app: &AppHandle,
    parent: Option<&WebviewWindow>,
) -> Result<Option<PathBuf>, AppError> {
    let (answer, receiver) = tokio::sync::oneshot::channel();
    let mut dialog = app.dialog().file().set_title("Open Folder");
    if let Some(parent) = parent {
        dialog = dialog.set_parent(parent);
    }
    dialog.pick_folder(move |chosen| {
        let _ = answer.send(chosen);
    });
    let Some(chosen) = receiver.await.map_err(AppError::internal)? else {
        return Ok(None);
    };
    chosen
        .into_path()
        .map(Some)
        .map_err(|error| AppError::new(ErrorCode::InvalidInput, error.to_string()))
}

/// Shows the system folder picker. `false` when the developer chose none.
#[tauri::command]
pub async fn open_folder(
    app: AppHandle,
    manager: State<'_, Arc<WindowManager>>,
) -> Result<bool, AppError> {
    let Some(folder) = pick_folder(&app, None).await? else {
        return Ok(false);
    };
    manager.open_project(folder, OpenReason::Developer).await?;
    Ok(true)
}

#[tauri::command]
pub async fn remove_recent(
    manager: State<'_, Arc<WindowManager>>,
    path: String,
) -> Result<WelcomeState, AppError> {
    let manager = manager.inner().clone();
    blocking::disk_named("remove_recent", None, move || {
        manager
            .remove_recent(&path)
            .map(|state| welcome_state_of(&state, Path::is_dir))
    })
    .await?
}

/// The file `file` names: itself when absolute, else relative to the root of `project_root`.
pub fn absolute_file(file: &str, project_root: Option<&Path>) -> Result<PathBuf, AppError> {
    let path = Path::new(file);
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }
    match project_root {
        Some(root) => Ok(root.join(path)),
        None => Err(AppError::new(
            ErrorCode::InvalidInput,
            "The file has no folder to start from.",
        )),
    }
}

/// `Open in editor`: opens `file` at `line` in the editor of the settings. `file` is absolute, or
/// relative to the project `project_id`. A failure carries the reason the UI shows in a notice.
#[tauri::command]
pub async fn open_in_editor(
    app: AppHandle,
    file: String,
    line: Option<u32>,
    project_id: Option<String>,
) -> Result<(), AppError> {
    blocking::disk_named("open_in_editor", None, move || {
        let state = app.state::<AppState>();
        let root = match project_id {
            Some(id) => Some(state.pictures.get(&id)?.root),
            None => None,
        };
        let file = absolute_file(&file, root.as_deref())?;
        let settings = state.settings.current();
        editor::open(
            &settings,
            &file,
            line,
            &ProgramSearch::of_process(),
            editor::GRACE,
        )
        .map_err(|reason| AppError::new(ErrorCode::InvalidInput, reason))
    })
    .await?
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::windows::local_state::LocalState;

    #[test]
    fn the_welcome_state_marks_the_folders_that_are_gone() {
        let mut state = LocalState::default();
        state.record_open("/gone/old", "old", "2026-11-02T07:00:00.000Z");
        state.record_open("/here/new", "new", "2026-11-02T08:00:00.000Z");
        let welcome = welcome_state_of(&state, |path| path == Path::new("/here/new"));
        assert_eq!(welcome.name, "Weaver");
        assert_eq!(welcome.version, env!("CARGO_PKG_VERSION"));
        let shown: Vec<_> = welcome
            .recent
            .iter()
            .map(|r| (r.name.as_str(), r.exists))
            .collect();
        assert_eq!(shown, [("new", true), ("old", false)], "most recent first");
    }

    #[test]
    fn a_relative_file_is_taken_from_the_project_root() {
        let root = Path::new("/work/project");
        let absolute = std::env::temp_dir().join("a.md");
        assert_eq!(
            absolute_file(&absolute.to_string_lossy(), Some(root)).unwrap(),
            absolute
        );
        assert_eq!(
            absolute_file(".weaver/model/system.md", Some(root)).unwrap(),
            root.join(".weaver/model/system.md")
        );
        assert_eq!(
            absolute_file("a.md", None).unwrap_err().code,
            ErrorCode::InvalidInput
        );
    }
}

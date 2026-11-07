//! Backend → UI events. The UI never polls and holds no domain state: it fetches a snapshot
//! when a window loads (or when it sees a gap in revisions) and applies these updates.

use serde::Serialize;
use tauri::{AppHandle, Emitter};
use ts_rs::TS;

use crate::error::AppError;
use crate::startup::StartupFailure;
use crate::updates::UpdateStatus;
use crate::usage::UsageStatus;
use weaver_core::omp::machine::OmpStatus;
use weaver_core::settings::SettingsSnapshot;
use weaver_core::system_certificates::Status as SystemCertificatesStatus;

/// Names of the events, as the UI listens to them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "kebab-case")]
pub enum EventName {
    ProjectChanged,
    StartupFailed,
    UpdateChanged,
    SettingsChanged,
    OmpChanged,
    /// The usage limits of the connected providers, or the reading of them, changed; carries the
    /// whole status (see [`crate::usage`]).
    UsageChanged,
    /// A window is asked to show a view (a notification click, a second launch); see [`crate::windows`].
    ShowRequested,
    /// A window could not go to the background after `Keep working in background` of the close
    /// confirmation: it stays shown and shows why; see [`crate::windows`].
    WindowHideFailed,
    /// The recent projects changed (an opening, a removal, `Clear Recent` of the File menu); sent
    /// to the welcome window only, with the whole list; see [`crate::windows::menu`].
    RecentChanged,
    /// An opening started from the File menu failed: sent only to the window it was chosen from
    /// (or to the welcome window when no window had the focus); see [`crate::windows::menu`].
    MenuOpenFailed,
    /// The open projects, the recent ones or the jobs that work in a project changed; sent to
    /// every project window, with the whole list (see [`crate::windows::switcher`]).
    ProjectSwitcherChanged,
    /// The branch or the user name of a project's folder changed; sent only to that project's
    /// window (see [`crate::windows::git_state`]).
    ProjectGitChanged,
    Notice,
    /// The Jobs list of a project changed; carries the whole list (see [`crate::jobs`]).
    JobsChanged,
    /// The conversation of a job changed; carries the entries that changed.
    JobEntries,
    /// The list of the sub-agents of a job or of General changed; carries the whole list (see
    /// [`crate::subagents`]).
    SubAgentsChanged,
    /// The conversation of the sub-agent on screen changed; carries the entries that changed.
    SubAgentEntries,
    /// The state of a project's General chat changed; carries its whole state (see [`crate::general`]).
    GeneralChanged,
    /// The conversation of a project's General changed; carries the entries that changed.
    GeneralEntries,
    /// An Ask more conversation of a project opened or changed its row (see [`crate::general`]).
    AskChanged,
    /// The conversation of an Ask more conversation changed; carries the entries that changed.
    AskEntries,
    /// An Ask more conversation closed.
    AskClosed,
    /// The open design requests of a project changed on disk or by a decision; see [`crate::requests`].
    RequestsChanged,
    /// The window of a project is closing: the request editor saves what is pending and answers.
    RequestsFlush,
    /// The window of a project is closing and its File view has unsaved changes: the File view
    /// sends the texts of its tabs (`files_flushed`); see [`crate::windows::unsaved`].
    FilesFlush,
    /// `Save all` could not save some files: the File view shows the tab of the first one with
    /// the reason; see [`crate::windows::unsaved`].
    FilesSaveFailed,
    /// Listed folders of a window's File view changed on disk, or by an action of the tree; carries
    /// the folders read again and the ones gone (see [`crate::file_view`]).
    FilesTreeChanged,
    /// **Close Window** was chosen while the File view of a window has a tab: it closes the active
    /// tab; see [`crate::windows::unsaved`].
    FilesCloseTab,
    /// The file of a tab changed, disappeared or reappeared on disk; see [`crate::file_tabs`].
    FileTabChanged,
    /// The agent lock of a file open in a tab was taken or released; see [`crate::file_lock`].
    FileLockChanged,
    /// The round presented in a project's window changed (opened, ended, or one queued behind it);
    /// carries the round on screen; see [`crate::rounds`].
    QuestionRoundChanged,
    /// The state of a project (Not initialised, Empty Model, Has a Model), the files it installs and the opening check
    /// changed; carries the whole status (see [`crate::project_setup`]).
    ProjectState,
    /// System Certificates changed (the Machine `system-certificates`); carries the whole status
    /// (see [`crate::system_certificates`]).
    SystemCertificatesChanged,
    /// The assets of a project changed (`workspace.ASSETS_CHANGED`); carries the project, a
    /// generation and the whole list (see [`crate::assets`]).
    AssetsChanged,
    /// The notes sheet of a project, or an image in its notes folder, changed on disk by anything
    /// other than Weaver's own writing (`workspace.NOTES_CHANGED`); carries the project, a
    /// generation, whether the sheet changed with its hash, and the names of the images (see
    /// [`crate::notes::changes`]).
    NotesChanged,
    /// The window of a project is closing: its Notes view answers with the text still waiting and
    /// the images to put in the trash (`notes_flushed`); see [`crate::notes`].
    NotesFlush,
}

impl EventName {
    pub fn as_str(self) -> &'static str {
        match self {
            EventName::ProjectChanged => "project-changed",
            EventName::StartupFailed => "startup-failed",
            EventName::UpdateChanged => "update-changed",
            EventName::SettingsChanged => "settings-changed",
            EventName::OmpChanged => "omp-changed",
            EventName::UsageChanged => "usage-changed",
            EventName::ShowRequested => "show-requested",
            EventName::WindowHideFailed => "window-hide-failed",
            EventName::RecentChanged => "recent-changed",
            EventName::MenuOpenFailed => "menu-open-failed",
            EventName::ProjectSwitcherChanged => "project-switcher-changed",
            EventName::ProjectGitChanged => "project-git-changed",
            EventName::Notice => "notice",
            EventName::JobsChanged => "jobs-changed",
            EventName::JobEntries => "job-entries",
            EventName::SubAgentsChanged => "sub-agents-changed",
            EventName::SubAgentEntries => "sub-agent-entries",
            EventName::GeneralChanged => "general-changed",
            EventName::GeneralEntries => "general-entries",
            EventName::AskChanged => "ask-changed",
            EventName::AskEntries => "ask-entries",
            EventName::AskClosed => "ask-closed",
            EventName::RequestsChanged => "requests-changed",
            EventName::RequestsFlush => "requests-flush",
            EventName::FilesFlush => "files-flush",
            EventName::FilesSaveFailed => "files-save-failed",
            EventName::FilesTreeChanged => "files-tree-changed",
            EventName::FilesCloseTab => "files-close-tab",
            EventName::FileTabChanged => "file-tab-changed",
            EventName::FileLockChanged => "file-lock-changed",
            EventName::QuestionRoundChanged => "question-round-changed",
            EventName::ProjectState => "project-state",
            EventName::SystemCertificatesChanged => "system-certificates-changed",
            EventName::AssetsChanged => "assets-changed",
            EventName::NotesChanged => "notes-changed",
            EventName::NotesFlush => "notes-flush",
        }
    }
}

/// A project's picture was rebuilt. Each rebuild increments `revision`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProjectChanged {
    /// The project hash (`p-<hash>` is the label of its window).
    pub project_id: String,
    #[ts(type = "number")]
    pub revision: u64,
    /// Names of the slices of the picture that changed.
    pub slices: Vec<String>,
}

pub fn emit_project_changed(app: &AppHandle, payload: &ProjectChanged) -> Result<(), AppError> {
    app.emit(EventName::ProjectChanged.as_str(), payload)
        .map_err(AppError::internal)
}

/// Something the developer must read at the top of the Board: a decision refused when it was
/// applied, or a Proposal sent back to Draft while he was deciding it. The UI shows it until
/// he closes it or the window closes; it keeps no history of them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Notice {
    /// The project hash.
    pub project_id: String,
    /// Distinguishes one notice from another; a notice sent twice with the same id shows once.
    pub id: String,
    pub text: String,
}

pub fn emit_notice(app: &AppHandle, payload: &Notice) -> Result<(), AppError> {
    app.emit(EventName::Notice.as_str(), payload)
        .map_err(AppError::internal)
}

/// A startup step failed after the window may already have loaded (see [`crate::startup`]).
pub fn emit_startup_failed(app: &AppHandle, payload: &StartupFailure) -> Result<(), AppError> {
    app.emit(EventName::StartupFailed.as_str(), payload)
        .map_err(AppError::internal)
}

/// What the update on offer is changed (found, hidden by **Later**, downloading); see [`crate::updates`].
pub fn emit_update_changed(app: &AppHandle, payload: &UpdateStatus) -> Result<(), AppError> {
    app.emit(EventName::UpdateChanged.as_str(), payload)
        .map_err(AppError::internal)
}

/// A setting changed; every window applies the snapshot (see [`crate::settings`]).
pub fn emit_settings_changed(app: &AppHandle, payload: &SettingsSnapshot) -> Result<(), AppError> {
    app.emit(EventName::SettingsChanged.as_str(), payload)
        .map_err(AppError::internal)
}

/// omp's readiness changed (the Machine `omp-setup`); every window applies the whole status.
pub fn emit_omp_changed(app: &AppHandle, payload: &OmpStatus) -> Result<(), AppError> {
    app.emit(EventName::OmpChanged.as_str(), payload)
        .map_err(AppError::internal)
}

/// The usage limits or their reading changed; every window applies the whole status (see
/// [`crate::usage`]).
pub fn emit_usage_changed(app: &AppHandle, payload: &UsageStatus) -> Result<(), AppError> {
    app.emit(EventName::UsageChanged.as_str(), payload)
        .map_err(AppError::internal)
}

/// System Certificates changed (setting, export started or ended); every window applies the whole
/// status (see [`crate::system_certificates`]).
pub fn emit_system_certificates_changed(
    app: &AppHandle,
    payload: &SystemCertificatesStatus,
) -> Result<(), AppError> {
    app.emit(EventName::SystemCertificatesChanged.as_str(), payload)
        .map_err(AppError::internal)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_event_is_called_project_changed_and_carries_id_revision_and_slices() {
        assert_eq!(EventName::ProjectChanged.as_str(), "project-changed");
        assert_eq!(
            serde_json::to_value(EventName::ProjectChanged).unwrap(),
            serde_json::json!("project-changed")
        );
        let json = serde_json::to_value(ProjectChanged {
            project_id: "0123456789abcdef".into(),
            revision: 3,
            slices: vec!["board".into()],
        })
        .unwrap();
        assert_eq!(
            json,
            serde_json::json!({ "projectId": "0123456789abcdef", "revision": 3, "slices": ["board"] })
        );
    }

    #[test]
    fn the_startup_failure_event_is_called_startup_failed() {
        assert_eq!(EventName::StartupFailed.as_str(), "startup-failed");
        assert_eq!(
            serde_json::to_value(EventName::StartupFailed).unwrap(),
            serde_json::json!("startup-failed")
        );
    }
}

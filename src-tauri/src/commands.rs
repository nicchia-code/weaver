//! Tauri commands: the actions the UI can take. Each returns a result or a typed
//! [`AppError`]; updates travel the other way as events (see [`crate::events`]).
//!
//! Adding a command:
//! 1. write the `#[tauri::command]` function here (or in a submodule), with payload types that
//!    derive `Serialize` and `ts_rs::TS`;
//! 2. add it to [`CommandName`], to `generate_handler!` in `lib.rs`, to `COMMANDS` in
//!    `build.rs` and to the permissions of `capabilities/default.json`;
//! 3. add its payload types to `bindings::export_all` and run
//!    `cargo test -p weaver --lib regenerate_bindings -- --ignored`.
//!
//! Disk work goes through [`crate::blocking::disk`] or [`crate::mutations::MutationQueues`].

use serde::Serialize;
use tauri::{AppHandle, State};
use ts_rs::TS;

use crate::blocking;
use crate::error::AppError;
use crate::state::AppState;

/// Names of the commands, as the UI invokes them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum CommandName {
    AppInfo,
    StartupFailures,
    OpenLog,
    OpenUrl,
    UpdateStatus,
    UpdateLater,
    UpdateInstall,
    SettingsGet,
    SettingsSet,
    BoardSnapshot,
    DeleteTask,
    ApproveProposal,
    RejectProposal,
    DiscardProposal,
    ChooseProposalModel,
    ResolveDrift,
    FixDriftCode,
    AcceptDriftProposal,
    RefineDriftProposal,
    DiscardDriftProposal,
    DismissDrift,
    HistorySnapshot,
    ViewSnapshot,
    ProjectStatus,
    InitialiseProject,
    UpdateSkills,
    SetSoloMode,
    SoloTrackedFiles,
    SetAtomicCommits,
    CheckAtomicCommits,
    MergeRefresh,
    MergeBranches,
    MergeStart,
    MergeAbort,
    ResolveMergeConflicts,
    ProjectFolder,
    ImportCode,
    OmpStatus,
    OmpInstall,
    OmpLogIn,
    OmpLogOut,
    OmpUsage,
    WelcomeState,
    OpenProject,
    OpenFolder,
    RemoveRecent,
    OpenInEditor,
    JobsSnapshot,
    JobConversation,
    JobSendMessage,
    JobStop,
    JobAnswerPermission,
    JobRemove,
    JobSubAgents,
    JobSubAgentConversation,
    JobSubAgentsView,
    RunTask,
    RunAll,
    ScanSystem,
    GeneralSnapshot,
    GeneralSendMessage,
    GeneralStop,
    GeneralAnswerPermission,
    GeneralClear,
    GeneralSubAgents,
    GeneralSubAgentConversation,
    GeneralSubAgentsView,
    AskOpen,
    AskList,
    AskSnapshot,
    AskSendMessage,
    AskStop,
    AskAnswerPermission,
    AskClose,
    AskSubAgents,
    AskSubAgentConversation,
    AskSubAgentsView,
    RequestsList,
    RequestRead,
    RequestNew,
    RequestSave,
    RequestSend,
    RequestDiscard,
    RequestImageAdd,
    RequestImageImport,
    RequestImage,
    RequestsFlushed,
    FilesUnsaved,
    FilesFlushed,
    FilesOpenView,
    FilesExpand,
    FilesCollapse,
    FilesFolders,
    FilesLeave,
    FilesCreate,
    FilesRename,
    FilesMove,
    FilesCopyIn,
    FilesDelete,
    FilesCloseKey,
    QuestionRoundGet,
    QuestionRoundAct,
    SystemCertificatesStatus,
    SystemCertificatesRefresh,
    SystemCertificatesDismissNotice,
    SystemCertificatesDismissFailure,
    FileOpen,
    FileReload,
    FileSave,
    FileClose,
    AssetsList,
    AssetsCopyIn,
    AssetsRename,
    AssetsDelete,
    AssetsDragOut,
    NotesRead,
    NotesWrite,
    NotesImage,
    NotesAddPasted,
    NotesAddDropped,
    NotesTrash,
    NotesFlushed,
}

impl CommandName {
    pub const ALL: &'static [CommandName] = &[
        CommandName::AppInfo,
        CommandName::StartupFailures,
        CommandName::OpenLog,
        CommandName::OpenUrl,
        CommandName::UpdateStatus,
        CommandName::UpdateLater,
        CommandName::UpdateInstall,
        CommandName::SettingsGet,
        CommandName::SettingsSet,
        CommandName::BoardSnapshot,
        CommandName::DeleteTask,
        CommandName::ApproveProposal,
        CommandName::RejectProposal,
        CommandName::DiscardProposal,
        CommandName::ChooseProposalModel,
        CommandName::ResolveDrift,
        CommandName::FixDriftCode,
        CommandName::AcceptDriftProposal,
        CommandName::RefineDriftProposal,
        CommandName::DiscardDriftProposal,
        CommandName::DismissDrift,
        CommandName::HistorySnapshot,
        CommandName::ViewSnapshot,
        CommandName::ProjectStatus,
        CommandName::InitialiseProject,
        CommandName::UpdateSkills,
        CommandName::SetSoloMode,
        CommandName::SoloTrackedFiles,
        CommandName::SetAtomicCommits,
        CommandName::CheckAtomicCommits,
        CommandName::MergeRefresh,
        CommandName::MergeBranches,
        CommandName::MergeStart,
        CommandName::MergeAbort,
        CommandName::ResolveMergeConflicts,
        CommandName::ProjectFolder,
        CommandName::ImportCode,
        CommandName::OmpStatus,
        CommandName::OmpInstall,
        CommandName::OmpLogIn,
        CommandName::OmpLogOut,
        CommandName::OmpUsage,
        CommandName::WelcomeState,
        CommandName::OpenProject,
        CommandName::OpenFolder,
        CommandName::RemoveRecent,
        CommandName::OpenInEditor,
        CommandName::JobsSnapshot,
        CommandName::JobConversation,
        CommandName::JobSendMessage,
        CommandName::JobStop,
        CommandName::JobAnswerPermission,
        CommandName::JobRemove,
        CommandName::JobSubAgents,
        CommandName::JobSubAgentConversation,
        CommandName::JobSubAgentsView,
        CommandName::RunTask,
        CommandName::RunAll,
        CommandName::ScanSystem,
        CommandName::GeneralSnapshot,
        CommandName::GeneralSendMessage,
        CommandName::GeneralStop,
        CommandName::GeneralAnswerPermission,
        CommandName::GeneralClear,
        CommandName::GeneralSubAgents,
        CommandName::GeneralSubAgentConversation,
        CommandName::GeneralSubAgentsView,
        CommandName::AskOpen,
        CommandName::AskList,
        CommandName::AskSnapshot,
        CommandName::AskSendMessage,
        CommandName::AskStop,
        CommandName::AskAnswerPermission,
        CommandName::AskClose,
        CommandName::AskSubAgents,
        CommandName::AskSubAgentConversation,
        CommandName::AskSubAgentsView,
        CommandName::RequestsList,
        CommandName::RequestRead,
        CommandName::RequestNew,
        CommandName::RequestSave,
        CommandName::RequestSend,
        CommandName::RequestDiscard,
        CommandName::RequestImageAdd,
        CommandName::RequestImageImport,
        CommandName::RequestImage,
        CommandName::RequestsFlushed,
        CommandName::FilesUnsaved,
        CommandName::FilesFlushed,
        CommandName::FilesOpenView,
        CommandName::FilesExpand,
        CommandName::FilesCollapse,
        CommandName::FilesFolders,
        CommandName::FilesLeave,
        CommandName::FilesCreate,
        CommandName::FilesRename,
        CommandName::FilesMove,
        CommandName::FilesCopyIn,
        CommandName::FilesDelete,
        CommandName::FilesCloseKey,
        CommandName::QuestionRoundGet,
        CommandName::QuestionRoundAct,
        CommandName::SystemCertificatesStatus,
        CommandName::SystemCertificatesRefresh,
        CommandName::SystemCertificatesDismissNotice,
        CommandName::SystemCertificatesDismissFailure,
        CommandName::FileOpen,
        CommandName::FileReload,
        CommandName::FileSave,
        CommandName::FileClose,
        CommandName::AssetsList,
        CommandName::AssetsCopyIn,
        CommandName::AssetsRename,
        CommandName::AssetsDelete,
        CommandName::AssetsDragOut,
        CommandName::NotesRead,
        CommandName::NotesWrite,
        CommandName::NotesImage,
        CommandName::NotesAddPasted,
        CommandName::NotesAddDropped,
        CommandName::NotesTrash,
        CommandName::NotesFlushed,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            CommandName::AppInfo => "app_info",
            CommandName::StartupFailures => "startup_failures",
            CommandName::OpenLog => "open_log",
            CommandName::OpenUrl => "open_url",
            CommandName::UpdateStatus => "update_status",
            CommandName::UpdateLater => "update_later",
            CommandName::UpdateInstall => "update_install",
            CommandName::SettingsGet => "settings_get",
            CommandName::SettingsSet => "settings_set",
            CommandName::BoardSnapshot => "board_snapshot",
            CommandName::DeleteTask => "delete_task",
            CommandName::ApproveProposal => "approve_proposal",
            CommandName::RejectProposal => "reject_proposal",
            CommandName::DiscardProposal => "discard_proposal",
            CommandName::ChooseProposalModel => "choose_proposal_model",
            CommandName::ResolveDrift => "resolve_drift",
            CommandName::FixDriftCode => "fix_drift_code",
            CommandName::AcceptDriftProposal => "accept_drift_proposal",
            CommandName::RefineDriftProposal => "refine_drift_proposal",
            CommandName::DiscardDriftProposal => "discard_drift_proposal",
            CommandName::DismissDrift => "dismiss_drift",
            CommandName::HistorySnapshot => "history_snapshot",
            CommandName::ViewSnapshot => "view_snapshot",
            CommandName::ProjectStatus => "project_status",
            CommandName::InitialiseProject => "initialise_project",
            CommandName::UpdateSkills => "update_skills",
            CommandName::SetSoloMode => "set_solo_mode",
            CommandName::SoloTrackedFiles => "solo_tracked_files",
            CommandName::SetAtomicCommits => "set_atomic_commits",
            CommandName::CheckAtomicCommits => "check_atomic_commits",
            CommandName::MergeRefresh => "merge_refresh",
            CommandName::MergeBranches => "merge_branches",
            CommandName::MergeStart => "merge_start",
            CommandName::MergeAbort => "merge_abort",
            CommandName::ResolveMergeConflicts => "resolve_merge_conflicts",
            CommandName::ProjectFolder => "project_folder",
            CommandName::ImportCode => "import_code",
            CommandName::OmpStatus => "omp_status",
            CommandName::OmpInstall => "omp_install",
            CommandName::OmpLogIn => "omp_log_in",
            CommandName::OmpLogOut => "omp_log_out",
            CommandName::OmpUsage => "omp_usage",
            CommandName::WelcomeState => "welcome_state",
            CommandName::OpenProject => "open_project",
            CommandName::OpenFolder => "open_folder",
            CommandName::RemoveRecent => "remove_recent",
            CommandName::OpenInEditor => "open_in_editor",
            CommandName::JobsSnapshot => "jobs_snapshot",
            CommandName::JobConversation => "job_conversation",
            CommandName::JobSendMessage => "job_send_message",
            CommandName::JobStop => "job_stop",
            CommandName::JobAnswerPermission => "job_answer_permission",
            CommandName::JobRemove => "job_remove",
            CommandName::JobSubAgents => "job_sub_agents",
            CommandName::JobSubAgentConversation => "job_sub_agent_conversation",
            CommandName::JobSubAgentsView => "job_sub_agents_view",
            CommandName::RunTask => "run_task",
            CommandName::RunAll => "run_all",
            CommandName::ScanSystem => "scan_system",
            CommandName::GeneralSnapshot => "general_snapshot",
            CommandName::GeneralSendMessage => "general_send_message",
            CommandName::GeneralStop => "general_stop",
            CommandName::GeneralAnswerPermission => "general_answer_permission",
            CommandName::GeneralClear => "general_clear",
            CommandName::GeneralSubAgents => "general_sub_agents",
            CommandName::GeneralSubAgentConversation => "general_sub_agent_conversation",
            CommandName::GeneralSubAgentsView => "general_sub_agents_view",
            CommandName::AskOpen => "ask_open",
            CommandName::AskList => "ask_list",
            CommandName::AskSnapshot => "ask_snapshot",
            CommandName::AskSendMessage => "ask_send_message",
            CommandName::AskStop => "ask_stop",
            CommandName::AskAnswerPermission => "ask_answer_permission",
            CommandName::AskClose => "ask_close",
            CommandName::AskSubAgents => "ask_sub_agents",
            CommandName::AskSubAgentConversation => "ask_sub_agent_conversation",
            CommandName::AskSubAgentsView => "ask_sub_agents_view",
            CommandName::RequestsList => "requests_list",
            CommandName::RequestRead => "request_read",
            CommandName::RequestNew => "request_new",
            CommandName::RequestSave => "request_save",
            CommandName::RequestSend => "request_send",
            CommandName::RequestDiscard => "request_discard",
            CommandName::RequestImageAdd => "request_image_add",
            CommandName::RequestImageImport => "request_image_import",
            CommandName::RequestImage => "request_image",
            CommandName::RequestsFlushed => "requests_flushed",
            CommandName::FilesUnsaved => "files_unsaved",
            CommandName::FilesFlushed => "files_flushed",
            CommandName::FilesOpenView => "files_open_view",
            CommandName::FilesExpand => "files_expand",
            CommandName::FilesCollapse => "files_collapse",
            CommandName::FilesFolders => "files_folders",
            CommandName::FilesLeave => "files_leave",
            CommandName::FilesCreate => "files_create",
            CommandName::FilesRename => "files_rename",
            CommandName::FilesMove => "files_move",
            CommandName::FilesCopyIn => "files_copy_in",
            CommandName::FilesDelete => "files_delete",
            CommandName::FilesCloseKey => "files_close_key",
            CommandName::QuestionRoundGet => "question_round_get",
            CommandName::QuestionRoundAct => "question_round_act",
            CommandName::SystemCertificatesStatus => "system_certificates_status",
            CommandName::SystemCertificatesRefresh => "system_certificates_refresh",
            CommandName::SystemCertificatesDismissNotice => "system_certificates_dismiss_notice",
            CommandName::SystemCertificatesDismissFailure => "system_certificates_dismiss_failure",
            CommandName::FileOpen => "file_open",
            CommandName::FileReload => "file_reload",
            CommandName::FileSave => "file_save",
            CommandName::FileClose => "file_close",
            CommandName::AssetsList => "assets_list",
            CommandName::AssetsCopyIn => "assets_copy_in",
            CommandName::AssetsRename => "assets_rename",
            CommandName::AssetsDelete => "assets_delete",
            CommandName::AssetsDragOut => "assets_drag_out",
            CommandName::NotesRead => "notes_read",
            CommandName::NotesWrite => "notes_write",
            CommandName::NotesImage => "notes_image",
            CommandName::NotesAddPasted => "notes_add_pasted",
            CommandName::NotesAddDropped => "notes_add_dropped",
            CommandName::NotesTrash => "notes_trash",
            CommandName::NotesFlushed => "notes_flushed",
        }
    }
}

/// Facts about the running app, shown by the base interface.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub name: String,
    pub version: String,
    pub identifier: String,
    pub local_data_dir: String,
}

#[tauri::command]
pub async fn app_info(app: AppHandle, state: State<'_, AppState>) -> Result<AppInfo, AppError> {
    let identifier = app.config().identifier.clone();
    let local_data_dir = state.dirs.local_data_dir().to_path_buf();
    blocking::disk(move || AppInfo {
        name: "Weaver".to_owned(),
        version: env!("CARGO_PKG_VERSION").to_owned(),
        identifier,
        local_data_dir: local_data_dir.to_string_lossy().into_owned(),
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_names_are_the_snake_case_function_names() {
        assert_eq!(CommandName::AppInfo.as_str(), "app_info");
        assert_eq!(CommandName::StartupFailures.as_str(), "startup_failures");
        assert_eq!(CommandName::OpenLog.as_str(), "open_log");
        assert_eq!(CommandName::OpenUrl.as_str(), "open_url");
        assert_eq!(CommandName::UpdateStatus.as_str(), "update_status");
        assert_eq!(CommandName::UpdateLater.as_str(), "update_later");
        assert_eq!(CommandName::UpdateInstall.as_str(), "update_install");
        let json = serde_json::to_value(CommandName::AppInfo).unwrap();
        assert_eq!(json, serde_json::json!("app_info"));
    }

    /// The capability grants exactly the commands of [`CommandName`] (as `allow-<name>`), and no
    /// plugin permission: plugins are called from Rust only.
    #[test]
    fn the_capability_grants_exactly_weavers_commands_and_core_event_and_window_permissions() {
        let text = include_str!("../capabilities/default.json");
        let capability: serde_json::Value = serde_json::from_str(text).unwrap();
        let permissions: Vec<&str> = capability["permissions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p.as_str().unwrap())
            .collect();
        let granted_commands: Vec<String> = permissions
            .iter()
            .filter(|p| !p.contains(':'))
            .map(|p| {
                p.strip_prefix("allow-")
                    .expect("command permission")
                    .replace('-', "_")
            })
            .collect();
        let mut expected: Vec<String> = CommandName::ALL
            .iter()
            .map(|c| c.as_str().to_owned())
            .collect();
        expected.sort();
        let mut granted = granted_commands;
        granted.sort();
        assert_eq!(granted, expected);
        for permission in permissions.iter().filter(|p| p.contains(':')) {
            assert!(
                permission.starts_with("core:event:") || permission.starts_with("core:window:"),
                "unexpected permission {permission}"
            );
        }
    }

    #[test]
    fn build_script_lists_the_same_commands() {
        let build = include_str!("../build.rs");
        let list = build
            .split_once("const COMMANDS")
            .and_then(|(_, rest)| rest.split_once("];"))
            .map(|(list, _)| list)
            .expect("COMMANDS in build.rs");
        let names: Vec<&str> = list.split('"').skip(1).step_by(2).collect();
        let expected: Vec<&str> = CommandName::ALL.iter().map(|c| c.as_str()).collect();
        assert_eq!(names, expected);
    }
}

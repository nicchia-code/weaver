//! TypeScript types generated from the Rust types with `ts-rs`, into `ui/src/bindings/`.
//!
//! The generated files are committed. The test `generated_types_are_up_to_date` fails when they
//! differ from what the Rust types produce now (CI runs it with `cargo test`); regenerate with
//! `cargo test -p weaver --lib regenerate_bindings -- --ignored` (or `pnpm gen:types` in `ui/`).

use std::path::{Path, PathBuf};

use ts_rs::TS;

use crate::agent_work::{Ghost, GhostKind, GhostPhase};
use crate::ask::build::AskTarget;
use crate::assets::{
    AssetRefusal, AssetRenamed, AssetsChanged, AssetsCopied, AssetsDeleted, AssetsSnapshot,
};
use crate::board::{
    BoardCard, BoardSnapshot, CardHead, CardRef, ProposalStanding, ResolutionDrift, TitledRef,
};
use crate::commands::{AppInfo, CommandName};
use crate::drift_view::{AttachedProposal, CodePlace, DriftView, FixOption};
use crate::error::{AppError, ErrorCode};
use crate::events::{EventName, Notice, ProjectChanged};
use crate::file_tabs::wire::{
    FileContent, FileImage, FileKind, FileLockChanged, FileOpenResult, FileReloadResult,
    FileSaveResult, FileTabChanged, ReadOnlyReason, TabChange,
};
use crate::file_view::{CopyInResult, EntryChange, TreeChanged};
use crate::general::cards::{CardActionInput, ChatCardState, ChatCardView, ChatCards};
use crate::general::proposals::{ChatProposal, ChatProposalStatus, ChatProposals};
use crate::general::wire::{
    AskClosed, AskEntries, AskKind, AskRow, AskSnapshot, AskStatus, AsksSnapshot, GeneralEntries,
    GeneralSnapshot, GeneralState,
};
use crate::history::{HistoryEntry, HistorySnapshot, HistoryTask};
use crate::jobs::conversation::{
    Changed, Delivery, EndLine, Entry, PermissionOption, PermissionState, Tool, ToolKind,
    ToolStatus,
};
use crate::jobs::wire::{ConversationSnapshot, EntriesEvent, JobRow, JobsSnapshot};
use crate::notes::changes::NotesChanged;
use crate::notes::{
    NotesFlush, NotesImageOutcome, NotesImagesAdded, NotesRefusalReason, NotesSheet,
    NotesTrashRefusal, NotesTrashed, NotesWritten, PastedImage,
};
use crate::project_setup::merge::{
    MergeAnswer, MergeBranches, MergeNotice, MergeProgress, MergeStatus,
};
use crate::project_setup::{InitialiseResult, ProjectStatus, SoloChange};
use crate::project_view::ViewSnapshot;
use crate::proposal_view::{
    ProposalControl, ProposalFiles, ProposalTask, ProposalView, RequiredProposal,
};
use crate::requests::{RequestStatus, RequestView, RequestsEvent};
use crate::rounds::view::{ActionInput, QuestionRoundChanged, QuestionRoundView};
use crate::startup::StartupFailure;
use crate::subagents::wire::{
    AgentNode, Owner, Step, SubAgentConversation, SubAgentEntries, SubAgentsEvent,
};
use crate::updates::{UpdateInfo, UpdateStatus};
use crate::usage::{UsagePhase, UsageStatus};
use crate::windows::commands::{RecentEntry, WelcomeState};
use crate::windows::git_state::ProjectGit;
use crate::windows::switcher::{OpenProjectEntry, ProjectSwitcher};
use crate::windows::manager::{HideFailed, MenuOpenFailed, ShowRequest, ShowView};
use crate::windows::unsaved::{
    FilesCloseTab, FilesFlush, FilesSaveFailed, SaveFailure, SavedFile, TabText,
};
use weaver_core::assets::Asset;
use weaver_core::file_view::machine::{TreeSnapshot, TreeUpdate};
use weaver_core::file_view::{FileEntry, FileEntryKind, FolderListing};
use weaver_core::jobs::notes::Note;
use weaver_core::jobs::subagents::{SubAgent, SubAgentState};
use weaver_core::jobs::{JobKind, JobState};
use weaver_core::omp::machine::{
    LogoutNote, LogoutPhase, OmpStatus, Phase as OmpPhase, TerminalProblem,
};
use weaver_core::omp::probe::OmpModel;
use weaver_core::omp::providers::ProviderRow;
use weaver_core::omp::usage::{
    LimitStatus as UsageLimitStatus, NoLimits as UsageNoLimits, ProviderUsage, Usage, UsageLimit,
    UsageSummary,
};
use weaver_core::project_files::folder_tree::TreeEntry;
use weaver_core::project_files::SyncReport;
use weaver_core::settings::{
    Choice, EditorPreset, EditorSettings, FieldKind, ModelSettings, Permissions, SettingChange,
    SettingField, SettingIssue, SettingKey, Settings, SettingsSnapshot, SystemCertificatesSettings,
    Theme,
};
use weaver_core::system_certificates::{
    Phase as SystemCertificatesPhase, Status as SystemCertificatesStatus,
};

/// Where the generated files are committed.
pub fn committed_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("ui")
        .join("src")
        .join("bindings")
}

/// Writes the TypeScript of every type crossing the backend↔UI boundary (and what they
/// depend on) into `dir`. Add each new command, event and payload type here.
pub fn export_all(dir: &Path) -> Result<(), ts_rs::ExportError> {
    CommandName::export_all_to(dir)?;
    EventName::export_all_to(dir)?;
    AppInfo::export_all_to(dir)?;
    AppError::export_all_to(dir)?;
    ErrorCode::export_all_to(dir)?;
    ProjectChanged::export_all_to(dir)?;
    StartupFailure::export_all_to(dir)?;
    UpdateInfo::export_all_to(dir)?;
    UpdateStatus::export_all_to(dir)?;
    Settings::export_all_to(dir)?;
    ModelSettings::export_all_to(dir)?;
    EditorSettings::export_all_to(dir)?;
    SystemCertificatesSettings::export_all_to(dir)?;
    EditorPreset::export_all_to(dir)?;
    Theme::export_all_to(dir)?;
    Permissions::export_all_to(dir)?;
    SettingKey::export_all_to(dir)?;
    SettingChange::export_all_to(dir)?;
    SettingIssue::export_all_to(dir)?;
    SettingField::export_all_to(dir)?;
    FieldKind::export_all_to(dir)?;
    Choice::export_all_to(dir)?;
    SettingsSnapshot::export_all_to(dir)?;
    Notice::export_all_to(dir)?;
    SystemCertificatesStatus::export_all_to(dir)?;
    SystemCertificatesPhase::export_all_to(dir)?;
    BoardSnapshot::export_all_to(dir)?;
    BoardCard::export_all_to(dir)?;
    CardHead::export_all_to(dir)?;
    CardRef::export_all_to(dir)?;
    TitledRef::export_all_to(dir)?;
    ProposalStanding::export_all_to(dir)?;
    ResolutionDrift::export_all_to(dir)?;
    ProposalView::export_all_to(dir)?;
    DriftView::export_all_to(dir)?;
    FixOption::export_all_to(dir)?;
    CodePlace::export_all_to(dir)?;
    AttachedProposal::export_all_to(dir)?;
    ProposalTask::export_all_to(dir)?;
    RequiredProposal::export_all_to(dir)?;
    ProposalFiles::export_all_to(dir)?;
    ProposalControl::export_all_to(dir)?;
    Ghost::export_all_to(dir)?;
    GhostKind::export_all_to(dir)?;
    GhostPhase::export_all_to(dir)?;
    HistorySnapshot::export_all_to(dir)?;
    HistoryEntry::export_all_to(dir)?;
    HistoryTask::export_all_to(dir)?;
    ViewSnapshot::export_all_to(dir)?;
    ProjectStatus::export_all_to(dir)?;
    InitialiseResult::export_all_to(dir)?;
    SoloChange::export_all_to(dir)?;
    MergeStatus::export_all_to(dir)?;
    MergeProgress::export_all_to(dir)?;
    MergeNotice::export_all_to(dir)?;
    MergeAnswer::export_all_to(dir)?;
    MergeBranches::export_all_to(dir)?;
    SyncReport::export_all_to(dir)?;
    TreeEntry::export_all_to(dir)?;
    OmpStatus::export_all_to(dir)?;
    OmpModel::export_all_to(dir)?;
    OmpPhase::export_all_to(dir)?;
    ProviderRow::export_all_to(dir)?;
    UsageStatus::export_all_to(dir)?;
    UsagePhase::export_all_to(dir)?;
    Usage::export_all_to(dir)?;
    ProviderUsage::export_all_to(dir)?;
    UsageLimit::export_all_to(dir)?;
    UsageSummary::export_all_to(dir)?;
    UsageLimitStatus::export_all_to(dir)?;
    UsageNoLimits::export_all_to(dir)?;
    TerminalProblem::export_all_to(dir)?;
    LogoutNote::export_all_to(dir)?;
    LogoutPhase::export_all_to(dir)?;
    RecentEntry::export_all_to(dir)?;
    OpenProjectEntry::export_all_to(dir)?;
    ProjectSwitcher::export_all_to(dir)?;
    ProjectGit::export_all_to(dir)?;
    WelcomeState::export_all_to(dir)?;
    ShowRequest::export_all_to(dir)?;
    HideFailed::export_all_to(dir)?;
    MenuOpenFailed::export_all_to(dir)?;
    ShowView::export_all_to(dir)?;
    JobsSnapshot::export_all_to(dir)?;
    ConversationSnapshot::export_all_to(dir)?;
    EntriesEvent::export_all_to(dir)?;
    GeneralSnapshot::export_all_to(dir)?;
    ChatProposals::export_all_to(dir)?;
    ChatProposal::export_all_to(dir)?;
    ChatProposalStatus::export_all_to(dir)?;
    ChatCards::export_all_to(dir)?;
    ChatCardView::export_all_to(dir)?;
    ChatCardState::export_all_to(dir)?;
    CardActionInput::export_all_to(dir)?;
    GeneralState::export_all_to(dir)?;
    GeneralEntries::export_all_to(dir)?;
    AskKind::export_all_to(dir)?;
    AskStatus::export_all_to(dir)?;
    AskRow::export_all_to(dir)?;
    AsksSnapshot::export_all_to(dir)?;
    AskSnapshot::export_all_to(dir)?;
    AskEntries::export_all_to(dir)?;
    AskClosed::export_all_to(dir)?;
    AskTarget::export_all_to(dir)?;
    JobRow::export_all_to(dir)?;
    JobKind::export_all_to(dir)?;
    JobState::export_all_to(dir)?;
    Entry::export_all_to(dir)?;
    Changed::export_all_to(dir)?;
    Delivery::export_all_to(dir)?;
    ToolKind::export_all_to(dir)?;
    ToolStatus::export_all_to(dir)?;
    Tool::export_all_to(dir)?;
    PermissionOption::export_all_to(dir)?;
    PermissionState::export_all_to(dir)?;
    EndLine::export_all_to(dir)?;
    SubAgent::export_all_to(dir)?;
    SubAgentState::export_all_to(dir)?;
    Owner::export_all_to(dir)?;
    Step::export_all_to(dir)?;
    AgentNode::export_all_to(dir)?;
    SubAgentsEvent::export_all_to(dir)?;
    SubAgentConversation::export_all_to(dir)?;
    SubAgentEntries::export_all_to(dir)?;
    Note::export_all_to(dir)?;
    RequestStatus::export_all_to(dir)?;
    RequestView::export_all_to(dir)?;
    RequestsEvent::export_all_to(dir)?;
    FileEntry::export_all_to(dir)?;
    FileEntryKind::export_all_to(dir)?;
    FolderListing::export_all_to(dir)?;
    TreeSnapshot::export_all_to(dir)?;
    TreeUpdate::export_all_to(dir)?;
    EntryChange::export_all_to(dir)?;
    CopyInResult::export_all_to(dir)?;
    TreeChanged::export_all_to(dir)?;
    FilesFlush::export_all_to(dir)?;
    TabText::export_all_to(dir)?;
    SaveFailure::export_all_to(dir)?;
    SavedFile::export_all_to(dir)?;
    FilesSaveFailed::export_all_to(dir)?;
    FilesCloseTab::export_all_to(dir)?;
    FileContent::export_all_to(dir)?;
    FileKind::export_all_to(dir)?;
    ReadOnlyReason::export_all_to(dir)?;
    FileImage::export_all_to(dir)?;
    FileOpenResult::export_all_to(dir)?;
    FileReloadResult::export_all_to(dir)?;
    FileSaveResult::export_all_to(dir)?;
    TabChange::export_all_to(dir)?;
    FileTabChanged::export_all_to(dir)?;
    FileLockChanged::export_all_to(dir)?;
    QuestionRoundView::export_all_to(dir)?;
    QuestionRoundChanged::export_all_to(dir)?;
    ActionInput::export_all_to(dir)?;
    Asset::export_all_to(dir)?;
    AssetsSnapshot::export_all_to(dir)?;
    AssetsChanged::export_all_to(dir)?;
    AssetsCopied::export_all_to(dir)?;
    AssetRenamed::export_all_to(dir)?;
    AssetRefusal::export_all_to(dir)?;
    NotesChanged::export_all_to(dir)?;
    AssetsDeleted::export_all_to(dir)?;
    NotesSheet::export_all_to(dir)?;
    NotesWritten::export_all_to(dir)?;
    PastedImage::export_all_to(dir)?;
    NotesRefusalReason::export_all_to(dir)?;
    NotesImageOutcome::export_all_to(dir)?;
    NotesImagesAdded::export_all_to(dir)?;
    NotesTrashRefusal::export_all_to(dir)?;
    NotesTrashed::export_all_to(dir)?;
    NotesFlush::export_all_to(dir)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    /// File name → content (CRLF normalised to LF) of the `.ts` files directly in `dir`.
    fn read_ts_files(dir: &Path) -> BTreeMap<String, String> {
        let mut files = BTreeMap::new();
        let Ok(entries) = std::fs::read_dir(dir) else {
            return files;
        };
        for entry in entries {
            let path = entry.unwrap().path();
            if path.extension().is_some_and(|e| e == "ts") {
                let text = std::fs::read_to_string(&path)
                    .unwrap()
                    .replace("\r\n", "\n");
                files.insert(
                    path.file_name().unwrap().to_string_lossy().into_owned(),
                    text,
                );
            }
        }
        files
    }

    /// Reports the differences between the generated and the committed files, `None` if equal.
    fn difference(
        generated: &BTreeMap<String, String>,
        committed: &BTreeMap<String, String>,
    ) -> Option<String> {
        let mut problems = Vec::new();
        for (name, text) in generated {
            match committed.get(name) {
                None => problems.push(format!("missing: {name}")),
                Some(old) if old != text => problems.push(format!("outdated: {name}")),
                Some(_) => {}
            }
        }
        for name in committed.keys().filter(|n| !generated.contains_key(*n)) {
            problems.push(format!("stale (no Rust type produces it): {name}"));
        }
        (!problems.is_empty()).then(|| problems.join(", "))
    }

    #[test]
    fn generated_types_are_up_to_date() {
        let fresh = tempfile::tempdir().unwrap();
        export_all(fresh.path()).unwrap();
        let problem = difference(
            &read_ts_files(fresh.path()),
            &read_ts_files(&committed_dir()),
        );
        assert!(
            problem.is_none(),
            "ui/src/bindings is not up to date ({}). Run `cargo test -p weaver --lib regenerate_bindings -- --ignored`",
            problem.unwrap_or_default()
        );
    }

    #[test]
    fn the_check_fails_on_an_outdated_missing_or_stale_file() {
        let generated: BTreeMap<_, _> = [
            ("A.ts".to_owned(), "a".to_owned()),
            ("B.ts".to_owned(), "b".to_owned()),
        ]
        .into();
        assert_eq!(difference(&generated, &generated), None);

        let mut outdated = generated.clone();
        outdated.insert("A.ts".into(), "changed by hand".into());
        assert!(difference(&generated, &outdated)
            .unwrap()
            .contains("outdated: A.ts"));

        let mut missing = generated.clone();
        missing.remove("B.ts");
        assert!(difference(&generated, &missing)
            .unwrap()
            .contains("missing: B.ts"));

        let mut stale = generated.clone();
        stale.insert("Old.ts".into(), "x".into());
        assert!(difference(&generated, &stale)
            .unwrap()
            .contains("stale (no Rust type produces it): Old.ts"));
    }

    /// Not a check: rewrites `ui/src/bindings`. Run it explicitly with `-- --ignored`.
    #[test]
    #[ignore = "rewrites the committed files; run explicitly to regenerate"]
    fn regenerate_bindings() {
        let dir = committed_dir();
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries {
                let path = entry.unwrap().path();
                if path.extension().is_some_and(|e| e == "ts") {
                    std::fs::remove_file(path).unwrap();
                }
            }
        }
        std::fs::create_dir_all(&dir).unwrap();
        export_all(&dir).unwrap();
    }
}

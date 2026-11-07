// The only place the UI talks to the Rust backend. Types come from `src/bindings/`, generated
// from the Rust types with ts-rs (never edit them by hand). The UI holds no domain state: it
// asks through commands and applies the backend's events.
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { getCurrentWebview, type DragDropEvent } from "@tauri-apps/api/webview";

import type { AppError } from "@/bindings/AppError";
import type { AppInfo } from "@/bindings/AppInfo";
import type { AtomicCommitsStatus } from "@/bindings/AtomicCommitsStatus";
import type { BoardSnapshot } from "@/bindings/BoardSnapshot";
import type { ChatProposals } from "@/bindings/ChatProposals";
import type { CardActionInput } from "@/bindings/CardActionInput";
import type { ChatCards } from "@/bindings/ChatCards";
import type { CommandName } from "@/bindings/CommandName";
import type { EventName } from "@/bindings/EventName";
import type { HistorySnapshot } from "@/bindings/HistorySnapshot";
import type { ConversationSnapshot } from "@/bindings/ConversationSnapshot";
import type { EntriesEvent } from "@/bindings/EntriesEvent";
import type { GeneralEntries } from "@/bindings/GeneralEntries";
import type { GeneralSnapshot } from "@/bindings/GeneralSnapshot";
import type { GeneralState } from "@/bindings/GeneralState";
import type { JobsSnapshot } from "@/bindings/JobsSnapshot";
import type { Notice } from "@/bindings/Notice";
import type { OmpStatus } from "@/bindings/OmpStatus";
import type { InitialiseResult } from "@/bindings/InitialiseResult";
import type { ProjectChanged } from "@/bindings/ProjectChanged";
import type { MergeAnswer } from "@/bindings/MergeAnswer";
import type { MergeBranches } from "@/bindings/MergeBranches";
import type { MergeStatus } from "@/bindings/MergeStatus";
import type { ProjectStatus } from "@/bindings/ProjectStatus";
import type { SoloChange } from "@/bindings/SoloChange";
import type { SoloMode } from "@/bindings/SoloMode";
import type { SkillsReport } from "@/bindings/SkillsReport";
import type { TreeEntry } from "@/bindings/TreeEntry";
import type { ShowRequest } from "@/bindings/ShowRequest";
import type { HideFailed } from "@/bindings/HideFailed";
import type { MenuOpenFailed } from "@/bindings/MenuOpenFailed";
import type { RecentEntry } from "@/bindings/RecentEntry";
import type { SettingChange } from "@/bindings/SettingChange";
import type { SettingsSnapshot } from "@/bindings/SettingsSnapshot";
import type { ViewSnapshot } from "@/bindings/ViewSnapshot";
import type { StartupFailure } from "@/bindings/StartupFailure";
import type { UpdateStatus } from "@/bindings/UpdateStatus";
import type { RequestView } from "@/bindings/RequestView";
import type { RequestsEvent } from "@/bindings/RequestsEvent";
import type { ActionInput } from "@/bindings/ActionInput";
import type { QuestionRoundChanged } from "@/bindings/QuestionRoundChanged";
import type { QuestionRoundView } from "@/bindings/QuestionRoundView";
import type { Step } from "@/bindings/Step";
import type { SubAgentConversation } from "@/bindings/SubAgentConversation";
import type { SubAgentEntries } from "@/bindings/SubAgentEntries";
import type { SubAgentsEvent } from "@/bindings/SubAgentsEvent";
import type { UsageStatus } from "@/bindings/UsageStatus";
import type { WelcomeState } from "@/bindings/WelcomeState";
import type { ProjectGit } from "@/bindings/ProjectGit";
import type { ProjectSwitcher } from "@/bindings/ProjectSwitcher";
import type { SystemCertificatesStatus } from "@/bindings/SystemCertificatesStatus";
import type { AskClosed } from "@/bindings/AskClosed";
import type { AskEntries } from "@/bindings/AskEntries";
import type { AskRow } from "@/bindings/AskRow";
import type { AskSnapshot } from "@/bindings/AskSnapshot";
import type { AsksSnapshot } from "@/bindings/AsksSnapshot";
import type { AskTarget } from "@/bindings/AskTarget";
import type { CopyInResult } from "@/bindings/CopyInResult";
import type { EntryChange } from "@/bindings/EntryChange";
import type { FileEntry } from "@/bindings/FileEntry";
import type { TreeChanged } from "@/bindings/TreeChanged";
import type { TreeSnapshot } from "@/bindings/TreeSnapshot";
import type { TreeUpdate } from "@/bindings/TreeUpdate";
import type { FilesCloseTab } from "@/bindings/FilesCloseTab";
import type { FilesFlush } from "@/bindings/FilesFlush";
import type { FilesSaveFailed } from "@/bindings/FilesSaveFailed";
import type { TabText } from "@/bindings/TabText";
import type { FileLockChanged } from "@/bindings/FileLockChanged";
import type { FileOpenResult } from "@/bindings/FileOpenResult";
import type { FileReloadResult } from "@/bindings/FileReloadResult";
import type { FileSaveResult } from "@/bindings/FileSaveResult";
import type { FileTabChanged } from "@/bindings/FileTabChanged";
import type { AssetRenamed } from "@/bindings/AssetRenamed";
import type { AssetsChanged } from "@/bindings/AssetsChanged";
import type { AssetsCopied } from "@/bindings/AssetsCopied";
import type { AssetsDeleted } from "@/bindings/AssetsDeleted";
import type { AssetsSnapshot } from "@/bindings/AssetsSnapshot";
import type { NotesChanged } from "@/bindings/NotesChanged";
import type { NotesFlush } from "@/bindings/NotesFlush";
import type { NotesImagesAdded } from "@/bindings/NotesImagesAdded";
import type { NotesSheet } from "@/bindings/NotesSheet";
import type { NotesTrashed } from "@/bindings/NotesTrashed";
import type { NotesWritten } from "@/bindings/NotesWritten";
import type { PastedImage } from "@/bindings/PastedImage";

/** Payload of each event, by name. */
export interface EventPayloads {
  "project-changed": ProjectChanged;
  "startup-failed": StartupFailure;
  "update-changed": UpdateStatus;
  /** A setting was written: the whole snapshot, so a window never merges. */
  "settings-changed": SettingsSnapshot;
  /** omp's readiness changed: the whole status of the Machine `omp-setup`, with its revision. */
  "omp-changed": OmpStatus;
  /** The usage limits, or their reading, changed: the whole status, with its revision. */
  "usage-changed": UsageStatus;
  /** This window came forward because of a notification or a second launch: what to show. */
  "show-requested": ShowRequest;
  /** This window could not go to the background after `Keep working in background`: it stays as it was; the reason. */
  "window-hide-failed": HideFailed;
  /** The recent projects changed (an opening, a removal, `Clear Recent` of the File menu): the whole list, sent to the welcome window only. */
  "recent-changed": RecentEntry[];
  /** An opening from the File menu failed: sent to the window it was chosen from only (the welcome window when none had the focus). */
  "menu-open-failed": MenuOpenFailed;
  /** The open projects, the recent ones or the jobs that work in a project changed: the whole list, sent to every project window. */
  "project-switcher-changed": ProjectSwitcher;
  /** The branch or the user name of this window's project changed: sent to that window only, with a revision. */
  "project-git-changed": ProjectGit;
  /** Something to read at the top of the Board: a refused decision, a Proposal sent back to Draft. */
  notice: Notice;
  /** The Jobs list of a project changed: the whole list. */
  "jobs-changed": JobsSnapshot;
  /** The conversation of a job changed: the entries that changed, in batches counted by `seq`. */
  "job-entries": EntriesEvent;
  /** The list of the sub-agents of a job or of General changed: the whole list. */
  "sub-agents-changed": SubAgentsEvent;
  /** The conversation of the sub-agent on screen changed: the entries that changed, in batches counted by `seq`. */
  "sub-agent-entries": SubAgentEntries;
  /** The state of the General of a project changed: its whole state. */
  "general-changed": GeneralState;
  /** The conversation of a General changed: the entries that changed, in batches counted by `seq`. */
  "general-entries": GeneralEntries;
  /** The cards of questions of a project's Chat changed: the whole list, at a revision. */
  "chat-cards-changed": ChatCards;
  /** An Ask more conversation opened or changed: its row. */
  "ask-changed": AskRow;
  /** The conversation of an Ask more conversation changed: the entries that changed. */
  "ask-entries": AskEntries;
  /** An Ask more conversation closed. */
  "ask-closed": AskClosed;
  /** The open design requests of a project changed (a decision, or a file changed on disk). */
  "requests-changed": RequestsEvent;
  /** The window is closing: the request editor saves what is pending and answers with `requests_flushed`. */
  "requests-flush": RequestsEvent;
  /** The window is closing and the File view has unsaved changes: it answers with `files_flushed`. */
  "files-flush": FilesFlush;
  /** `Save all` could not save these files: the File view shows the first one's tab, with the reason. */
  "files-save-failed": FilesSaveFailed;
  /** `Cmd+W` / `Ctrl+W` while the File view has a tab: the menu's Close Window closes the active tab instead. */
  "files-close-tab": FilesCloseTab;
  /** The file of a tab changed, disappeared or reappeared on disk, by anything but the tab's own save. */
  "file-tab-changed": FileTabChanged;
  /** The agent's lock of a file open in a tab was taken or released. */
  "file-lock-changed": FileLockChanged;
  /** Listed folders of the File view's tree changed on disk or by an action of the tree: the folders read again and the ones gone, at a generation. */
  "files-tree-changed": TreeChanged;
  /** The round on screen in a project changed (one opened, ended, or queued behind it). */
  "question-round-changed": QuestionRoundChanged;
  /** The state of a project, the files it installs and the opening check changed: the whole status. */
  "project-state": ProjectStatus;
  /** The status of System Certificates changed: the whole status, with its revision. */
  "system-certificates-changed": SystemCertificatesStatus;
  /** The assets of a project changed (`workspace.ASSETS_CHANGED`): the whole list, at a generation. */
  "assets-changed": AssetsChanged;
  /** The notes sheet or an image of the notes folder changed on disk by anything but Weaver's own write (`workspace.NOTES_CHANGED`). */
  "notes-changed": NotesChanged;
  /** The window is closing: the Notes view answers with `notes_flushed`, with its waiting text and the images to trash. */
  "notes-flush": NotesFlush;
}

/** Arguments and result of each command, by name. */
export interface Commands {
  app_info: { args: Record<string, never>; result: AppInfo };
  startup_failures: { args: Record<string, never>; result: StartupFailure[] };
  /** Reveals the log file in the system's file manager. */
  open_log: { args: Record<string, never>; result: null };
  /** Opens an `http`/`https` link in the system browser (the backend refuses any other). */
  open_url: { args: { url: string }; result: null };
  /** The update on offer (none when there is none or **Later** hid it). */
  update_status: { args: Record<string, never>; result: UpdateStatus };
  /** **Later**: hides the update until the next check. */
  update_later: { args: Record<string, never>; result: null };
  /** **Install and restart**: downloads the update, then quits Weaver through its quit path. */
  update_install: { args: Record<string, never>; result: null };
  /** The settings in force, what the file ignored and the description of every setting. */
  settings_get: { args: Record<string, never>; result: SettingsSnapshot };
  /** Validates and stores one change; `invalid-input` carries the message for the field. */
  settings_set: { args: { change: SettingChange }; result: SettingsSnapshot };
  /** The whole Board of an open project, worked out by the backend at one revision. */
  board_snapshot: { args: { projectId: string }; result: BoardSnapshot };
  /** **Delete** on a Task in To do; `refused` carries the notice when it no longer holds. */
  delete_task: { args: { projectId: string; taskId: string }; result: null };
  /** **Approve** on a Proposal: approved, or Refining for a stale change; `refused` carries the notice. */
  approve_proposal: { args: { projectId: string; proposalId: string }; result: null };
  /** **Reject** (`feedback` is the optional reason) and, with `rework`, **Rework with feedback…**. */
  reject_proposal: { args: { projectId: string; proposalId: string; feedback: string | null; rework: boolean }; result: null };
  /** **Discard** on a Draft a refinement left half-way. */
  discard_proposal: { args: { projectId: string; proposalId: string }; result: null };
  /** The model control of a Proposal: a model of omp, or `null` for the Settings models. */
  choose_proposal_model: { args: { projectId: string; proposalId: string; model: string | null }; result: null };
  /** **Resolve** on a Drift: a round of the prepared question, or a resolve job; `refused` carries the notice. */
  resolve_drift: { args: { projectId: string; driftId: string }; result: null };
  /** **Fix the code** on a Drift: the fix option (required when it offers some) and the note. */
  fix_drift_code: { args: { projectId: string; driftId: string; option: number | null; note: string | null }; result: null };
  /** **Accept Proposal** on a Drift with an attached Proposal. */
  accept_drift_proposal: { args: { projectId: string; driftId: string }; result: null };
  /** **Refine** on a Drift whose attached Proposal is stale or was left half-way: a refinement job adapts it; the Drift is not decided. */
  refine_drift_proposal: { args: { projectId: string; driftId: string }; result: null };
  /** **Discard** on a Drift whose attached Draft a refinement left half-way; the Drift is not decided. */
  discard_drift_proposal: { args: { projectId: string; driftId: string }; result: null };
  /** **Dismiss Drift**, once confirmed. */
  dismiss_drift: { args: { projectId: string; driftId: string }; result: null };
  /** The whole History of an open project (entries newest first), worked out at one revision. */
  history_snapshot: { args: { projectId: string }; result: HistorySnapshot };
  /**
   * The Requirements and Specs of an open project at one revision, with the Proposal `overlay`
   * drawn over the Model when it is still open (`null`: the current Model). The answer names the
   * Proposal it really drew.
   */
  view_snapshot: { args: { projectId: string; overlay: string | null }; result: ViewSnapshot };
  /** The state of the project (Not initialised, Empty Model, Has a Model), what Get Started lists and the opening check. */
  project_status: { args: { projectId: string }; result: ProjectStatus };
  /**
   * **Initialise**: creates `.weaver` and the agent files; `force` confirms overwriting agent files that differ (else they
   * come back as `conflicts`); `solo` is the mode of Solo mode chosen first (`off`: git is not called). A Solo mode that
   * cannot be turned on refuses the initialisation with its reason and nothing is created.
   */
  initialise_project: { args: { projectId: string; force: boolean; solo: SoloMode }; result: InitialiseResult };
  /** **Solo mode** of the working copy: the new state with the tracked files the block excludes, or the reason it could not change. */
  set_solo_mode: { args: { projectId: string; mode: SoloMode }; result: SoloChange };
  /** The files git tracks that Solo mode's block excludes, computed now: asked each time Settings is shown. */
  solo_tracked_files: { args: { projectId: string }; result: string[] };
  /** **Atomic commits** of the working copy: the new state, or the reason it could not change (only Settings changes it). */
  set_atomic_commits: { args: { projectId: string; enabled: boolean }; result: AtomicCommitsStatus };
  /** Atomic commits with git asked whether it can be turned on: asked each time Settings is shown. */
  check_atomic_commits: { args: { projectId: string }; result: AtomicCommitsStatus };
  /**
   * The state of **Merge** worked out again (the Board is shown), in the project's background work; `null` while Solo mode is
   * off. Also arrives with every `project-state` as `ProjectStatus.merge`.
   */
  merge_refresh: { args: { projectId: string }; result: MergeStatus | null };
  /** The branches the Dialog of Merge offers (local, then remote ones git already knows) and Weaver's files git still tracks. */
  merge_branches: { args: { projectId: string }; result: MergeBranches };
  /**
   * **Merge**: merges `branch` into the current branch. A refusal, a branch that brings nothing new or a time limit come back
   * as the `notice` of the answer; the new state is its `merge`.
   */
  merge_start: { args: { projectId: string; branch: string }; result: MergeAnswer };
  /** **Abort merge**, once confirmed: the new state, with a `notice` when git could not abort. */
  merge_abort: { args: { projectId: string }; result: MergeAnswer };
  /** **Resolve with the agent**: starts the job that resolves the conflicts; the new state. */
  resolve_merge_conflicts: { args: { projectId: string }; result: MergeStatus };
  /** **Update skills**: the structured report the command line prints too. */
  update_skills: { args: { projectId: string }; result: SkillsReport };
  /** The entries of one folder of the project (`path` relative, empty for the project folder) for the import tree. */
  project_folder: { args: { projectId: string; path: string }; result: TreeEntry[] };
  /** **Start import**: starts the import job, leaving out the `excluded` paths (relative, `/`). */
  import_code: { args: { projectId: string; excluded: string[] }; result: null };
  /** The status of omp now (the Machine `omp-setup`). */
  omp_status: { args: Record<string, never>; result: OmpStatus };
  /** **Install omp** / **Open installer again**: opens the system terminal with the official installer. */
  omp_install: { args: Record<string, never>; result: null };
  /** **Log in**: opens the system terminal with `omp login <provider>`; `provider` is an id of the fixed list. */
  omp_log_in: { args: { provider: string }; result: null };
  /** **Log out**: `omp auth-broker logout <provider>`; the outcome arrives with `omp-changed`. */
  omp_log_out: { args: { provider: string }; result: null };
  /** `omp usage --json` for the connected providers (only while omp is Ready); answers the status when the reading ends. */
  omp_usage: { args: Record<string, never>; result: UsageStatus };
  /** The usage limits as the windows last heard of them, without reading anything (the summary in the sidebar of a window that just loaded). */
  usage_status: { args: Record<string, never>; result: UsageStatus };
  /** The name, version and recent projects of the welcome window. */
  welcome_state: { args: Record<string, never>; result: WelcomeState };
  /** `developer.OPEN_PROJECT` for a folder: opens its window or brings it forward. */
  open_project: { args: { path: string }; result: null };
  /** The system folder picker, then the same as `open_project`; `false` when none was chosen. */
  open_folder: { args: Record<string, never>; result: boolean };
  /** `Open Folder…` of the project switcher and the palette: the picker of the File menu, modal to this window; answers at once, and a failed opening comes back as `menu-open-failed`. */
  open_folder_from_window: { args: Record<string, never>; result: null };
  /** The open projects (with their working jobs) and the recent ones without a window, for the menu of the project switcher. */
  project_switcher: { args: Record<string, never>; result: ProjectSwitcher };
  /** The branch and the user name of this window's project, as they are now. */
  project_git: { args: Record<string, never>; result: ProjectGit };
  /** Removes a recent project; answers the welcome state without it. */
  remove_recent: { args: { path: string }; result: WelcomeState };
  /** `Open in editor`: `file` (absolute, or relative to the project) at `line`; a failure carries the reason. */
  open_in_editor: { args: { file: string; line?: number | null; projectId?: string | null }; result: null };
  /** The Jobs list of an open project. */
  jobs_snapshot: { args: { projectId: string }; result: JobsSnapshot };
  /** The conversation of a job, with the count of change batches it holds. */
  job_conversation: { args: { projectId: string; jobId: string }; result: ConversationSnapshot };
  /** A message to the agent of a job; it reaches the agent without stopping it. */
  job_send_message: { args: { projectId: string; jobId: string; text: string }; result: null };
  /** `Stop the agent`. */
  job_stop: { args: { projectId: string; jobId: string }; result: null };
  /** The developer's choice on a permission card: one of the options omp offered. */
  job_answer_permission: { args: { projectId: string; jobId: string; request: number; option: string }; result: null };
  /** The `X` of a finished job's row. */
  job_remove: { args: { projectId: string; jobId: string }; result: null };
  /** The list of the sub-agents of a job, each with the ones it started. */
  job_sub_agents: { args: { projectId: string; jobId: string }; result: SubAgentsEvent };
  /** The read-only conversation of the sub-agent at `path` of a job. */
  job_sub_agent_conversation: { args: { projectId: string; jobId: string; path: Step[] }; result: SubAgentConversation };
  /** What the Jobs view shows of a job's sub-agents: whether their list is on screen, and the conversation of which one. */
  job_sub_agents_view: { args: { projectId: string; jobId: string; listShown: boolean; path: Step[] | null }; result: null };
  /** `Run` on a Task; a refusal (`refused`) carries the notice. */
  run_task: { args: { projectId: string; taskId: string }; result: null };
  /** `Run all`; a refusal carries the notice. */
  run_all: { args: { projectId: string }; result: null };
  /** `Scan System`, once confirmed. */
  scan_system: { args: { projectId: string }; result: null };
  /** The conversation and the state of the General of an open project. */
  general_snapshot: { args: { projectId: string }; result: GeneralSnapshot };
  /** A message to the General; omp starts when it is not running. */
  general_send_message: { args: { projectId: string; text: string }; result: null };
  /** `Stop` during a turn of the General. */
  general_stop: { args: { projectId: string }; result: null };
  /** The developer's choice on a permission card of the General. */
  general_answer_permission: { args: { projectId: string; request: number; option: string }; result: null };
  /** `Clear General?`, once confirmed: the conversation and its record are deleted. */
  general_clear: { args: { projectId: string }; result: null };
  /** The Proposals that the listed jobs of the Chat handed over, in creation order. */
  general_proposals: { args: { projectId: string; jobs: string[] }; result: ChatProposals };
  /** The cards of questions of the Chat, in publication order; after that `chat-cards-changed` brings the changes. */
  general_cards: { args: { projectId: string }; result: ChatCards };
  /** What the developer does in the active card of questions; the cards afterwards. */
  general_card_act: { args: { projectId: string; card: string; action: CardActionInput }; result: ChatCards };
  /** `developer.SEND_ANSWERS`: the answers of the active card go to the agent; refused while a question has neither an answer nor a skip. */
  general_send_answers: { args: { projectId: string; card: string }; result: null };
  /** The list of the sub-agents of the General of an open project. */
  general_sub_agents: { args: { projectId: string }; result: SubAgentsEvent };
  /** The read-only conversation of the sub-agent at `path` of the General of an open project. */
  general_sub_agent_conversation: { args: { projectId: string; path: Step[] }; result: SubAgentConversation };
  /** What the Chat view shows of the sub-agents: the conversation of which one (`null`: none). */
  general_sub_agents_view: { args: { projectId: string; path: Step[] | null }; result: null };
  /** `Ask more`: reads the chosen thing and opens a new conversation; its identity. Refused when the thing is gone. */
  ask_open: { args: { projectId: string; target: AskTarget }; result: string };
  /** The Ask more conversations of a project, the most recent first. */
  ask_list: { args: { projectId: string }; result: AsksSnapshot };
  /** One Ask more conversation as it is now. */
  ask_snapshot: { args: { askId: string }; result: AskSnapshot };
  /** A message to an Ask more conversation; the first one starts omp and carries the instruction. */
  ask_send_message: { args: { askId: string; text: string }; result: null };
  /** `Stop` during a turn of an Ask more conversation. */
  ask_stop: { args: { askId: string }; result: null };
  /** The developer's choice on a permission card of an Ask more conversation. */
  ask_answer_permission: { args: { askId: string; request: number; option: string }; result: null };
  /** Closes an Ask more conversation (confirmed when its agent works). */
  ask_close: { args: { askId: string }; result: null };
  /** The list of the sub-agents of an Ask more conversation. */
  ask_sub_agents: { args: { askId: string }; result: SubAgentsEvent };
  /** The read-only conversation of the sub-agent at `path` of an Ask more conversation. */
  ask_sub_agent_conversation: { args: { askId: string; path: Step[] }; result: SubAgentConversation };
  /** What the Jobs view shows of the sub-agents of an Ask more conversation. */
  ask_sub_agents_view: { args: { askId: string; listShown: boolean; path: Step[] | null }; result: null };
  /** The text and standing of one request. */
  request_read: { args: { projectId: string; requestId: string }; result: RequestView };
  /** `developer.NEW_REQUEST`: an empty request named after the local time `YYYY-MM-DDTHH:MM:SS`; its id. */
  request_new: { args: { projectId: string; localTime: string }; result: string };
  /** Saves the text of a request being written. */
  request_save: { args: { projectId: string; requestId: string; text: string }; result: null };
  /** `developer.SEND_REQUEST`; an empty request does nothing. */
  request_send: { args: { projectId: string; requestId: string }; result: null };
  /** `developer.DISCARD_REQUEST`: the file and the images are deleted. */
  request_discard: { args: { projectId: string; requestId: string }; result: null };
  /** `developer.LEAVE_REQUEST` (`closing` false) or `window.CLOSED` (`closing` true): discards the request when it is in writing and its file is empty; nothing otherwise. */
  request_leave: { args: { projectId: string; requestId: string; closing: boolean }; result: null };
  /** Saves a pasted image (base64 `data`); the path to link, relative to the request file. */
  request_image_add: { args: { projectId: string; requestId: string; data: string }; result: string };
  /** Links a dropped image file; the path to link, relative to the request file. */
  request_image_import: { args: { projectId: string; requestId: string; path: string }; result: string };
  /** An image of the request's own images folder as a data URL, for the preview. */
  request_image: { args: { projectId: string; requestId: string; link: string }; result: string };
  /** The editor saved what was pending (the answer to `requests-flush`). */
  requests_flushed: { args: { projectId: string }; result: null };
  /** The File view says in which files it has unsaved changes (every path, each time; none when it has none). */
  files_unsaved: { args: { projectId: string; paths: string[] }; result: null };
  /** The answer to `files-flush`: the texts of the tabs with unsaved changes. */
  files_flushed: { args: { projectId: string; tabs: TabText[] }; result: null };
  /** The File view has a tab, so `Cmd+W` / `Ctrl+W` closes it instead of the window (`files-close-tab`). */
  files_close_key: { args: { projectId: string; claimed: boolean }; result: null };
  /** `developer.OPEN_FILE`: opens a file in a tab. `tabs` are the open ones, least recently shown first, for the limit of 10. */
  file_open: { args: { projectId: string; path: string; tabs: { path: string; unsaved: boolean }[] }; result: FileOpenResult };
  /** Reads the file of a tab again (`developer.RELOAD_FILE`, a change on disk, a release of the lock). */
  file_reload: { args: { projectId: string; path: string }; result: FileReloadResult };
  /** `developer.SAVE_FILE` (`check`), `developer.OVERWRITE_FILE` (`overwrite`) and `Save again` (`again`); `hash` is the one the tab read, the one `check` compares with the file on disk. */
  file_save: { args: { projectId: string; path: string; text: string; hash: string; mode: "check" | "overwrite" | "again" }; result: FileSaveResult };
  /** `file-tab.CLOSED`: frees the tab's state, its watch and its lock tracking. */
  file_close: { args: { projectId: string; path: string }; result: null };
  /** `developer.OPEN_FILE_VIEW`: the root's entries, or the tree as it was within 10 s of leaving. */
  files_open_view: { args: { projectId: string }; result: TreeSnapshot };
  /** `developer.EXPAND_FOLDER`: reads that folder only. */
  files_expand: { args: { projectId: string; path: string }; result: TreeUpdate };
  /** `developer.COLLAPSE_FOLDER`: the backend forgets the folder and those inside it. */
  files_collapse: { args: { projectId: string; path: string }; result: null };
  /** The folders of `path` for the tree of `Move to…`; nothing is kept or watched. */
  files_folders: { args: { projectId: string; path: string }; result: FileEntry[] };
  /** `developer.LEAVE_FILE_VIEW`, once the tabs closed: the backend keeps the tree 10 s, then drops it. */
  files_leave: { args: { projectId: string }; result: null };
  /** `developer.CREATE_ENTRY`: an empty file, or with `folder` a folder; a refusal (`refused`) carries the reason. */
  files_create: { args: { projectId: string; parent: string; name: string; folder: boolean }; result: EntryChange };
  /** `developer.RENAME_ENTRY`. */
  files_rename: { args: { projectId: string; path: string; name: string }; result: EntryChange };
  /** `developer.MOVE_ENTRY` into the folder `target` (empty: the project root). */
  files_move: { args: { projectId: string; path: string; target: string }; result: EntryChange };
  /** `developer.DROP_EXTERNAL_ENTRIES`: copies the dropped `paths` (absolute, as Tauri reports them) into the folder `target` (empty: the project root); a refusal or a failure (`refused`) carries the reason, and nothing was copied. */
  files_copy_in: { args: { projectId: string; target: string; paths: string[] }; result: CopyInResult };
  /** `developer.DELETE_ENTRY`: to the system's trash. */
  files_delete: { args: { projectId: string; path: string }; result: TreeUpdate };
  /** The round presented in the window of a project, when there is one. */
  question_round_get: { args: { projectId: string }; result: QuestionRoundView | null };
  /** What the developer does in the presented round; the round on screen afterwards. `refused` carries why not. */
  question_round_act: { args: { projectId: string; roundId: number; action: ActionInput }; result: QuestionRoundView | null };
  /** The status of System Certificates now (the Machine `system-certificates`). */
  system_certificates_status: { args: Record<string, never>; result: SystemCertificatesStatus };
  /** **Refresh**: exports the certificates again; refused (`invalid-input`) while the setting is off. */
  system_certificates_refresh: { args: Record<string, never>; result: SystemCertificatesStatus };
  /** Closes the notice that Weaver turned System Certificates on, in every window. */
  system_certificates_dismiss_notice: { args: Record<string, never>; result: SystemCertificatesStatus };
  /** Closes the banner of a failed automatic export, in every window. */
  system_certificates_dismiss_failure: { args: Record<string, never>; result: SystemCertificatesStatus };
  /** The assets of an open project as the backend holds them, at a generation; later changes arrive as `assets-changed`. */
  assets_list: { args: { projectId: string }; result: AssetsSnapshot };
  /** `developer.DROP_ASSETS`: copies the dropped `paths` (absolute, as Tauri reports them) into `.weaver/assets/`; a refusal or a failure (`refused`) carries the reason, and nothing was copied. An ignored drop (the project's own assets) copies nothing and answers no names. */
  assets_copy_in: { args: { projectId: string; paths: string[] }; result: AssetsCopied };
  /** `developer.RENAME_ASSET`: the asset `name` takes the name `newName`; a refusal (`refused`) carries the reason. */
  assets_rename: { args: { projectId: string; name: string; newName: string }; result: AssetRenamed };
  /** `developer.DELETE_ASSETS`: each asset to the system's trash; the ones the trash refused come back with their reasons. */
  assets_delete: { args: { projectId: string; names: string[] }; result: AssetsDeleted };
  /** `developer.DRAG_ASSETS_OUT`: starts a native drag of the assets `names` out of this window, as copies; the assets never change. */
  assets_drag_out: { args: { projectId: string; names: string[] }; result: null };
  /** The notes sheet as it is on disk (CRLF read as LF), with the SHA-256 of its bytes; no text and no hash when the file is missing. */
  notes_read: { args: { projectId: string }; result: NotesSheet };
  /** `developer.EDIT_NOTES`: writes the whole sheet atomically (UTF-8, LF), creating the notes folder first; the new hash. A failure rejects with the reason. */
  notes_write: { args: { projectId: string; text: string }; result: NotesWritten };
  /** An image of the notes folder as `data:<mime>;base64,<…>`, from one read; rejects for a name that is not a PNG, JPEG, GIF or WebP file there. */
  notes_image: { args: { projectId: string; name: string }; result: string };
  /** `developer.ADD_NOTES_IMAGES`: copies images pasted from the clipboard (bytes in base64, with the file name when there is one); one outcome per file, in order. `localTime` is `YYYY-MM-DDTHH:MM:SS`. */
  notes_add_pasted: { args: { projectId: string; files: PastedImage[]; localTime: string }; result: NotesImagesAdded };
  /** `developer.ADD_NOTES_IMAGES`: copies image files dropped on the sheet (absolute paths); one outcome per file, in order. */
  notes_add_dropped: { args: { projectId: string; paths: string[]; localTime: string }; result: NotesImagesAdded };
  /** `developer.LEAVE_NOTES_VIEW`: each image `names` to the system's trash, one at a time; the ones the trash refused come back with their reasons. */
  notes_trash: { args: { projectId: string; names: string[] }; result: NotesTrashed };
  /** The answer to `notes-flush`: the sheet's waiting text (none when nothing waits) and the images to trash after it is written. */
  notes_flushed: { args: { projectId: string; text: string | null; trash: string[] }; result: null };
}

// Compile-time proof that the maps above cover exactly the generated names.
type AssertSame<A, B> = [A] extends [B] ? ([B] extends [A] ? true : never) : never;
const _commands: AssertSame<keyof Commands, CommandName> = true;
const _events: AssertSame<keyof EventPayloads, EventName> = true;
void _commands;
void _events;

/** A command failed with the typed error of the backend. */
export function isAppError(value: unknown): value is AppError {
  return typeof value === "object" && value !== null && "code" in value && "message" in value;
}

export function call<N extends CommandName>(
  name: N,
  args: Commands[N]["args"] = {},
): Promise<Commands[N]["result"]> {
  return invoke<Commands[N]["result"]>(name, args);
}

export function on<N extends EventName>(
  name: N,
  handler: (payload: EventPayloads[N]) => void,
): Promise<UnlistenFn> {
  return listen<EventPayloads[N]>(name, (event) => handler(event.payload));
}

/**
 * The label of this webview's window: `welcome`, or `p-<project hash>` for a project window. Outside
 * Tauri (the dev server in a browser) there is no window and the label is `welcome`.
 */
export function currentWindowLabel(): string {
  try {
    return getCurrentWindow().label;
  } catch {
    return "welcome";
  }
}

/**
 * Calls `handler` with every event of the system drag and drop of the webview (`enter`, `over`,
 * `drop`, `leave`; positions in physical pixels). Outside Tauri nothing is ever dragged this way.
 * Returns what stops the listening.
 */
export async function onDragDrop(handler: (event: DragDropEvent) => void): Promise<UnlistenFn> {
  try {
    return await getCurrentWebview().onDragDropEvent((event) => handler(event.payload));
  } catch {
    return () => {};
  }
}

/**
 * Calls `handler` with the paths of the files dropped on this window (the system drag and drop of
 * the webview; outside Tauri nothing is ever dropped this way). Returns what stops the listening.
 */
export function onFilesDropped(handler: (paths: string[]) => void): Promise<UnlistenFn> {
  return onDragDrop((event) => {
    if (event.type === "drop") handler(event.paths);
  });
}

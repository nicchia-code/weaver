# Changelog

All notable changes to Weaver that a user of the app can see are documented in this file.

The format is based on [Keep a Changelog 1.1.0](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

## [0.3.0] - 2026-11-05

### Added

- When omp's own download channel is blocked by a corporate proxy, Weaver downloads omp from its GitHub releases by itself, on macOS, Windows and Linux: in the installer's terminal after the official installer fails, and in the background after `omp update` fails (only for an omp in the folder of its installer). The file is installed only when its size and SHA-256 are the ones GitHub publishes and it answers `--version`.
- The File menu of every window, project and welcome, opens a folder (`Open Folder…`, `Cmd+O` on macOS and `Ctrl+O` on Windows and Linux) or one of the recent projects (`Open Recent`) without touching the window it is chosen from, on macOS, Windows and Linux; a project already open only comes to the front, a failed opening says why in the window it was chosen from, and `Clear Recent` empties the list of recent projects.
- `Merge` in the Board header, while Solo mode is on, brings another branch (local, or a remote one git already knows) into the current branch with git: a banner shows the merge in progress with the number of files with conflicts, which you resolve by hand or with the agent before committing the merge in git, or abort. Weaver never commits a merge with conflicts and never fetches. When the merge is concluded Weaver starts by itself a scan of the whole System that reads the files the merge brought, and its Drifts from the merge recommend adapting the Model.

### Changed

- A failed omp installer is explained by its cause, with its exit code: a blocked download, a certificate that could not be verified, a file from GitHub that is incomplete or altered and was not installed, or an omp from GitHub that does not start.
- A failed download of an omp update, in the background, says its cause in the same plain words: a blocked download, a certificate that could not be verified, an incomplete or altered file, an omp that does not start, a failed download with its exit code, or a download that timed out.

### Fixed

- In Chat and in Ask more the agent no longer tries to design a change itself with the design skill, which only works inside a job, and no longer gets stuck between that skill and `weaver job new`: it says that a change of the Model or the Requirements starts from a design request written with `+ Work Item` on the Board, and sends one already written when the developer asks.

## [0.2.0] - 2026-11-05

### Added

- The releases hold the Windows build (`Weaver_<version>_x64-setup.exe`) and the Linux builds (`Weaver_<version>_amd64.AppImage` and `Weaver_<version>_amd64.deb`) again, next to the macOS one, and Weaver updates itself from them on every system.
- A new `Solo mode`, for using Weaver in a git repository where the others do not: while it is on, Weaver's files stay only in this working copy and never go into a commit, because Weaver keeps a block of its own in the repository's local git exclusions (`.git/info/exclude`). It belongs to the working copy (another clone or a linked worktree starts with it off) and keeps out either the whole `.weaver/`, `.agents/skills/` and `.omp/` folders or `.weaver/` and only the files Weaver installs in the other two. Choose it in Get Started before initialising, in Settings → Project, or with `solo` in `weaver project new` and `weaver project edit`; Settings and `weaver project show` list the Weaver files git already tracks, which an exclusion cannot hide, and Weaver writes its block again by itself when a project opens. It needs git, and Weaver's files are then versioned nowhere: losing the working copy loses them.
- Files and folders dragged from the Finder or Explorer onto the tree in Files are copied into the project: onto a folder, or onto the empty space of the tree for the project's root. Folders are copied with everything inside them and symbolic links stay links; the originals stay where they are. A toast shows the copy while it runs, and when it ends the folder opens and the first copied entry is selected. A drop is refused as a whole, with the reason, when something dropped is already in the project, when a name is taken or repeated, or when the target is `.weaver/`, and a copy that fails part way is removed.
- A tab in Files on an image (PNG, JPEG, GIF, WebP, BMP or ICO, recognised by its first bytes, up to 10 MB) now shows the image, with its size in pixels and its weight, fitted to the panel; a click, or `Enter` or `Space`, switches to the real size, and the tab reads the image again when the file changes. An image over 10 MB opens as a binary file and says that it is too large. A tab on an SVG file switches between its drawing and its text with `Code` / `Preview`, keeping the cursor and the undo history.
- A new `Assets` view, after Files in the sidebar, for the files you keep for the agent in `.weaver/assets/`, which is versioned with the project and made when the first asset arrives. Files dragged from the Finder or Explorer onto the grid are copied (a drop is refused as a whole for a folder, a link, a name already used or not valid, or two files with one name); the assets show as icons by kind in name order, are selected with the mouse or the keyboard, renamed (`F2`), deleted to the system's trash after a confirmation, and dragged out to other apps, which receive copies. An asset over 10 MB is copied with a warning and carries a mark.
- A new `Notes` view, after Assets in the sidebar, with one Notion-style Markdown sheet per project, saved automatically as you write. Markdown shortcuts work as you type (`# ` for a heading, `[ ] ` for a checklist item) and ticked checklist items stay in place, struck through. Images (PNG, JPEG, GIF or WebP, up to 10 MB) are pasted or dragged in from the Finder or Explorer. The notes stay on this computer only, in `.weaver/notes/`: they are not versioned and never shown to the agent.
- When a window cannot go to the background after `Keep working in background`, it stays as it was and an error toast, `Could not hide the window.`, tells why.

### Changed

- Weaver has a new icon: a "W" woven from pastel ribbons on a cream rounded square.
- `Refine` now serves every stale Proposal, whatever its kind (change, setup, import or resolution), attached to a Drift or not, whatever else blocks it and also while a Run works. It is refused only while the agent that handed the Proposal over is still finishing and, for a resolution, when none of its Drifts is still open; a half-way refinement continues under the same conditions.
- A Drift whose attached Proposal is stale and Open now has `Refine` in its details in place of `Accept Proposal` (tooltip `Refine on the current Model`): a refinement job adapts the Proposal to the current Model and Requirements and the Drift stays open, to be decided again once the Proposal is handed over. It is disabled only while the agent that handed the Proposal over is still finishing, also during a Run. When a refinement was left half-way, the details show `Refinement interrupted` with `Refine` (`Continue refining`) and `Discard` instead. While the attached Proposal is `Refining`, `Accept Proposal`, `Fix the code`, `Resolve` and `Dismiss Drift` are disabled with `Being refined`; a resolution being refined counts as one being written, and one left half-way as one waiting for a decision.

### Removed

- The `Hide` entry of the sidebar is gone: a window goes to the background only with `Keep working in background` in the confirmation that appears when you close it while agent jobs work.

### Fixed

- Moving a file or folder by dragging its row in the tree in Files now works in the app: the target folder is highlighted, a collapsed folder held over for a second opens, and the tree scrolls when the pointer is near its top or bottom edge.

## [0.1.0] - 2026-11-03

### Added

- Ask more opens a new conversation with omp about the Proposal, Task or Drift in a details dialog, a History entry with the Tasks it cites, the question in view of a round or the Requirements as a whole, from a button in the toolbar of the Requirements view. The first message reaches the agent together with what Weaver knows about the chosen thing; for the Requirements, the agent loads by itself the Requirement skills that fit the question. The conversation is in the Jobs view, is not a job and ends with the window.
- System Certificates, off by default, makes every omp process Weaver starts trust the certificate authorities the system trusts, so omp can install itself and reach its provider behind a corporate proxy that inspects HTTPS traffic. It is in Settings → omp, in the omp box of Get Started and in the welcome window.
- Weaver turns System Certificates on by itself, once, when Zscaler is installed (macOS and Windows), and never turns it on again after it has been turned off.
- Where Weaver shows why omp failed, certificate errors, HTTP errors and a failed installer are explained in plain words, with the suggestion to turn on System Certificates when it is off and a corporate proxy is the likely cause.
- Usage, in the sidebar above Settings, opens a dialog with the usage limits of the providers omp is connected to and when each limit resets. Weaver asks omp for them only when Usage is opened, and never by calling a model.
- In Jobs, the sub-agents of the selected job are listed under its row, with the ones they started, and each opens its own read-only conversation, while the job works and after it ends. Each sub-agent line under a `task` row in a conversation opens it too, and a sub-agent that finished no longer shows as working.
- In Chat, each sub-agent the agent starts opens its own read-only conversation in place of the chat, with a `Back to Chat` button that returns to the chat where it was.
- Files, in the sidebar after Jobs, shows the project's files as a tree (what git ignores is left out) and opens them in tabs with an editor that colours the syntax. You can create, rename, move (by dragging or with `Move to…`) and delete files and folders from the tree; a deleted one goes to the system's trash. While the agent is changing a file, the tab and the tree show a lock and the file cannot be renamed, moved or deleted. `.weaver/` shows but stays read-only here, and closing the view with unsaved changes asks to save or discard them.
- Hide, in the sidebar before Usage and Settings, sends the window of a project out of sight, off the screen and, on Windows and Linux, off the taskbar, without suspending anything: jobs, Chat, Ask more conversations and question rounds keep working, and the window shows the same view as it was when it comes back. It comes back from the Dock (macOS), by launching Weaver again, or from the tray icon on Windows and Linux (`Show Weaver`; a left click on Windows), which also has `Quit`. On macOS a window in full screen leaves full screen before going out of sight, so no black screen is left behind, and goes back to full screen when it comes back.
- Closing a window with jobs working now offers `Keep working in background` next to `Suspend and Close` and `Cancel`: the jobs keep working and the window goes to the background instead of closing.

### Changed

- The omp installer now reports its own failure instead of leaving omp silently missing.
- Settings is now at the bottom of the sidebar, apart from the other entries, which stay at the top.
- The skills and sub-agents Weaver installs are now named `weaver-…` (the Requirement skills `weaver-req-…`). In projects already initialised, the old `trama-*` skills and sub-agents of Weaver are no longer used or touched; they can be deleted by hand.
- The sidebar, the toolbars at the top of each view and the content under them have more room from the top of the window, so nothing sits pressed against it.

### Fixed

- Chat, job, sub-agent and Ask more conversations keep scrolling to the latest text while the agent writes, and stop only when you scroll up, instead of stopping by themselves during fast answers.

# Changelog

All notable changes to Weaver that a user of the app can see are documented in this file.

The format is based on [Keep a Changelog 1.1.0](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added

- A command palette, opened from anywhere in a project window with `Cmd+K` (`Ctrl+K` on Windows and Linux, also with the focus in the editor of Files, in Notes or in a text field) or with `Search or run…` in the sidebar, finds a view, an action (`Scan System`, `Run all`, `New Work Item`, `Open Folder…`, `Usage`), a project, a card of the Board, a section of the Requirements or a History entry and goes to it or runs it as its own control does, with the same confirmation and the same reasons for refusing; it never searches the project's files, assets or notes.
- The sidebar of a project window has the redesigned layout, 248 px wide: a project switcher at the top (logo, folder name and the git branch, read again when the window comes forward and whenever git changes branch, with a menu of the open and recent projects and `Open Folder…`), `Search or run…`, Board, Chat and Jobs with their numbers (Jobs shows a `<n> running` pill when none waits), the `PROJECT` group, the Usage summary and a user row with git's user name and the `Settings` button; below 1024 px it shows icons only.
- Every project window keeps in sight, in a box at the bottom of its sidebar, the usage limit of the model providers that is closest to being used up (how much is used, which limit and when it resets, in Peach when almost used up and in Rose when used up); choosing the box opens Usage. Weaver reads the limits by itself every five minutes while the agent is ready and a window is on the screen, as well as each time Usage is opened or `Retry` is chosen, one reading at a time for the whole app, never by calling a model, and every window shows the last result at once.
- A design job now shows Weaver's own ghost on the Board from the moment it is created, as setup and import jobs already do, titled with the title of the request it received (`Untitled request` when it has none; the ghost of a setup job takes that title too, instead of `Setup`), and the header of a setup or design job's conversation in Jobs has `View request`, which opens the request it received, read-only, in the request editor while that request is still sent to that job; the editor's back control then says `Jobs` and returns to that conversation.
- The Specs view shows the monochrome logo of each technology (the `simple-icons` collection, bundled in the app and never fetched from the network, loaded the first time the view opens) in the tile of its card, and an icon before the title of each field; an entry has a logo only when its whole name equals the name or slug of a logo, ignoring case, spaces and punctuation, and until the collection is loaded, or when it cannot be, the tile shows its field's icon. The cards now share the Board's ring and shadow and the same minimum height, with the overlay label and `Agent's choice` on one row at the bottom, and the fields start at the left edge of the view's title, the focus tint no longer moving them sideways.

### Changed

- The bar of a minimized question round (`The agent is waiting for your answers (question <n> of <N>)` and `Open questions`) now sits at the bottom of the work surface, below the selected view, instead of over it: the view gets shorter by the height of the bar, so nothing is covered, whether a message box, the end of a list, a Board column, an editor or a sheet. The bar is one line at a fixed height with the text cut with `…` and its full text in a tooltip, and toasts show above it while it is there.
- The Jobs view has one header band across the list and the conversation (the title `Jobs` and `Hide jobs list` over the list, the name, state and buttons of the selected job, sub-agent or Ask more conversation over the conversation), a list you can resize by dragging its edge or with the keyboard (240 to 420 px, 280 px at first, narrowing by itself in a narrow window) and hide with `Hide jobs list` / `Show jobs list`, both remembered for each project across restarts, and conversations that look like Chat's: a centred column of at most 860 px, the sender row with the time, a closed summary of the steps that reads `Working · <n> steps · <latest step>` while the agent works, and Chat's message box. Sub-agents show their own name on their messages.
- The agent of an Ask more conversation now speaks only in the words of the app: it names a card by its short number and title, tells why Weaver refused something in its own words, points to the Board, a card's details or the Settings when you have to do something, and mentions commands, files, skills or the agent itself only when you ask about them.
- Asking for a Proposal in Chat now sends a design request and starts a design job, with its own question rounds, instead of Chat writing the Proposal itself; Clear no longer discards anything but the conversation.
- Weaver now looks for a new version of itself every hour while it is open, instead of once a day.
- Chat no longer writes, edits or hands over Proposals: the Board has no card for a Draft of Chat and a turn of Chat never keeps a Proposal from being approved, rejected or discarded, and `weaver proposal new`, `weaver proposal edit` and `weaver proposal move <id> open` are refused with a message that points to `weaver request new` whenever they run in Chat.
- Outside Settings the interface no longer names omp: the agent is `Weaver` where it speaks or acts (`Message Weaver…`, `Weaver asks permission`, `Starting Weaver…`) and `the agent` in states and reasons (`The agent is not installed.`, `the agent did not answer within 30 s`), the welcome window, Get Started, the empty Board, Chat, Ask more and Usage offer `Install the agent` and `Log in` instead of the omp names, and the reasons that Settings and the rest of the app both show are written with `the agent`.
- Weaver has a new look: a warm Paper window with a white work surface inset from its edges, Geist and Geist Mono bundled in the app, Ink (black) as the only colour of actions instead of blue, rounded shapes with 1 px rings in place of borders, and a dark theme with the same roles. Colour now tells only a state, with the five threads of the new logo: Sky for queued, running and what the agent is doing, Sage for done and success, Peach for waiting for you, blocked, warnings and Drifts, Lavender for Proposals, questions and the agent's choices, Rose for failures and destructive actions; labels are plain text in an outline badge, with no colour of their own.
- Weaver has a new logo, a woven "W" of the five threads on a cream rounded square, shown in the welcome window and used as the app icon in the Dock, the taskbar, the tray, notifications and the preview when dragging assets.
- `Start from Scratch` now opens the oldest design request you were still writing, when there is one, and creates a new request only when there is none; the `+` of the Backlog still always creates a new one. Changes made to the requests outside Weaver (another editor, the command line, an agent) now show on the Board, which is sent again only when a request appears, vanishes or changes its title or whether it is empty.
- The design requests you are still writing are now cards of the Backlog on the Board, after its Proposals and Drifts and oldest first, counted in the Backlog and in the sidebar: each shows `Request`, its title (`Untitled request` when it has none), `Not sent · <time>`, `Send` and `Discard`, and a click on it opens the request. The request editor shows one request across the whole Board view, with a back control (`← Board`, or `← Jobs` from `View request`) in its header; the strip `N requests are open`, the list of requests, `New request` and the `Draft` and `Sent` badges are gone. Leaving an empty request (the back control, another view) or closing the window while it is open deletes it and its images, with no confirmation, and the Board shows a request that came back from a job that ended without a Proposal from the moment its ghost leaves. Every text calls it a request, never a work item, the one Ask more gives the agent included (`New Request` in the Backlog).

## [0.4.0] - 2026-11-06

### Added

- A new `Atomic commits` switch in Settings → Project, off by default and kept for each working copy, makes Weaver commit with git the code of every History entry the agent writes, one commit per entry with an English message written by the agent, so that the repository's history follows the work feature by feature.
- Weaver records how long each design, rework, refinement and Run lasted and how long the agent actually worked, with the files it read, its searches and its tokens for the main agent, its sub-agents and the advisors of a Run, in `.weaver/timings` on your computer (never committed), so that jobs can be compared before and after a change of the way the agent works; `weaver job show <id>` prints the measure of a finished job.
- Chat goes back to being a brainstorming conversation with omp about the project and Weaver: on your explicit order it writes change Proposals and resolutions of Drifts (each with its own Draft card on the Board), single Tasks and Drifts, and it can ask you questions with a round when you ask for one. Clear discards the Drafts that Chat wrote.

### Changed

- The Tasks written by a design, a scan or a compare carry notes for the agent that implements them, made of what the agent already read of the code and with the commit it read it on, and the agent that implements a Task starts from them and explores again only the files that changed since that commit.

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

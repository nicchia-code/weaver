# Quality

## General

### Performance

- No child process (omp, git, `weaver`, installer, terminal), file scan or Model parse runs on the UI thread or on the thread that serves Tauri IPC commands. An IPC command that starts long work returns at once; the result reaches the UI as an event.
- Typing, saving a draft, switching views, opening or closing a card and scrolling start no process.
- File-system events are handled in batches: one batch of changes produces one new picture of the project (Model, Requirements, cards) and one event to the UI, never one per file.
- Building the picture costs time linear in the number of files under `.weaver/`; no pass compares every file with every other file. Unchanged files are not parsed again (content hash or modification time per file).
- Every operation that takes more than 200 ms is logged at `info` with its name, project and duration in milliseconds, and without any file content.

### Reliability

- Every decision is all-or-nothing. A decision that changes several files writes each one atomically (temp file in the same folder, then rename) and, when a later write fails, restores the files already written to their previous content; the developer sees one error and the project is as before.
- A command (from the UI, the CLI or the agent) is fully validated before its first write. A refused command writes nothing, creates no folder and starts no process.
- Every asynchronous result is tagged with the input it was computed from (project path, omp path, settings revision, file hashes or a monotonically increasing generation number). A result whose tag no longer matches the current input is dropped, never shown and never acted on.
- No inactivity timer, quiet period, heartbeat timeout or watchdog ends a job, releases a Task, withdraws a ghost or closes a question round. Only ACP session events, the exit of the omp process, developer actions and the states Weaver finds at start do.
- Every watcher, debounce, queue and cache is keyed by project; work for one project never reads or writes another project's state.
- Startup is a list of isolated steps (logging, settings, local socket, recent projects, each project window, skills sync per project). A step that fails does not stop the others: the app, or the affected project window, shows `Weaver could not start: <reason>` or `<project> could not open: <reason>` with an **Open log** button that reveals the log file.
- A panic in a background task is caught, logged with its backtrace and reported as a failure of that operation; it never kills the process.

### Logs

- Logs are written to the OS log folder Tauri resolves for the app (`app_log_dir`): macOS `~/Library/Logs/<bundle id>`, Windows `%LOCALAPPDATA%\<bundle id>\logs`, Linux `$XDG_DATA_HOME/<bundle id>/logs`.
- One file per day of use, rotated at 10 MB; the 7 most recent files are kept and older ones deleted at start.
- Default level `info`; `debug` only when the `WEAVER_LOG=debug` environment variable is set.
- Each line has an ISO 8601 UTC timestamp, the level, the module and, when there is one, the project folder name and the job id.
- Logs never contain prompts, agent messages, answers of the developer, request text, file contents, environment variable values or secrets. They may contain file paths, card ids, command names, exit codes and durations.
- The `weaver` CLI writes no log file; it reports on stderr only.

### Accessibility

- Every action is reachable and usable with the keyboard alone: Tab order follows the visual order, Enter and Space activate, Esc closes dialogs, menus and popovers, arrow keys move inside lists, columns and tabs.
- Dialogs trap focus while open and return it to the element that opened them.
- Focus is always visible: a ring of at least 2 px with at least 3:1 contrast against its background, in light and dark themes.
- Contrast is at least 4.5:1 for text and at least 3:1 for icons, borders of controls and focus indicators (WCAG 2.2 AA), in both themes.
- A state, status or kind is never shown by colour alone: it also has an icon or text, plus a tooltip and an accessible name.
- Every icon-only button has an accessible name; every image has alt text.
- With `prefers-reduced-motion: reduce`, transitions and spinners that move content are replaced by instant changes; progress stays visible without motion.
- Text can be zoomed to 200% without loss of content or function.

### Tests

- Core logic lives in the Rust core crate as pure functions (no I/O, no clock, no randomness passed implicitly) with unit tests, at least for: blocking of Tasks and Proposals, staleness of Proposals and prepared questions, which Run or resolve may start, which cards a job holds and releases when it ends, short-number assignment and reuse, retention of finished Tasks (History citation, 20 more Tasks finished), notification decisions (window focus), Model and Requirement validation, Requirement-skill budget and mode.
- File operations are tested against temporary folders; tests never touch the developer's home, settings or keychain.
- CLI contract tests run the built `weaver-cli` binary and check, for every command in the registry: `--help` and `--help --json` output, input validation with exit code 2 and field paths, exit codes 0 and 1, Markdown and `--json` output shape, `--input -` with and without a UTF-8 BOM.
- Job tests drive a scripted fake ACP agent binary; no test starts a real omp or uses the network.
- UI components are tested with Vitest and Testing Library, including their keyboard paths (Tab order, Enter, Esc, arrows) and accessible names.
- The end-to-end smoke test drives the packaged app through tauri-driver: the app starts, the welcome window shows, a fixture project opens in its window, the Board and the Requirements render, a card opens, the window closes and the app quits without error. With the project window at an odd width and height, the opened card's details Dialog has its left and top on whole CSS pixels.
  - The project window is found by its WebDriver handle, the one that is not the welcome window's, never by the WebDriver title (that is the page's `document.title`, `Weaver` in every window). Its native title, read through Tauri in that window, must equal the fixture folder's name.
  - The closing stage fails when no project window was found; it checks that closing the last project window brings back the welcome window and that closing the welcome window quits the app without error.
- The suite passes the same with or without any `WEAVER_*` variable in the environment; tests that need one set it themselves and never read the inherited value.
- Windows paths and line endings are covered: backslash paths, drive letters, paths with spaces and non-ASCII characters, files with CRLF and a UTF-8 BOM on read, LF on write. Tests are portable; a test skipped on one OS states why in its skip reason.
- Tests are deterministic: the clock, random ids and file-watcher timing are injected, never slept on.

### Texts

- Texts Weaver writes for the agent (instructions, skills, sub-agent definitions, CLI help and errors) and for the developer (UI, notifications, errors) use no first person (`I`, `we`, `my`, `our`). They name `Weaver` and `the agent`.
- The UI is always in English; only texts the agent writes follow the language setting.

## solo-mode

- **Reliability:** Weaver never changes a line of the exclusions file outside its own block of this working copy, never touches git's index, and never leaves a block behind a failed initialisation.
- **Performance:** Solo mode starts git only when the project opens, when Solo mode changes, when the Settings view is shown and, for the File view, as part of its one `git check-ignore` per listed folder; never on the UI or IPC thread, never more than 10 s per call.
- **Tests,** against temporary git repositories with real git:
  - the block for both modes at the repository's root and in a subfolder, a prefix with special characters, a linked worktree writing into the main repository's file;
  - other lines kept byte for byte, with and without a final newline; writing the same block twice leaving the file untouched; two working copies' blocks independent; duplicate and broken blocks; removal leaving the rest;
  - git missing (an empty PATH), a folder outside a repository and a git call over the limit (injected limit);
  - initialisation with each mode: the block written before any other file, refused outside git with nothing created, a later failure removing the block;
  - the opening repair with an empty Model and with a Model, silent and never changing the opening check's outcome; `.weaver` vanishing removing the block;
  - the tracked files listed for each mode, team files in `.agents/skills/` left out in `files` mode;
  - the File view listing what a Solo mode block ignores and still leaving out what other rules ignore;
  - the CLI `solo` field of `project new`, `project edit` and `project show`;
  - the control, notes, warnings and list in Settings and Get Started in Vitest, with their keyboard paths and accessible names.

## solo-merge

- **Reliability:** Weaver itself never commits a merge with conflicts, never stages a file (only the agent of the merge conflicts job marks the files it resolved), never runs `git fetch`, never overwrites a file Solo mode keeps out of git, and never leaves `merge/merge.json` behind an ended merge; a merge stopped by the time limit leaves the branch as it was. A git call that fails while checking a merge in progress leaves it in progress.
- **Performance:** Merge starts git only at the moments of `requirements/rules#solo-merge` (**Check of a merge in progress**) and `requirements/tech#solo-merge` (**State in the app**), never on a timer, never on the UI or IPC thread; one computation of its state runs at a time per project.
- **Tests,** against temporary git repositories with real git, a local branch and a remote-tracking one:
  - every availability reason in its order, Weaver's files excepted from uncommitted changes in both Solo mode modes, and the reminder of tracked Weaver files;
  - nothing to merge; a fast-forward and a merge commit concluded at once; conflicts leading to Merging with their count; git's refusal for local changes; the refusal for files Solo mode keeps out of git; the time limit (injected) aborting the merge;
  - the check of a merge in progress: still in progress, committed outside Weaver (also with later commits on top), aborted outside Weaver, another branch, a reset, an unreadable `merge.json`, a crash after `merge.json` was written;
  - abort, and its refusal while the merge conflicts job has not ended; Solo mode refused while merging;
  - the merged files of a fast-forward, a merge commit and a merge with resolved conflicts, in a project at the repository's root and in a subfolder;
  - the scan after a merge: started without confirmation, `merged` in its record and in `job show`, the count reset; admission refusing Runs, Run all, resolves, scans and imports during a merge;
  - `drift new` with `merge` refused outside a scan after a merge, and a prepared question of a merge Drift refused without `proposal` recommended;
  - the button, Dialog, banner, notices, badge and details in Vitest, with their keyboard paths and accessible names.

---
description: "Use when building anything that must stay fast, reliable, logged, accessible or tested."
---
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
- The name `omp` appears only in the Settings view and in what omp or the agent write themselves (their messages, tool titles, the error lines under `Show details`). Everywhere else the interface calls the agent `Weaver` where it speaks or acts (`Weaver · ready`, `Message Weaver…`, `Weaver asks permission`) and `the agent` in states and reasons (`The agent is not installed.`, `the agent did not answer within 30 s`); controls that act on omp outside Settings keep their action with a generic name (`Install the agent`). A text given elsewhere in the Requirements that names omp outside Settings is shown with this rule applied. Reasons computed once by the backend and shown both in Settings and elsewhere are written with `the agent`.

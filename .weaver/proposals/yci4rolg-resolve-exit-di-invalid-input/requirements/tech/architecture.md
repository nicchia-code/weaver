---
description: "Use when building the app structure, backend state, the project's file watching, the requests behind the Board's drafts, IPC with the CLI or question rounds."
---
# Architecture

## General
- **Cargo workspace** at the repository root:
  - `crates/core`: library `weaver-core`, with parsing, validation, rules, mutations, skill generation, embedded installed files and the CLI↔app protocol types. It has no Tauri dependency, no async runtime and does sync `std` I/O only.
  - `crates/cli`: crate `weaver-cli`, building the binary `weaver-cli`, with arguments, output and the socket client. It never builds a binary named `weaver`, the name of the app's executable.
  - `src-tauri`: app crate `weaver`, with commands, events, watchers, the ACP client, the socket server, processes and notifications. It runs on Tokio. Disk work runs in `spawn_blocking`, never on the main or IPC thread.
  - `ui/`: Vite, TypeScript `strict`, pnpm.
- **Folders:** the Tauri identifier is `dev.nicchia.weaver`. `weaver-core` derives the app config, local data and runtime folders from it, so the CLI finds them with the app closed.
- **Bundled CLI:** `weaver-cli` is a Tauri `externalBin` sidecar, bundled under that name next to the app's executable `weaver`. At each start Weaver copies it into `<app local data>/bin/` as `weaver` (`weaver.exe` on Windows) when its content differs, and removes from that folder a `trama` (`trama.exe`) left there by earlier versions. That folder holds only `weaver` and goes first on the PATH of every omp Weaver starts. The copy keeps a stable path even under AppImage.
- **Tauri plugins:** `single-instance`, `window-state`, `notification`, `dialog`, `opener`, `updater`.
  - No `deep-link`, `shell`, `fs` or `http` plugin.
  - Plugins are called from Rust.
  - The webview capability grants only Weaver's commands and core event/window permissions.
  - The CSP allows no remote origin.
- **Backend ↔ UI:**
  - Actions are Tauri commands that return a result or a typed error. Updates are backend events.
  - The UI holds no domain state.
  - TypeScript types for commands, events and payloads are generated from the Rust types with `ts-rs`. CI fails on a stale file.
- **Project picture:**
  - Each open project has one in-memory picture built from its files.
  - It is rebuilt 150 ms after the last watcher event of a batch, and at most 1 s after the first event under continuous changes.
  - Parsing is incremental: each file is kept parsed with its content hash, and only changed or removed paths are parsed again. Derived data (blocking, design membership, short numbers, Board) is recomputed whole.
  - Files of the code never enter the picture and never cause a rebuild; the File view keeps its own state (`requirements/tech#files`).
  - Each rebuild increments a revision. The event `project-changed` carries the project id, the revision and the changed slices.
  - The UI fetches a full snapshot when a window loads, or when it sees a gap in revisions.
- **Mutations:**
  - Every mutation of a project, from the UI or forwarded by the CLI, runs through one serial queue for that project.
  - Each mutation holds an OS advisory lock (`flock` / `LockFileEx`) on `<app local data>/locks/<project hash>.lock`, the same lock a CLI writing in-process takes.
  - `<project hash>` is the first 16 hex characters of the SHA-256 of the canonical project root.
  - Under the lock the mutation re-reads its files from disk, validates the result after the change, then writes. A refusal writes nothing.
  - If the lock is not obtained within 10 s, the mutation fails with "project busy".
- **CLI ↔ app channel:** via the `interprocess` crate. No TCP or HTTP port.
  - Endpoint:
    - Unix: a domain socket `weaver-<8 random>.sock` in a `0700` folder: `$XDG_RUNTIME_DIR/weaver/` when set, else `<app local data>/run/`, or `/tmp/weaver-<uid>/` when the path would exceed 100 bytes.
    - Windows: a named pipe `\\.\pipe\weaver-<16 random>`.
  - Runtime file: at each launch the app atomically writes `<app local data>/runtime.json` (mode `0600`) with `{endpoint, secret, pid, version}`.
  - Secret: 32 bytes from the OS CSPRNG, hex-encoded, new per launch. It is never in env vars, arguments, logs or `.weaver/`. It is compared in constant time; a wrong or missing secret closes the connection with no response.
  - Messages: newline-delimited JSON, at most 1 MiB, one request per connection, each with a protocol version. On a mismatch the CLI acts as if no app answered.
- **Processes:** every child is spawned without a shell, with a program path and an argument array. No user text is ever interpolated into a command line. On Windows children get `CREATE_NO_WINDOW`.

## project
- **Watcher:** one `notify` recommended watcher per open project. Every debounce, rebuild and skill regeneration is keyed by project hash.
- **Watched paths:**
  - the project root, non-recursively, only to see `.weaver` appear or vanish; a `.trama` folder appearing, changing or vanishing is ignored;
  - `.weaver/`, recursively, while it exists;
  - git's HEAD file of the working copy, alone, only to read the branch again (`requirements/tech#window`); its events never touch the picture;
  - no code, no other file of git, no skills folder, no `.omp/`; the File view watches its folders and files with its own watcher (`requirements/tech#file-view`).
- **Ignored events:** temp files of atomic writes, and the local folders of `.weaver/` except `requests/`, where a deletion is `workspace.REQUEST_DELETED` and any change refreshes only the Board's drafts (`design-request`), and `notes/`, whose events only reach the Notes view (`requirements/tech#notes-view`). Events under `assets/` only refresh the asset list (`requirements/tech#assets`). Neither `assets/` nor `notes/` ever touches the picture.
- **`.weaver` vanishes:** the recursive watch is dropped, then `PROJECT_FOLDER_REMOVED`.
- **`.weaver` appears:** the watch is added and the picture fully rebuilt, then `PROJECT_FOLDER_APPEARED`.
- A rescan or overflow event triggers a full rebuild.
- On Linux, if inotify returns `ENOSPC`, the project falls back to the poll watcher every 2 s, with a one-time warning naming `fs.inotify.max_user_watches`.
- **Self-writes:** the app records the hash of each file it writes. An event whose content matches the recorded hash updates the picture but is not `workspace.FILES_CHANGED`. Writes by the CLI and the agent are `FILES_CHANGED`.
- **Requirement skills:** when a batch touches `.weaver/requirements/`, they are regenerated in the same rebuild through the project queue, by one core function.
- **Embedded files:** the installed files are embedded in `weaver-core` with `include_dir`, so the CLI and the app write identical bytes:
  - skills `weaver-setup`, `weaver-design` (+ format reference and command reference), `weaver-import`, `weaver-scan`, `weaver-compare`, `weaver-implement`, `weaver-resolve-drifts`;
  - `.omp/agents/weaver-analysis.md` and `.omp/agents/weaver-implementer.md`;
  - `.omp/WATCHDOG.yml`.
- **Comparing installed files:**
  - Pure content: read the file, normalise CRLF to LF, compare byte for byte with the embedded or generated content.
  - No version stamp, no timestamp, no network.
  - Only differing or missing files are written, atomically.
  - At most one check per project at a time; requests made meanwhile coalesce into one more run.
  - The check never delays the window.

## design-request
- **Drafts on the Board:** the Board snapshot's Backlog carries, after the open Proposals and Drifts, one card per request in Writing, oldest first by its id: its id, title (`requirements/data#design-request`), creation time, whether it is empty and whether it came back from a job that ended without a Proposal. Requests stay out of the core picture of the Model and the cards: the app adds the drafts when it builds the Board snapshot, from the request store, so that the UI never merges two lists.
- **Ghost titles:** Weaver's own ghost of a setup or design job (`requirements/ui#work`) takes the title of the request the job received, read when the job is created and kept with the job in memory; it is never written in the job's record or the request file.
- **Refresh:** the `board` slice is touched, with `project-changed`, when a request is created, sent (in the same picture as its job's first ghost, never one where both the draft and the ghost show), discarded, deleted, consumed or back in Writing (in the same picture in which its job's ghosts leave), and when a save changes its title or whether it is empty; a save that changes neither touches nothing.
- **Return on opening:** a request whose job ended while the app was not running goes back to Writing in the project's mutation queue before the first Board snapshot of the window, so its draft card shows from the start.
- **Leaving empty:** the UI asks the backend to discard the open request when it leaves the editor or the window closes with an empty text; the backend checks emptiness on the file under the project lock and discards nothing that is not empty or not in Writing.
- **Job → request:** the job's row tells the UI, for a setup or design job, which request it received and whether that request is still Sent to it, so that `View request` shows only then (`requirements/ui#job`).

## question-round
- **Transport:**
  - `weaver question new` validates the request with `weaver-core`, then connects to `WEAVER_ENDPOINT` and sends `{secret, job, general, request}`, with the identities it has.
  - For a job it blocks reading the reply; for Chat it reads one reply, `published` or a refusal, and exits. Neither side has a timeout.
- **Routing:**
  - Only by the job identity in `WEAVER_JOB` or, without it, by Chat's identity in `WEAVER_GENERAL`, never by cwd, focus or path. Rounds from sub-agents and git worktrees reach the job's or Chat's project.
  - The app accepts a round only for a job it launched that is Running or Waiting for the developer, or a card of questions only for the live Chat of an open project while a turn is in progress.
  - A card of Chat never enters the round queue, registers no waiter and no close handler, and changes no state of Chat: the app stores it, tells the UI, and answers `published` at once, or `card-already-published` when the turn already has one.
  - Each project has one round queue, created when it opens and never recreated while it is open.
- **Outcomes:** exactly one JSON document on stdout:
  - `answered` (job) and `published` (Chat): exit 0.
  - `unavailable` with a reason: exit 0, returned at once when:
    - both `WEAVER_JOB` and `WEAVER_GENERAL`, or `WEAVER_ENDPOINT`, are missing;
    - the runtime file is missing or names another endpoint;
    - the connection fails;
    - the protocol version differs;
    - the job is unknown or ended, or the Chat identity is not the live one or has no turn in progress.
  - `interrupted`: exit 1, with a code and message and no answers. Returned when the round is interrupted, or when the connection drops after submission, detected by end-of-stream at once.
  - `invalid-input`: exit 2, naming the question and field.
  - `card-already-published`: exit 1, Chat only.

  These outcomes are never conflated.
- **Abandoned:** the CLI closes the connection on SIGINT, SIGTERM or a closed stdout (Windows: Ctrl+C or pipe closure). End-of-stream on a queued or presented round is `agent.STOP_WAITING`.
- Round state, drafts included, lives only in backend memory. Free-text drafts reach it debounced at 300 ms. Nothing of a round is written to disk or logged.
- **Cards of Chat:** the backend keeps, per project, the cards of Chat's conversation in publication order: identity, the turn that published it, questions, confirmed answers and skips, state (active, answered, answered with a message) and, while the window is open, drafts and position. It reuses the round's model (`visit`, preselection on the first visit, confirm, skip), without minimize or skip all.
  - Confirmed answers, skips and state are written to the record of `requirements/data#general-chat` at each change; drafts and position never are, nor is anything of a card logged. After a reload the active card restarts from its first question without an answer.
  - Sending, from the card or with a message, closes the card under the same lock that takes the message, then follows the path of a developer message (`Event::SendMessage`, held while loading, steered during a turn, starting omp when it is not running) with the answers block of `requirements/rules#question-round`.
  - **Placement:** live, a card belongs to the turn that published it and shows after that turn's last entry. In a replay, each developer message that carries an answers block is matched by the card identity in the block: the card shows right before it and the message is shown without the block, or not at all when nothing else is in it. The active card shows at the end, after the replayed conversation and the lines of `byChat`. A card that no replayed message names and that is not active shows at the end too, closed.
  - `New chat` and a kept conversation that could not be reloaded delete every card of the conversation, in memory and in the record.
- **Ask more:** the request names the round, or Chat's card, by its identity and the question by its position. The backend reads, from the presented round or from the active card, its questions and options and either its asking job (name and identity), Chat for a card of Chat, or, for a prepared question, the Drift's permanent identity, which the round keeps besides the short number it shows; it never reads the drafts. When another round, or none, is presented, or the card is no longer active, nothing opens.

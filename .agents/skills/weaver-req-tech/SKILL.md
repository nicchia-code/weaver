---
name: weaver-req-tech
description: "Use when choosing or using languages, frameworks, libraries, platforms, processes, IPC, the omp/ACP integration or project structure."
---

Weaver is a Tauri 2 desktop app for macOS, Windows and Linux with a Rust core shared by the app and the `weaver` command line, and it drives omp over ACP.

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

## ACP client

Use when building the ACP client of jobs and General: command line, process lifetime, environment, sessions and resume.

Sections: `job`, `general-chat`

File: `acp.md`

## Architecture

Use when building the app structure, backend state, the project's file watching, IPC with the CLI or question rounds.

Sections: `General`, `project`, `question-round`

File: `architecture.md`

## Ask more

Use when building Ask more's omp processes, sessions, environment, the reading of the chosen thing and the prompt that carries Weaver's instruction.

Sections: `ask-conversation`

File: `ask-more.md`

## Assets

Use when building the backend of Assets or `@` asset tags.

Sections: `assets`, `asset-view`

File: `assets.md`

## Files

Use when building the File view's backend: listing, watching, reading and saving code files, tree actions and the agent lock.

Sections: `files`, `file-view`, `file-tab`, `file-lock`

File: `files.md`

## Notes

Use when building the Notes view's backend or editor: the notes folder, reading and writing the sheet, images, watching and TipTap.

Sections: `notes-view`

File: `notes.md`

## omp

Use when finding, checking, installing, updating or logging in omp, downloading it from GitHub, or opening the system terminal for its installer and logins.

Sections: `omp`, `omp-setup`

File: `omp.md`

## Solo mode

Use when building Solo mode: git calls, the exclusions block at initialisation, change and opening, its state in the app and the command line, the File view's listing, and Merge: its git steps, commands, state and jobs.

Sections: `solo-mode`, `solo-merge`

File: `solo-mode.md`

## System Certificates

Use when building System Certificates: exporting the system's certificate authorities, the bundle, the variables given to omp, the Zscaler turn-on.

Sections: `system-certificates`

File: `system-certificates.md`

## omp usage

Use when reading, parsing or keeping omp's usage limits of the connected providers.

Sections: `usage`

File: `usage.md`

## Windows and notifications

Use when building windows, their closing and quitting, or system notifications.

Sections: `window`, `notification`

File: `windows.md`

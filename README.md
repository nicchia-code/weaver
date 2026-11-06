# Weaver

Weaver is a desktop app for macOS, Windows and Linux that works on top of Weaver projects (the `.weaver/` folder of a repository). It shows a project's Model, Requirements and Tasks on a Board, and it drives [omp](https://omp.sh) over the Agent Client Protocol to design, scan and implement them. The same Rust core powers the app and the `weaver` command line. A project window also has a File view, with a tree of the project's files and a tabbed editor with syntax colours, for small changes to the project's files next to your own editor, and a Notes view with one Markdown sheet per project for your own notes; Weaver never analyses your code (the agent does) and reads or writes a file of it only when you open or change it there.

## Install

Download the build for your system from the [Releases](https://github.com/nicchia-code/weaver/releases) page and open it. Weaver keeps itself up to date from the same releases.

> **Builds are not signed or notarised for now.** Code signing and notarisation on macOS and signing on Windows are deliberately left out and will come with a dedicated Proposal, so macOS and Windows warn that the app comes from an unidentified developer. The warning does not mean the file is damaged. Open an unsigned build like this:
>
> * **macOS** (`Weaver_<version>_universal.dmg`): drag Weaver to Applications, right-click it and choose **Open**, then **Open** again. If macOS still refuses, open System Settings → Privacy & Security and choose **Open Anyway** next to Weaver.
> * **Windows** (`Weaver_<version>_x64-setup.exe`): when SmartScreen shows "Windows protected your PC", choose **More info** and then **Run anyway**.
> * **Linux** (`Weaver_<version>_amd64.AppImage`, or `Weaver_<version>_amd64.deb`): make the AppImage executable with `chmod +x Weaver_<version>_amd64.AppImage` and run it; install the deb with `sudo apt install ./Weaver_<version>_amd64.deb`.

## Behind a corporate proxy

A corporate proxy that inspects HTTPS traffic (Zscaler, for example) re-signs the connections with its own certificate authority. omp does not trust that authority, so it fails to install or to reach its model provider.

**System Certificates** fixes this: every omp process Weaver starts trusts the authorities the system trusts. It is off by default and it is in Settings → omp, under the omp box of Get Started and in the welcome window.

* **Which certificates.** Those the system trusts (on macOS the user, admin and system trust settings; on Windows the user's and the machine's root stores; on Linux the system bundle), plus Mozilla's public roots. Weaver exports them to one PEM bundle, refreshes it at start when it is older than a day, every day while Weaver is open, and when you choose **Refresh**.
* **Which variables.** `NODE_EXTRA_CA_CERTS`, `SSL_CERT_FILE`, `REQUESTS_CA_BUNDLE`, `CURL_CA_BUNDLE`, `GIT_SSL_CAINFO`, `PIP_CERT`, `CARGO_HTTP_CAINFO`, `AWS_CA_BUNDLE` and `DENO_CERT` point to the bundle, and `PI_REQUEST_COMPRESSION=0` is set too. They overwrite the values Weaver inherits. They reach every omp process Weaver starts and its terminals for the installer and the login, so the commands the agent runs inherit them.
* **Turned on for Zscaler.** When Zscaler is installed, Weaver turns System Certificates on by itself, once. This happens on macOS and Windows only, and it is never repeated after you turn the setting off.
* **Explained errors.** Where Weaver shows why omp failed (a certificate error, an HTTP error, a failed installer, a failed download of an update), it explains in plain words when a corporate proxy is the likely cause, with one text for each cause the installer and the download report (blocked download, certificate not verified, incomplete or altered file, omp that does not start), and suggests turning on System Certificates while it is off.
* **Not covered.** Java's tools and every program that reads a truststore instead of a PEM bundle.

### When the proxy blocks omp's download

Some proxies block the site that omp's official installer and `omp update` download from (`omp.sh`, which serves its files from Cloudflare R2), whatever the certificates: they take it for a file-hosting site. System Certificates does not help there. omp's releases on GitHub (`can1357/oh-my-pi`) carry the same program and usually pass, so Weaver downloads it from there by itself, on macOS, Windows and Linux (musl too), x64 and arm64, with no setting:

* **When.** In the installer's terminal, after the official installer fails; and in the background, with no terminal, after `omp update` fails, only when the latest GitHub release is newer than omp. The background download is only for an omp in the folder of its installer (`PI_INSTALL_DIR`, or the default folder below), never for one installed with a package manager or bun, nor for one set in `omp path`. Both receive the System Certificates variables while it is on.
* **Where.** In `PI_INSTALL_DIR` when it is set, else in `%LOCALAPPDATA%\omp` on Windows or `~/.local/bin`, as the official installer does; an update replaces omp where it is. On Windows an installation adds that folder to your user `Path` when it is missing; on macOS and Linux it only prints how to add it to `PATH` and touches no file of your shell.
* **Which file.** The one of the latest release for your system and processor, read from GitHub's REST API. It is installed only when its size and SHA-256 are the ones GitHub publishes and, once in place, it answers `--version`; otherwise your earlier omp stays where it was, working. On Windows the omp in use is renamed aside (`omp.exe.<time>.old`) first, so running jobs continue, and the old files are deleted when they are free. No certificate or hash check is ever skipped.
* **When that fails too.** Weaver shows the exit code with its cause: `90` the download is blocked (an HTTP refusal, an HTML page of the proxy, no connection), `91` a certificate that could not be verified, `92` a file that is incomplete or altered and was not installed, `93` an omp from GitHub that does not start. Any other code is explained as a failed installer with a proxy as the likely cause.

## Solo mode

Solo mode lets you use Weaver in a git repository where the others do not. While it is on, Weaver's files stay only in this working copy and never go into a commit: Weaver adds a block of its own to git's local exclusions of the repository (`.git/info/exclude`, found with `git rev-parse --git-path info/exclude`), which git never commits, and the shared repository sees only the code.

* **It belongs to the working copy, not to you.** The choice is kept in this working copy only, in `.weaver/solo/solo.json`, never in the repository and never in Weaver's settings: another clone of the same repository, or a linked worktree, starts with it off. Choose it in Get Started, before you initialise the project, so that Weaver's files are never seen by git; or turn it on and off later in Settings → Project, or with `solo` in `weaver project new` and `weaver project edit` (`off`, `folders` or `files`). A change applies at once, with no confirmation.
* **Two modes.** `Whole folders` (the default) keeps out `.weaver/`, the agent's skills folder `.agents/skills/` and omp's folder `.omp/`. `Weaver's files only` keeps out `.weaver/` and just the files Weaver installs in the other two (`weaver-*` skills, `weaver-*` sub-agent definitions and `.omp/WATCHDOG.yml`): use it when your team already keeps its own skills or omp files in those folders.
* **Files git already tracks are not hidden.** An exclusion never hides a file git tracks, and Weaver never touches git's index and offers no way to stop tracking a file. While Solo mode is on, Settings lists the Weaver files that git still tracks, and warns that their changes still go into commits; `weaver project show` lists them too.
* **It needs git, and it has a cost.** Every change needs git (found on the `PATH`) and a repository around the project folder; if either is missing Weaver says why and changes nothing. Weaver writes its block again by itself at every opening of the project when it is missing. With Solo mode on, Weaver's files are versioned nowhere: losing the working copy loses them.

### Merge

Others change the code without updating the Model, so Solo mode also offers `Merge`: it brings another branch into the current one and finds, with a scan, what the others' work changed against the Model.

* **Where and when.** `Merge` sits in the Board header, to the left of `Scan System`, only while Solo mode is on. It is disabled, with the reason as its tooltip, while an agent job works, while a merge is in progress, when git is missing or fails, when git is in the middle of a merge, rebase or cherry-pick, when HEAD is not on a branch, and when tracked files have uncommitted changes (Weaver's own files do not count). You choose the branch among the local ones and the remote ones git already knows, and confirm.
* **What Weaver does with git.** It runs `git merge` itself, with git's own message when there is no conflict, and never runs `git fetch`. It never commits a merge that has conflicts and never stages a file: you resolve them by hand or with `Resolve with the agent` in the banner of the merge in progress, then commit the merge in git. `Abort merge` runs `git merge --abort` after a confirmation. The merge in progress is remembered only in `.weaver/merge/merge.json`, which is never versioned.
* **The scan after the merge.** When the merge is concluded, by git at once or by your commit, Weaver starts by itself a scan of the whole System, telling the agent which files the merge brought. Its Drifts that come from the merge recommend adapting the Model. While a merge is in progress Runs, Run all, resolves, scans and imports are refused, and Solo mode cannot be changed.

## Atomic commits

Atomic commits makes Weaver commit the code of every History entry the agent writes, so that the repository's history follows your work feature by feature. After each History entry the agent writes, or each update that adds Tasks to an entry, Weaver commits the code of that entry: one commit per entry, with an English message written by the agent. An update that adds no Task makes no commit, and a commit already made is never changed.

* **Off by default, and it belongs to the working copy.** It is not yours: another clone of the same repository starts with it off. Turn it on only in Settings → Project, with the `Atomic commits` switch; there is no command and no command-line field that changes it. It needs git (found on the `PATH`) and a repository around the project folder, and Weaver never pushes or fetches.
* **What goes into a commit.** The files the agent names for the entry and whatever you already staged. Outside Solo mode the commit also holds the History entry's file and the removal of the files of the Tasks it records, and its message ends with `Co-authored-by: Weaver <weaver@users.noreply.github.com>`. With Solo mode on no file of Weaver goes into the commit, and there is no `Co-authored-by` line. The author is always you, as git is configured.
* **Git runs as you configured it.** Hooks and signing run as the repository sets them, and Weaver never amends a commit. A commit is skipped, with a notice on the Board, when the final checks of the Run failed or when git cannot commit: outside a repository, git missing, a merge, rebase or cherry-pick in progress, a detached HEAD, or a hook or signature that refuses. The History entry stays written all the same.

## Branches

* `dev`: where the work lands.
* `main`: what is released. It changes only from `dev`, through a pull request whose `ci` checks passed.

## Repository layout

| Path | What it is |
|---|---|
| `crates/core` | Library `weaver-core`: app folders, CLI↔app protocol, project lock, process spawning. No Tauri, no async runtime, synchronous `std` I/O only. |
| `crates/cli` | Binary `weaver-cli` (clap): one command registry (`registry`, `commands/`) drives parsing, validation, dispatch, help, JSON Schemas and the command reference of the skills. Library `weaver_cli` holds the code so tests and the app can reach it. |
| `src-tauri` | App crate `weaver` on Tauri 2 and Tokio: commands, events, socket server, mutation queues, sidecar install. |
| `ui` | Vite + React 19 + TypeScript (`strict`), pnpm, Tailwind CSS 4, shadcn/ui (zinc, blue primary). Holds no domain state. |
| `.weaver` | The Model, Requirements, Specs and Tasks of Weaver itself. |

### `weaver-core` modules

| Module | Responsibility |
|---|---|
| `paths` | Identifier `dev.nicchia.weaver`; config, local data and runtime folders; project hash (first 16 hex characters of the SHA-256 of the canonical root) |
| `atomic` | Atomic file writes (temp file + rename) with an optional Unix mode |
| `endpoint` | CSPRNG helpers; socket path (`weaver-<8>.sock`) or named pipe (`\\.\pipe\weaver-<16>`) naming |
| `runtime` | `<app local data>/runtime.json` with `{endpoint, secret, pid, version}`, mode `0600` |
| `protocol` | Newline-delimited JSON messages, at most 1 MiB, protocol version, constant-time secret check |
| `ipc` | Local-socket transport (`interprocess`), one request per connection, client and server sides |
| `lock` | OS advisory lock `<app local data>/locks/<project hash>.lock`; not obtained in 10 s is "project busy" |
| `process` | The single function every child process starts with: no shell, `CREATE_NO_WINDOW` on Windows |
| `agent_files` | The texts installed for the agent, embedded from `crates/core/assets/` with `include_dir`; edit the sources there, never the installed copies; `COMMANDS.md` is generated from the CLI command registry |
| `job_instruction` | The pure function that composes the one-line instruction of every job |
| `command` | The request kind `command`: a `weaver` command the app runs in the project's queue, its reply and the error codes |
| `project::{browse, init, resolve}` | Requirement sections that apply to an element and Model search; what `weaver project new` writes; naming a card by identity, prefix or short number |
| `project::requests` | The Machine `design-request` (pure) and the files of design requests: `.weaver/requests/<id>.md`, `<id>.images/`, `.state.json`, title and emptiness rules, images |
| `omp::*` | omp: the Machine `omp-setup` (pure, events and clock injected), the lookup of the executable, the login PATH, `omp --version` / `models --json` / `update` probes with their limits, failure reasons, the fixed provider list, the quoting of the system terminal commands, the download of omp from its GitHub releases (installer and background update) and `MIN_OMP_VERSION` |
| `file_tab::{path, read, save, limit}` | The files open in tabs of the File view: paths relative to the project root checked before any file is touched (no absolute path, no `..`, nothing in `.git`, nothing whose canonical form leaves the root); what a tab keeps of a file (SHA-256 of the raw bytes, byte order mark, line ending, binary and size rules); saving only when the file on disk is still the one the tab read, through the atomic write that keeps permissions and writes the target of a symbolic link; the limit of 10 tabs and what an event of a tab's file means |
| `file_view::{listing, names, ops, rename, machine}` | The tree of the File view: one folder at a time (names and kinds, `.git` and what git ignores left out), valid names, create, rename, move and delete with every refusal checked when they run, and the Machine `file-view` (Closed, Open, Releasing) |
| `file_lock` | The Machine `file-lock`: which files open in a tab an agent is changing, from its running tools that change files |
| `notes::{sheet, images}` | The developer's notes in `.weaver/notes/`: the sheet `notes.md` (LF text, hash of the bytes, atomic write) and its images (naming, accepting, copying in, reading one for display, the trash) |

### `weaver` (app) modules

| Module | Responsibility |
|---|---|
| `commands` | Tauri commands the UI calls; each returns a result or a typed `AppError` |
| `events` | Backend → UI events such as `project-changed` (project id, revision, changed slices) |
| `bindings` | `ts-rs` generation of `ui/src/bindings/` and the check that fails when it is stale |
| `mutations` | One serial queue per project; each mutation holds the OS project lock |
| `server` | CLI↔app socket server; publishes `runtime.json` |
| `cli_handler` | The app side of `weaver` commands that write: runs them in the project's queue with the same function the CLI runs when no app answers |
| `sidecar` | Copies the bundled `weaver-cli` into `<app local data>/bin/` when its content differs |
| `blocking` | `spawn_blocking` helper: disk work never runs on the main or IPC thread |
| `state` | App-wide state managed by Tauri |
| `updates` | Updates of Weaver: the check at start and every 24 hours, **Install and restart** through the quit path, **Later**, the last check in `settings.json` |
| `omp` | The app side of `omp-setup`: runs the probes, the installer, the login and the log out off the UI thread, schedules the re-checks, emits `omp-changed`, and tells the jobs when omp is ready (`OmpSetup::wait_ready`) |
| `windows` | The Machine `window`: one window per project (`p-<hash>`) and the `welcome` window, `state.json` (recent projects, last session), restore at start, close and quit confirmations, the file watcher of each project, the menu, `Open in editor`; the jobs plug in through `WindowJobs` |
| `requests` | The design request: the editor's commands (save, Send, Discard, images), the setup or design job `Send` starts, consumption at the Proposal hand-over, return to writing when the job ends, and the flush before a window closes |
| `file_view` | The tree of the File view of each window, in memory only: its listings and its watches (one non-recursive watch per expanded folder, separate from the project's), the actions of the tree, the release 10 s after the view stops being shown, and `files-tree-changed` |
| `file_tabs` | The tabs of the File view: reading a file when it opens in a tab, its watch (only the files open in tabs), saving with the check against the file on disk, and `file-tab-changed` |
| `file_lock` | The agent lock of the File view: the live `session/update` notifications of every job, of General and of every Ask more conversation tell which files an agent is changing; `file-lock-changed`. Replayed updates, sub-agents and commands never count |
| `notes` | The Notes view of each window: the sheet's writes (one at a time, the latest text coalesced), the hash of Weaver's own last write, image commands that take names and never paths, the flush before a window closes, and `notes-changed` |

The bundled `weaver-cli` is a Tauri `externalBin` sidecar. `src-tauri/build.rs` builds it with the app's profile and target into `target/sidecar` and stores it as `src-tauri/binaries/weaver-cli-<target triple>`.

## Development

Requirements: the toolchains pinned in the repository (Rust in `rust-toolchain.toml`, with `rustfmt` and `clippy`; Node in `.nvmrc`; pnpm in `packageManager` of the root `package.json`). On Linux also `libwebkit2gtk-4.1-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev` and `patchelf`.

```sh
pnpm install                  # once (workspace root: ui + Tauri CLI)
pnpm --dir ui build            # type-check (tsc --noEmit) and build the UI
cargo build --workspace        # core, CLI and app
cargo test --workspace         # core, CLI and app tests
pnpm tauri dev                # run the app with the Vite dev server
```

### Generated TypeScript types

The types of commands, events and payloads in `ui/src/bindings/` are generated from the Rust types with `ts-rs` and committed. `cargo test -p weaver` fails when they are stale. After changing a Rust type that crosses the UI boundary, list it in `bindings::export_all` and run:

```sh
pnpm --dir ui gen:types        # cargo test -p weaver --lib regenerate_bindings -- --ignored
```

### The command reference of the skills

`crates/core/assets/skills/weaver-design/COMMANDS.md` is generated from the command registry of `crates/cli` (`weaver_cli::registry::command_reference()`); never edit it by hand. `cargo test -p weaver-cli` fails when it differs from the registry. After changing a command, regenerate it and build again (the file is embedded in `weaver-core`):

```sh
cargo run -p weaver-cli --example write_command_reference
```

### Where Weaver keeps files

Only in the project's `.weaver/` folder and in the system folders of the identifier `dev.nicchia.weaver` (`<config>/dev.nicchia.weaver`, `<local data>/dev.nicchia.weaver`). No database; secrets would go to the system keychain (none are kept yet).

In the local data folder, System Certificates keeps the exported bundle `system-certificates.pem` and the markers of the omp installer in `omp-install/`. The download of omp from GitHub writes omp in the folder of omp's own installer, outside Weaver's folders, with a temporary `.omp.download` file (`.omp.download.exe` on Windows) and, on Windows, the `omp.exe.*.old` files of a replaced omp.

The File view keeps nothing on disk and writes nothing outside the files you change in its tabs, or create, rename, move and delete from its tree (a deletion goes to the system's trash).

`.weaver/assets/` holds the files you keep for the agent (images, documents, examples), flat and versioned by git like the Model; it is made when the first asset is copied in, never at initialisation. The Assets view lists them as icons, copies in the files you drag onto it (the originals stay where they are), renames them, sends them to the system's trash and drags them out to other apps as copies; it reads only their names and sizes, and the File view shows the folder read-only like the rest of `.weaver/`.

`.weaver/notes/` holds `notes.md`, the project's one Markdown sheet, and the images pasted or dropped into it. The folder is local only, with a `.gitignore` of `*`, so another clone starts without notes; it is made when the first note or image is written, never at initialisation, and only the Notes view writes in it. Git ignores its files, so the File view shows the folder empty. The project's watcher gives what happens there to the Notes view alone, as `notes-changed` 150 ms after the last event (a write of Weaver's own sheet excluded): it never reaches the project's picture or `project-changed`. The agent never receives the notes.

`.weaver/commits/` holds `commits.json`, the state of Atomic commits of this working copy. Like the other local-only folders it has a `.gitignore` of `*`, so another clone starts with Atomic commits off; it is made when Atomic commits is turned on, and turning it off deletes the whole folder.

## Releases and updates

The workflow `.github/workflows/release.yml` (`release`) runs when `ci` completed with success on a push to `main`, and works on that run's commit; it never runs for `dev` or a pull request.

1. `prepare` reads `version` from `src-tauri/tauri.conf.json` and looks for the tag `v<version>` on `origin`. When the tag exists the workflow ends with a `::notice::` and builds nothing: the push only ran the tests. Otherwise it creates the draft release `Weaver v<version>` for that commit (a draft does not create the tag), with notes that name the file to download on each platform and how to open an unsigned build.
2. `build` runs `tauri-apps/tauri-action` on `macos-latest` (`universal-apple-darwin`: `.dmg`, `.app.tar.gz` + `.sig`), `windows-latest` (`x86_64-pc-windows-msvc`: NSIS `-setup.exe` + `.sig`, no MSI) and `ubuntu-22.04` (`x86_64-unknown-linux-gnu`: `.AppImage` + `.sig`, `.deb`) and uploads to the draft. `weaver-cli` is built for the same target as the app and bundled as its sidecar; on macOS the arm64 and x64 builds are joined with `lipo`.
3. `manifest` checks that the draft holds every file the notes name and attaches `latest.json` (platforms `darwin-aarch64`, `darwin-x86_64`, `windows-x86_64`, `linux-x86_64`), made by `scripts/release.mjs` from the `.sig` files.
4. `publish` publishes the draft, which creates the tag `v<version>` on that commit, only when every build job succeeded. When one failed or was cancelled, `discard` deletes the draft and no tag is created. The workflow never moves, deletes or recreates a tag and never edits a published release.

To release, raise the version in the five files `scripts/check-versions.mjs` checks and merge `dev` into `main`.

### Updater signing key

The updater files are signed with the Tauri updater key, which is needed even without code signing. Generate it once with `pnpm tauri signer generate -w <file>`, then:

* put the public key in `plugins.updater.pubkey` of `src-tauri/tauri.conf.json`;
* store the private key and its password as the GitHub Actions secrets `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`;
* keep the private key somewhere outside GitHub too: losing it stops the updates of every installed copy.

`prepare` fails before building anything when a secret or the public key is missing.

### Updates inside the app

Weaver uses the Tauri updater plugin with the endpoint `https://github.com/nicchia-code/weaver/releases/latest/download/latest.json`. It checks at start and then at most once every 24 hours while it runs; the instant of the last check is stored in `settings.json` (`lastUpdateCheck`). A failed check is logged and retried at the next due time, never shown. An update found shows its version and notes; **Install and restart** downloads it, verifies its signature and quits Weaver through the usual quit path (with its confirmation when jobs are working), applying the update when the app exits; **Later** hides it until the next check. An update whose signature does not verify is refused and logged.

## Tests and CI

The workflow `.github/workflows/ci.yml` (`ci`) runs on every push to `dev` or `main` and every pull request to `dev`, on `macos-latest` (arm64), `windows-latest` (x64) and `ubuntu-22.04` (x64), with `fail-fast: false`. A pull request from `dev` to `main` does not start its own run: it takes its checks from the run of the push to `dev` on the same commit, and the run of the push to `main` tests the merge commit. A change to `main` needs `ci` to pass, and every system decides it: a failure on any of them fails `ci`. Per platform it installs from the lockfiles (`pnpm install --frozen-lockfile`, `cargo --locked`), builds the frontend, checks that the `ts-rs` types are up to date, runs `cargo test --workspace --locked --no-fail-fast` and Vitest, builds the app in release mode (with `weaver-cli` as its sidecar), smoke-tests the built `weaver-cli`, runs the CLI contract tests and, on Linux (`xvfb-run`) and Windows (`msedgedriver`), the end-to-end smoke test. A separate job fails when the version in `src-tauri/tauri.conf.json` differs from `src-tauri/Cargo.toml`, `crates/core/Cargo.toml`, `crates/cli/Cargo.toml` or `ui/package.json`.

| What | Where | Run it |
|---|---|---|
| Core, CLI, app | `cargo test` in the crates | `cargo test --workspace --locked` |
| Interface (Vitest, Testing Library) | `ui/src/**/*.test.tsx` | `pnpm --dir ui test` |
| Generated `ts-rs` types are current | `src-tauri/src/bindings.rs` | `cargo test -p weaver --lib generated_types_are_up_to_date` |
| Version alignment | `scripts/check-versions.mjs` (tests: `scripts/check-versions.test.mjs`) | `node scripts/check-versions.mjs` and `node --test scripts/` |
| Smoke of `weaver-cli` | `scripts/smoke-weaver-cli.mjs` | `node scripts/smoke-weaver-cli.mjs target/release/weaver-cli` |
| CLI contract tests (the built `weaver-cli`, every command of the registry) | `tests/cli-contract/` | `node --test tests/cli-contract/` (binary from `CLI_CONTRACT_BIN`, else `target/release` or `target/debug`) |
| End-to-end smoke (tauri-driver, WebdriverIO; release build) | `e2e/` | Linux and Windows only, in CI: `pnpm tauri build --no-bundle`, install `tauri-driver`, then `pnpm --dir e2e test` |

Tests never touch the developer's home, settings or keychain, never use the network (the download of omp from GitHub is tested against a server on the loopback of the test itself) or a real omp, and pass the same with or without `WEAVER_*` and `WEAVER_*` variables in the environment (the contract and smoke runs remove them from the child's environment). The end-to-end suite has no WebDriver on macOS, so it runs on GitHub Actions only.

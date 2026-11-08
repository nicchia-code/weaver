# Settings and local state

## developer.CHANGE_SETTINGS

- The settings are one JSON file, `settings.json`, in the app's config folder given by Tauri's path API (`app_config_dir`). Nothing under the project, in the repository or in the environment is ever read as a setting.
- One typed schema: a Rust struct in the core crate with its defaults and validation. The TypeScript type the UI uses is generated from it at build time, never written by hand. The backend is the only reader and writer; the UI receives the effective values and sends changes, and shows the backend's value again after a change is refused or cancelled.
- Keys and defaults:

| Key | Type | Default | Valid values |
|---|---|---|---|
| `ompPath` | string | `""` | trimmed; empty means "look for omp" (`requirements/tech#omp`) |
| `models.analysis` | string | `""` | model name as omp accepts it, optional `:<thinking level>` suffix; empty means omp's default |
| `models.scan` | string | `""` | as above |
| `models.execution` | string | `""` | as above |
| `maxParallelAgents` | integer | `20` | ≥ 1 |
| `language` | string | `""` | free text, trimmed; the language of the agent's texts; empty adds no language sentence |
| `theme` | string | `"auto"` | `auto` (follows the system), `light`, `dark` |
| `editor.preset` | string | `"vscode"` | `vscode`, `cursor`, `zed`, `jetbrains`, `sublime`, `custom` |
| `editor.command` | string | `""` | used only with `custom`; must contain `{file}` |
| `permissions` | string | `"yolo"` | `yolo` (omp runs everything without asking), `ask` |
| `systemCertificates.enabled` | boolean | `false` | `true` turns System Certificates on (`requirements/tech#system-certificates`) |

- The model values and the thinking suffix are passed to omp whole and never checked against a list; a wrong name is reported by omp in the job's conversation.
- Editor presets open `{file}` at `{line}` with these argument lists: `vscode` `code --goto {file}:{line}`, `cursor` `cursor --goto {file}:{line}`, `zed` `zed {file}:{line}`, `jetbrains` `idea --line {line} {file}`, `sublime` `subl {file}:{line}`. The program is looked up with the same PATH rules as omp.
- `custom`: the template is split into arguments with POSIX shell-word quoting rules, then `{file}` (absolute path) and `{line}` (1-based, `1` when unknown) are replaced inside each argument; it runs without a shell. A template that does not split, is empty or lacks `{file}` is invalid.
- Only values that differ from the default are stored; setting a value back to its default removes its key. The file holds `"version": 1` for future migrations.
- Reading: a missing file means every default. An unknown key is ignored and dropped at the next write. A key with a wrong type or an invalid value counts as its default, and the Settings view says which key was ignored and why.
- An unparsable file counts as every default and the error is shown in Settings; it is not rewritten until the developer changes a setting, when it is first renamed to `settings.invalid-<UTC timestamp>.json` next to the new one.
- Writes are atomic (temp file in the same folder, then rename) and serialised; a failed write leaves the earlier file and the earlier values in force and shows the error.
- A change takes effect for the omp processes started after it; jobs already working keep the values they started with.
- Secrets live only in the OS keychain (the `keyring` crate, service `weaver`), never in files, arguments, logs, the UI or IPC payloads. Weaver v1 stores no secret.

## window

- Local state that is not a setting lives in Tauri's local data folder (`app_local_data_dir`), never in a project, except the window-state file below. `<project hash>` is the first 16 hex characters of the SHA-256 of the canonical project root path; on Windows the path is lowercased first, so one folder written with different cases gives one hash.
- `state.json`:
  - `recent`: up to 10 projects, most recently opened first, each `{path, name, opened}`; one entry per canonical path; an entry leaves only when pushed out by newer ones, even if its folder no longer exists, when the developer removes it from the welcome window, or when `Clear Recent` of the File menu empties the list. Emptying the list changes neither `lastSession` nor the open windows.
  - `lastSession`: the canonical paths of the projects whose window was open when Weaver last quit, in opening order. It is written at quit and on each window open or close; closing a window removes it from the list, quitting does not.
  - `jobsList`: the Jobs list of each project's window (`requirements/ui#job`), keyed by `<project hash>`: `{width, hidden}`, `width` the chosen width in px, an integer from 240 to 420, `hidden` a boolean. A project without an entry, or a value out of range or of the wrong type, counts as 280 px and shown. The backend writes it when a drag of the list's rule ends, at each keyboard step of the rule and when the list is hidden or shown, never during a drag; the narrowing of a narrow window is never written. The UI receives the values with the window's state and sends the changes.
- Window size and position are kept by Tauri's window-state plugin in its own file, in the app's config folder (`app_config_dir`) where the plugin puts it, keyed by the window's label, which carries the project hash.

## job

- Each job has a registry record `jobs/<job id>.json` in the local data folder, written when the job is created and at each change of its state. It holds what Weaver needs to show the Jobs list and resume a Suspended job:
  - `id`, `kind` (`setup`, `design`, `import`, `scan`, `run`, `resolve`, `rework`, `refinement`, `merge`), `name`, `project` (canonical root path), `target` (the request id, Task, Drift or Proposal identity it works on, when any);
  - `merged`: only for a scan after a merge, the merged files (`requirements/rules#solo-merge`), relative paths with `/`, sorted;
  - `state`, `reason` (for Failed), `created`, `ended`;
  - `sessionId` (omp's ACP session), `omp` (program path), `args` and `env` (the arguments and the Weaver-set environment variables it was started with; never inherited variables, and never the System Certificates variables, which are computed again at every start and resume);
  - `announcements`: `{id, kind, title}` of each card it announced, plus `card` (`{kind, id}`) once the card was filed with that announcement, so that after a restart Weaver still knows which announced cards already arrived; an announcement waits until its card is on the Board, and all are withdrawn when the job ends;
  - `notes`: its last notes with their instants;
  - `byChat: true`: only for a job that Chat started or whose request Chat sent, so that the Chat shows its `Started <job name>` line and the cards of its Proposals again after a restart (`requirements/ui#general-chat`);
  - `running`: only for a measured job (`design`, `rework`, `refinement`, `run`), the milliseconds it has spent in Running so far and, while it is Running, the instant it entered Running, updated at each change of its state, so that a resumed job goes on counting; `runningIncomplete: true` once Weaver found it Running or Waiting for the developer after stopping unexpectedly (crash or forced quit): the time since its last change of state is unknown and never counted.
- No conversation text is stored by Weaver: omp keeps it in its own session files, and Weaver reloads it from there.
- Records of ended jobs are kept for the Jobs list: the 50 most recent per project; older ones are deleted.
- A record that cannot be read is reported and ignored; its job counts as ended (holdings released when its project opens).
- **Measure** of an ended measured job: one line of JSON appended to `.weaver/timings/jobs.jsonl` in the project, in the local-only folder `timings/` (`requirements/data#General`), created with the first line. One line per job, written once when the job ends (Done, Failed or Stopped), never rewritten; Weaver never deletes or shortens the file. Other files in `timings/` are never read or touched. Fields, in this order:
  - `job`, `kind`, `name`, `target` (when any), `state` (`done`, `failed`, `stopped`), `created`, `ended`;
  - `durationMs`: from `created` to `ended`; `activeMs`: the time spent in Running only, without Queued, Waiting for the developer and Suspended; `activeIncomplete: true` only when the record carries `runningIncomplete`;
  - `main`, `subAgents`, `advisors` (Runs only): each `{reads, files, searches, inputTokens, outputTokens, cacheReadTokens, cacheWriteTokens}`, plus `count` (the number of transcripts summed) for `subAgents` and `advisors`; `files` across all parts at the top level too, as the distinct files read by the whole job;
  - `unavailable`: instead of the parts and the top-level `files`, the reason why the session could not be read (`requirements/tech#job`, **Measure**).
- `reads` counts the calls of omp's `read` tool; `files` the distinct paths they read, without a line selector, project-relative with `/` inside the project and as given otherwise (internal URLs included); `searches` the calls of `grep` and `glob`. Shell commands never count.

## general-chat

- General keeps `general/<project hash>.json` in the local data folder: `{project, sessionId, updated}`. It is written when omp opens or reloads the session and removed by Clear; with no record, General starts empty.
- Like jobs, no text of the conversation is stored by Weaver.

## system-certificates

- The bundle is one file, `system-certificates.pem`, in the local data folder, shared by every window and project; never in a project. It holds PEM `CERTIFICATE` blocks only, LF line endings, no keys. Its modification time is when it was written; its certificate count is read back from the file at start.
- Writing it is atomic: `system-certificates.pem.<pid>.tmp` in the same folder, then a rename over the bundle. A failed write or rename leaves the earlier bundle whole and removes the temp file.
- Turning the setting off keeps the file on disk, unused.
- `state.json` also holds `systemCertificates`: `{autoEnabledAt}`, the UTC instant (ISO 8601) when Weaver turned the setting on by itself because Zscaler is installed. Present means Weaver never does it again; it is never removed by Weaver. A `state.json` that cannot be read counts as without it.
- The marker files of omp's installer live in `omp-install/` in the local data folder: `<nonce>.code`, decimal exit code as text (`requirements/tech#omp-setup`). Starting the installer deletes every earlier marker; omp being found deletes them too.

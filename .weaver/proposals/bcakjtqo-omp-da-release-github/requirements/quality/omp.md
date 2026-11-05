---
description: "Use when testing, logging or securing omp's setup: its installer, the GitHub download, its updates and System Certificates."
---
# omp setup and System Certificates

## omp-setup

- **Security:** the GitHub download never skips its checks: no `-k`/`--insecure`, no `-SkipCertificateCheck`, no file kept or run before its size and SHA-256 match the release. The repository, the API address and the asset names are constants of `weaver-core`; no file of a project, no environment variable other than `PI_INSTALL_DIR`, `LOCALAPPDATA`, the processor variables and the certificate variables, and no message from the UI chooses what is downloaded or where.
- **Reliability:** a failed GitHub download leaves the earlier omp, if any, in place and working, and no `.omp.download` file behind; on Windows a failed version check restores the earlier `omp.exe`. The background download never blocks the start, a window, the UI or a running job.
- **Logs:** each background GitHub download at `info`: the tag, the asset, the outcome with its exit code, and the duration; a failure also at `warn` with the reason. Never the user `Path` value.
- **Tests:**
  - the installer script per system: the official installer first, the GitHub download only after a non-zero code, the code written to the marker in each case;
  - the asset chosen for each system and processor variable, musl detection, Rosetta on macOS and an unknown processor;
  - the GitHub download against a local test server: status other than 200, an HTML page, JSON without the asset or its digest, a truncated file, a wrong SHA-256, a file that does not answer `--version`, a timeout, each with its exit code and the earlier omp left in place;
  - the Windows replacement: the running file renamed aside, restored when the version check fails, old files deleted when free;
  - the user `Path` added once on Windows, not added twice with a different case or a final `\`, and the hint printed on macOS and Linux;
  - the update fallback: run only after `omp update` fails, only for an omp in `PI_INSTALL_DIR` or the default folder with `ompPath` empty and no `.bunx` next to it, skipped (`94`) when the release is not newer, and each code mapped to `omp.UPDATE_ENDED` or `omp.UPDATE_FAILED` with its reason;
  - the failure text for each exit code, with System Certificates on and off, in Rust and in Vitest.

## system-certificates

- **Security:** Weaver never turns TLS checking off: no `NODE_TLS_REJECT_UNAUTHORIZED`, `GIT_SSL_NO_VERIFY`, `PYTHONHTTPSVERIFY=0`, `-k`/`--insecure` or `strict-ssl=false`, ever. Only the machine's and the current user's stores are read, never another user's. No file of a project, no environment variable and no message from the UI chooses the sources, the bundle path, the variables or the processes that get them; the UI can only ask for an export while the setting is on.
- **Privacy:** no network call for the export or the Zscaler check. The bundle holds public certificates only, no keys. Error output is tested in memory only.
- **Reliability:** an export never blocks the start, a window, the UI or a job, and ends within 30 s; a failed or late export leaves the earlier bundle whole. With the setting off, every process Weaver starts gets exactly the environment it got before System Certificates existed. A failure of the Zscaler check changes nothing and shows nothing.
- **Logs:** each export at `info`: automatic or asked, outcome, certificate count, duration and, on failure, the reason; the Zscaler turn-on at `info` and a failure of its writes at `warn`. Never the bundle's content, certificate subjects or the variables' values.
- **Tests:**
  - bundle building: duplicates, non-CA and unreadable certificates, no system authority, public roots added after the system's, PEM shape;
  - the atomic write with a failing rename leaving the earlier bundle, and the 30 s limit with an injected slow reader;
  - the 24 h rule at start and the daily timer, with an injected clock;
  - the variables with the setting on, off and without a bundle, their precedence over inherited ones, and their presence at every spawn site: the omp commands, jobs, General, resumes, and the installer and login terminals of each system (in the environment on Windows and Linux, in the command on macOS and in the Linux script), never in the job registry;
  - Zscaler detection per system with an injected file system, and the turn-on: flag absent or present, setting already on, Zscaler absent, failed setting write, notification and banners sent once;
  - the installer command per system and its marker: a code, `0`, no marker, a stale nonce, forgotten at the next install and when omp is found;
  - each certificate marker and HTTP form and code, other codes, bare numbers, certificate precedence, the explanation with the setting on and off, and that no error text names a product;
  - the switch, status line, `Refresh` and banners in Vitest with their keyboard paths and accessible names.

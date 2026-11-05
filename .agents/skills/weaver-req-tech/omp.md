# omp

## omp

- **Lookup:** the first executable found wins (`omp.exe` on Windows):
  1. the `ompPath` setting, trimmed. When set, it is used alone, with no fallback;
  2. PATH;
  3. the installer folders:
     - macOS and Linux: `$PI_INSTALL_DIR`, `~/.local/bin`, then `$BUN_INSTALL/bin` or `~/.bun/bin`;
     - Windows: `%PI_INSTALL_DIR%`, `%LOCALAPPDATA%\omp`, `%USERPROFILE%\.bun\bin`.
- **PATH of a GUI app:**
  - macOS and Linux: at start Weaver reads the login PATH once by running `$SHELL -l -c 'printf %s "$PATH"'`, with a 5 s limit. This is the only shell Weaver starts on its own. On failure it keeps the inherited PATH.
  - Windows: every lookup reads the current user and machine `Path` from the registry, so a fresh install is seen without restarting Weaver.
- **Child PATH:** every omp process gets `<app local data>/bin`, omp's folder and bun's bin folder, then the PATH above.
- **Certificate variables:** every omp process, of the table below, of jobs and of General, also gets the System Certificates variables of `requirements/tech#system-certificates`, read at each start.
- **Minimum version:** `MIN_OMP_VERSION = 18.15.0`, one tested constant in `weaver-core`. It is compared as semver against `omp/<version>`. An unreadable version counts as outdated. Weaver has no fallbacks for older omp.
- **Commands:** no shell, fixed arguments. Stdout is parsed and stderr kept for reasons. On timeout the process is killed and the run fails.

| Command | Limit | Use |
|---|---|---|
| `omp --version` | 15 s | version |
| `omp models --json` | 15 s | `selector`, `provider`; no models means `NO_PROVIDER` |
| `omp update --check` | 30 s | `UPDATE_AVAILABLE` |
| `omp update` | 5 min | `UPDATE_ENDED` / `UPDATE_FAILED` |
| `omp auth-broker logout <provider>` | 30 s | log out |
| `omp usage --json` | 30 s | usage limits of the connected providers |

- **Readiness:** one app-wide state, with each result tagged by the omp path it was computed for.
- **Failure reasons:** the last non-empty stderr line, else `exit code N`, `could not start omp` or `timed out`. The same rule applies to ACP errors (opening, turn and message errors) and job stderr tails.
  - A reason that is a certificate or HTTP error (`requirements/rules#system-certificates`) is replaced by the explanation of `requirements/ui#system-certificates`, written when the failure happens with the setting as it is then. omp's own lines stay in the details where the surface has them.

## omp-setup

- **System terminal launch:** the command runs where the developer sees it, and the terminal stays open at the end.
  - macOS: `/usr/bin/osascript` with fixed `-e` lines: Terminal runs `do script "<command>"` and comes forward. Terminal starts the developer's own shell, which does not inherit Weaver's environment: with System Certificates on, `<command>` starts with `/usr/bin/env <NAME>=<bundle path> … ` and runs the rest under `/bin/sh -c`.
  - Windows: Windows PowerShell 5.1, `powershell.exe -NoExit -NoProfile -ExecutionPolicy Bypass -Command <command>`, with `CREATE_NEW_CONSOLE`. The certificate variables go in its process environment.
  - Linux: the first of `x-terminal-emulator -e`, `gnome-terminal --`, `konsole -e`, `xfce4-terminal -x`, `xterm -e`. It runs `sh -c '<command>; printf "\nPress Enter to close"; read _'`; with System Certificates on, the script starts with `export <NAME>=<bundle path> …;`, since some terminals do not pass Weaver's environment on. If none exists, Weaver shows the command to copy, certificate variables included.
- **Quoting:** these terminal commands do pass through a shell. One tested function quotes for the target shell. Only omp's absolute path, a provider id from the fixed list, the bundle path, the installer marker path, and omp's folder and version for the background GitHub download are inserted.
- **Commands:**
  - Installer: it downloads the official installer to a temporary file and runs it, never piping it into a shell, so that a failed download counts as a failure. When that step ends with any code other than `0`, the same script runs the GitHub download below. Then it writes the code of the last step that ran (`0` when the official installer succeeded) to `<app local data>/omp-install/<nonce>.code`, with a new nonce at each `INSTALL_OMP`, and deletes the temporary files. Weaver never writes `ompPath` afterwards.
    - macOS and Linux: `curl -fsSL https://omp.sh/install -o <temp>`, then `sh <temp>`, in `/bin/sh`; the GitHub download runs in the same `/bin/sh`.
    - Windows: `Invoke-RestMethod https://omp.sh/install.ps1 -OutFile <temp>.ps1`, then a child `powershell.exe -NoProfile -ExecutionPolicy Bypass -File <temp>.ps1`, so that the installer's `exit` ends only the child; its `$LASTEXITCODE` is the code, and a terminating error of the download is `1`. The GitHub download runs in the parent PowerShell.
    - Before the GitHub download the terminal prints `The official installer failed (exit code <N>). Downloading omp from GitHub instead…`.
- **GitHub download:** one script per shell (POSIX `sh` with `curl`; Windows PowerShell 5.1 with `$ProgressPreference = 'SilentlyContinue'`), the same text for the installer terminal and the background update. The backend has no HTTP client of its own for it.
  - Asset of the release: Windows `omp-windows-x64.exe` or `omp-windows-arm64.exe`, from `PROCESSOR_ARCHITEW6432`, else `PROCESSOR_ARCHITECTURE` (`AMD64`, `ARM64`); macOS `omp-darwin-arm64` when `sysctl -n hw.optional.arm64` prints `1` (also under Rosetta), else `omp-darwin-x64`; Linux `omp-linux-x64` or `omp-linux-arm64` from `uname -m` (`x86_64`/`amd64`, `aarch64`/`arm64`), with `linux-musl` in place of `linux` when `/etc/alpine-release` exists or `ldd --version` mentions `musl`. Any other processor fails with code `1` and prints `omp has no GitHub build for <system> <processor>`.
  - Release: `https://api.github.com/repos/can1357/oh-my-pi/releases/latest` with `Accept: application/vnd.github+json`, 60 s limit. From it: `tag_name`, and for the asset with exactly the name above `browser_download_url`, `size` and `digest` (`sha256:` and 64 hex digits). An answer that is not that JSON, or lacks the asset or its digest, counts as blocked.
  - Folder: for an installation `PI_INSTALL_DIR` when set, else `%LOCALAPPDATA%\omp` on Windows or `~/.local/bin`, created when missing; for an update the folder of the omp file found. The file is `omp.exe` on Windows, else `omp`.
  - Download: to `<folder>/.omp.download` (`.omp.download.exe` on Windows), following redirects, 900 s limit: `curl -fL --connect-timeout 10 --max-time 900 -o`; `Invoke-WebRequest -UseBasicParsing -TimeoutSec 900 -OutFile`.
  - Checks, in this order, each failure deleting the downloaded file: final status 200; size equal to `size`; SHA-256 equal to `digest` (`sha256sum`, else `shasum -a 256`; `Get-FileHash -Algorithm SHA256`); on macOS and Linux `chmod 755`; the file moved into place; `<folder>/<file> --version` exits `0` within 15 s.
  - Replacing: on macOS and Linux the move replaces the old file. On Windows an existing `omp.exe` is first renamed `omp.exe.<unix seconds>.old`, so that running omp processes keep working; when the version check fails, the new file is deleted and the old one renamed back. Each GitHub download first deletes the `omp.exe.*.old` files of its folder that can be deleted.
  - PATH, for an installation only: on Windows, when the folder is not in the user `Path` (compared ignoring case and a final `\`), `[Environment]::SetEnvironmentVariable('Path', '<user Path>;<folder>', 'User')`; on macOS and Linux, when the folder is not in `PATH`, it prints `Add <folder> to your PATH to use omp in your own terminal.` and edits no shell file.
  - Exit codes: `0` installed; `90` blocked: the API or the download answered an HTTP status other than 200, an HTML page or not the release JSON, or the connection failed or timed out; `91` certificate: curl exit `60` or `77`, or a PowerShell error whose message is a certificate error of `requirements/rules#system-certificates` or contains `trust relationship`; `92` the size or the SHA-256 differ; `93` the placed omp does not answer `--version`; `94` the release is not newer than the version given (update only); `1` for any other failure. Each failure prints the step, the address and the HTTP status or the tool's message.
  - Login: `omp login <provider>`.
- **Providers,** in this order: `openai-codex` (ChatGPT), `anthropic`, `github-copilot`, `google-gemini-cli`, `google-antigravity`, `cursor`, `xai-oauth`.
  - A provider is connected when any model from `omp models --json` has it as `provider`.
  - `cursor` is offered through login only. Weaver has no API key field and stores no secret.
- **Re-checks:** every 3 s while Installing or Logging in, one at a time. A check runs the lookup, `omp --version` and `omp models --json`. Re-checks stop when the state is left.
  - While Installing, a check that finds no omp then reads the marker of the current nonce: a code other than `0` is `omp.INSTALL_FAILED`, with that code; `0`, no marker or an unreadable one is `omp.MISSING`. Weaver keeps the code until the next `INSTALL_OMP` or until omp is found.
- **Updates:** the check runs at most once per hour app-wide, and its result is cached in memory; it stays `omp update --check` alone. One update runs at a time app-wide, the GitHub fallback included.
  - Fallback: when `omp update` fails or times out, `ompPath` is empty, and the found omp file, with symlinks resolved, sits directly in `PI_INSTALL_DIR` or the default folder above, and on Windows has no `.bunx` file next to it, Weaver runs the GitHub download for that folder with omp's version, as a hidden child process (`/bin/sh -c`; `powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -Command` with `CREATE_NO_WINDOW`), with the certificate variables, 15 min limit, and no PATH change. The script exits `94` without downloading when `tag_name`, without a leading `v`, is not semver-newer than that version.
  - Result: `0` is `omp.UPDATE_ENDED`; `94` is `omp.UPDATE_FAILED` with `omp update`'s own reason; any other code, or the time limit, is `omp.UPDATE_FAILED` with the reason of `requirements/ui#system-certificates` for that code.

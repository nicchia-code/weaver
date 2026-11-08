//! Opening the system terminal for omp's installer and login (`requirements/tech#omp-setup`).
//!
//! The command runs where the developer sees it and the terminal stays open at its end. These
//! launches do pass through a shell, so [`quote`] is the one place text is quoted for a target
//! shell; the only text inserted into a command is the absolute path of omp, the id of a provider
//! of the fixed list ([`Provider`]), the path of the System Certificates bundle and the path of
//! the installer marker (the folder and version of the background GitHub download are inserted by
//! [`super::github`], quoted with [`quote`]).
//!
//! | System  | Launch |
//! |---------|--------|
//! | macOS   | `/usr/bin/osascript` with fixed `-e` lines: Terminal runs `do script "<command>"` and comes forward |
//! | Windows | `powershell.exe -NoExit -NoProfile -ExecutionPolicy Bypass -Command <command>` in a new console |
//! | Linux   | the first available of `x-terminal-emulator -e`, `gnome-terminal --`, `konsole -e`, `xfce4-terminal -x`, `xterm -e`, running `sh -c '<command>; printf "\nPress Enter to close"; read _'` |
//!
//! # System Certificates
//!
//! With the variables of `crate::system_certificates` given, they reach the terminal like this:
//! Windows, in the environment of the `powershell.exe` process; macOS, Terminal starts the
//! developer's own shell, so the command of `do script` starts with `/usr/bin/env <NAME>=<path> … `
//! and the rest runs under `/bin/sh -c`; Linux, the script starts with `export <NAME>=<path> …;`.
//! The command to copy, when no terminal can be opened, includes them too.
//!
//! # The installer
//!
//! It downloads the official installer to a temporary file and runs it, never piping it into a
//! shell, so that a failed download counts as a failure. When that step ends with a code other than
//! `0` it prints `The official installer failed (exit code <N>). Downloading the agent from GitHub
//! instead…` and runs the GitHub download of [`super::github`] in the same shell. Then it writes
//! the code of the last step that ran, else `0`, to the marker (`super::installer`) and deletes
//! the temporary files.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;

use super::github::{self, Config, Target};
use super::lookup::Os;
use super::providers::Provider;
use crate::system_certificates::Variables;

/// The official installer for macOS and Linux.
pub const INSTALLER_UNIX_URL: &str = "https://omp.sh/install";
/// The official installer for Windows.
pub const INSTALLER_WINDOWS_URL: &str = "https://omp.sh/install.ps1";

/// The emulators tried on Linux, in order, with the flag that precedes the command.
pub const LINUX_TERMINALS: [(&str, &str); 5] = [
    ("x-terminal-emulator", "-e"),
    ("gnome-terminal", "--"),
    ("konsole", "-e"),
    ("xfce4-terminal", "-x"),
    ("xterm", "-e"),
];

/// How long `osascript` may take to open Terminal.
const OSASCRIPT_LIMIT: Duration = Duration::from_secs(15);

#[cfg(windows)]
const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TerminalError {
    #[error("the path of the agent is not absolute")]
    NotAbsolute,
    #[error("a path or value holds a line break or a quote that cannot be passed to a terminal")]
    UnsafePath,
    #[error("no terminal emulator was found")]
    NoTerminal,
    #[error("{0}")]
    Launch(String),
}

/// The shell a command is quoted for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shell {
    /// `sh`, `bash`, `zsh`, `dash` and `fish`.
    Posix,
    /// Windows PowerShell.
    PowerShell,
}

/// `text` as one word of `shell`, with nothing in it left to the shell to read: in single quotes,
/// where a quote inside is `'\''` for a POSIX shell and doubled for PowerShell, which also reads
/// the typographic single quotes as quotes, so those are doubled too. The one function that
/// quotes for a terminal.
pub fn quote(shell: Shell, text: &str) -> String {
    match shell {
        Shell::Posix => format!("'{}'", text.replace('\'', r"'\''")),
        Shell::PowerShell => {
            let mut quoted = String::with_capacity(text.len() + 2);
            quoted.push('\'');
            for c in text.chars() {
                quoted.push(c);
                if matches!(c, '\'' | '\u{2018}' | '\u{2019}' | '\u{201A}' | '\u{201B}') {
                    quoted.push(c);
                }
            }
            quoted.push('\'');
            quoted
        }
    }
}

/// `text` as an AppleScript string literal: `\` and `"` are escaped.
pub fn applescript_string(text: &str) -> String {
    format!("\"{}\"", text.replace('\\', r"\\").replace('"', r#"\""#))
}

/// What the terminal runs.
#[derive(Debug, Clone, Copy)]
pub enum Task<'a> {
    /// omp's official installer; it leaves its exit code in `marker`.
    Installer { marker: &'a Path },
    /// `omp login <provider>`.
    Login {
        omp: &'a Path,
        provider: &'a Provider,
    },
}

/// A task written for the system the terminal is on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    /// What the terminal is told to run.
    pub terminal: String,
    /// Windows: the certificate variables for the process environment of `powershell.exe`; the
    /// other systems carry them in `terminal`.
    pub environment: Vec<(String, String)>,
    /// What the developer can copy into a terminal of his own when none can be opened: the same
    /// task with the certificate variables.
    pub copy: String,
}

/// The command to copy when `task` cannot be written for the terminal (a path that cannot be
/// passed): no certificate variable, no marker.
pub fn plain_command(os: Os, task: Task) -> String {
    match task {
        Task::Login { provider, .. } => format!("omp login {}", provider.id),
        Task::Installer { .. } if os == Os::Windows => format!(
            "Invoke-RestMethod {INSTALLER_WINDOWS_URL} -OutFile install.ps1; powershell.exe -NoProfile -ExecutionPolicy Bypass -File install.ps1"
        ),
        Task::Installer { .. } => {
            format!("curl -fsSL {INSTALLER_UNIX_URL} -o install.sh && sh install.sh")
        }
    }
}

/// Writes `task` for `os`, with `variables` (the certificate variables; empty when System
/// Certificates is off or has no bundle). Only the absolute path of omp, a provider id, the
/// marker path and the values of `variables` are inserted, quoted for the shell that reads them.
pub fn command(os: Os, task: Task, variables: &Variables) -> Result<Command, TerminalError> {
    let environment = checked_variables(os, variables)?;
    if os == Os::Windows {
        let body = windows_body(os, task)?;
        let assignments: String = environment
            .iter()
            .map(|(name, value)| format!("$env:{name} = {}; ", quote(Shell::PowerShell, value)))
            .collect();
        return Ok(Command {
            terminal: body.clone(),
            environment,
            copy: format!("{assignments}{body}"),
        });
    }

    let body = posix_body(os, task)?;
    let installer = matches!(task, Task::Installer { .. });
    let assignments: Vec<String> = environment
        .iter()
        .map(|(name, value)| format!("{name}={}", quote(Shell::Posix, value)))
        .collect();
    // From the developer's own shell: the variables, then the rest under `/bin/sh`. The installer
    // is a script for `sh`, so it always runs there.
    let copy = if installer || !assignments.is_empty() {
        let mut words = Vec::new();
        if !assignments.is_empty() {
            words.push("/usr/bin/env".to_owned());
            words.extend(assignments.iter().cloned());
        }
        words.push("/bin/sh -c".to_owned());
        words.push(quote(Shell::Posix, &body));
        words.join(" ")
    } else {
        body.clone()
    };
    let terminal = match os {
        Os::Linux if !assignments.is_empty() => {
            let exports = assignments.join(" ");
            format!("export {exports}; {body}")
        }
        Os::Linux => body,
        _ => copy.clone(),
    };
    Ok(Command {
        terminal,
        environment: Vec::new(),
        copy,
    })
}

/// The variables as text, each name and value checked.
fn checked_variables(
    os: Os,
    variables: &Variables,
) -> Result<Vec<(String, String)>, TerminalError> {
    variables
        .iter()
        .map(|(name, value)| {
            let valid_name = !name.is_empty()
                && name
                    .bytes()
                    .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_');
            let value = value.to_str().ok_or(TerminalError::UnsafePath)?;
            if !valid_name || is_unsafe(os, value) {
                return Err(TerminalError::UnsafePath);
            }
            Ok(((*name).to_owned(), value.to_owned()))
        })
        .collect()
}

/// Whether `text` holds a character that cannot be passed to a terminal: a control character,
/// or on Windows a double quote, which the command line of `powershell.exe` cannot carry.
fn is_unsafe(os: Os, text: &str) -> bool {
    text.chars()
        .any(|c| c.is_control() || (os == Os::Windows && c == '"'))
}

/// `path` as text: absolute on `os` and safe to pass.
pub(super) fn absolute_text(os: Os, path: &Path) -> Result<&str, TerminalError> {
    let text = path.to_str().ok_or(TerminalError::UnsafePath)?;
    if !is_absolute_on(os, text) {
        return Err(TerminalError::NotAbsolute);
    }
    if is_unsafe(os, text) {
        return Err(TerminalError::UnsafePath);
    }
    Ok(text)
}

/// The text for `sh` (macOS and Linux).
fn posix_body(os: Os, task: Task) -> Result<String, TerminalError> {
    Ok(match task {
        Task::Login { omp, provider } => format!(
            "{} login {}",
            quote(Shell::Posix, absolute_text(os, omp)?),
            provider.id
        ),
        Task::Installer { marker } => format!(
            "{} t=$(mktemp \"${{TMPDIR:-/tmp}}/omp-install.XXXXXX\"); c=$?; \
             if [ \"$c\" -eq 0 ]; then \
             curl -fsSL {INSTALLER_UNIX_URL} -o \"$t\"; c=$?; \
             if [ \"$c\" -eq 0 ]; then sh \"$t\"; c=$?; fi; \
             rm -f \"$t\"; \
             fi; \
             if [ \"$c\" -ne 0 ]; then \
             printf '%s\\n' \"The official installer failed (exit code $c). Downloading the agent from GitHub instead…\"; \
             {}; c=$?; \
             fi; \
             printf '%s\\n' \"$c\" > {}",
            github::definitions(Shell::Posix, Target::Install, &Config::default()),
            github::call(Shell::Posix),
            quote(Shell::Posix, absolute_text(os, marker)?)
        ),
    })
}

/// The text for Windows PowerShell 5.1.
fn windows_body(os: Os, task: Task) -> Result<String, TerminalError> {
    Ok(match task {
        Task::Login { omp, provider } => format!(
            "& {} login {}",
            quote(Shell::PowerShell, absolute_text(os, omp)?),
            provider.id
        ),
        // The child `powershell.exe` takes the `exit` of the installer; its `$LASTEXITCODE` is the
        // code, and a terminating error of the download is `1`.
        Task::Installer { marker } => format!(
            "{} $b = [IO.Path]::GetTempFileName(); $t = $b + '.ps1'; $c = 0; \
             try {{ Invoke-RestMethod {INSTALLER_WINDOWS_URL} -OutFile $t -ErrorAction Stop }} catch {{ $c = 1 }}; \
             if ($c -eq 0) {{ powershell.exe -NoProfile -ExecutionPolicy Bypass -File $t; \
             $c = $LASTEXITCODE; if ($null -eq $c) {{ $c = 1 }} }}; \
             Remove-Item -LiteralPath $b, $t -ErrorAction SilentlyContinue; \
             if ($c -ne 0) {{ Write-Host ('The official installer failed (exit code ' + $c + '). Downloading the agent from GitHub instead…'); \
             {}; $c = $global:OmpCode }}; \
             Set-Content -LiteralPath {} -Value $c -Encoding ASCII",
            github::definitions(Shell::PowerShell, Target::Install, &Config::default()),
            github::call(Shell::PowerShell),
            quote(Shell::PowerShell, absolute_text(os, marker)?)
        ),
    })
}

/// Whether `text` is an absolute path on `os` (a drive letter or a UNC prefix on Windows),
/// whichever system Weaver runs on.
fn is_absolute_on(os: Os, text: &str) -> bool {
    match os {
        Os::Windows => {
            let bytes = text.as_bytes();
            let drive = bytes.len() >= 3
                && bytes[0].is_ascii_alphabetic()
                && bytes[1] == b':'
                && matches!(bytes[2], b'\\' | b'/');
            drive || text.starts_with("\\\\")
        }
        Os::MacOs | Os::Linux => text.starts_with('/'),
    }
}

/// A terminal to start: the program, its arguments and whether it needs its own console.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Launch {
    pub program: PathBuf,
    pub args: Vec<OsString>,
    /// Windows: variables for the process environment of the terminal.
    pub environment: Vec<(String, String)>,
    /// Windows: the process is created with `CREATE_NEW_CONSOLE`.
    pub new_console: bool,
}

fn os_args(args: &[&str]) -> Vec<OsString> {
    args.iter().map(OsString::from).collect()
}

/// How to run `command` in a terminal of `os`. `which` finds a program of the Linux list by name.
pub fn launch_plan(
    os: Os,
    command: &Command,
    which: &dyn Fn(&str) -> Option<PathBuf>,
) -> Result<Launch, TerminalError> {
    let text = command.terminal.as_str();
    match os {
        Os::MacOs => {
            let run = format!("do script {}", applescript_string(text));
            Ok(Launch {
                program: PathBuf::from("/usr/bin/osascript"),
                args: os_args(&[
                    "-e",
                    "tell application \"Terminal\"",
                    "-e",
                    "activate",
                    "-e",
                    &run,
                    "-e",
                    "end tell",
                ]),
                environment: Vec::new(),
                new_console: false,
            })
        }
        Os::Windows => Ok(Launch {
            program: PathBuf::from("powershell.exe"),
            args: os_args(&[
                "-NoExit",
                "-NoProfile",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                text,
            ]),
            environment: command.environment.clone(),
            new_console: true,
        }),
        Os::Linux => {
            let script = format!("{text}; printf \"\\nPress Enter to close\"; read _");
            let (program, flag) = LINUX_TERMINALS
                .iter()
                .find_map(|(name, flag)| which(name).map(|found| (found, *flag)))
                .ok_or(TerminalError::NoTerminal)?;
            Ok(Launch {
                program,
                args: os_args(&[flag, "sh", "-c", &script]),
                environment: Vec::new(),
                new_console: false,
            })
        }
    }
}

/// Starts the terminal. On macOS it waits for `osascript` (which fails when the developer has not
/// allowed Weaver to control Terminal); elsewhere the terminal is left running and reaped in a
/// thread.
pub fn open(os: Os, launch: &Launch) -> Result<(), TerminalError> {
    let mut command = crate::process::command(&launch.program, &launch.args);
    for (name, value) in &launch.environment {
        command.env(name, value);
    }
    #[cfg(windows)]
    if launch.new_console {
        use std::os::windows::process::CommandExt;
        command.creation_flags(CREATE_NEW_CONSOLE);
    }
    if os == Os::MacOs {
        return crate::process::timed::run_command(command, OSASCRIPT_LIMIT)
            .map(|_| ())
            .map_err(|failure| TerminalError::Launch(failure.reason()));
    }
    command
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    let mut child = command
        .spawn()
        .map_err(|e| TerminalError::Launch(e.to_string()))?;
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::omp::providers::provider;

    fn anthropic() -> &'static Provider {
        provider("anthropic").unwrap()
    }

    fn linux_with<'a>(available: &'a [&'a str]) -> impl Fn(&str) -> Option<PathBuf> + 'a {
        move |name| {
            available
                .contains(&name)
                .then(|| PathBuf::from("/usr/bin").join(name))
        }
    }

    fn strings(args: &[OsString]) -> Vec<String> {
        args.iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect()
    }

    fn certificates(bundle: &str) -> Variables {
        vec![
            ("NODE_EXTRA_CA_CERTS", OsString::from(bundle)),
            ("SSL_CERT_FILE", OsString::from(bundle)),
            ("PI_REQUEST_COMPRESSION", OsString::from("0")),
        ]
    }

    fn login(os: Os, omp: &str, variables: &Variables) -> Command {
        command(
            os,
            Task::Login {
                omp: Path::new(omp),
                provider: anthropic(),
            },
            variables,
        )
        .unwrap()
    }

    fn install(os: Os, marker: &str, variables: &Variables) -> Command {
        command(
            os,
            Task::Installer {
                marker: Path::new(marker),
            },
            variables,
        )
        .unwrap()
    }

    #[test]
    fn posix_quoting_wraps_in_single_quotes_and_escapes_quotes() {
        let posix = |text| quote(Shell::Posix, text);
        assert_eq!(posix("/usr/bin/omp"), "'/usr/bin/omp'");
        assert_eq!(posix("/Users/o'neil/omp"), r"'/Users/o'\''neil/omp'");
        assert_eq!(
            posix("/a b/$HOME/`id`/\"x\"/;rm"),
            "'/a b/$HOME/`id`/\"x\"/;rm'"
        );
        assert_eq!(posix(""), "''");
        assert_eq!(posix("''"), r"''\'''\'''");
    }

    #[test]
    fn powershell_quoting_doubles_every_kind_of_single_quote() {
        let ps = |text| quote(Shell::PowerShell, text);
        assert_eq!(
            ps(r"C:\Program Files\omp\omp.exe"),
            r"'C:\Program Files\omp\omp.exe'"
        );
        assert_eq!(ps("C:\\o'neil\\omp.exe"), "'C:\\o''neil\\omp.exe'");
        assert_eq!(ps("a\u{2019}b"), "'a\u{2019}\u{2019}b'");
        assert_eq!(ps("$env:X `n $(calc)"), "'$env:X `n $(calc)'");
    }

    #[test]
    fn applescript_quoting_escapes_backslash_and_double_quote() {
        assert_eq!(applescript_string("echo hi"), "\"echo hi\"");
        assert_eq!(
            applescript_string(r#"say "hi" \ done"#),
            r#""say \"hi\" \\ done""#
        );
    }

    #[test]
    fn the_login_command_quotes_the_path_for_each_shell() {
        assert_eq!(
            login(Os::MacOs, "/Users/o'neil/bin/omp", &vec![]).terminal,
            r"'/Users/o'\''neil/bin/omp' login anthropic"
        );
        assert_eq!(
            command(
                Os::Linux,
                Task::Login {
                    omp: Path::new("/home/a b/omp"),
                    provider: provider("openai-codex").unwrap()
                },
                &vec![]
            )
            .unwrap()
            .terminal,
            "'/home/a b/omp' login openai-codex"
        );
        assert_eq!(
            command(
                Os::Windows,
                Task::Login {
                    omp: Path::new(r"C:\Users\o'neil\omp.exe"),
                    provider: provider("cursor").unwrap()
                },
                &vec![]
            )
            .unwrap()
            .terminal,
            r"& 'C:\Users\o''neil\omp.exe' login cursor"
        );
    }

    #[test]
    fn a_path_that_is_not_absolute_or_holds_a_line_break_is_refused() {
        let refused = |os, omp: &str| {
            command(
                os,
                Task::Login {
                    omp: Path::new(omp),
                    provider: anthropic(),
                },
                &vec![],
            )
            .unwrap_err()
        };
        assert_eq!(refused(Os::Linux, "omp"), TerminalError::NotAbsolute);
        assert_eq!(
            refused(Os::Linux, "/a\n; touch /tmp/x\n/omp"),
            TerminalError::UnsafePath
        );
        assert_eq!(refused(Os::MacOs, "/a\r/omp"), TerminalError::UnsafePath);
        assert_eq!(
            refused(Os::Windows, r#"C:\a"b\omp.exe"#),
            TerminalError::UnsafePath
        );
        // The marker and the bundle are checked the same way.
        assert_eq!(
            command(
                Os::Linux,
                Task::Installer {
                    marker: Path::new("relative.code")
                },
                &vec![]
            )
            .unwrap_err(),
            TerminalError::NotAbsolute
        );
        assert_eq!(
            command(
                Os::Linux,
                Task::Login {
                    omp: Path::new("/omp"),
                    provider: anthropic()
                },
                &certificates("/b\n/x.pem")
            )
            .unwrap_err(),
            TerminalError::UnsafePath
        );
    }

    #[test]
    fn macos_starts_with_env_and_runs_the_rest_under_sh_when_certificates_are_on() {
        let command = login(Os::MacOs, "/opt/omp", &certificates("/data/bundle.pem"));
        assert_eq!(
            command.terminal,
            "/usr/bin/env NODE_EXTRA_CA_CERTS='/data/bundle.pem' SSL_CERT_FILE='/data/bundle.pem' \
             PI_REQUEST_COMPRESSION='0' /bin/sh -c ''\\''/opt/omp'\\'' login anthropic'"
        );
        assert_eq!(command.copy, command.terminal);
        assert!(command.environment.is_empty());
    }

    #[test]
    fn linux_exports_the_variables_at_the_start_of_the_script() {
        let command = login(Os::Linux, "/opt/omp", &certificates("/data/bundle.pem"));
        assert_eq!(
            command.terminal,
            "export NODE_EXTRA_CA_CERTS='/data/bundle.pem' SSL_CERT_FILE='/data/bundle.pem' \
             PI_REQUEST_COMPRESSION='0'; '/opt/omp' login anthropic"
        );
        // To copy into any shell: the same as macOS.
        assert!(command.copy.starts_with(
            "/usr/bin/env NODE_EXTRA_CA_CERTS='/data/bundle.pem' SSL_CERT_FILE='/data/bundle.pem'"
        ));
        assert!(command.copy.contains("/bin/sh -c "));
        let off = login(Os::Linux, "/opt/omp", &vec![]);
        assert_eq!(off.terminal, "'/opt/omp' login anthropic");
        assert_eq!(off.copy, off.terminal);
    }

    #[test]
    fn windows_gives_the_variables_to_the_process_and_the_copy_sets_them() {
        let command = login(
            Os::Windows,
            r"C:\omp\omp.exe",
            &certificates(r"C:\d a\b.pem"),
        );
        assert_eq!(command.terminal, r"& 'C:\omp\omp.exe' login anthropic");
        assert_eq!(
            command.environment,
            [
                ("NODE_EXTRA_CA_CERTS".to_owned(), r"C:\d a\b.pem".to_owned()),
                ("SSL_CERT_FILE".to_owned(), r"C:\d a\b.pem".to_owned()),
                ("PI_REQUEST_COMPRESSION".to_owned(), "0".to_owned()),
            ]
        );
        assert_eq!(
            command.copy,
            r"$env:NODE_EXTRA_CA_CERTS = 'C:\d a\b.pem'; $env:SSL_CERT_FILE = 'C:\d a\b.pem'; $env:PI_REQUEST_COMPRESSION = '0'; & 'C:\omp\omp.exe' login anthropic"
        );
    }

    #[test]
    fn the_installer_downloads_to_a_file_runs_it_and_leaves_its_code_in_the_marker() {
        let posix = install(Os::Linux, "/data/omp-install/ab12.code", &vec![]);
        for needle in [
            "curl -fsSL https://omp.sh/install -o \"$t\"",
            "sh \"$t\"",
            "rm -f \"$t\"",
            "> '/data/omp-install/ab12.code'",
            // The GitHub download only after a code other than 0, then the code of the last step.
            "if [ \"$c\" -ne 0 ]; then printf '%s\\n' \"The official installer failed (exit code $c). Downloading the agent from GitHub instead…\"; omp_github; c=$?; fi; printf '%s\\n' \"$c\" > '/data/omp-install/ab12.code'",
        ] {
            assert!(posix.terminal.contains(needle), "{needle}");
        }
        // The official installer comes first, and is never piped into a shell.
        let official = posix
            .terminal
            .find("curl -fsSL https://omp.sh/install")
            .unwrap();
        let github = posix.terminal.find("omp_github; c=$?").unwrap();
        assert!(official < github);
        for piped in ["| sh", "| bash", "|sh"] {
            assert!(!posix.terminal.contains(piped), "{piped}");
        }
        // The developer's shell is not assumed: the copy runs it under `/bin/sh`.
        assert!(posix.copy.starts_with("/bin/sh -c "), "{}", posix.copy);
        assert_eq!(
            install(Os::MacOs, "/m/ab.code", &vec![]).terminal,
            install(Os::MacOs, "/m/ab.code", &vec![]).copy
        );
        assert!(install(Os::MacOs, "/m/ab.code", &vec![])
            .terminal
            .starts_with("/bin/sh -c '"));

        let windows = install(Os::Windows, r"C:\d\omp-install\ab12.code", &vec![]);
        for needle in [
            "Invoke-RestMethod https://omp.sh/install.ps1 -OutFile $t",
            "-ErrorAction Stop } catch { $c = 1 }",
            "powershell.exe -NoProfile -ExecutionPolicy Bypass -File $t",
            "$c = $LASTEXITCODE",
            "Remove-Item -LiteralPath $b, $t",
            "Set-Content -LiteralPath 'C:\\d\\omp-install\\ab12.code' -Value $c",
            "if ($c -ne 0) { Write-Host ('The official installer failed (exit code ' + $c + '). Downloading the agent from GitHub instead…'); Invoke-OmpGithub; $c = $global:OmpCode }; Set-Content",
        ] {
            assert!(windows.terminal.contains(needle), "{needle}");
        }
        // The GitHub download runs in the parent PowerShell, after the child that ran the official
        // installer.
        let official = windows.terminal.find("-File $t").unwrap();
        let github = windows.terminal.find("; Invoke-OmpGithub;").unwrap();
        assert!(official < github);
        assert!(!windows.terminal.contains("iex"));
        assert!(!windows.terminal.contains('"'));
    }

    #[test]
    fn the_marker_path_is_quoted_for_the_shell_that_reads_it() {
        let posix = install(Os::Linux, "/d/o'neil/ab.code", &vec![]);
        assert!(posix.terminal.ends_with(r"> '/d/o'\''neil/ab.code'"));
        let windows = install(Os::Windows, r"C:\o'neil\ab.code", &vec![]);
        assert!(windows.terminal.contains(r"'C:\o''neil\ab.code'"));
    }

    fn plan(os: Os, command: &Command) -> Launch {
        launch_plan(os, command, &linux_with(&["xterm", "konsole"])).unwrap()
    }

    #[test]
    fn macos_runs_the_command_in_terminal_through_osascript_with_fixed_lines() {
        let command = Command {
            terminal: r#"'/o"mp' login anthropic"#.to_owned(),
            environment: vec![],
            copy: String::new(),
        };
        let launch = plan(Os::MacOs, &command);
        assert_eq!(launch.program, PathBuf::from("/usr/bin/osascript"));
        assert_eq!(
            strings(&launch.args),
            [
                "-e",
                "tell application \"Terminal\"",
                "-e",
                "activate",
                "-e",
                r#"do script "'/o\"mp' login anthropic""#,
                "-e",
                "end tell"
            ]
        );
        assert!(!launch.new_console);
    }

    #[test]
    fn windows_runs_powershell_51_in_a_new_console_with_the_variables() {
        let command = login(Os::Windows, r"C:\omp.exe", &certificates(r"C:\b.pem"));
        let launch = plan(Os::Windows, &command);
        assert_eq!(launch.program, PathBuf::from("powershell.exe"));
        assert_eq!(
            strings(&launch.args),
            [
                "-NoExit",
                "-NoProfile",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                r"& 'C:\omp.exe' login anthropic"
            ]
        );
        assert_eq!(launch.environment, command.environment);
        assert!(launch.new_console);
    }

    #[test]
    fn linux_uses_the_first_emulator_of_the_list_that_exists() {
        let command = login(Os::Linux, "/home/a/omp", &vec![]);
        let launch = plan(Os::Linux, &command);
        assert_eq!(launch.program, PathBuf::from("/usr/bin/konsole"));
        assert_eq!(
            strings(&launch.args),
            [
                "-e",
                "sh",
                "-c",
                "'/home/a/omp' login anthropic; printf \"\\nPress Enter to close\"; read _"
            ]
        );
        assert!(launch.environment.is_empty());
        for (name, flag) in LINUX_TERMINALS {
            let only = [name];
            let launch = launch_plan(Os::Linux, &command, &linux_with(&only)).unwrap();
            assert_eq!(launch.program, PathBuf::from("/usr/bin").join(name));
            assert_eq!(strings(&launch.args)[0], flag);
        }
    }

    #[test]
    fn linux_with_certificates_starts_the_script_with_the_exports() {
        let command = login(Os::Linux, "/home/a/omp", &certificates("/d/b.pem"));
        let launch = plan(Os::Linux, &command);
        assert_eq!(
            strings(&launch.args)[3],
            "export NODE_EXTRA_CA_CERTS='/d/b.pem' SSL_CERT_FILE='/d/b.pem' \
             PI_REQUEST_COMPRESSION='0'; '/home/a/omp' login anthropic; \
             printf \"\\nPress Enter to close\"; read _"
        );
    }

    #[test]
    fn the_plain_command_has_no_variable_marker_or_pipe() {
        let marker = Path::new("/m/ab.code");
        let unix = plain_command(Os::Linux, Task::Installer { marker });
        assert_eq!(
            unix,
            "curl -fsSL https://omp.sh/install -o install.sh && sh install.sh"
        );
        let windows = plain_command(Os::Windows, Task::Installer { marker });
        assert!(windows.starts_with("Invoke-RestMethod https://omp.sh/install.ps1 -OutFile "));
        assert!(!windows.contains("iex"));
        let login = Task::Login {
            omp: Path::new("/omp"),
            provider: anthropic(),
        };
        assert_eq!(plain_command(Os::MacOs, login), "omp login anthropic");
    }

    #[test]
    fn linux_without_a_terminal_cannot_open_one() {
        let command = login(Os::Linux, "/omp", &vec![]);
        assert_eq!(
            launch_plan(Os::Linux, &command, &|_| None),
            Err(TerminalError::NoTerminal)
        );
    }

    /// The installer and the variables, run for real by `/bin/sh` with a `curl` that is a script.
    #[cfg(unix)]
    mod run_posix {
        use std::os::unix::fs::PermissionsExt;
        use std::process::Command as Process;

        use super::*;

        struct Rig {
            _root: tempfile::TempDir,
            fake_bin: PathBuf,
            tmp: PathBuf,
            marker: PathBuf,
            curl_args: PathBuf,
            seen: PathBuf,
        }

        impl Rig {
            fn new() -> Self {
                let root = tempfile::tempdir().unwrap();
                let base = root.path();
                let fake_bin = base.join("bin");
                let tmp = base.join("o'neil tmp");
                std::fs::create_dir_all(&fake_bin).unwrap();
                std::fs::create_dir_all(&tmp).unwrap();
                std::fs::create_dir_all(base.join("omp-install")).unwrap();
                let curl_args = base.join("curl-args");
                let curl = fake_bin.join("curl");
                // Every call is recorded. A call for the official installer fails with
                // FAKE_CURL_EXIT when it is not 0, else writes FAKE_INSTALLER (a shell script)
                // where `-o` says; a call for GitHub is blocked (curl exit 6, no host).
                std::fs::write(
                    &curl,
                    format!(
                        "#!/bin/sh\nprintf '%s ' \"$@\" >> '{}'\nprintf '\\n' >> '{}'\n\
                         case \"$*\" in *omp.sh*) ;; *) exit 6;; esac\n\
                         [ \"${{FAKE_CURL_EXIT:-0}}\" -ne 0 ] && exit \"$FAKE_CURL_EXIT\"\n\
                         while [ $# -gt 0 ]; do\n\
                         if [ \"$1\" = -o ]; then printf '%s\\n' \"$FAKE_INSTALLER\" > \"$2\"; fi\n\
                         shift\ndone\n",
                        curl_args.display(),
                        curl_args.display()
                    ),
                )
                .unwrap();
                std::fs::set_permissions(&curl, std::fs::Permissions::from_mode(0o755)).unwrap();
                Rig {
                    marker: base.join("omp-install").join("ab12.code"),
                    seen: base.join("seen"),
                    _root: root,
                    fake_bin,
                    tmp,
                    curl_args,
                }
            }

            /// Runs `text` as the terminal would (`sh -c`), with the fake `curl`.
            fn run(&self, text: &str, curl_exit: &str, installer: &str) {
                let path = format!("{}:/usr/bin:/bin", self.fake_bin.display());
                let status = Process::new("/bin/sh")
                    .args(["-c", text])
                    .env("PATH", path)
                    .env("TMPDIR", &self.tmp)
                    // GitHub's default folder is made under HOME, never the real one.
                    .env("HOME", self._root.path().join("home"))
                    .env_remove("PI_INSTALL_DIR")
                    .env("FAKE_CURL_EXIT", curl_exit)
                    .env("FAKE_INSTALLER", installer)
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .status()
                    .unwrap();
                assert!(status.success());
            }

            /// The URLs curl was called with, one call per line.
            fn curl_calls(&self) -> Vec<String> {
                std::fs::read_to_string(&self.curl_args)
                    .unwrap()
                    .lines()
                    .map(str::to_owned)
                    .collect()
            }

            fn code(&self) -> String {
                std::fs::read_to_string(&self.marker).unwrap()
            }

            fn leftovers(&self) -> usize {
                std::fs::read_dir(&self.tmp).unwrap().count()
            }
        }

        fn installer_text(rig: &Rig) -> String {
            install(Os::Linux, rig.marker.to_str().unwrap(), &vec![]).terminal
        }

        #[test]
        fn a_good_installer_leaves_zero_and_never_asks_github() {
            let rig = Rig::new();
            rig.run(&installer_text(&rig), "0", "exit 0");
            assert_eq!(rig.code(), "0\n");
            assert_eq!(rig.leftovers(), 0);
            let calls = rig.curl_calls();
            assert_eq!(calls.len(), 1, "{calls:?}");
            assert!(
                calls[0].starts_with("-fsSL https://omp.sh/install -o "),
                "{calls:?}"
            );
        }

        #[test]
        fn a_failed_download_of_the_installer_falls_back_on_github_and_leaves_its_code() {
            let rig = Rig::new();
            rig.run(
                &installer_text(&rig),
                "22",
                &format!("touch '{}'", rig.seen.display()),
            );
            // The official installer never ran; GitHub was asked and blocked (`90`).
            assert_eq!(rig.code(), "90\n");
            assert!(!rig.seen.exists());
            assert_eq!(rig.leftovers(), 0);
            let calls = rig.curl_calls();
            assert_eq!(calls.len(), 2, "{calls:?}");
            assert!(
                calls[1].contains("https://api.github.com/repos/can1357/oh-my-pi/releases/latest")
            );
            // The default folder of the official installer was made and holds no temporary file.
            let folder = rig._root.path().join("home").join(".local").join("bin");
            assert!(folder.is_dir());
            assert_eq!(std::fs::read_dir(folder).unwrap().count(), 0);
        }

        #[test]
        fn a_failed_installer_is_followed_by_github_and_the_marker_holds_the_last_code() {
            let rig = Rig::new();
            rig.run(&installer_text(&rig), "0", "exit 7");
            // The installer ran and failed with 7; GitHub was blocked, and its code is the last.
            assert_eq!(rig.code(), "90\n");
            assert_eq!(rig.curl_calls().len(), 2);
            assert_eq!(rig.leftovers(), 0);
        }

        #[test]
        fn the_variables_reach_the_installer_through_env_and_sh() {
            let rig = Rig::new();
            let bundle = "/d/o'neil dir/bundle.pem";
            let command = install(
                Os::MacOs,
                rig.marker.to_str().unwrap(),
                &certificates(bundle),
            );
            rig.run(
                &command.terminal,
                "0",
                &format!(
                    "printf '%s|%s' \"$SSL_CERT_FILE\" \"$PI_REQUEST_COMPRESSION\" > '{}'",
                    rig.seen.display()
                ),
            );
            assert_eq!(
                std::fs::read_to_string(&rig.seen).unwrap(),
                format!("{bundle}|0")
            );
            assert_eq!(rig.code(), "0\n");
            // What is copied runs the same way from any shell.
            std::fs::remove_file(&rig.seen).unwrap();
            rig.run(
                &command.copy,
                "0",
                &format!(
                    "printf '%s' \"$NODE_EXTRA_CA_CERTS\" > '{}'",
                    rig.seen.display()
                ),
            );
            assert_eq!(std::fs::read_to_string(&rig.seen).unwrap(), bundle);
        }

        #[test]
        fn linux_exports_reach_the_login_command() {
            let rig = Rig::new();
            let omp = rig.fake_bin.join("omp");
            std::fs::write(
                &omp,
                format!(
                    "#!/bin/sh\nprintf '%s %s' \"$SSL_CERT_FILE\" \"$*\" > '{}'\n",
                    rig.seen.display()
                ),
            )
            .unwrap();
            std::fs::set_permissions(&omp, std::fs::Permissions::from_mode(0o755)).unwrap();
            let command = login(Os::Linux, omp.to_str().unwrap(), &certificates("/d/b.pem"));
            rig.run(&command.terminal, "0", "");
            assert_eq!(
                std::fs::read_to_string(&rig.seen).unwrap(),
                "/d/b.pem login anthropic"
            );
        }
    }
}

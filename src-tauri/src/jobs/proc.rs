//! One omp process: spawned in a group of its own, with its streams on the JSON-RPC connection,
//! its last stderr lines kept in memory, and the ordered shutdown of `requirements/tech#job`.
//!
//! The thread that spawns the child also waits for it: `PR_SET_PDEATHSIG` is tied to the spawning
//! thread (Linux), which must therefore live as long as the child.

use std::collections::VecDeque;
use std::ffi::OsString;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc as std_mpsc, Arc, Mutex};
use std::time::Duration;

use serde_json::json;
use tokio::sync::{mpsc, watch};
use weaver_core::omp::failure;
use weaver_core::process_group::ProcessGroup;

use super::rpc::{Incoming, Rpc};

/// `session/close` is given this long before the process is signalled.
pub const CLOSE_LIMIT: Duration = Duration::from_secs(2);
/// After `SIGTERM` the group has this long before `SIGKILL`.
pub const TERM_LIMIT: Duration = Duration::from_secs(3);

const TAIL_LINES: usize = 40;
const TAIL_BYTES: usize = 4096;

/// What to run.
#[derive(Debug, Clone)]
pub struct SpawnSpec {
    pub program: PathBuf,
    pub args: Vec<String>,
    /// The whole environment of the process (nothing is inherited).
    pub env: Vec<(OsString, OsString)>,
    pub cwd: PathBuf,
    /// For the names of the threads.
    pub name: String,
}

/// How the process ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExitInfo {
    pub code: Option<i32>,
}

/// The last lines omp wrote on stderr: at most 40 lines and 4 KB, in memory only.
#[derive(Debug, Default)]
pub struct StderrTail {
    lines: VecDeque<String>,
    bytes: usize,
}

impl StderrTail {
    pub fn push(&mut self, line: &str) {
        let line = line.trim_end();
        if line.is_empty() {
            return;
        }
        self.bytes += line.len() + 1;
        self.lines.push_back(line.to_owned());
        while self.lines.len() > TAIL_LINES || (self.bytes > TAIL_BYTES && self.lines.len() > 1) {
            if let Some(dropped) = self.lines.pop_front() {
                self.bytes -= dropped.len() + 1;
            }
        }
    }

    pub fn text(&self) -> String {
        self.lines.iter().cloned().collect::<Vec<_>>().join("\n")
    }
}

/// A running omp.
pub struct OmpProcess {
    pub rpc: Arc<Rpc>,
    pub pid: u32,
    group: Arc<ProcessGroup>,
    exit: watch::Receiver<Option<ExitInfo>>,
    stderr: Arc<Mutex<StderrTail>>,
    stderr_done: watch::Receiver<bool>,
    shutting_down: Arc<AtomicBool>,
}

struct Parts {
    stdin: std::process::ChildStdin,
    stdout: std::process::ChildStdout,
    stderr: std::process::ChildStderr,
    group: Arc<ProcessGroup>,
    pid: u32,
}

impl OmpProcess {
    /// Spawns the process. Blocking (fork and exec): call it from the blocking pool.
    pub fn spawn(
        spec: SpawnSpec,
    ) -> std::io::Result<(OmpProcess, mpsc::UnboundedReceiver<Incoming>)> {
        let (ready_tx, ready_rx) = std_mpsc::channel::<std::io::Result<Parts>>();
        let (exit_tx, exit_rx) = watch::channel(None);
        let name = spec.name.clone();
        std::thread::Builder::new()
            .name(format!("{name}-omp"))
            .spawn(move || {
                let mut command = weaver_core::process::command(&spec.program, &spec.args);
                command
                    .env_clear()
                    .envs(spec.env.iter().map(|(n, v)| (n, v)))
                    .current_dir(&spec.cwd);
                command
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped());
                match weaver_core::process_group::spawn(&mut command) {
                    Ok((mut child, group)) => {
                        let (stdin, stdout, stderr) =
                            (child.stdin.take(), child.stdout.take(), child.stderr.take());
                        let (Some(stdin), Some(stdout), Some(stderr)) = (stdin, stdout, stderr)
                        else {
                            let _ = ready_tx
                                .send(Err(std::io::Error::other("the agent has no standard streams")));
                            return;
                        };
                        let pid = child.id();
                        let group = Arc::new(group);
                        let _ = ready_tx.send(Ok(Parts {
                            stdin,
                            stdout,
                            stderr,
                            group,
                            pid,
                        }));
                        let code = child.wait().ok().and_then(|status| status.code());
                        let _ = exit_tx.send(Some(ExitInfo { code }));
                    }
                    Err(error) => {
                        let _ = ready_tx.send(Err(error));
                    }
                }
            })?;
        let parts = ready_rx
            .recv()
            .map_err(|_| std::io::Error::other("the agent could not be started"))??;

        let tail = Arc::new(Mutex::new(StderrTail::default()));
        let reader_tail = tail.clone();
        let stderr = parts.stderr;
        let (done_tx, stderr_done) = watch::channel(false);
        std::thread::Builder::new()
            .name(format!("{name}-err"))
            .spawn(move || {
                for line in BufReader::new(stderr).lines() {
                    let Ok(line) = line else { break };
                    reader_tail
                        .lock()
                        .expect("stderr tail poisoned")
                        .push(&line);
                }
                let _ = done_tx.send(true);
            })?;

        let (rpc, incoming) = Rpc::start(parts.stdout, parts.stdin, &name)?;
        Ok((
            OmpProcess {
                rpc,
                pid: parts.pid,
                group: parts.group,
                exit: exit_rx,
                stderr: tail,
                stderr_done,
                shutting_down: Arc::new(AtomicBool::new(false)),
            },
            incoming,
        ))
    }

    /// Waits (a moment at most) until stderr has ended, so that the last lines of an exit are all
    /// there. A grandchild that keeps the pipe open does not hold it back.
    pub async fn settle_stderr(&self) {
        let mut done = self.stderr_done.clone();
        let _ = tokio::time::timeout(
            Duration::from_millis(500),
            done.wait_for(|finished| *finished),
        )
        .await;
    }

    /// The last lines of stderr.
    pub fn stderr_text(&self) -> String {
        self.stderr.lock().expect("stderr tail poisoned").text()
    }

    pub fn has_exited(&self) -> bool {
        self.exit.borrow().is_some()
    }

    /// Whether Weaver asked this process to end (its exit is then not an `EXITED`).
    pub fn is_shutting_down(&self) -> bool {
        self.shutting_down.load(Ordering::SeqCst)
    }

    /// Waits until the process exited.
    pub async fn wait_exit(&self) -> ExitInfo {
        let mut exit = self.exit.clone();
        let reached = exit
            .wait_for(Option::is_some)
            .await
            .map(|value| value.clone());
        match reached {
            Ok(Some(info)) => info,
            _ => ExitInfo { code: None },
        }
    }

    /// The reason and the details of an exit Weaver did not ask for: the proxy explanation when
    /// the stderr tail is a certificate or an HTTP error, else omp's last error line, else
    /// `the agent exited with code N`; the details are the last lines. `system_certificates_on` is the
    /// setting at this moment.
    pub fn exit_reason(
        &self,
        exit: &ExitInfo,
        system_certificates_on: bool,
    ) -> (String, Option<String>) {
        exit_reason_of(&self.stderr_text(), exit.code, system_certificates_on)
    }

    /// Ends the process in order: `session/cancel` when a turn runs, `session/close` (2 s),
    /// `SIGTERM` to the group, `SIGKILL` after 3 s. A shutdown Weaver started produces no
    /// `EXITED`.
    pub async fn shutdown(&self, session: Option<&str>, turn_running: bool) {
        self.shutting_down.store(true, Ordering::SeqCst);
        if self.has_exited() {
            return;
        }
        if let Some(session) = session {
            if turn_running {
                self.rpc
                    .notify("session/cancel", json!({ "sessionId": session }));
            }
            let _ = tokio::time::timeout(
                CLOSE_LIMIT,
                self.rpc
                    .request("session/close", json!({ "sessionId": session })),
            )
            .await;
        }
        if self.has_exited() {
            return;
        }
        self.group.terminate();
        if tokio::time::timeout(TERM_LIMIT, self.wait_exit())
            .await
            .is_err()
        {
            self.group.kill();
            let _ = tokio::time::timeout(Duration::from_secs(5), self.wait_exit()).await;
        }
    }

    /// Ends the group at once, without any courtesy (Weaver is going down).
    pub fn kill_now(&self) {
        self.shutting_down.store(true, Ordering::SeqCst);
        self.group.kill();
    }

    pub fn group_pid(&self) -> u32 {
        self.group.pid()
    }
}

/// The reason and the details of an exit, from the stderr tail and the exit code.
fn exit_reason_of(
    stderr: &str,
    code: Option<i32>,
    system_certificates_on: bool,
) -> (String, Option<String>) {
    let plain = match (failure::last_line(stderr), code) {
        (Some(line), _) => line.to_owned(),
        (None, Some(code)) => format!("the agent exited with code {code}"),
        (None, None) => "the agent was stopped".to_owned(),
    };
    (
        super::session::explained(&plain, &[stderr], system_certificates_on),
        (!stderr.is_empty()).then(|| stderr.to_owned()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tail_keeps_the_last_40_lines_and_4_kb() {
        let mut tail = StderrTail::default();
        for n in 0..100 {
            tail.push(&format!("line {n}"));
        }
        let text = tail.text();
        assert_eq!(text.lines().count(), 40);
        assert!(text.starts_with("line 60") && text.ends_with("line 99"));

        let mut big = StderrTail::default();
        for n in 0..10 {
            big.push(&format!("{n}{}", "x".repeat(999)));
        }
        assert!(big.text().len() <= TAIL_BYTES, "{}", big.text().len());
        assert!(big.text().ends_with(&format!("9{}", "x".repeat(999))));
        // One line longer than the limit stays (the last line is never lost).
        let mut huge = StderrTail::default();
        huge.push(&"y".repeat(10_000));
        assert_eq!(huge.text().len(), 10_000);
    }

    #[test]
    fn blank_lines_are_not_kept() {
        let mut tail = StderrTail::default();
        tail.push("");
        tail.push("   ");
        tail.push("error: boom  ");
        assert_eq!(tail.text(), "error: boom");
    }

    #[cfg(unix)]
    fn spec(script: &str) -> SpawnSpec {
        SpawnSpec {
            program: PathBuf::from("/bin/sh"),
            args: vec!["-c".into(), script.into()],
            env: vec![(OsString::from("PATH"), OsString::from("/usr/bin:/bin"))],
            cwd: std::env::temp_dir(),
            name: "t".into(),
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn the_exit_code_and_the_last_stderr_lines_are_the_reason() {
        let (process, _incoming) = OmpProcess::spawn(spec(
            "echo 'first' >&2; echo 'error: no provider' >&2; exit 3",
        ))
        .unwrap();
        let exit = process.wait_exit().await;
        assert_eq!(exit.code, Some(3));
        // The stderr reader may lag the exit by a moment: wait for its last line.
        for _ in 0..200 {
            if process.stderr_text().contains("no provider") {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        let (reason, details) = process.exit_reason(&exit, false);
        assert_eq!(reason, "error: no provider");
        assert_eq!(details.as_deref(), Some("first\nerror: no provider"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn without_stderr_the_reason_is_the_exit_code() {
        let (process, _incoming) = OmpProcess::spawn(spec("exit 7")).unwrap();
        let exit = process.wait_exit().await;
        assert_eq!(
            process.exit_reason(&exit, true),
            ("the agent exited with code 7".to_owned(), None)
        );
    }

    #[test]
    fn a_certificate_or_http_error_in_the_stderr_tail_replaces_the_reason_of_an_exit() {
        // Not on the last line: the whole tail is tested.
        let tail =
            "fetch failed: UNABLE_TO_GET_ISSUER_CERT_LOCALLY\nat node:internal\nerror: giving up";
        let (reason, details) = exit_reason_of(tail, Some(1), false);
        assert_eq!(reason, failure::explain(tail, false).unwrap());
        assert!(
            reason.contains("turning on System Certificates"),
            "{reason}"
        );
        assert_eq!(
            details.as_deref(),
            Some(tail),
            "Show details keeps omp's lines"
        );
        let (on, _) = exit_reason_of(tail, Some(1), true);
        assert!(on.contains("even with System Certificates on"), "{on}");

        let http = "request failed: HTTP 407\nerror: giving up";
        let (reason, details) = exit_reason_of(http, None, false);
        assert!(
            reason.starts_with("the agent received an HTTP 407 error"),
            "{reason}"
        );
        assert!(
            reason.contains("turning on System Certificates"),
            "{reason}"
        );
        assert_eq!(details.as_deref(), Some(http));
        let (on, _) = exit_reason_of(http, None, true);
        assert!(!on.contains("turning on"), "{on}");
    }

    #[test]
    fn an_exit_that_is_not_a_proxy_problem_keeps_its_reason() {
        assert_eq!(
            exit_reason_of("first\nerror: no provider", Some(3), true).0,
            "error: no provider"
        );
        assert_eq!(exit_reason_of("status 404", Some(3), false).0, "status 404");
        assert_eq!(
            exit_reason_of("", Some(7), false),
            ("the agent exited with code 7".to_owned(), None)
        );
        assert_eq!(exit_reason_of("", None, false).0, "the agent was stopped");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn the_environment_is_exactly_the_one_given() {
        let mut s = spec("test -z \"$HOME\" && test \"$WEAVER_X\" = 1 && exit 0; exit 9");
        s.env
            .push((OsString::from("WEAVER_X"), OsString::from("1")));
        let (process, _incoming) = OmpProcess::spawn(s).unwrap();
        assert_eq!(
            process.wait_exit().await.code,
            Some(0),
            "nothing is inherited"
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn shutdown_ends_a_process_that_ignores_close_with_sigterm_and_is_not_an_exit_weaver_did_not_ask_for(
    ) {
        let (process, _incoming) = OmpProcess::spawn(spec("sleep 30")).unwrap();
        assert!(!process.is_shutting_down());
        process.shutdown(Some("s1"), true).await;
        assert!(process.has_exited());
        assert!(process.is_shutting_down());
        assert_eq!(process.wait_exit().await.code, None, "ended by a signal");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn shutdown_kills_what_ignores_sigterm() {
        let dir = tempfile::tempdir().unwrap();
        let marker = dir.path().join("trap-installed");
        let mut s = spec("trap '' TERM; : > \"$0\"; while true; do sleep 1; done");
        s.args.push(marker.to_string_lossy().into_owned());
        let (process, _incoming) = OmpProcess::spawn(s).unwrap();
        // The signal must come after the shell installed its trap.
        while !marker.exists() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        let started = std::time::Instant::now();
        process.shutdown(None, false).await;
        assert!(process.has_exited());
        assert!(
            started.elapsed() >= Duration::from_secs(3),
            "SIGKILL came after 3 s"
        );
    }
}

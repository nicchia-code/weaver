//! The time-limited runner of a child process, shared by omp (`omp::run`), Solo mode
//! (`solo_mode`) and Atomic commits (`history_commit`): the command runs with stdin closed (unless
//! input is given through `run_command_with_input`) and stdout and stderr captured, and the
//! process is killed when its limit passes.

use std::io::Read;
use std::process::Stdio;
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// The most stdout kept (a list of models is a few hundred KB).
const MAX_STDOUT: u64 = 16 * 1024 * 1024;
/// The most stderr kept, from its start (the last line of a longer text is still near the end of
/// what omp prints in practice).
const MAX_STDERR: u64 = 256 * 1024;
/// How long to wait for the pipes to close after the process was killed.
const DRAIN_AFTER_KILL: Duration = Duration::from_secs(1);
const POLL: Duration = Duration::from_millis(15);

/// What a command that ended with success printed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Output {
    pub stdout: String,
    pub stderr: String,
}

/// A command that did not complete normally.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failure {
    /// The program could not be started.
    NotStarted,
    /// The limit passed; the process was killed.
    TimedOut { limit: Duration },
    /// The process ended with a failure status (`code` is `None` when a signal ended it).
    Exit { code: Option<i32>, stderr: String },
}

/// Runs an already built command (stdin closed, stdout and stderr captured) for at most `limit`.
pub fn run_command(command: std::process::Command, limit: Duration) -> Result<Output, Failure> {
    run(command, limit, None)
}

/// Like [`run_command`], with `input` written to the process's standard input and then closed.
pub fn run_command_with_input(
    command: std::process::Command,
    limit: Duration,
    input: &[u8],
) -> Result<Output, Failure> {
    run(command, limit, Some(input.to_vec()))
}

fn run(
    mut command: std::process::Command,
    limit: Duration,
    input: Option<Vec<u8>>,
) -> Result<Output, Failure> {
    command
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().map_err(|_| Failure::NotStarted)?;
    if let (Some(input), Some(mut stdin)) = (input, child.stdin.take()) {
        // Written on its own thread: a long input must not block while the output is unread, and
        // a process that ends early only closes the pipe.
        std::thread::spawn(move || {
            let _ = std::io::Write::write_all(&mut stdin, &input);
        });
    }
    let stdout = child.stdout.take().map(|pipe| drain(pipe, MAX_STDOUT));
    let stderr = child.stderr.take().map(|pipe| drain(pipe, MAX_STDERR));

    let deadline = Instant::now() + limit;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) if Instant::now() >= deadline => break None,
            Ok(None) => std::thread::sleep(POLL),
            Err(_) => break None,
        }
    };
    let Some(status) = status else {
        let _ = child.kill();
        let _ = child.wait();
        return Err(Failure::TimedOut { limit });
    };

    let stdout = collect(stdout);
    let stderr = collect(stderr);
    if status.success() {
        Ok(Output { stdout, stderr })
    } else {
        Err(Failure::Exit {
            code: status.code(),
            stderr,
        })
    }
}

/// Reads a pipe to its end in a thread; the receiver gets the text once the pipe closes.
fn drain<R: Read + Send + 'static>(pipe: R, cap: u64) -> mpsc::Receiver<String> {
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = pipe.take(cap).read_to_end(&mut bytes);
        let _ = sender.send(String::from_utf8_lossy(&bytes).into_owned());
    });
    receiver
}

/// The text of a drained pipe; a grandchild that keeps the pipe open cannot block Weaver.
fn collect(receiver: Option<mpsc::Receiver<String>>) -> String {
    receiver
        .and_then(|r| r.recv_timeout(DRAIN_AFTER_KILL).ok())
        .unwrap_or_default()
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    fn sh(script: &str) -> std::process::Command {
        crate::process::command("/bin/sh", ["-c", script])
    }

    #[test]
    fn success_returns_stdout_and_stderr() {
        let output = run_command(sh("echo out; echo err >&2"), Duration::from_secs(10)).unwrap();
        assert_eq!(output.stdout, "out\n");
        assert_eq!(output.stderr, "err\n");
    }

    #[test]
    fn a_failure_status_carries_the_code_and_stderr() {
        let failure =
            run_command(sh("echo broken >&2; exit 7"), Duration::from_secs(10)).unwrap_err();
        assert_eq!(
            failure,
            Failure::Exit {
                code: Some(7),
                stderr: "broken\n".to_owned()
            }
        );
    }

    #[test]
    fn a_program_that_does_not_exist_is_not_started() {
        let failure = run_command(
            crate::process::command("/nonexistent/omp", ["x"]),
            Duration::from_secs(10),
        )
        .unwrap_err();
        assert_eq!(failure, Failure::NotStarted);
    }

    #[test]
    fn a_process_over_its_limit_is_killed() {
        let started = Instant::now();
        let failure = run_command(sh("sleep 30"), Duration::from_millis(100)).unwrap_err();
        assert_eq!(
            failure,
            Failure::TimedOut {
                limit: Duration::from_millis(100)
            }
        );
        assert!(started.elapsed() < Duration::from_secs(10));
    }
}

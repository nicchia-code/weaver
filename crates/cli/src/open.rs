//! `weaver open [<folder>]`: asks the running Weaver to open a folder as a project
//! (`developer.OPEN_PROJECT`) or to show its welcome window (`developer.SHOW_WELCOME`)
//! (`requirements/cli#developer.OPEN_PROJECT`).
//!
//! It does not use the request kind `command`: the folder need not be a project, and nothing is
//! written in it. The request is its own kind ([`weaver_core::open::KIND`]), sent to the app that
//! is already running; the app is never started. Everything that can be refused is refused before
//! the app is contacted: an agent (`WEAVER_JOB`, `WEAVER_GENERAL` or `WEAVER_ASK` is set), a folder
//! that does not exist or is not a folder.

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use weaver_core::command::code;
use weaver_core::ipc::{self, SendError};
use weaver_core::open::{
    OpenRequest, AGENT_REFUSAL, ANSWER_LIMIT_SECONDS, KIND, NOT_RUNNING_MESSAGE, NO_ANSWER_MESSAGE,
    UNSUPPORTED, UNSUPPORTED_MESSAGE,
};
use weaver_core::paths::{self, AppDirs};
use weaver_core::protocol::{ProtocolError, Request, Response};

use crate::cli::Environment;
use crate::client::{self, Connection};
use crate::spec::Failure;

/// How long the command waits for the app to answer.
pub const ANSWER_LIMIT: Duration = Duration::from_secs(ANSWER_LIMIT_SECONDS);

/// The folder of the command: the word joined to the current folder (an absolute word replaces
/// it) and made canonical, as the identity of the project. A folder without `.weaver/` is fine.
pub fn canonical_folder(word: &str, cwd: &Path) -> Result<PathBuf, Failure> {
    let wanted = cwd.join(word);
    match paths::canonical_root(&wanted) {
        Ok(canonical) if canonical.is_dir() => Ok(canonical),
        Ok(canonical) => Err(Failure::refused(format!(
            "`{}` is not a folder.",
            canonical.display()
        ))),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Err(Failure::refused(
            format!("`{}` does not exist.", wanted.display()),
        )),
        Err(error) => Err(Failure::refused(format!(
            "cannot use the folder `{}`: {error}",
            wanted.display()
        ))),
    }
}

/// The failure of no app answering.
fn not_running() -> Failure {
    Failure::new(code::APP_NOT_RUNNING, NOT_RUNNING_MESSAGE)
}

/// Runs `weaver open` for `word` (the folder as typed, or none for the welcome window): refuses an
/// agent, checks the folder, then asks the running app and waits at most `limit` for its answer.
/// Nothing is sent when anything is refused.
pub fn run(
    word: Option<&str>,
    env: &Environment,
    dirs: Option<&AppDirs>,
    cwd: &Path,
    limit: Duration,
) -> Result<(), Failure> {
    if env.job.is_some() || env.general.is_some() || env.ask.is_some() {
        return Err(Failure::refused(AGENT_REFUSAL));
    }
    let path = word
        .map(|word| canonical_folder(word, cwd))
        .transpose()?
        .map(|folder| folder.to_string_lossy().into_owned());
    // The app is never started: no folders, no record or nobody listening is an app not running.
    let Some(dirs) = dirs else {
        return Err(not_running());
    };
    let Connection { stream, secret } = client::open(dirs, None).map_err(|_| not_running())?;
    let payload = serde_json::to_value(OpenRequest::new(path))
        .map_err(|e| Failure::failed(format!("cannot encode the request: {e}")))?;
    let request = Request::new(secret, KIND, payload);

    let (reply, wait) = mpsc::channel();
    thread::spawn(move || {
        let _ = reply.send(ipc::exchange(stream, &request));
    });
    match wait.recv_timeout(limit) {
        Ok(answer) => outcome(answer),
        // The thread stays with the connection until the process ends.
        Err(_) => Err(Failure::failed(NO_ANSWER_MESSAGE)),
    }
}

/// What the answer of the app, or its loss, means for the command.
fn outcome(answer: Result<Response, SendError>) -> Result<(), Failure> {
    match answer {
        Ok(response) => match response.error {
            Some(error) if error.code == UNSUPPORTED => {
                Err(Failure::failed(UNSUPPORTED_MESSAGE))
            }
            Some(error) => Err(Failure::new(&error.code, error.message)),
            None => Ok(()),
        },
        // An app that speaks another version of the protocol cannot have the kind.
        Err(SendError::BadReply(ProtocolError::VersionMismatch { .. })) => {
            Err(Failure::failed(UNSUPPORTED_MESSAGE))
        }
        Err(SendError::NoReply) => Err(Failure::failed(
            "Weaver closed the connection without an answer; the folder may still open.",
        )),
        Err(SendError::BadReply(error)) => Err(Failure::failed(format!(
            "Weaver sent an answer that cannot be read: {error}"
        ))),
        Err(error) => Err(Failure::failed(format!("cannot reach Weaver: {error}"))),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use weaver_core::endpoint::{new_endpoint, new_secret};
    use weaver_core::ipc::{serve_connection, Listener, Served};
    use weaver_core::protocol::ErrorBody;
    use weaver_core::runtime::{self, RuntimeInfo};

    use super::*;

    const PATIENCE: Duration = Duration::from_secs(10);

    struct Sandbox {
        _base: tempfile::TempDir,
        cwd: PathBuf,
        dirs: AppDirs,
    }

    fn sandbox() -> Sandbox {
        let base = tempfile::tempdir().unwrap();
        // As the command makes it canonical: without the `\\?\` prefix of Windows.
        let cwd = paths::canonical_root(base.path()).unwrap().join("cwd");
        fs::create_dir_all(&cwd).unwrap();
        let dirs = AppDirs::from_bases(
            &base.path().join("config"),
            &base.path().join("data"),
            Some(&base.path().join("run")),
        );
        fs::create_dir_all(base.path().join("run")).unwrap();
        fs::create_dir_all(dirs.local_data_dir()).unwrap();
        Sandbox {
            _base: base,
            cwd,
            dirs,
        }
    }

    /// An app that answers one request with what `answer` makes of it; the request is returned.
    fn app(dirs: &AppDirs, answer: impl FnOnce(&Request) -> Option<Response> + Send + 'static) -> thread::JoinHandle<Request> {
        let endpoint = new_endpoint(dirs).unwrap();
        let secret = new_secret();
        let listener = Listener::bind(&endpoint).unwrap();
        runtime::write(
            dirs,
            &RuntimeInfo {
                endpoint,
                secret: secret.clone(),
                pid: std::process::id(),
                version: "test".into(),
            },
        )
        .unwrap();
        thread::spawn(move || {
            let stream = listener.accept().unwrap();
            let mut seen = None;
            let served = serve_connection(stream, &secret, |request| {
                seen = Some(request.clone());
                answer(&request).unwrap_or_else(|| Response::failure("never", "no answer"))
            })
            .unwrap();
            assert!(matches!(served, Served::Handled));
            seen.unwrap()
        })
    }

    fn ok(dirs: &AppDirs) -> thread::JoinHandle<Request> {
        app(dirs, |_| Some(Response::ok(serde_json::json!({}))))
    }

    #[test]
    fn a_folder_is_joined_to_the_current_folder_and_made_canonical() {
        let s = sandbox();
        fs::create_dir_all(s.cwd.join("shop").join("src")).unwrap();
        let found = canonical_folder("shop/../shop/src", &s.cwd).unwrap();
        assert_eq!(found, s.cwd.join("shop").join("src"));
        let absolute = canonical_folder(s.cwd.join("shop").to_str().unwrap(), Path::new("/nowhere"));
        assert_eq!(absolute.unwrap(), s.cwd.join("shop"));
        // A folder named like an object is a folder.
        fs::create_dir(s.cwd.join("model")).unwrap();
        assert_eq!(canonical_folder("model", &s.cwd).unwrap(), s.cwd.join("model"));
    }

    #[cfg(unix)]
    #[test]
    fn a_link_to_a_folder_is_resolved() {
        let s = sandbox();
        fs::create_dir(s.cwd.join("real")).unwrap();
        std::os::unix::fs::symlink(s.cwd.join("real"), s.cwd.join("link")).unwrap();
        assert_eq!(canonical_folder("link", &s.cwd).unwrap(), s.cwd.join("real"));
    }

    #[test]
    fn a_folder_that_does_not_exist_or_is_a_file_is_refused_with_its_path() {
        let s = sandbox();
        let missing = canonical_folder("nope", &s.cwd).unwrap_err();
        assert_eq!(missing.code, code::REFUSED);
        assert_eq!(missing.message, format!("`{}` does not exist.", s.cwd.join("nope").display()));
        fs::write(s.cwd.join("file.txt"), "x").unwrap();
        let file = canonical_folder("file.txt", &s.cwd).unwrap_err();
        assert_eq!(file.code, code::REFUSED);
        assert_eq!(file.message, format!("`{}` is not a folder.", s.cwd.join("file.txt").display()));
    }

    #[test]
    fn an_agent_is_refused_before_anything_is_checked_or_sent() {
        let s = sandbox();
        for env in [
            Environment { job: Some("j".into()), ..Environment::default() },
            Environment { general: Some("g".into()), ..Environment::default() },
            Environment { ask: Some("a".into()), ..Environment::default() },
        ] {
            let failure = run(Some("nope"), &env, Some(&s.dirs), &s.cwd, PATIENCE).unwrap_err();
            assert_eq!(failure.code, code::REFUSED);
            assert_eq!(failure.message, AGENT_REFUSAL);
            assert_eq!(failure.exit_code(), 1);
        }
    }

    #[test]
    fn no_app_is_app_not_running_and_a_missing_folder_is_told_without_contacting_it() {
        let s = sandbox();
        let env = Environment::default();
        let failure = run(None, &env, Some(&s.dirs), &s.cwd, PATIENCE).unwrap_err();
        assert_eq!(failure.code, code::APP_NOT_RUNNING);
        assert_eq!(failure.message, "`weaver open` needs Weaver, and Weaver is not running.");
        let failure = run(None, &env, None, &s.cwd, PATIENCE).unwrap_err();
        assert_eq!(failure.code, code::APP_NOT_RUNNING);

        // Even with an app listening, a folder that is not there is told by the command alone.
        let listening = ok(&s.dirs);
        let failure = run(Some("nope"), &env, Some(&s.dirs), &s.cwd, PATIENCE).unwrap_err();
        assert!(failure.message.ends_with("does not exist."));
        drop(listening);
    }

    #[test]
    fn a_folder_is_sent_as_the_canonical_path_and_the_welcome_window_with_none() {
        let s = sandbox();
        fs::create_dir(s.cwd.join("shop")).unwrap();
        let server = ok(&s.dirs);
        run(Some("shop"), &Environment::default(), Some(&s.dirs), &s.cwd, PATIENCE).unwrap();
        let request = server.join().unwrap();
        assert_eq!(request.kind, "open");
        assert_eq!(request.job, None);
        assert_eq!(request.general, None);
        assert_eq!(
            request.payload,
            serde_json::json!({ "path": s.cwd.join("shop").to_string_lossy() })
        );

        let server = ok(&s.dirs);
        run(None, &Environment::default(), Some(&s.dirs), &s.cwd, PATIENCE).unwrap();
        assert_eq!(server.join().unwrap().payload, serde_json::json!({}));
    }

    #[test]
    fn the_reason_of_the_app_is_the_message_and_unsupported_asks_for_an_update() {
        let s = sandbox();
        let refused = app(&s.dirs, |_| {
            Some(Response {
                version: weaver_core::protocol::PROTOCOL_VERSION,
                payload: serde_json::Value::Null,
                error: Some(ErrorBody { code: "refused".into(), message: "the folder does not exist".into() }),
            })
        });
        let failure = run(None, &Environment::default(), Some(&s.dirs), &s.cwd, PATIENCE).unwrap_err();
        refused.join().unwrap();
        assert_eq!(failure.code, code::REFUSED);
        assert_eq!(failure.message, "the folder does not exist");

        let old = app(&s.dirs, |request| Some(Response::failure("unsupported", format!("unknown request kind `{}`", request.kind))));
        let failure = run(None, &Environment::default(), Some(&s.dirs), &s.cwd, PATIENCE).unwrap_err();
        old.join().unwrap();
        assert_eq!(failure.message, "This version of Weaver cannot open folders from the command line; update Weaver.");
        assert_eq!(failure.exit_code(), 1);
    }

    #[test]
    fn an_app_that_does_not_answer_in_time_is_told_with_the_limit() {
        let s = sandbox();
        let (release, held) = mpsc::channel::<()>();
        let server = app(&s.dirs, move |_| {
            let _ = held.recv_timeout(PATIENCE);
            None
        });
        let failure = run(None, &Environment::default(), Some(&s.dirs), &s.cwd, Duration::from_millis(200)).unwrap_err();
        assert_eq!(failure.message, "Weaver did not answer within 30 s; the folder may still open.");
        assert_eq!(failure.exit_code(), 1);
        drop(release);
        let _ = server.join();
    }

    #[test]
    fn the_limit_is_thirty_seconds() {
        assert_eq!(ANSWER_LIMIT, Duration::from_secs(30));
    }
}

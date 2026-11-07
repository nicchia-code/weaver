//! `weaver question new` against a fake app: every outcome with its exit code, a reply that comes
//! late (no time limit anywhere), and a stop (SIGINT, SIGTERM, a closed standard output) that
//! closes the connection.
//!
//! The real `weaver` binary runs with its own `HOME`, so no test touches the developer's folders.
//! Unix only: on Windows the folders come from the Known Folder API, which no variable redirects,
//! and the signals are Unix signals.
#![cfg(unix)]

use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use serde_json::{json, Value};
use weaver_core::endpoint::{new_endpoint, new_secret};
use weaver_core::ipc::{serve_connection_watched, Listener, Peer};
use weaver_core::paths::AppDirs;
use weaver_core::protocol::{Request, Response};
use weaver_core::runtime::{self, RuntimeInfo};

const SECRET_JOB: &str = "abcd1234";
const SECRET_CHAT: &str = "efgh5678";
/// Long enough for any real stop to arrive, short enough for a test.
const PATIENCE: Duration = Duration::from_secs(20);

struct Sandbox {
    _base: tempfile::TempDir,
    home: PathBuf,
    root: PathBuf,
    dirs: AppDirs,
}

fn sandbox() -> Sandbox {
    let base = tempfile::tempdir().unwrap();
    let home = base.path().join("home");
    let root = base.path().join("project");
    std::fs::create_dir_all(root.join(".weaver")).unwrap();
    std::fs::create_dir_all(home.join("run")).unwrap();
    let dirs = if cfg!(target_os = "macos") {
        let support = home.join("Library/Application Support");
        AppDirs::from_bases(&support, &support, Some(&home.join("run")))
    } else {
        AppDirs::from_bases(
            &home.join("config"),
            &home.join("data"),
            Some(&home.join("run")),
        )
    };
    Sandbox {
        _base: base,
        home,
        root,
        dirs,
    }
}

fn round() -> Value {
    json!({ "questions": [{
        "id": "storage",
        "question": "Where should the data live?",
        "options": [
            { "label": "Disk", "description": "A file." },
            { "label": "Memory", "description": "Gone at exit." },
        ],
        "recommended": 0,
        "multi": false,
    }]})
}

fn answered() -> Value {
    json!({ "status": "answered", "answers": { "storage": { "status": "answered", "selected": ["Disk"], "text": "Because." } } })
}

/// What the fake app saw and what it does with a round.
struct App {
    endpoint: String,
    /// The request, as soon as it arrived.
    seen: mpsc::Receiver<Request>,
    /// Fires when the client closed the connection.
    closed: mpsc::Receiver<()>,
    done: thread::JoinHandle<()>,
}

/// Starts an app that serves one connection: `decide` runs with the request and the peer, and its
/// response is the reply.
fn app(
    sandbox: &Sandbox,
    decide: impl FnOnce(&Request, &Peer) -> Option<Response> + Send + 'static,
) -> App {
    let endpoint = new_endpoint(&sandbox.dirs).unwrap();
    let secret = new_secret();
    runtime::write(
        &sandbox.dirs,
        &RuntimeInfo {
            endpoint: endpoint.clone(),
            secret: secret.clone(),
            pid: 1,
            version: "0.1.0".into(),
        },
    )
    .unwrap();
    let listener = Listener::bind(&endpoint).unwrap();
    let (seen_tx, seen) = mpsc::channel();
    let (closed_tx, closed) = mpsc::channel();
    let done = thread::spawn(move || {
        let stream = listener.accept().unwrap();
        let _ = serve_connection_watched(stream, &secret, |request, peer| {
            peer.on_close(move || {
                let _ = closed_tx.send(());
            });
            let _ = seen_tx.send(request.clone());
            // `None`: nobody is left to read the reply, so what it is does not matter.
            decide(&request, &peer)
                .unwrap_or_else(|| Response::failure("dropped", "the client left"))
        });
    });
    App {
        endpoint,
        seen,
        closed,
        done,
    }
}

fn weaver(sandbox: &Sandbox, env: &[(&str, &str)]) -> Child {
    let mut command = Command::new(env!("CARGO_BIN_EXE_weaver-cli"));
    command
        .args(["question", "new", "--input", "-", "--root"])
        .arg(&sandbox.root)
        .env_clear()
        .env("HOME", &sandbox.home)
        .env("XDG_CONFIG_HOME", sandbox.home.join("config"))
        .env("XDG_DATA_HOME", sandbox.home.join("data"))
        .env("XDG_RUNTIME_DIR", sandbox.home.join("run"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (name, value) in env {
        command.env(name, value);
    }
    command.spawn().unwrap()
}

fn give_input(child: &mut Child, input: &Value) {
    let mut stdin = child.stdin.take().unwrap();
    stdin.write_all(input.to_string().as_bytes()).unwrap();
}

struct Result {
    exit: Option<i32>,
    stdout: String,
    stderr: String,
}

fn finish(child: Child) -> Result {
    let output = child.wait_with_output().unwrap();
    Result {
        exit: output.status.code(),
        stdout: String::from_utf8(output.stdout).unwrap(),
        stderr: String::from_utf8(output.stderr).unwrap(),
    }
}

impl Result {
    /// Exactly one JSON document on stdout.
    fn json(&self) -> Value {
        assert_eq!(
            self.stdout.trim_end().lines().count(),
            1,
            "stdout must be one document: {:?}",
            self.stdout
        );
        serde_json::from_str(&self.stdout).unwrap()
    }
}

fn run(sandbox: &Sandbox, env: &[(&str, &str)], input: &Value) -> Result {
    let mut child = weaver(sandbox, env);
    give_input(&mut child, input);
    finish(child)
}

fn in_job(endpoint: &str) -> Vec<(&str, &str)> {
    vec![("WEAVER_JOB", SECRET_JOB), ("WEAVER_ENDPOINT", endpoint)]
}

fn in_chat(endpoint: &str) -> Vec<(&str, &str)> {
    vec![("WEAVER_GENERAL", SECRET_CHAT), ("WEAVER_ENDPOINT", endpoint)]
}

fn assert_unavailable(result: &Result) {
    assert_eq!(result.exit, Some(0), "{} {}", result.stdout, result.stderr);
    let found = result.json();
    assert_eq!(found["status"], "unavailable");
    assert_eq!(found["reason"], "Weaver cannot show questions for this command. Do not ask in any other way: stop and tell the developer.");
}

fn published() -> Value {
    json!({ "status": "published", "card": "abcd1234", "message": "End your turn now." })
}

#[test]
fn the_chat_publishes_a_card_with_its_identity_and_no_job_and_the_command_returns_at_once() {
    let sandbox = sandbox();
    let app = app(&sandbox, |_, _| Some(Response::ok(published())));
    let result = run(&sandbox, &in_chat(&app.endpoint.clone()), &round());
    assert_eq!((result.exit, result.json()), (Some(0), published()));

    let request = app.seen.recv_timeout(PATIENCE).unwrap();
    assert_eq!(request.kind, "question.new");
    assert_eq!(request.job, None);
    assert_eq!(request.general.as_deref(), Some(SECRET_CHAT));
    assert_eq!(request.payload, round());
    app.done.join().unwrap();
}

#[test]
fn a_second_card_in_the_turn_is_exit_one_with_its_code_and_publishes_nothing() {
    let sandbox = sandbox();
    let app = app(&sandbox, |_, _| {
        Some(Response::failure(
            "card-already-published",
            "Put every question of the turn in one card.",
        ))
    });
    let result = run(&sandbox, &in_chat(&app.endpoint.clone()), &round());
    assert_eq!(result.exit, Some(1));
    let found = result.json();
    assert_eq!(found["error"]["code"], "card-already-published");
    assert_eq!(
        found["error"]["message"],
        "Put every question of the turn in one card."
    );
    assert!(found.get("status").is_none() && found.get("card").is_none());
}

#[test]
fn a_caller_with_both_identities_sends_both() {
    let sandbox = sandbox();
    let app = app(&sandbox, |_, _| Some(Response::ok(answered())));
    let endpoint = app.endpoint.clone();
    let mut env = in_job(&endpoint);
    env.push(("WEAVER_GENERAL", SECRET_CHAT));
    assert_eq!(run(&sandbox, &env, &round()).exit, Some(0));

    let request = app.seen.recv_timeout(PATIENCE).unwrap();
    assert_eq!(request.job.as_deref(), Some(SECRET_JOB));
    assert_eq!(request.general.as_deref(), Some(SECRET_CHAT));
    app.done.join().unwrap();
}

#[test]
fn an_answered_round_prints_the_answers_and_exits_zero() {
    let sandbox = sandbox();
    let app = app(&sandbox, |_, _| Some(Response::ok(answered())));
    let result = run(&sandbox, &in_job(&app.endpoint.clone()), &round());
    assert_eq!((result.exit, result.json()), (Some(0), answered()));

    // What the app received: its own kind, the job of the caller, the input as it was.
    let request = app.seen.recv_timeout(PATIENCE).unwrap();
    assert_eq!(request.kind, "question.new");
    assert_eq!(request.job.as_deref(), Some(SECRET_JOB));
    assert_eq!(request.payload, round());
    app.done.join().unwrap();
}

#[test]
fn a_reply_that_comes_late_is_waited_for_with_no_time_limit() {
    let sandbox = sandbox();
    let app = app(&sandbox, |_, _| {
        thread::sleep(Duration::from_millis(1500));
        Some(Response::ok(answered()))
    });
    let result = run(&sandbox, &in_job(&app.endpoint.clone()), &round());
    assert_eq!((result.exit, result.json()), (Some(0), answered()));
}

#[test]
fn the_app_that_cannot_show_questions_is_unavailable_with_exit_zero() {
    let sandbox = sandbox();
    let app = app(&sandbox, |_, _| {
        Some(Response::failure("unavailable", "the job is not working"))
    });
    assert_unavailable(&run(&sandbox, &in_job(&app.endpoint.clone()), &round()));
}

#[test]
fn nothing_is_submitted_when_no_app_can_be_reached() {
    let sandbox = sandbox();
    // Neither a job nor the Chat.
    assert_unavailable(&run(&sandbox, &[], &round()));
    // The Chat, with no runtime file.
    assert_unavailable(&run(&sandbox, &[("WEAVER_GENERAL", SECRET_CHAT)], &round()));
    // In a job, with no runtime file.
    assert_unavailable(&run(&sandbox, &[("WEAVER_JOB", SECRET_JOB)], &round()));

    // A runtime file with nobody listening.
    let endpoint = new_endpoint(&sandbox.dirs).unwrap();
    runtime::write(
        &sandbox.dirs,
        &RuntimeInfo {
            endpoint: endpoint.clone(),
            secret: new_secret(),
            pid: 1,
            version: "0.1.0".into(),
        },
    )
    .unwrap();
    assert_unavailable(&run(&sandbox, &in_job(&endpoint), &round()));

    // A runtime file that names another endpoint than the one of the job.
    assert_unavailable(&run(&sandbox, &in_job("/another/endpoint"), &round()));
}

#[test]
fn without_weaver_endpoint_nothing_is_submitted_even_when_an_app_runs() {
    let sandbox = sandbox();
    // An app that would fail the test if it were contacted.
    let app = app(&sandbox, |_, _| panic!("the app must not be contacted"));
    assert_unavailable(&run(&sandbox, &[("WEAVER_JOB", SECRET_JOB)], &round()));
    assert_unavailable(&run(&sandbox, &[("WEAVER_GENERAL", SECRET_CHAT)], &round()));
    assert!(app.seen.try_recv().is_err());
}

#[test]
fn an_interrupted_round_is_exit_one_with_its_code_and_no_answers() {
    let sandbox = sandbox();
    let app = app(&sandbox, |_, _| {
        Some(Response::failure("interrupted", "The job ended."))
    });
    let result = run(&sandbox, &in_job(&app.endpoint.clone()), &round());
    assert_eq!(result.exit, Some(1));
    let found = result.json();
    assert_eq!(found["error"]["code"], "interrupted");
    assert!(found.get("answers").is_none() && found.get("status").is_none());
}

#[test]
fn the_connection_lost_after_submission_is_interrupted_at_once() {
    let sandbox = sandbox();
    // The app reads the request and goes away: no reply, only the end of the stream.
    let endpoint = new_endpoint(&sandbox.dirs).unwrap();
    let secret = new_secret();
    runtime::write(
        &sandbox.dirs,
        &RuntimeInfo {
            endpoint: endpoint.clone(),
            secret: secret.clone(),
            pid: 1,
            version: "0.1.0".into(),
        },
    )
    .unwrap();
    let listener = Listener::bind(&endpoint).unwrap();
    let server = thread::spawn(move || {
        let mut stream = listener.accept().unwrap();
        let mut byte = [0u8; 1];
        loop {
            stream.read_exact(&mut byte).unwrap();
            if byte[0] == b'\n' {
                break;
            }
        }
        // Dropped with the request read and no reply written.
    });
    let result = run(&sandbox, &in_job(&endpoint), &round());
    server.join().unwrap();
    assert_eq!(result.exit, Some(1));
    assert_eq!(result.json()["error"]["code"], "interrupted");
}

#[test]
fn an_invalid_round_is_refused_before_the_app_is_contacted() {
    let sandbox = sandbox();
    // An app that would fail the test if it were contacted.
    let app = app(&sandbox, |_, _| panic!("the app must not be contacted"));
    let mut bad = round();
    bad["questions"][0]["options"][0]["label"] = json!("");
    bad["questions"][0]["recommended"] = json!(5);
    let result = run(&sandbox, &in_job(&app.endpoint.clone()), &bad);
    assert_eq!(result.exit, Some(2), "{}", result.stderr);
    let found = result.json();
    assert_eq!(found["error"]["code"], "invalid-input");
    let message = found["error"]["message"].as_str().unwrap();
    assert!(
        message.contains("questions[0].options[0].label")
            && message.contains("questions[0].recommended"),
        "{message}"
    );
    assert!(app.seen.try_recv().is_err());
}

#[test]
fn the_app_that_refuses_the_round_is_invalid_input_with_exit_two() {
    let sandbox = sandbox();
    let app = app(&sandbox, |_, _| {
        Some(Response::failure(
            "invalid-input",
            "questions[0].id: refused",
        ))
    });
    let result = run(&sandbox, &in_job(&app.endpoint.clone()), &round());
    assert_eq!(result.exit, Some(2));
    assert_eq!(result.json()["error"]["code"], "invalid-input");
}

/// The app waits for its client to go away, as it does for a round that nobody has answered.
fn waiting_app(sandbox: &Sandbox) -> App {
    app(sandbox, |_, peer| {
        let (gone, is_gone) = mpsc::channel::<()>();
        peer.on_close(move || {
            let _ = gone.send(());
        });
        let _ = is_gone.recv_timeout(PATIENCE);
        None
    })
}

fn signal(child: &Child, name: &str) {
    let status = Command::new("kill")
        .arg(format!("-{name}"))
        .arg(child.id().to_string())
        .status()
        .unwrap();
    assert!(status.success());
}

fn stopped_by(name: &str) {
    let sandbox = sandbox();
    let app = waiting_app(&sandbox);
    let mut child = weaver(&sandbox, &in_job(&app.endpoint.clone()));
    give_input(&mut child, &round());
    // The round is submitted and waits.
    app.seen.recv_timeout(PATIENCE).unwrap();
    thread::sleep(Duration::from_millis(300));
    signal(&child, name);
    // The connection closes, which is how the agent stops waiting.
    app.closed
        .recv_timeout(PATIENCE)
        .expect("the connection was not closed");
    let result = finish(child);
    assert_eq!(result.exit, Some(1), "{}", result.stderr);
    let found = result.json();
    assert_eq!(found["error"]["code"], "cancelled");
    app.done.join().unwrap();
}

#[test]
fn sigint_withdraws_the_round_and_closes_the_connection() {
    stopped_by("INT");
}

#[test]
fn sigterm_withdraws_the_round_and_closes_the_connection() {
    stopped_by("TERM");
}

#[test]
fn a_closed_standard_output_withdraws_the_round() {
    let sandbox = sandbox();
    let app = waiting_app(&sandbox);
    let mut child = weaver(&sandbox, &in_job(&app.endpoint.clone()));
    give_input(&mut child, &round());
    app.seen.recv_timeout(PATIENCE).unwrap();
    thread::sleep(Duration::from_millis(300));
    drop(child.stdout.take());
    app.closed
        .recv_timeout(PATIENCE)
        .expect("the connection was not closed");
    let status = child.wait().unwrap();
    assert_eq!(status.code(), Some(1));
    app.done.join().unwrap();
}

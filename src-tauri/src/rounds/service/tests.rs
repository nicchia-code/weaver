use std::io::Write;
use std::sync::mpsc::Receiver;
use std::thread::{self, JoinHandle};

use serde_json::{json, Value};
use weaver_core::endpoint::new_endpoint;
use weaver_core::ipc::{connect, serve_connection_watched, Listener};
use weaver_core::paths::AppDirs;
use weaver_core::protocol::{encode, Request};
use weaver_core::question::{Action, ActionError, Answer, Position, KIND};

use super::*;
use crate::rounds::view::OriginView;

#[derive(Default)]
struct FakeHost {
    jobs: Mutex<HashMap<String, JobFacts>>,
    log: Mutex<Vec<String>>,
    /// The live Chats with a turn in progress: identity to project.
    chats: Mutex<HashMap<String, String>>,
    changes: Mutex<Option<Sender<QuestionRoundChanged>>>,
}

impl FakeHost {
    fn log(&self) -> Vec<String> {
        self.log.lock().unwrap().clone()
    }

    fn push(&self, line: String) {
        self.log.lock().unwrap().push(line);
    }
}

impl RoundsHost for FakeHost {
    fn job(&self, job: &str) -> Option<JobFacts> {
        self.jobs.lock().unwrap().get(job).cloned()
    }

    fn round_opened(&self, job: &str) {
        self.push(format!("opened {job}"));
    }

    fn round_closed(&self, job: &str) {
        self.push(format!("closed {job}"));
    }

    fn chat(&self, token: &str) -> Option<String> {
        self.chats.lock().unwrap().get(token).cloned()
    }

    fn chat_round_opened(&self, project_id: &str) {
        self.push(format!("chat opened {project_id}"));
    }

    fn chat_round_closed(&self, project_id: &str) {
        self.push(format!("chat closed {project_id}"));
    }

    fn focus_project(&self, project_id: &str) {
        self.push(format!("focus {project_id}"));
    }

    fn changed(&self, change: &QuestionRoundChanged) {
        if let Some(sender) = self.changes.lock().unwrap().as_ref() {
            let _ = sender.send(change.clone());
        }
    }
}

struct Fixture {
    rounds: Arc<Rounds>,
    host: Arc<FakeHost>,
    changes: Receiver<QuestionRoundChanged>,
}

fn fixture() -> Fixture {
    let host = Arc::new(FakeHost::default());
    let facts = |project: &str, name: &str, accepting: bool| JobFacts {
        project_id: project.into(),
        name: name.into(),
        accepting,
    };
    {
        let mut jobs = host.jobs.lock().unwrap();
        jobs.insert("job1".into(), facts("p1", "Scan", true));
        jobs.insert("job2".into(), facts("p1", "Run", true));
        jobs.insert("job3".into(), facts("p2", "Other project", true));
        jobs.insert("idle".into(), facts("p1", "Finished", false));
    }
    {
        let mut chats = host.chats.lock().unwrap();
        chats.insert("chat1".into(), "p1".into());
        chats.insert("chat2".into(), "p2".into());
    }
    let (sender, changes) = mpsc::channel();
    *host.changes.lock().unwrap() = Some(sender);
    Fixture {
        rounds: Rounds::new(host.clone()),
        host,
        changes,
    }
}

fn payload(ids: &[&str]) -> Value {
    let questions: Vec<Value> = ids
        .iter()
        .map(|id| {
            json!({
                "id": id,
                "question": "Which one?",
                "options": [{ "label": "a", "description": "first" }, { "label": "b", "description": "second" }],
                "recommended": 0,
                "multi": false,
            })
        })
        .collect();
    json!({ "questions": questions })
}

fn request(job: Option<&str>, payload: Value) -> Request {
    let mut request = Request::new("secret", KIND, payload);
    request.job = job.map(Into::into);
    request
}

/// Asks a round as an agent does and waits until it is in its queue.
fn ask(f: &Fixture, job: &str, ids: &[&str]) -> JoinHandle<Response> {
    let rounds = f.rounds.clone();
    let request = request(Some(job), payload(ids));
    let handle = thread::spawn(move || rounds.handle(request, Peer::default()));
    f.changes
        .recv_timeout(std::time::Duration::from_secs(20))
        .expect("the round was not submitted");
    handle
}

fn presented(f: &Fixture, project: &str) -> QuestionRoundView {
    f.rounds.view(project).expect("a round is presented")
}

fn code_of(response: &Response) -> String {
    response
        .error
        .as_ref()
        .map(|e| e.code.clone())
        .unwrap_or_default()
}

#[test]
fn a_request_that_is_not_a_valid_round_of_a_working_job_is_refused_and_opens_nothing() {
    let f = fixture();
    let mut bad = payload(&["a"]);
    bad["questions"][0]["options"][1]["label"] = json!("");
    let refused = f.rounds.handle(request(Some("job1"), bad), Peer::default());
    assert_eq!(code_of(&refused), "invalid-input");
    assert!(refused
        .error
        .unwrap()
        .message
        .contains("questions[0].options[1].label"));

    for job in [None, Some("nobody"), Some("idle")] {
        let response = f
            .rounds
            .handle(request(job, payload(&["a"])), Peer::default());
        assert_eq!(code_of(&response), "unavailable", "{job:?}");
        assert_eq!(
            response.error.unwrap().message,
            question::UNAVAILABLE_REASON
        );
    }
    assert!(f.host.log().is_empty() && f.rounds.view("p1").is_none());
}

#[test]
fn a_round_is_presented_answered_and_closed_for_its_job() {
    let f = fixture();
    let waiting = ask(&f, "job1", &["a", "b"]);
    let view = presented(&f, "p1");
    assert_eq!(
        view.origin,
        OriginView::Agent {
            job_name: "Scan".into()
        }
    );
    assert_eq!(
        (view.questions.len(), view.waiting, view.minimized),
        (2, 0, false)
    );
    assert_eq!(f.host.log(), ["opened job1", "focus p1"]);

    let id = view.id;
    // Nothing is on screen in a question that was not visited, so it cannot be confirmed.
    assert_eq!(
        f.rounds
            .act("p1", id, Action::Confirm { index: 1 })
            .unwrap_err(),
        ActionError::NothingToConfirm
    );
    // The recommended option is on screen on the first visit; confirming takes it.
    f.rounds
        .act("p1", id, Action::Go(Position::Question { index: 1 }))
        .unwrap();
    f.rounds
        .act(
            "p1",
            id,
            Action::Draft {
                index: 1,
                selected: vec![1],
                text: " because ".into(),
            },
        )
        .unwrap();
    f.rounds
        .act("p1", id, Action::Confirm { index: 1 })
        .unwrap();
    f.rounds
        .act("p1", id, Action::Go(Position::Question { index: 0 }))
        .unwrap();
    f.rounds
        .act("p1", id, Action::Confirm { index: 0 })
        .unwrap();
    let after = f.rounds.act("p1", id, Action::Send).unwrap();
    assert!(after.is_none(), "nothing else waits");

    let response = waiting.join().unwrap();
    assert!(response.error.is_none());
    assert_eq!(
        response.payload,
        json!({ "status": "answered", "answers": {
            "a": { "status": "answered", "selected": ["a"] },
            "b": { "status": "answered", "selected": ["b"], "text": "because" },
        }})
    );
    assert_eq!(f.host.log(), ["opened job1", "focus p1", "closed job1"]);
}

/// Asks a round as the agent of the Chat does and waits until it is in its queue.
fn ask_as_chat(f: &Fixture, token: &str, ids: &[&str]) -> JoinHandle<Response> {
    let rounds = f.rounds.clone();
    let mut request = request(None, payload(ids));
    request.general = Some(token.into());
    let handle = thread::spawn(move || rounds.handle(request, Peer::default()));
    f.changes
        .recv_timeout(std::time::Duration::from_secs(20))
        .expect("the round was not submitted");
    handle
}

#[test]
fn the_chat_during_a_turn_asks_a_round_that_belongs_to_it_and_its_project() {
    let f = fixture();
    let waiting = ask_as_chat(&f, "chat1", &["a"]);
    let view = presented(&f, "p1");
    assert_eq!(view.origin, OriginView::Chat);
    assert_eq!(view.project_id, "p1");
    // The Chat hears of the opening, not a job; the window of the project comes forward.
    assert_eq!(f.host.log(), ["chat opened p1", "focus p1"]);
    assert!(f.rounds.view("p2").is_none(), "no other project shows it");

    let seen = f.rounds.question_in_view("p1", view.id, 0).unwrap();
    assert_eq!(seen.origin, Origin::Chat { project: "p1".into() });
    assert_eq!(seen.job, None);

    f.rounds
        .act("p1", view.id, Action::Go(Position::Question { index: 0 }))
        .unwrap();
    f.rounds
        .act(
            "p1",
            view.id,
            Action::Draft {
                index: 0,
                selected: vec![1],
                text: String::new(),
            },
        )
        .unwrap();
    f.rounds
        .act("p1", view.id, Action::Confirm { index: 0 })
        .unwrap();
    f.rounds.act("p1", view.id, Action::Send).unwrap();

    let response = waiting.join().unwrap();
    assert!(response.error.is_none());
    assert_eq!(
        response.payload,
        json!({ "status": "answered", "answers": { "a": { "status": "answered", "selected": ["b"] } } })
    );
    // The close reaches the Chat, with no job in between.
    assert_eq!(
        f.host.log(),
        ["chat opened p1", "focus p1", "chat closed p1"]
    );
}

#[test]
fn a_chat_identity_that_is_not_the_live_one_or_has_no_turn_is_unavailable() {
    let f = fixture();
    // Unknown, or known with no turn in progress: the host does not vouch for it.
    for token in ["nobody", "chat-without-turn"] {
        let mut asking = request(None, payload(&["a"]));
        asking.general = Some(token.into());
        let response = f.rounds.handle(asking, Peer::default());
        assert_eq!(code_of(&response), "unavailable", "{token}");
        assert_eq!(
            response.error.unwrap().message,
            question::UNAVAILABLE_REASON
        );
    }
    // The job identity decides: an unknown job is not saved by a live Chat identity.
    let mut both = request(Some("nobody"), payload(&["a"]));
    both.general = Some("chat1".into());
    assert_eq!(
        code_of(&f.rounds.handle(both, Peer::default())),
        "unavailable"
    );
    assert!(f.host.log().is_empty() && f.rounds.view("p1").is_none());
}

#[test]
fn a_job_and_the_chat_of_a_project_share_its_queue_and_each_hears_of_its_own_round() {
    let f = fixture();
    let job = ask(&f, "job1", &["j"]);
    let chat = ask_as_chat(&f, "chat1", &["c"]);
    let first = presented(&f, "p1");
    assert_eq!(
        (first.origin.clone(), first.waiting),
        (
            OriginView::Agent {
                job_name: "Scan".into()
            },
            1
        )
    );
    assert_eq!(f.host.log(), ["opened job1", "focus p1"]);

    // The Chat's round is presented when the job's closes, and only then does the Chat hear of it.
    f.rounds.abandon("p1", first.id);
    assert_eq!(job.join().unwrap().error.unwrap().code, "cancelled");
    assert_eq!(presented(&f, "p1").origin, OriginView::Chat);
    assert_eq!(
        f.host.log(),
        ["opened job1", "focus p1", "closed job1", "chat opened p1", "focus p1"]
    );

    // Closing the window interrupts the Chat's round and tells the Chat.
    f.rounds.window_closed("p1");
    assert_eq!(chat.join().unwrap().error.unwrap().code, "interrupted");
    assert_eq!(
        f.host.log().last().map(String::as_str),
        Some("chat closed p1")
    );
}

#[test]
fn the_end_of_a_connection_of_the_chat_closes_its_round_for_the_chat() {
    let f = fixture();
    let waiting = ask_as_chat(&f, "chat1", &["a"]);
    let id = presented(&f, "p1").id;
    // Stop, Clear or an exit of omp end the command: its connection closes.
    f.rounds.abandon("p1", id);
    assert_eq!(waiting.join().unwrap().error.unwrap().code, "cancelled");
    assert_eq!(
        f.host.log(),
        ["chat opened p1", "focus p1", "chat closed p1"]
    );
    assert!(f.rounds.view("p1").is_none());
}

#[test]
fn the_backend_refuses_to_send_while_a_question_has_neither_an_answer_nor_a_skip() {
    let f = fixture();
    let waiting = ask(&f, "job1", &["a", "b"]);
    let id = presented(&f, "p1").id;
    f.rounds
        .act("p1", id, Action::Confirm { index: 0 })
        .unwrap();
    assert_eq!(
        f.rounds.act("p1", id, Action::Send).unwrap_err(),
        ActionError::Missing(1)
    );
    assert_eq!(presented(&f, "p1").id, id, "the round stays as it was");
    f.rounds.act("p1", id, Action::SkipAll).unwrap();
    let response = waiting.join().unwrap();
    assert_eq!(
        response.payload["answers"]["b"],
        json!({ "status": "skipped" })
    );
    assert_eq!(response.payload["answers"]["a"]["status"], "answered");
}

#[test]
fn rounds_of_a_project_wait_first_in_first_out_and_never_replace_the_presented_one() {
    let f = fixture();
    let first = ask(&f, "job1", &["a"]);
    let second = ask(&f, "job2", &["b"]);
    let view = presented(&f, "p1");
    assert_eq!(
        (view.origin.clone(), view.waiting),
        (
            OriginView::Agent {
                job_name: "Scan".into()
            },
            1
        )
    );
    assert_eq!(
        f.host.log(),
        ["opened job1", "focus p1"],
        "the second one only waits"
    );

    f.rounds.act("p1", view.id, Action::SkipAll).unwrap();
    assert!(first.join().unwrap().error.is_none());
    let view = presented(&f, "p1");
    assert_eq!(
        (view.origin, view.waiting),
        (
            OriginView::Agent {
                job_name: "Run".into()
            },
            0
        )
    );
    assert_eq!(
        f.host.log(),
        [
            "opened job1",
            "focus p1",
            "closed job1",
            "opened job2",
            "focus p1"
        ]
    );
    f.rounds.act("p1", view.id, Action::SkipAll).unwrap();
    assert!(second.join().unwrap().error.is_none());
}

#[test]
fn projects_never_wait_for_each_other_and_a_round_belongs_to_its_project() {
    let f = fixture();
    let one = ask(&f, "job1", &["a"]);
    let other = ask(&f, "job3", &["b"]);
    let (view_one, view_other) = (presented(&f, "p1"), presented(&f, "p2"));
    assert_eq!((view_one.waiting, view_other.waiting), (0, 0));
    // A round cannot be reached through another project.
    assert_eq!(
        f.rounds
            .act("p1", view_other.id, Action::SkipAll)
            .unwrap_err(),
        ActionError::NoSuchRound
    );
    f.rounds.act("p1", view_one.id, Action::SkipAll).unwrap();
    f.rounds.act("p2", view_other.id, Action::SkipAll).unwrap();
    one.join().unwrap();
    other.join().unwrap();
}

#[test]
fn the_end_of_a_job_interrupts_its_rounds_and_the_next_one_is_presented() {
    let f = fixture();
    let first = ask(&f, "job1", &["a"]);
    let second = ask(&f, "job2", &["b"]);
    f.rounds.job_ended("p1", "job1");
    let response = first.join().unwrap();
    assert_eq!(code_of(&response), "interrupted");
    assert_eq!(response.error.unwrap().message, INTERRUPTED_MESSAGE);
    assert_eq!(
        presented(&f, "p1").origin,
        OriginView::Agent {
            job_name: "Run".into()
        }
    );
    f.rounds.job_ended("p1", "job2");
    assert_eq!(code_of(&second.join().unwrap()), "interrupted");
    assert!(f.rounds.view("p1").is_none());
}

#[test]
fn closing_the_window_interrupts_every_round_including_a_prepared_question() {
    let f = fixture();
    let agent = ask(&f, "job1", &["a"]);
    let prepared = f.rounds.present_prepared(
        "p1",
        "drift0001",
        "dr1ftd1d",
        "A Drift",
        serde_json::from_value(payload(&["q"])).unwrap(),
    );
    f.changes.recv().unwrap();
    assert_eq!(presented(&f, "p1").waiting, 1);

    f.rounds.window_closed("p1");
    assert_eq!(code_of(&agent.join().unwrap()), "interrupted");
    assert_eq!(prepared.recv().unwrap(), Outcome::Interrupted);
    assert!(f.rounds.view("p1").is_none());
    assert_eq!(f.host.log().last().map(String::as_str), Some("closed job1"));
}

#[test]
fn the_prepared_question_of_a_drift_is_asked_with_no_agent_and_a_skip_carries_nothing() {
    let f = fixture();
    let outcome = f.rounds.present_prepared(
        "p1",
        "drift0001",
        "dr1ftd1d",
        "Choose the storage",
        serde_json::from_value(payload(&["q"])).unwrap(),
    );
    let view = presented(&f, "p1");
    assert_eq!(
        view.origin,
        OriginView::Drift {
            drift: "dr1ftd1d".into(),
            title: "Choose the storage".into()
        }
    );
    assert_eq!(
        f.host.log(),
        ["focus p1"],
        "no job is told: nobody waits but Weaver"
    );

    f.rounds.act("p1", view.id, Action::SkipAll).unwrap();
    assert_eq!(
        outcome.recv().unwrap(),
        Outcome::Answered(vec![Answer::Skipped])
    );
    assert_eq!(f.host.log(), ["focus p1"]);
}

#[test]
fn a_connection_that_closes_abandons_the_round_queued_or_presented() {
    let f = fixture();
    let base = tempfile::tempdir().unwrap();
    let endpoint = new_endpoint(&AppDirs::from_bases(
        base.path(),
        base.path(),
        Some(base.path()),
    ))
    .unwrap();
    let listener = Listener::bind(&endpoint).unwrap();
    let serving = f.rounds.clone();
    let server = thread::spawn(move || {
        let stream = listener.accept().unwrap();
        // The client is gone when the reply is written: that write failing is the point.
        let _ = serve_connection_watched(stream, "secret", |request, peer| {
            serving.handle(request, peer)
        });
    });

    let mut stream = connect(&endpoint).unwrap();
    stream
        .write_all(&encode(&request(Some("job1"), payload(&["a"]))).unwrap())
        .unwrap();
    f.changes
        .recv_timeout(std::time::Duration::from_secs(20))
        .expect("the round was not submitted");
    assert_eq!(presented(&f, "p1").questions.len(), 1);

    drop(stream);
    server.join().unwrap();
    assert!(f.rounds.view("p1").is_none(), "no round is left");
    assert_eq!(f.host.log(), ["opened job1", "focus p1", "closed job1"]);
}

#[test]
fn the_queue_of_a_project_is_created_when_its_window_opens() {
    let f = fixture();
    f.rounds.window_opened("p9");
    assert!(f.rounds.view("p9").is_none());
    f.rounds.window_closed("p9");
    f.rounds.window_closed("never-opened");
}

#[test]
fn an_ask_more_conversation_reads_only_the_question_in_view_of_the_presented_round() {
    let f = fixture();
    let agent = ask(&f, "job1", &["a", "b"]);
    let view = presented(&f, "p1");

    let seen = f
        .rounds
        .question_in_view("p1", view.id, 0)
        .expect("the question in view is read");
    assert_eq!(seen.request.questions.len(), 2);
    assert_eq!(seen.origin, Origin::Agent { job: "job1".into() });
    assert_eq!(seen.job, Some(("job1".to_owned(), Some("Scan".to_owned()))));

    // Another position, another round, another project and an index out of range open nothing.
    assert!(f.rounds.question_in_view("p1", view.id, 1).is_none());
    assert!(f.rounds.question_in_view("p1", view.id + 1, 0).is_none());
    assert!(f.rounds.question_in_view("p2", view.id, 0).is_none());
    f.rounds
        .act("p1", view.id, Action::Go(Position::Question { index: 1 }))
        .unwrap();
    assert!(f.rounds.question_in_view("p1", view.id, 0).is_none());
    assert!(f.rounds.question_in_view("p1", view.id, 1).is_some());
    f.rounds
        .act("p1", view.id, Action::Go(Position::Summary))
        .unwrap();
    assert!(f.rounds.question_in_view("p1", view.id, 1).is_none());

    // A round that closed is gone.
    f.rounds.window_closed("p1");
    assert!(f.rounds.question_in_view("p1", view.id, 0).is_none());
    let _ = agent.join();
}

#[test]
fn the_round_of_a_prepared_question_keeps_the_permanent_identity_of_its_drift() {
    let f = fixture();
    let _outcome = f.rounds.present_prepared(
        "p1",
        "drift0001",
        "D1",
        "A Drift",
        serde_json::from_value(payload(&["q"])).unwrap(),
    );
    let view = presented(&f, "p1");
    let seen = f.rounds.question_in_view("p1", view.id, 0).unwrap();
    assert_eq!(
        seen.origin,
        Origin::Drift {
            id: "drift0001".into(),
            drift: "D1".into(),
            title: "A Drift".into()
        }
    );
    assert_eq!(seen.job, None);
}

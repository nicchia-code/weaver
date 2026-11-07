//! `weaver question new`: submits the questions to the app (`requirements/cli#question-round`). In
//! a job it waits, with no time limit, until a round has a result; in Chat it reads one reply, as
//! soon as the app has published the card of questions or refused it.
//!
//! It does not use the request kind `command`: that kind runs inside the project's queue under
//! the project lock, and a round waits for the developer for as long as he needs. The request is
//! its own kind ([`KIND`]), routed only by the job identity of the caller (`WEAVER_JOB`) or,
//! without it, by the identity of the Chat (`WEAVER_GENERAL`); the request carries the ones the
//! caller has.
//!
//! Outcomes, never conflated: `answered` (job) and `published` (Chat), exit 0; `unavailable`
//! (exit 0, returned at once when nothing can be submitted); `interrupted`, `cancelled` and
//! `card-already-published` (exit 1); `invalid-input` (exit 2). Closing the connection is how the
//! agent of a job stops waiting, so a stop (SIGINT, SIGTERM, a closed standard output) only has
//! to end the process.

use std::sync::mpsc;
use std::thread;

use serde_json::{json, Map, Value};
use weaver_core::command::{code, CommandReply};
use weaver_core::ipc::{self, SendError};
use weaver_core::paths::AppDirs;
use weaver_core::protocol::{self, ProtocolError, Request, Response};
use weaver_core::question::{
    self, code as round_code, CANCELLED_MESSAGE, INTERRUPTED_MESSAGE, KIND,
};

use crate::cli::Environment;
use crate::client::{self, Connection};
use crate::interrupts::Interrupts;
use crate::spec::Failure;

/// The result when no round can be shown.
pub fn unavailable() -> CommandReply {
    CommandReply {
        markdown: question::UNAVAILABLE_REASON.to_owned(),
        json: json!({ "status": "unavailable", "reason": question::UNAVAILABLE_REASON }),
        exit: 0,
        note: None,
    }
}

/// What ended the wait.
enum Event {
    Reply(Result<Response, SendError>),
    /// SIGINT, SIGTERM or a closed standard output.
    Stopped,
}

/// Submits `input` (already validated against the registry) and waits for the result.
pub fn run(
    input: Map<String, Value>,
    env: &Environment,
    dirs: Option<&AppDirs>,
    interrupts: &dyn Interrupts,
) -> Result<CommandReply, Failure> {
    // Without an identity or without `WEAVER_ENDPOINT` nobody can have started this command for
    // an app to answer: unavailable at once.
    let (Some(dirs), Some(_), true) = (
        dirs,
        env.endpoint.as_deref(),
        env.job.is_some() || env.general.is_some(),
    ) else {
        return Ok(unavailable());
    };
    let (job, general) = (env.job.as_deref(), env.general.as_deref());
    // The rules of a round again, from the one source the app uses: nothing is sent that it would refuse.
    let input = Value::Object(input);
    let problems = question::problems(&input);
    if !problems.is_empty() {
        return Err(Failure::invalid(question::describe(&problems)));
    }

    let (events, wait) = mpsc::channel();
    let stop = events.clone();
    interrupts.watch(Box::new(move || {
        let _ = stop.send(Event::Stopped);
    }));

    let Connection { stream, secret } = match client::open(dirs, env.endpoint.as_deref()) {
        Ok(connection) => connection,
        Err(_) => return Ok(unavailable()),
    };
    let mut request = Request::new(secret, KIND, input);
    request.job = job.map(str::to_owned);
    request.general = general.map(str::to_owned);
    if let Err(ProtocolError::TooLarge) = protocol::encode(&request).map(drop) {
        return Err(Failure::invalid(
            "the round is larger than 1 MiB: shorten the texts",
        ));
    }
    // The connection stays open while this thread waits for the reply; the process ending closes it.
    thread::spawn(move || {
        let _ = events.send(Event::Reply(ipc::exchange(stream, &request)));
    });

    match wait.recv() {
        Ok(Event::Reply(reply)) => outcome(reply),
        Ok(Event::Stopped) | Err(_) => Err(Failure::new(round_code::CANCELLED, CANCELLED_MESSAGE)),
    }
}

/// What the reply of the app (or its loss) means for the command.
fn outcome(reply: Result<Response, SendError>) -> Result<CommandReply, Failure> {
    match reply {
        Ok(response) => match response.error {
            Some(error) if error.code == round_code::UNAVAILABLE => Ok(unavailable()),
            Some(error) => Err(Failure::new(&error.code, error.message)),
            None => result(response.payload),
        },
        Err(SendError::BadReply(ProtocolError::VersionMismatch { .. })) => Ok(unavailable()),
        Err(SendError::BadRequest(_)) => Err(Failure::invalid(
            "the round is larger than 1 MiB: shorten the texts",
        )),
        // The connection was lost after submission: detected at once, the round is gone.
        Err(SendError::NoReply | SendError::Io(_) | SendError::Connect(_)) => {
            Err(Failure::new(round_code::INTERRUPTED, INTERRUPTED_MESSAGE))
        }
        Err(SendError::BadReply(e)) => Err(Failure::failed(format!(
            "Weaver sent an answer that cannot be read: {e}"
        ))),
    }
}

/// The result of the questions: `answered` for a round, `published` for a card of Chat.
fn result(payload: Value) -> Result<CommandReply, Failure> {
    let readable = match payload.get("status").and_then(Value::as_str) {
        Some("answered") => payload.get("answers").is_some_and(Value::is_object),
        Some("published") => {
            payload.get("card").is_some_and(Value::is_string)
                && payload.get("message").is_some_and(Value::is_string)
        }
        _ => false,
    };
    if !readable {
        return Err(Failure::new(
            code::FAILED,
            "Weaver sent an answer that cannot be read",
        ));
    }
    Ok(CommandReply {
        markdown: payload.to_string(),
        json: payload,
        exit: 0,
        note: None,
    })
}

#[cfg(test)]
mod tests {
    use weaver_core::protocol::ErrorBody;

    use super::*;

    fn response(error: Option<(&str, &str)>, payload: Value) -> Response {
        let mut response = Response::ok(payload);
        response.error = error.map(|(code, message)| ErrorBody {
            code: code.into(),
            message: message.into(),
        });
        response
    }

    #[test]
    fn every_outcome_has_its_own_code_and_exit() {
        let answered = json!({"status": "answered", "answers": {"a": {"status": "skipped"}}});
        let ok = outcome(Ok(response(None, answered.clone()))).unwrap();
        assert_eq!((ok.exit, ok.json), (0, answered));

        let unavailable = outcome(Ok(response(Some(("unavailable", "no")), Value::Null))).unwrap();
        assert_eq!(
            (unavailable.exit, unavailable.json["status"].clone()),
            (0, json!("unavailable"))
        );

        let interrupted =
            outcome(Ok(response(Some(("interrupted", "gone")), Value::Null))).unwrap_err();
        assert_eq!(
            (interrupted.code.as_str(), interrupted.exit_code()),
            ("interrupted", 1)
        );
        let lost = outcome(Err(SendError::NoReply)).unwrap_err();
        assert_eq!((lost.code.as_str(), lost.exit_code()), ("interrupted", 1));
        let invalid = outcome(Ok(response(
            Some(("invalid-input", "questions[0].id")),
            Value::Null,
        )))
        .unwrap_err();
        assert_eq!(
            (invalid.code.as_str(), invalid.exit_code()),
            ("invalid-input", 2)
        );
        let version = outcome(Err(SendError::BadReply(ProtocolError::VersionMismatch {
            got: 9,
        })))
        .unwrap();
        assert_eq!(version.json["status"], json!("unavailable"));
    }

    #[test]
    fn a_published_card_exits_0_and_a_second_card_in_the_turn_exits_1() {
        let published = json!({"status": "published", "card": "abcd1234", "message": "End your turn."});
        let ok = outcome(Ok(response(None, published.clone()))).unwrap();
        assert_eq!((ok.exit, ok.json), (0, published));

        let again = outcome(Ok(response(
            Some(("card-already-published", "Put every question in one card.")),
            Value::Null,
        )))
        .unwrap_err();
        assert_eq!(
            (again.code.as_str(), again.exit_code()),
            ("card-already-published", 1)
        );
    }

    #[test]
    fn a_published_reply_without_its_card_or_message_is_a_failure() {
        for payload in [
            json!({"status": "published"}),
            json!({"status": "published", "card": "abcd1234"}),
            json!({"status": "published", "card": 3, "message": "m"}),
        ] {
            assert_eq!(outcome(Ok(response(None, payload))).unwrap_err().code, "failed");
        }
    }

    #[test]
    fn a_reply_that_is_not_an_answer_is_a_failure() {
        assert_eq!(
            outcome(Ok(response(None, json!({"status": "other"}))))
                .unwrap_err()
                .code,
            "failed"
        );
        assert_eq!(
            outcome(Ok(response(None, Value::Null))).unwrap_err().code,
            "failed"
        );
    }
}

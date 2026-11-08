//! The ACP calls Weaver makes to omp (`requirements/tech#job`): opening a session, prompting,
//! steering. Each is a function over [`Rpc`]; what they decide is pure and tested here.

use std::path::Path;
use std::time::Duration;

use serde_json::{json, Value};
use weaver_core::jobs::machine::SteeringOutcome;
use weaver_core::omp::failure;

use super::rpc::{Rpc, RpcError};

/// One limit covers spawn, `initialize` and `session/new` (or `session/load`).
pub const OPEN_LIMIT: Duration = Duration::from_secs(30);

/// The fixed text an elicitation or an ask of omp is declined with.
pub const DECLINE_TEXT: &str =
    "In Weaver jobs, ask the developer only with `weaver question new` (bash timeout 0). Do not ask in any other way.";

/// Why a session could not be opened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenError {
    /// The limit passed.
    Timeout,
    /// the agent answered with an error, or closed the connection.
    Failed(String),
    /// omp lacks something Weaver needs: `loadSession` or the steering extension.
    Unsupported(String),
}

impl OpenError {
    /// The reason the developer reads.
    pub fn reason(&self) -> String {
        match self {
            OpenError::Timeout => "the agent did not answer within 30 s".to_owned(),
            OpenError::Failed(text) | OpenError::Unsupported(text) => text.clone(),
        }
    }
}

impl From<RpcError> for OpenError {
    fn from(error: RpcError) -> Self {
        OpenError::Failed(error.text())
    }
}

/// `initialize`: protocol version 1, no `fs` and no `terminal` (Weaver offers omp neither), with
/// `elicitation.form` so that omp asks through forms, which Weaver declines.
pub fn initialize_params() -> Value {
    json!({
        "protocolVersion": 1,
        "clientCapabilities": { "elicitation": { "form": {} } },
        "clientInfo": { "name": "weaver", "title": "Weaver", "version": env!("CARGO_PKG_VERSION") },
    })
}

/// A missing `loadSession` or `_meta.steering.supported` is a start failure.
pub fn check_capabilities(result: &Value) -> Result<(), OpenError> {
    let loads = result
        .pointer("/agentCapabilities/loadSession")
        .and_then(Value::as_bool)
        == Some(true);
    let steers = result
        .pointer("/_meta/steering/supported")
        .and_then(Value::as_bool)
        == Some(true);
    match (loads, steers) {
        (true, true) => Ok(()),
        (false, _) => Err(OpenError::Unsupported(
            "this version of the agent cannot reload sessions; update it".to_owned(),
        )),
        (_, false) => Err(OpenError::Unsupported(
            "this version of the agent cannot take messages while the agent works; update it".to_owned(),
        )),
    }
}

/// How the turn ended, from the reply to `session/prompt`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TurnEnd {
    /// `end_turn` and `cancelled`.
    Ended,
    Failed(String),
    /// The connection closed before the reply: the process is gone, and its exit says why.
    Closed,
}

/// The stop reason of the reply of `session/prompt`, or a request error.
pub fn turn_end(reply: Result<Value, RpcError>) -> TurnEnd {
    match reply {
        Ok(result) => match result.get("stopReason").and_then(Value::as_str) {
            Some("end_turn" | "cancelled") => TurnEnd::Ended,
            Some("refusal") => TurnEnd::Failed("the agent refused".to_owned()),
            Some("max_tokens") => TurnEnd::Failed("the agent ran out of tokens".to_owned()),
            Some("max_turn_requests") => {
                TurnEnd::Failed("the agent reached its turn limit".to_owned())
            }
            Some(other) => TurnEnd::Failed(format!("the turn ended with `{other}`")),
            None => TurnEnd::Failed("the agent answered the prompt without a stop reason".to_owned()),
        },
        Err(RpcError::Closed) => TurnEnd::Closed,
        Err(error) => TurnEnd::Failed(error.text()),
    }
}

/// The answer of `_session/steering`: `injected`, `promptRequired`, or `failed` (or an error).
pub fn steering_outcome(reply: Result<Value, RpcError>) -> SteeringOutcome {
    match reply {
        Ok(result) => match result.get("outcome").and_then(Value::as_str) {
            Some("injected") => SteeringOutcome::Injected,
            Some("promptRequired") => SteeringOutcome::PromptRequired,
            Some("failed") => SteeringOutcome::Failed(
                result
                    .get("reason")
                    .and_then(Value::as_str)
                    .unwrap_or("the agent could not take the message")
                    .to_owned(),
            ),
            other => {
                SteeringOutcome::Failed(format!("the agent answered `{}`", other.unwrap_or("nothing")))
            }
        },
        Err(error) => SteeringOutcome::Failed(error.text()),
    }
}

/// The reason a failure shows (`requirements/ui#system-certificates`, "Where they replace the
/// reason"): the proxy explanation when any of `texts` (the whole ACP error message, the stderr
/// tail, the reason itself) is a certificate or an HTTP error, else `plain`. `system_certificates_on`
/// is the setting at the moment of the failure. The result is stored as written: a later change of
/// the setting rewrites nothing.
pub fn explained(plain: &str, texts: &[&str], system_certificates_on: bool) -> String {
    failure::explain(&texts.join("\n"), system_certificates_on).unwrap_or_else(|| plain.to_owned())
}

impl TurnEnd {
    /// A failed turn with its reason [`explained`] by the error and the stderr tail.
    pub fn explained(self, stderr: &str, system_certificates_on: bool) -> TurnEnd {
        match self {
            TurnEnd::Failed(reason) => {
                let shown = explained(&reason, &[&reason, stderr], system_certificates_on);
                TurnEnd::Failed(shown)
            }
            other => other,
        }
    }
}

/// A refused message with its reason [`explained`]; the other outcomes stay.
pub fn explained_outcome(
    outcome: SteeringOutcome,
    stderr: &str,
    system_certificates_on: bool,
) -> SteeringOutcome {
    match outcome {
        SteeringOutcome::Failed(reason) => {
            let shown = explained(&reason, &[&reason, stderr], system_certificates_on);
            SteeringOutcome::Failed(shown)
        }
        other => other,
    }
}

fn text_prompt(text: &str) -> Value {
    json!([{ "type": "text", "text": text }])
}

/// `initialize`, and the check of what omp can do.
pub async fn initialize(rpc: &Rpc) -> Result<(), OpenError> {
    let result = rpc.request("initialize", initialize_params()).await?;
    check_capabilities(&result)
}

/// `session/new` in `cwd`; the session id.
pub async fn new_session(rpc: &Rpc, cwd: &Path) -> Result<String, OpenError> {
    let result = rpc
        .request(
            "session/new",
            json!({ "cwd": cwd.to_string_lossy(), "mcpServers": [] }),
        )
        .await?;
    result
        .get("sessionId")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| OpenError::Failed("the agent opened a session without an identity".to_owned()))
}

/// `session/load`, sent ordered like `session/prompt`: the history is replayed as `session/update`
/// notifications before it answers, and the answer comes back as an
/// [`super::rpc::Incoming::Response`] with the id returned, always after every replayed update
/// that was read before it. The caller handles the load as finished only at that response, so no
/// replayed update reaches the live conversation.
pub fn load_session(rpc: &Rpc, session: &str, cwd: &Path) -> u64 {
    rpc.send_ordered(
        "session/load",
        json!({ "sessionId": session, "cwd": cwd.to_string_lossy(), "mcpServers": [] }),
    )
}

/// `session/prompt`: the answer comes back as an [`super::rpc::Incoming::Response`] with the id
/// returned, after everything omp said during the turn.
pub fn prompt(rpc: &Rpc, session: &str, text: &str) -> u64 {
    rpc.send_ordered(
        "session/prompt",
        json!({ "sessionId": session, "prompt": text_prompt(text) }),
    )
}

/// `_session/steering`: while the agent works a message goes out with no `_meta`; a message that
/// waited for the end of a wait goes out as a follow-up.
pub async fn steer(rpc: &Rpc, session: &str, text: &str, follow_up: bool) -> SteeringOutcome {
    let mut params = json!({ "sessionId": session, "prompt": text_prompt(text) });
    if follow_up {
        params["_meta"] = json!({ "steering": { "queue": "followUp" } });
    }
    steering_outcome(rpc.request("_session/steering", params).await)
}

/// The answer to a permission request that the developer settled with `option`.
pub fn selected(option: &str) -> Value {
    json!({ "outcome": { "outcome": "selected", "optionId": option } })
}

pub fn cancelled() -> Value {
    json!({ "outcome": { "outcome": "cancelled" } })
}

/// `yolo`: the first `allow_always` option, else `allow_once`; `None` when there is neither.
pub fn auto_option(options: &Value) -> Option<String> {
    let options = options.as_array()?;
    let find = |kind: &str| {
        options
            .iter()
            .find(|o| o.get("kind").and_then(Value::as_str) == Some(kind))
    };
    find("allow_always")
        .or_else(|| find("allow_once"))
        .and_then(|o| o.get("optionId"))
        .and_then(Value::as_str)
        .map(str::to_owned)
}

/// The answer to an elicitation or an ask: declined, with the fixed text.
pub fn decline() -> Value {
    json!({ "action": "decline", "_meta": { "message": DECLINE_TEXT } })
}

/// Whether a method of omp is an elicitation or an ask of the agent that Weaver declines.
pub fn is_elicitation(method: &str) -> bool {
    method == "elicitation/create" || method.starts_with("elicitation/") || method.contains("ask")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initialize_offers_no_fs_and_no_terminal_and_asks_through_forms() {
        let params = initialize_params();
        assert_eq!(params["protocolVersion"], 1);
        let capabilities = params["clientCapabilities"].as_object().unwrap();
        assert!(!capabilities.contains_key("fs") && !capabilities.contains_key("terminal"));
        assert!(capabilities["elicitation"]["form"].is_object());
    }

    #[test]
    fn a_session_that_cannot_be_reloaded_or_steered_is_a_start_failure() {
        let both = json!({ "agentCapabilities": { "loadSession": true }, "_meta": { "steering": { "supported": true } } });
        assert_eq!(check_capabilities(&both), Ok(()));
        let no_load = json!({ "agentCapabilities": { "loadSession": false }, "_meta": { "steering": { "supported": true } } });
        assert!(
            matches!(check_capabilities(&no_load), Err(OpenError::Unsupported(text)) if text.contains("reload"))
        );
        let no_steering = json!({ "agentCapabilities": { "loadSession": true } });
        assert!(
            matches!(check_capabilities(&no_steering), Err(OpenError::Unsupported(text)) if text.contains("while the agent works"))
        );
        assert_eq!(
            OpenError::Timeout.reason(),
            "the agent did not answer within 30 s"
        );
    }

    #[test]
    fn the_stop_reason_decides_how_the_turn_ended() {
        let reply = |reason: &str| turn_end(Ok(json!({ "stopReason": reason })));
        assert_eq!(reply("end_turn"), TurnEnd::Ended);
        assert_eq!(reply("cancelled"), TurnEnd::Ended);
        assert_eq!(
            reply("refusal"),
            TurnEnd::Failed("the agent refused".into())
        );
        assert_eq!(
            reply("max_tokens"),
            TurnEnd::Failed("the agent ran out of tokens".into())
        );
        assert_eq!(
            reply("max_turn_requests"),
            TurnEnd::Failed("the agent reached its turn limit".into())
        );
        assert!(matches!(reply("weird"), TurnEnd::Failed(text) if text.contains("weird")));
        assert!(matches!(turn_end(Ok(json!({}))), TurnEnd::Failed(_)));
        assert_eq!(
            turn_end(Err(RpcError::Remote {
                code: -1,
                message: "quota exceeded".into(),
                data: None
            })),
            TurnEnd::Failed("quota exceeded".into())
        );
        assert_eq!(turn_end(Err(RpcError::Closed)), TurnEnd::Closed);
    }

    #[test]
    fn the_steering_answer_is_injected_prompt_required_or_failed() {
        assert_eq!(
            steering_outcome(Ok(json!({ "outcome": "injected", "queue": "steer" }))),
            SteeringOutcome::Injected
        );
        assert_eq!(
            steering_outcome(Ok(json!({ "outcome": "promptRequired" }))),
            SteeringOutcome::PromptRequired
        );
        assert_eq!(
            steering_outcome(Ok(json!({ "outcome": "failed", "reason": "no turn" }))),
            SteeringOutcome::Failed("no turn".into())
        );
        assert!(matches!(
            steering_outcome(Ok(json!({ "outcome": "odd" }))),
            SteeringOutcome::Failed(_)
        ));
        assert_eq!(
            steering_outcome(Err(RpcError::Remote {
                code: -32602,
                message: "prompt is empty".into(),
                data: None
            })),
            SteeringOutcome::Failed("prompt is empty".into())
        );
    }

    fn remote(message: &str, data: Option<&str>) -> RpcError {
        RpcError::Remote {
            code: -32603,
            message: message.into(),
            data: data.map(str::to_owned),
        }
    }

    #[test]
    fn a_proxy_error_carried_only_in_the_data_of_the_acp_error_is_explained() {
        let cause = r#"{"cause":"fetch failed: UNABLE_TO_GET_ISSUER_CERT_LOCALLY"}"#;
        for on in [false, true] {
            let expected = failure::explain(cause, on).unwrap();
            let turn = turn_end(Err(remote("Internal error", Some(cause)))).explained("", on);
            assert_eq!(turn, TurnEnd::Failed(expected.clone()), "{on}");
            let steer = explained_outcome(
                steering_outcome(Err(remote("Internal error", Some(cause)))),
                "",
                on,
            );
            assert_eq!(steer, SteeringOutcome::Failed(expected.clone()), "{on}");
            let open = OpenError::from(remote("Internal error", Some(cause)));
            assert_eq!(explained(&open.reason(), &[&open.reason()], on), expected);
        }
        let http = turn_end(Err(remote("Internal error", Some("HTTP 407")))).explained("", false);
        assert!(
            matches!(http, TurnEnd::Failed(text) if text.starts_with("the agent received an HTTP 407 error"))
        );
        // A certificate error in the data wins over an HTTP one in the message.
        let both = remote("HTTP 502", Some("SELF_SIGNED_CERT_IN_CHAIN")).text();
        assert!(failure::explain(&both, false)
            .unwrap()
            .contains("verify a server's certificate"));
    }

    #[test]
    fn the_data_of_an_error_that_is_not_a_proxy_problem_stays_out_of_its_text() {
        assert_eq!(
            remote("quota exceeded", Some(r#"{"code":429}"#)).text(),
            "quota exceeded"
        );
        assert_eq!(
            remote("quota exceeded", Some("HTTP 429")).text(),
            "quota exceeded"
        );
        assert_eq!(remote("HTTP 503", Some("HTTP 407")).text(), "HTTP 503");
        assert_eq!(remote("quota exceeded", None).text(), "quota exceeded");
    }

    #[test]
    fn yolo_picks_the_first_allow_always_else_allow_once() {
        let options = json!([
            { "optionId": "r", "kind": "reject_once", "name": "No" },
            { "optionId": "once", "kind": "allow_once", "name": "Once" },
            { "optionId": "always", "kind": "allow_always", "name": "Always" },
            { "optionId": "always2", "kind": "allow_always", "name": "Always 2" },
        ]);
        assert_eq!(auto_option(&options).as_deref(), Some("always"));
        assert_eq!(
            auto_option(&json!([{ "optionId": "once", "kind": "allow_once" }])).as_deref(),
            Some("once")
        );
        assert_eq!(
            auto_option(&json!([{ "optionId": "r", "kind": "reject_once" }])),
            None
        );
        assert_eq!(auto_option(&json!(null)), None);
        assert_eq!(
            selected("x"),
            json!({ "outcome": { "outcome": "selected", "optionId": "x" } })
        );
        assert_eq!(
            cancelled(),
            json!({ "outcome": { "outcome": "cancelled" } })
        );
    }

    #[test]
    fn forms_and_asks_are_declined_with_the_fixed_text() {
        assert_eq!(decline()["action"], "decline");
        assert_eq!(decline()["_meta"]["message"], DECLINE_TEXT);
        assert!(is_elicitation("elicitation/create"));
        assert!(is_elicitation("session/ask"));
        assert!(!is_elicitation("session/request_permission"));
        assert!(!is_elicitation("fs/read_text_file"));
    }
}

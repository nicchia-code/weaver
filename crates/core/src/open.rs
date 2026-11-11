//! The request kind `open`: `weaver open [<folder>]` asks the running app to open a folder as a
//! project (`developer.OPEN_PROJECT`) or to show its welcome window (`developer.SHOW_WELCOME`).
//!
//! It has the envelope and the protocol version of every other kind. The payload is
//! [`OpenRequest`]: the canonical path of the folder, absent for the welcome window. The app
//! answers once the project window is open or at the front (or the welcome window is shown) with
//! an empty object, or with a failure whose message is the reason the folder did not open. An app
//! of an older version answers `unsupported`.
//!
//! The command is for the developer's own terminal: the CLI refuses it for an agent before sending
//! anything, and the app refuses a request that carries the identity of a job or of the Chat.

use serde::{Deserialize, Serialize};

/// `Request::kind` of `weaver open`.
pub const KIND: &str = "open";

/// The code with which an app that does not know the kind answers.
pub const UNSUPPORTED: &str = "unsupported";

/// How long the CLI waits for the answer, in seconds.
pub const ANSWER_LIMIT_SECONDS: u64 = 30;

/// Why an agent cannot open folders: the CLI and the app refuse with the same words.
pub const AGENT_REFUSAL: &str =
    "`weaver open` is for the developer's terminal: an agent cannot open folders in Weaver.";

/// What the CLI says when the app answers `unsupported`.
pub const UNSUPPORTED_MESSAGE: &str =
    "This version of Weaver cannot open folders from the command line; update Weaver.";

/// What the CLI says when no app answers.
pub const NOT_RUNNING_MESSAGE: &str = "`weaver open` needs Weaver, and Weaver is not running.";

/// What the CLI says when the app does not answer in time.
pub const NO_ANSWER_MESSAGE: &str = "Weaver did not answer within 30 s; the folder may still open.";

/// The payload of a request `open`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenRequest {
    /// The canonical path of the folder to open; absent to show the welcome window.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// `WEAVER_GENERAL`, when the request comes from the agent of the Chat. The CLI never sets it
    /// (it refuses an agent itself); the app refuses a request that has it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub general: Option<String>,
    /// `WEAVER_ASK`, when the request comes from an Ask more conversation; refused like `general`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ask: Option<String>,
}

impl OpenRequest {
    /// A request for `path`, or for the welcome window when `None`.
    pub fn new(path: Option<String>) -> Self {
        Self {
            path,
            general: None,
            ask: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn a_request_has_only_the_path_and_none_for_the_welcome_window() {
        let with_path = serde_json::to_value(OpenRequest::new(Some("/p".into()))).unwrap();
        assert_eq!(with_path, json!({ "path": "/p" }));
        let welcome = serde_json::to_value(OpenRequest::new(None)).unwrap();
        assert_eq!(welcome, json!({}));
    }

    #[test]
    fn a_request_reads_back_and_a_payload_without_fields_is_the_welcome_window() {
        let request: OpenRequest = serde_json::from_value(json!({ "path": "/p" })).unwrap();
        assert_eq!(request, OpenRequest::new(Some("/p".into())));
        let welcome: OpenRequest = serde_json::from_value(json!({})).unwrap();
        assert_eq!(welcome.path, None);
    }

    #[test]
    fn the_wait_limit_is_the_one_the_message_names() {
        assert_eq!(ANSWER_LIMIT_SECONDS, 30);
        assert!(NO_ANSWER_MESSAGE.contains("30 s"));
    }

    #[test]
    fn no_message_is_in_the_first_person() {
        for text in [AGENT_REFUSAL, UNSUPPORTED_MESSAGE, NOT_RUNNING_MESSAGE, NO_ANSWER_MESSAGE] {
            for word in text.split(|c: char| !c.is_alphanumeric() && c != '\'') {
                assert!(!["I", "we", "We", "my", "our"].contains(&word), "{text}");
            }
        }
    }
}

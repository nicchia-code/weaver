//! CLI↔app protocol: newline-delimited JSON, at most 1 MiB per message, one request per
//! connection, every message carrying the protocol version.
//!
//! The functions here are transport-agnostic and synchronous; [`crate::ipc`] runs them over a
//! local socket. Future commands add values of `Request::kind` and their payload types in the
//! module that handles them, without changing the envelope.

use std::io::{BufRead, Read};

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;
use subtle::ConstantTimeEq;

/// Current protocol version; a message with another version is rejected.
pub const PROTOCOL_VERSION: u32 = 1;

/// Largest message in bytes, newline excluded: 1 MiB.
pub const MAX_MESSAGE_BYTES: usize = 1024 * 1024;

/// A request from the CLI to the app.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Request {
    pub version: u32,
    /// The per-launch secret from `runtime.json`.
    pub secret: String,
    /// Job identity (`WEAVER_JOB`), when the request comes from a job.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub job: Option<String>,
    /// Chat identity (`WEAVER_GENERAL`), when the request comes from the agent of the Chat.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub general: Option<String>,
    /// What is asked, for example `question.new`.
    pub kind: String,
    #[serde(default)]
    pub payload: Value,
}

impl Request {
    /// A request of the current protocol version.
    pub fn new(secret: impl Into<String>, kind: impl Into<String>, payload: Value) -> Self {
        Self {
            version: PROTOCOL_VERSION,
            secret: secret.into(),
            job: None,
            general: None,
            kind: kind.into(),
            payload,
        }
    }
}

/// The reply of the app.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Response {
    pub version: u32,
    #[serde(default)]
    pub payload: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorBody>,
}

impl Response {
    pub fn ok(payload: Value) -> Self {
        Self {
            version: PROTOCOL_VERSION,
            payload,
            error: None,
        }
    }

    pub fn failure(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            version: PROTOCOL_VERSION,
            payload: Value::Null,
            error: Some(ErrorBody {
                code: code.into(),
                message: message.into(),
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ErrorBody {
    pub code: String,
    pub message: String,
}

#[derive(Debug, thiserror::Error)]
pub enum ProtocolError {
    /// The peer closed the connection before a complete message arrived.
    #[error("connection closed before a complete message")]
    Closed,
    #[error("message exceeds {MAX_MESSAGE_BYTES} bytes")]
    TooLarge,
    #[error("malformed message: {0}")]
    Malformed(String),
    #[error("protocol version {got} is not {PROTOCOL_VERSION}")]
    VersionMismatch { got: u32 },
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// Serialises a message to one line (JSON + `\n`), refusing more than [`MAX_MESSAGE_BYTES`].
pub fn encode<T: Serialize>(message: &T) -> Result<Vec<u8>, ProtocolError> {
    let mut bytes =
        serde_json::to_vec(message).map_err(|e| ProtocolError::Malformed(e.to_string()))?;
    if bytes.len() > MAX_MESSAGE_BYTES {
        return Err(ProtocolError::TooLarge);
    }
    bytes.push(b'\n');
    Ok(bytes)
}

/// Reads one line without its newline. Never buffers more than [`MAX_MESSAGE_BYTES`] + 2 bytes.
///
/// End of stream before the newline is [`ProtocolError::Closed`]; a line longer than
/// [`MAX_MESSAGE_BYTES`] is [`ProtocolError::TooLarge`].
pub fn read_line<R: BufRead>(reader: &mut R) -> Result<Vec<u8>, ProtocolError> {
    let limit = MAX_MESSAGE_BYTES as u64 + 2;
    let mut line = Vec::new();
    reader.by_ref().take(limit).read_until(b'\n', &mut line)?;
    if line.last() == Some(&b'\n') {
        line.pop();
        if line.len() > MAX_MESSAGE_BYTES {
            return Err(ProtocolError::TooLarge);
        }
        Ok(line)
    } else if line.len() as u64 >= limit {
        Err(ProtocolError::TooLarge)
    } else {
        Err(ProtocolError::Closed)
    }
}

#[derive(Deserialize)]
struct Header {
    version: u32,
}

/// Decodes a line into `T`, checking the protocol version before the rest of the shape, so a
/// message of another version is a mismatch and not a parse error.
pub fn decode<T: DeserializeOwned>(line: &[u8]) -> Result<T, ProtocolError> {
    if line.len() > MAX_MESSAGE_BYTES {
        return Err(ProtocolError::TooLarge);
    }
    let header: Header =
        serde_json::from_slice(line).map_err(|e| ProtocolError::Malformed(e.to_string()))?;
    if header.version != PROTOCOL_VERSION {
        return Err(ProtocolError::VersionMismatch {
            got: header.version,
        });
    }
    serde_json::from_slice(line).map_err(|e| ProtocolError::Malformed(e.to_string()))
}

/// Constant-time comparison of the secrets. A different length compares unequal.
pub fn secrets_match(expected: &str, given: &str) -> bool {
    expected.as_bytes().ct_eq(given.as_bytes()).into()
}

/// Why the server refuses a connection. The connection is closed with no response in every case.
#[derive(Debug, thiserror::Error)]
pub enum Rejection {
    #[error("wrong or missing secret")]
    BadSecret,
    #[error(transparent)]
    Protocol(#[from] ProtocolError),
}

/// Server-side gate for one request line: size, version, shape, then secret.
pub fn accept_request(line: &[u8], expected_secret: &str) -> Result<Request, Rejection> {
    let request: Request = decode(line)?;
    if secrets_match(expected_secret, &request.secret) {
        Ok(request)
    } else {
        Err(Rejection::BadSecret)
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use serde_json::json;

    use super::*;

    const SECRET: &str = "s3cret";

    fn request_line(secret: &str) -> Vec<u8> {
        let mut line = encode(&Request::new(secret, "ping", json!({"a": 1}))).unwrap();
        line.pop();
        line
    }

    #[test]
    fn a_valid_request_is_accepted() {
        let request = accept_request(&request_line(SECRET), SECRET).unwrap();
        assert_eq!(request.kind, "ping");
        assert_eq!(request.version, PROTOCOL_VERSION);
    }

    #[test]
    fn wrong_or_missing_secret_is_rejected() {
        assert!(matches!(
            accept_request(&request_line("nope"), SECRET),
            Err(Rejection::BadSecret)
        ));
        assert!(matches!(
            accept_request(&request_line(""), SECRET),
            Err(Rejection::BadSecret)
        ));
        let no_secret = br#"{"version":1,"kind":"ping"}"#;
        assert!(matches!(
            accept_request(no_secret, SECRET),
            Err(Rejection::Protocol(ProtocolError::Malformed(_)))
        ));
    }

    #[test]
    fn another_version_is_rejected_even_when_the_shape_differs() {
        let future = br#"{"version":2,"something":"else"}"#;
        assert!(matches!(
            accept_request(future, SECRET),
            Err(Rejection::Protocol(ProtocolError::VersionMismatch {
                got: 2
            }))
        ));
        let missing = br#"{"kind":"ping","secret":"s3cret"}"#;
        assert!(matches!(
            accept_request(missing, SECRET),
            Err(Rejection::Protocol(ProtocolError::Malformed(_)))
        ));
    }

    #[test]
    fn a_message_over_one_mib_is_refused_when_encoding() {
        let big = Request::new(SECRET, "ping", json!("x".repeat(MAX_MESSAGE_BYTES)));
        assert!(matches!(encode(&big), Err(ProtocolError::TooLarge)));
    }

    #[test]
    fn a_message_of_exactly_one_mib_is_the_largest_allowed() {
        let mut line = vec![b'a'; MAX_MESSAGE_BYTES];
        line.push(b'\n');
        assert_eq!(
            read_line(&mut Cursor::new(line)).unwrap().len(),
            MAX_MESSAGE_BYTES
        );
    }

    #[test]
    fn a_line_over_one_mib_is_refused_when_reading_without_buffering_it_all() {
        let mut data = vec![b'a'; MAX_MESSAGE_BYTES + 1];
        data.push(b'\n');
        assert!(matches!(
            read_line(&mut Cursor::new(data)),
            Err(ProtocolError::TooLarge)
        ));

        // No newline at all: the reader stops after the limit, it does not wait for more.
        let endless = std::io::repeat(b'a');
        let mut reader = std::io::BufReader::new(endless);
        assert!(matches!(
            read_line(&mut reader),
            Err(ProtocolError::TooLarge)
        ));
    }

    #[test]
    fn end_of_stream_before_the_newline_is_closed() {
        assert!(matches!(
            read_line(&mut Cursor::new(b"{}".to_vec())),
            Err(ProtocolError::Closed)
        ));
        assert!(matches!(
            read_line(&mut Cursor::new(Vec::new())),
            Err(ProtocolError::Closed)
        ));
    }

    #[test]
    fn only_the_first_line_is_read() {
        let mut cursor = Cursor::new(b"one\ntwo\n".to_vec());
        assert_eq!(read_line(&mut cursor).unwrap(), b"one");
    }

    #[test]
    fn secrets_of_other_length_do_not_match() {
        assert!(secrets_match("abc", "abc"));
        assert!(!secrets_match("abc", "abcd"));
        assert!(!secrets_match("abc", ""));
    }

    #[test]
    fn responses_round_trip() {
        let line = encode(&Response::failure("project-busy", "project busy")).unwrap();
        let back: Response = decode(&line[..line.len() - 1]).unwrap();
        assert_eq!(back.error.unwrap().code, "project-busy");
    }
}

//! JSON-RPC 2.0 over the standard streams of omp: one JSON value per line (ACP's stdio transport).
//!
//! Plain threads do the blocking I/O (one reads, one writes), so that no async task ever waits on a
//! pipe: requests are answered through one-shot channels, and what omp sends on its own (its
//! notifications and its requests to the client) arrives on one channel in the order it was read.

use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader, Read, Write};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc as std_mpsc, Arc, Mutex};

use serde_json::{json, Value};
use tokio::sync::{mpsc, oneshot};

/// Why a request got no result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RpcError {
    /// the agent answered with an error: its message and the `data` of the JSON-RPC error (a string as
    /// it is, any other value serialized), where omp may carry the cause.
    Remote {
        code: i64,
        message: String,
        data: Option<String>,
    },
    /// The connection closed (omp exited or closed its output) before the answer.
    Closed,
}

impl RpcError {
    /// The text the developer reads: the message. The `data` follows it only when it changes what
    /// the rules (`requirements/rules#system-certificates`) find: it is a certificate or an HTTP
    /// error and the message is not, or it is a certificate error and the message an HTTP one.
    /// So the whole ACP error is tested wherever the reason is, and other errors keep their text.
    pub fn text(&self) -> String {
        use weaver_core::omp::failure::{classify, ProxyProblem};
        match self {
            RpcError::Remote {
                message,
                data: Some(data),
                ..
            } if matches!(
                (classify(message), classify(data)),
                (None, Some(_))
                    | (
                        Some(ProxyProblem::Http { .. }),
                        Some(ProxyProblem::Certificate { .. })
                    )
            ) =>
            {
                format!("{message}: {data}")
            }
            RpcError::Remote { message, .. } => message.clone(),
            RpcError::Closed => "the agent closed the connection".to_owned(),
        }
    }
}

/// What omp sent on its own.
#[derive(Debug, Clone, PartialEq)]
pub enum Incoming {
    Notification {
        method: String,
        params: Value,
    },
    /// A request of omp to the client; `id` goes back as it came.
    Request {
        id: Value,
        method: String,
        params: Value,
    },
    /// The answer to a request sent with [`Rpc::send_ordered`], in the order it was read among the
    /// notifications: what omp said before the answer is handled before it.
    Response {
        id: u64,
        result: Result<Value, RpcError>,
    },
    /// The output of omp ended: no more messages.
    Closed,
}

type Pending = Arc<Mutex<HashMap<u64, oneshot::Sender<Result<Value, RpcError>>>>>;

/// The client side of one connection.
pub struct Rpc {
    writer: std_mpsc::Sender<String>,
    next_id: AtomicU64,
    pending: Pending,
    ordered: Arc<Mutex<HashSet<u64>>>,
    closed: Arc<AtomicBool>,
    incoming: mpsc::UnboundedSender<Incoming>,
}

impl Rpc {
    /// Starts the reader and the writer thread over `input` (omp's stdout) and `output` (omp's
    /// stdin). Returns the connection and the receiver of what omp sends on its own.
    pub fn start<R, W>(
        input: R,
        mut output: W,
        name: &str,
    ) -> std::io::Result<(Arc<Rpc>, mpsc::UnboundedReceiver<Incoming>)>
    where
        R: Read + Send + 'static,
        W: Write + Send + 'static,
    {
        let pending: Pending = Arc::default();
        let (incoming_tx, incoming_rx) = mpsc::unbounded_channel();
        let (writer_tx, writer_rx) = std_mpsc::channel::<String>();

        std::thread::Builder::new()
            .name(format!("{name}-write"))
            .spawn(move || {
                for line in writer_rx {
                    if output
                        .write_all(line.as_bytes())
                        .and_then(|()| output.write_all(b"\n"))
                        .and_then(|()| output.flush())
                        .is_err()
                    {
                        break;
                    }
                }
            })?;

        let closed = Arc::new(AtomicBool::new(false));
        let ordered: Arc<Mutex<HashSet<u64>>> = Arc::default();
        let reader_pending = pending.clone();
        let reader_closed = closed.clone();
        let reader_ordered = ordered.clone();
        let rpc_incoming = incoming_tx.clone();
        std::thread::Builder::new()
            .name(format!("{name}-read"))
            .spawn(move || {
                for line in BufReader::new(input).lines() {
                    let Ok(line) = line else { break };
                    let Ok(message) = serde_json::from_str::<Value>(&line) else {
                        continue;
                    };
                    route(message, &reader_pending, &reader_ordered, &incoming_tx);
                }
                // The output ended: nothing waits for an answer that cannot come.
                reader_closed.store(true, Ordering::SeqCst);
                for (_, waiting) in reader_pending.lock().expect("pending poisoned").drain() {
                    let _ = waiting.send(Err(RpcError::Closed));
                }
                let unanswered: Vec<u64> = reader_ordered
                    .lock()
                    .expect("ordered poisoned")
                    .drain()
                    .collect();
                for id in unanswered {
                    let _ = incoming_tx.send(Incoming::Response {
                        id,
                        result: Err(RpcError::Closed),
                    });
                }
                let _ = incoming_tx.send(Incoming::Closed);
            })?;

        Ok((
            Arc::new(Rpc {
                writer: writer_tx,
                next_id: AtomicU64::new(1),
                pending,
                ordered,
                closed,
                incoming: rpc_incoming,
            }),
            incoming_rx,
        ))
    }

    fn send(&self, message: &Value) {
        // A closed writer means omp is gone: the reader reports it.
        let _ = self.writer.send(message.to_string());
    }

    /// Sends a request and waits for its answer.
    pub async fn request(&self, method: &str, params: Value) -> Result<Value, RpcError> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = oneshot::channel();
        self.pending
            .lock()
            .expect("pending poisoned")
            .insert(id, tx);
        if self.closed.load(Ordering::SeqCst) {
            self.pending.lock().expect("pending poisoned").remove(&id);
            return Err(RpcError::Closed);
        }
        self.send(&json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }));
        rx.await.unwrap_or(Err(RpcError::Closed))
    }

    /// Sends a request whose answer comes back as [`Incoming::Response`], in order with the
    /// notifications omp sends meanwhile. Returns the id of the request.
    pub fn send_ordered(&self, method: &str, params: Value) -> u64 {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        self.ordered.lock().expect("ordered poisoned").insert(id);
        if self.closed.load(Ordering::SeqCst) {
            self.ordered.lock().expect("ordered poisoned").remove(&id);
            let _ = self.incoming.send(Incoming::Response {
                id,
                result: Err(RpcError::Closed),
            });
            return id;
        }
        self.send(&json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }));
        id
    }

    pub fn notify(&self, method: &str, params: Value) {
        self.send(&json!({ "jsonrpc": "2.0", "method": method, "params": params }));
    }

    /// Answers a request of omp.
    pub fn respond(&self, id: &Value, result: Value) {
        self.send(&json!({ "jsonrpc": "2.0", "id": id, "result": result }));
    }

    pub fn respond_error(&self, id: &Value, code: i64, message: &str) {
        self.send(
            &json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } }),
        );
    }
}

fn route(
    message: Value,
    pending: &Pending,
    ordered: &Mutex<HashSet<u64>>,
    incoming: &mpsc::UnboundedSender<Incoming>,
) {
    let method = message
        .get("method")
        .and_then(Value::as_str)
        .map(str::to_owned);
    match (method, message.get("id").cloned()) {
        (Some(method), Some(id)) => {
            let _ = incoming.send(Incoming::Request {
                id,
                method,
                params: message.get("params").cloned().unwrap_or(Value::Null),
            });
        }
        (Some(method), None) => {
            let _ = incoming.send(Incoming::Notification {
                method,
                params: message.get("params").cloned().unwrap_or(Value::Null),
            });
        }
        (None, Some(id)) => {
            let Some(id) = id.as_u64() else { return };
            let answer = match message.get("error") {
                Some(error) => Err(RpcError::Remote {
                    code: error.get("code").and_then(Value::as_i64).unwrap_or(-32603),
                    message: error
                        .get("message")
                        .and_then(Value::as_str)
                        .unwrap_or("the agent answered with an error")
                        .to_owned(),
                    data: error
                        .get("data")
                        .filter(|data| !data.is_null())
                        .map(|data| {
                            data.as_str()
                                .map_or_else(|| data.to_string(), str::to_owned)
                        }),
                }),
                None => Ok(message.get("result").cloned().unwrap_or(Value::Null)),
            };
            if ordered.lock().expect("ordered poisoned").remove(&id) {
                let _ = incoming.send(Incoming::Response { id, result: answer });
            } else if let Some(waiting) = pending.lock().expect("pending poisoned").remove(&id) {
                let _ = waiting.send(answer);
            }
        }
        (None, None) => {}
    }
}

#[cfg(all(test, unix))]
mod tests {
    use std::io::Cursor;
    use std::sync::{Arc, Mutex};

    use super::*;

    /// A writer the test can read back.
    #[derive(Clone, Default)]
    struct Sink(Arc<Mutex<Vec<u8>>>);

    impl Write for Sink {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    /// A pipe: what the test writes the reader reads.
    fn pipe() -> (
        std::os::unix::net::UnixStream,
        std::os::unix::net::UnixStream,
    ) {
        std::os::unix::net::UnixStream::pair().unwrap()
    }

    #[tokio::test]
    async fn a_request_waits_for_its_answer_and_an_error_is_remote() {
        let (mut omp_out, client_in) = pipe();
        let sink = Sink::default();
        let (rpc, _incoming) = Rpc::start(client_in, sink.clone(), "t").unwrap();
        let first = {
            let rpc = rpc.clone();
            tokio::spawn(async move {
                rpc.request("initialize", json!({ "protocolVersion": 1 }))
                    .await
            })
        };
        // The request goes out as one line with id 1.
        loop {
            if !sink.0.lock().unwrap().is_empty() {
                break;
            }
            tokio::task::yield_now().await;
        }
        let sent: Value = serde_json::from_slice(
            sink.0
                .lock()
                .unwrap()
                .split(|b| *b == b'\n')
                .next()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(
            (
                sent["id"].clone(),
                sent["method"].clone(),
                sent["jsonrpc"].clone()
            ),
            (json!(1), json!("initialize"), json!("2.0"))
        );
        writeln!(
            omp_out,
            "{}",
            json!({ "jsonrpc": "2.0", "id": 1, "result": { "ok": true } })
        )
        .unwrap();
        assert_eq!(first.await.unwrap(), Ok(json!({ "ok": true })));

        let failing = {
            let rpc = rpc.clone();
            tokio::spawn(async move { rpc.request("session/new", json!({})).await })
        };
        loop {
            if sink
                .0
                .lock()
                .unwrap()
                .iter()
                .filter(|b| **b == b'\n')
                .count()
                >= 2
            {
                break;
            }
            tokio::task::yield_now().await;
        }
        writeln!(
            omp_out,
            "{}",
            json!({ "jsonrpc": "2.0", "id": 2, "error": { "code": -32000, "message": "no model" } })
        )
        .unwrap();
        assert_eq!(
            failing.await.unwrap(),
            Err(RpcError::Remote {
                code: -32000,
                message: "no model".into(),
                data: None
            })
        );

        // The `data` of the error is kept: a string as it is, anything else serialized.
        for (id, data, expected) in [
            (3_u64, json!("HTTP 407"), Some("HTTP 407".to_owned())),
            (
                4,
                json!({ "cause": "x" }),
                Some(r#"{"cause":"x"}"#.to_owned()),
            ),
            (5, Value::Null, None),
        ] {
            let waiting = {
                let rpc = rpc.clone();
                tokio::spawn(async move { rpc.request("session/new", json!({})).await })
            };
            let lines = usize::try_from(id).unwrap();
            while sink
                .0
                .lock()
                .unwrap()
                .iter()
                .filter(|b| **b == b'\n')
                .count()
                < lines
            {
                tokio::task::yield_now().await;
            }
            writeln!(
                omp_out,
                "{}",
                json!({ "jsonrpc": "2.0", "id": id, "error": { "code": -32603, "message": "Internal error", "data": data } })
            )
            .unwrap();
            assert_eq!(
                waiting.await.unwrap(),
                Err(RpcError::Remote {
                    code: -32603,
                    message: "Internal error".into(),
                    data: expected
                })
            );
        }
    }

    #[tokio::test]
    async fn notifications_and_requests_of_omp_arrive_in_order_and_garbage_is_skipped() {
        let (mut omp_out, client_in) = pipe();
        let (rpc, mut incoming) = Rpc::start(client_in, Sink::default(), "t").unwrap();
        writeln!(omp_out, "not json").unwrap();
        writeln!(
            omp_out,
            "{}",
            json!({ "jsonrpc": "2.0", "method": "session/update", "params": { "n": 1 } })
        )
        .unwrap();
        writeln!(omp_out, "{}", json!({ "jsonrpc": "2.0", "id": "r-9", "method": "session/request_permission", "params": { "n": 2 } })).unwrap();
        assert_eq!(
            incoming.recv().await,
            Some(Incoming::Notification {
                method: "session/update".into(),
                params: json!({ "n": 1 })
            })
        );
        let Some(Incoming::Request { id, method, .. }) = incoming.recv().await else {
            panic!("a request")
        };
        assert_eq!(
            (id.clone(), method.as_str()),
            (json!("r-9"), "session/request_permission")
        );
        rpc.respond(&id, json!({ "outcome": { "outcome": "cancelled" } }));
        rpc.respond_error(&json!(4), -32601, "no such method");
    }

    #[tokio::test]
    async fn the_answer_of_an_ordered_request_is_handled_after_the_notifications_read_before_it() {
        let (mut omp_out, client_in) = pipe();
        let (rpc, mut incoming) = Rpc::start(client_in, Sink::default(), "t").unwrap();
        let load = rpc.send_ordered("session/load", json!({ "sessionId": "s" }));
        // omp replays the history, then answers, all in one go: nothing separates the lines.
        for n in 1..=3 {
            writeln!(
                omp_out,
                "{}",
                json!({ "jsonrpc": "2.0", "method": "session/update", "params": { "n": n } })
            )
            .unwrap();
        }
        writeln!(
            omp_out,
            "{}",
            json!({ "jsonrpc": "2.0", "id": load, "result": { "loaded": true } })
        )
        .unwrap();
        for n in 1..=3 {
            assert_eq!(
                incoming.recv().await,
                Some(Incoming::Notification {
                    method: "session/update".into(),
                    params: json!({ "n": n })
                })
            );
        }
        assert_eq!(
            incoming.recv().await,
            Some(Incoming::Response {
                id: load,
                result: Ok(json!({ "loaded": true }))
            })
        );
    }

    #[tokio::test]
    async fn the_end_of_the_output_fails_every_request_that_waits_and_is_reported_once() {
        let (omp_out, client_in) = pipe();
        let (rpc, mut incoming) = Rpc::start(client_in, Sink::default(), "t").unwrap();
        let waiting = {
            let rpc = rpc.clone();
            tokio::spawn(async move { rpc.request("session/prompt", json!({})).await })
        };
        tokio::task::yield_now().await;
        drop(omp_out);
        assert_eq!(waiting.await.unwrap(), Err(RpcError::Closed));
        assert_eq!(incoming.recv().await, Some(Incoming::Closed));
        // A request after the end fails at once.
        assert_eq!(rpc.request("x", json!({})).await, Err(RpcError::Closed));
    }

    #[test]
    fn reading_from_memory_works_too() {
        let input = Cursor::new(b"{\"jsonrpc\":\"2.0\",\"method\":\"m\",\"params\":{}}\n".to_vec());
        let (_rpc, mut incoming) = Rpc::start(input, Sink::default(), "t").unwrap();
        let received = tauri::async_runtime::block_on(async {
            (incoming.recv().await, incoming.recv().await)
        });
        assert_eq!(
            received.0,
            Some(Incoming::Notification {
                method: "m".into(),
                params: json!({})
            })
        );
        assert_eq!(received.1, Some(Incoming::Closed));
    }
}

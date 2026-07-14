use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::process::Child;
use tokio::sync::{mpsc, oneshot};

use crate::protocol::{ClientRequest, RequestId, ServerNotification, ServerRequest};
use tokio::io::AsyncWriteExt as _;

use crate::transport::{self, AsyncBufRead, AsyncWrite, OutgoingReply};
use crate::{Error, Result};

/// Default timeout for [`CodexAppServerClient::call_request`] (and therefore
/// every generated per-method wrapper). Override with
/// [`CodexAppServerClient::with_call_timeout`]. This bounds one request/response
/// round trip - it has nothing to do with how long a turn takes to finish
/// generating, since that streams via [`crate::Event::Notification`] instead
/// of blocking the request that started it.
pub const DEFAULT_CALL_TIMEOUT: Duration = Duration::from_secs(120);

/// Timeout for a single outgoing line write. A `write_all`/`flush` to a healthy
/// pipe or socket completes in microseconds; if it hasn't completed in this
/// long the peer is almost certainly stalled (backpressure with nobody
/// reading), and we'd rather tear the connection down - failing every pending
/// and future call with a clear error - than hang forever with an
/// ever-growing outgoing queue and no diagnostics.
const WRITE_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone)]
struct RpcErrorPayload {
    code: i64,
    message: String,
    data: Option<serde_json::Value>,
}

type PendingSender = oneshot::Sender<std::result::Result<serde_json::Value, RpcErrorPayload>>;
type PendingMap = Arc<Mutex<HashMap<i64, PendingSender>>>;

/// An event pushed from the app-server connection.
#[derive(Debug)]
pub enum Event {
    /// A fire-and-forget server notification (`turn/completed`, `item/started`, etc.).
    Notification(ServerNotification),
    /// A request the app-server expects a reply to (approvals, elicitation, etc.).
    Request(PendingServerRequest),
    /// The transport closed (EOF or the child process exited). No further
    /// events will be produced; the [`EventStream`] is exhausted.
    Closed,
}

/// Receives [`Event`]s from one app-server connection.
///
/// Own exactly one `EventStream` per connection and keep draining it (even if
/// you only care about requests, not notifications) - the reader task pushes
/// into an unbounded channel, so a stalled consumer only grows memory, but an
/// abandoned one means you'll never see incoming approval/elicitation requests
/// (which then leak, per the caveat on [`PendingServerRequest`]).
pub struct EventStream {
    rx: mpsc::UnboundedReceiver<Event>,
}

impl EventStream {
    pub async fn recv(&mut self) -> Option<Event> {
        self.rx.recv().await
    }
}

/// Drop signal decoupled from any internal task's channel clones. Only
/// [`CodexAppServerClient`] holds (a clone of) the `Arc` wrapping this; the
/// reader task and per-request reply-forwarding tasks intentionally do *not*,
/// so the connection's lifetime tracks "the caller still holds a client
/// handle" rather than "some internal task still happens to hold a sender."
struct ShutdownOnDrop(Mutex<Option<oneshot::Sender<()>>>);

impl Drop for ShutdownOnDrop {
    fn drop(&mut self) {
        if let Some(tx) = self.0.lock().unwrap().take() {
            let _ = tx.send(());
        }
    }
}

/// Async client for the Codex app-server v2 JSON-RPC protocol.
///
/// Cheap to `Clone`; every clone shares the same underlying connection and
/// pending-request table. Construct one with [`Self::spawn`] (launches
/// `codex app-server` as a child process, the common case), [`Self::connect_streams`]
/// (bring your own duplex byte stream), or [`Self::connect_unix`] (a Unix
/// socket to an already-running `codex app-server daemon`).
///
/// You must complete the `initialize` / `initialized` handshake before calling
/// any other method - the app-server rejects everything else on a fresh
/// connection with a `"Not initialized"` error. See [`Self::initialize`] and
/// [`Self::send_initialized`].
#[derive(Clone)]
pub struct CodexAppServerClient {
    write_tx: mpsc::UnboundedSender<String>,
    pending: PendingMap,
    next_id: Arc<AtomicI64>,
    call_timeout: Duration,
    _lifetime: Arc<ShutdownOnDrop>,
}

impl CodexAppServerClient {
    /// Spawns `command app-server` (default `command = "codex"`) with stdio
    /// piped and connects to it over the stdio JSONL transport.
    ///
    /// `extra_args` are appended after `app-server`, e.g.
    /// `["--enable".into(), "some_feature".into()]` or
    /// `["-c".into(), r#"model="gpt-5.4""#.into()]`.
    ///
    /// The child process is killed (`kill_on_drop`) once every clone of the
    /// returned client has been dropped - dropping the last clone always
    /// terminates the child, whether or not it's currently mid-write. It's
    /// also killed automatically if the connection is torn down for any other
    /// reason (a write that exceeds the internal stall timeout, a transport
    /// read error, clean EOF).
    pub fn spawn(command: &str, extra_args: &[String]) -> Result<(Self, EventStream)> {
        let (stdin, reader, child) = transport::spawn_app_server(command, extra_args)?;
        Ok(Self::connect(reader, stdin, Some(child)))
    }

    /// Connects to an already-open duplex stream (e.g. any
    /// `AsyncBufRead + AsyncWrite` pair) speaking the same NDJSON JSON-RPC
    /// protocol. See also [`Self::connect_unix`] for the common case of a
    /// `codex app-server daemon`'s Unix domain socket.
    pub fn connect_streams<R, W>(reader: R, writer: W) -> (Self, EventStream)
    where
        R: AsyncBufRead + Unpin + Send + 'static,
        W: AsyncWrite + Unpin + Send + 'static,
    {
        Self::connect(reader, writer, None)
    }

    /// Connects to a `codex app-server` (or `codex app-server daemon`)
    /// listening on a Unix domain socket (`--listen unix://PATH`).
    #[cfg(unix)]
    pub async fn connect_unix(path: impl AsRef<std::path::Path>) -> Result<(Self, EventStream)> {
        let stream = tokio::net::UnixStream::connect(path).await?;
        let (writer, reader) = transport::split_unix_stream(stream);
        Ok(Self::connect(reader, writer, None))
    }

    /// Returns a client that applies `timeout` to every request/response call
    /// instead of [`DEFAULT_CALL_TIMEOUT`]. Cloning preserves the timeout;
    /// mixing timeouts across clones of the same connection is fine (it's a
    /// per-call setting checked in [`Self::call_request`], not a property of
    /// the shared connection state).
    pub fn with_call_timeout(mut self, timeout: Duration) -> Self {
        self.call_timeout = timeout;
        self
    }

    fn connect<R, W>(mut reader: R, mut writer: W, child: Option<Child>) -> (Self, EventStream)
    where
        R: AsyncBufRead + Unpin + Send + 'static,
        W: AsyncWrite + Unpin + Send + 'static,
    {
        let (write_tx, mut write_rx) = mpsc::unbounded_channel::<String>();
        let (events_tx, events_rx) = mpsc::unbounded_channel::<Event>();
        let pending: PendingMap = Arc::new(Mutex::new(HashMap::new()));
        let (shutdown_tx, mut shutdown_rx) = oneshot::channel::<()>();

        // Writer task: owns the write half + (if spawned) the child process,
        // so the process stays alive exactly as long as this task is running.
        // Exits (and so kills the child) on: an explicit shutdown signal (the
        // last CodexAppServerClient clone was dropped), a write that exceeds
        // WRITE_TIMEOUT (peer stalled), a write I/O error, or every sender
        // having been dropped (fallback - should be unreachable in practice
        // since the reader task and forwarding tasks are careful not to hold
        // long-lived clones past their own task's natural lifetime, but a
        // real fallback rather than an assumption).
        tokio::spawn(async move {
            let _child = child; // kept alive here; dropped (kill_on_drop) when this task ends
            loop {
                tokio::select! {
                    biased;
                    _ = &mut shutdown_rx => break,
                    maybe_line = write_rx.recv() => {
                        let Some(line) = maybe_line else { break };
                        match tokio::time::timeout(WRITE_TIMEOUT, transport::write_line(&mut writer, &line)).await {
                            Ok(Ok(())) => {}
                            Ok(Err(err)) => {
                                tracing::warn!(error = %err, "app-server transport write error; closing connection");
                                break;
                            }
                            Err(_elapsed) => {
                                tracing::warn!(timeout = ?WRITE_TIMEOUT, "app-server write stalled past the timeout; closing connection");
                                break;
                            }
                        }
                    }
                }
            }
            // Explicitly shut down the write half rather than relying on drop
            // alone: for a spawned child, this closes its stdin, giving it a
            // chance to notice and exit cleanly before `kill_on_drop` resorts
            // to a hard kill; for split-stream transports (`connect_streams`,
            // `connect_unix`), a bare drop doesn't reliably signal EOF to the
            // peer when a `ReadHalf` derived from the same underlying stream
            // is still alive elsewhere (as it is here, in the reader task).
            let _ = writer.shutdown().await;
        });

        // Reader task: the only place incoming lines are parsed and dispatched.
        // It needs its own `write_tx` clone to send replies (error responses
        // to undecodable requests, forwarded PendingServerRequest replies),
        // so it necessarily holds one for its whole lifetime - that's fine
        // now, because the writer task's shutdown no longer depends on every
        // `write_tx` clone being dropped (see `ShutdownOnDrop`'s doc comment
        // and the `select!` above); it depends on the client itself dropping.
        let pending_reader = pending.clone();
        let write_tx_for_reader = write_tx.clone();
        tokio::spawn(async move {
            let mut buf = String::new();
            loop {
                match transport::read_line(&mut reader, &mut buf).await {
                    Ok(0) => break, // EOF
                    Ok(_) => {
                        let line = buf.trim();
                        if line.is_empty() {
                            continue;
                        }
                        dispatch_incoming_line(
                            line,
                            &pending_reader,
                            &events_tx,
                            &write_tx_for_reader,
                        );
                    }
                    Err(err) => {
                        tracing::warn!(error = %err, "app-server transport read error");
                        break;
                    }
                }
            }
            pending_reader.lock().unwrap().clear(); // drops senders -> pending calls see TransportClosed
            let _ = events_tx.send(Event::Closed);
        });

        let client = CodexAppServerClient {
            write_tx,
            pending,
            next_id: Arc::new(AtomicI64::new(0)),
            call_timeout: DEFAULT_CALL_TIMEOUT,
            _lifetime: Arc::new(ShutdownOnDrop(Mutex::new(Some(shutdown_tx)))),
        };
        (client, EventStream { rx: events_rx })
    }

    /// Issues one typed request and awaits its response as a raw JSON value,
    /// bounded by [`Self::call_timeout`] (see [`Self::with_call_timeout`]).
    /// Used by the generated per-method wrapper functions
    /// (`thread_start`, `turn_start`, ...); most callers should use those
    /// instead of this directly.
    pub(crate) async fn call_request(
        &self,
        build: impl FnOnce(RequestId) -> ClientRequest,
    ) -> Result<serde_json::Value> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let request = build(RequestId::Int64(id));
        let (tx, rx) = oneshot::channel();
        self.pending.lock().unwrap().insert(id, tx);

        // Removes this id's map entry when this guard drops, which happens
        // whether `call_request` returns normally, is cancelled (e.g. wrapped
        // in `tokio::time::timeout` or a `select!` branch that loses), or
        // panics. Without this, a cancelled call whose response never arrives
        // (or arrives after the caller already gave up) leaks its map entry
        // and `oneshot::Sender` for the life of the connection. Redundant
        // (and harmless - `HashMap::remove` on a missing key is a no-op) on
        // the normal-completion path, where `dispatch_incoming_line` already
        // removed the entry.
        struct RemoveOnDrop<'a> {
            pending: &'a PendingMap,
            id: i64,
        }
        impl Drop for RemoveOnDrop<'_> {
            fn drop(&mut self) {
                self.pending.lock().unwrap().remove(&self.id);
            }
        }
        let _guard = RemoveOnDrop {
            pending: &self.pending,
            id,
        };

        let line = serde_json::to_string(&request)?;
        if self.write_tx.send(line).is_err() {
            return Err(Error::TransportClosed);
        }

        match tokio::time::timeout(self.call_timeout, rx).await {
            Ok(Ok(Ok(value))) => Ok(value),
            Ok(Ok(Err(err))) => Err(Error::Rpc {
                code: err.code,
                message: err.message,
                data: err.data,
            }),
            Ok(Err(_recv_error)) => Err(Error::TransportClosed),
            Err(_elapsed) => Err(Error::Timeout {
                after: self.call_timeout,
            }),
        }
    }

    /// Sends the `initialized` notification. Call this exactly once,
    /// immediately after [`Self::initialize`] succeeds - the app-server
    /// rejects every other method until it's received.
    pub fn send_initialized(&self) -> Result<()> {
        let line = serde_json::to_string(&serde_json::json!({ "method": "initialized" }))?;
        self.write_tx.send(line).map_err(|_| Error::TransportClosed)
    }
}

/// Builds a raw JSON-RPC error response using `id` exactly as received (never
/// reinterpreted through [`RequestId`]), so it round-trips correctly whether
/// the app-server used a string or integer id.
fn error_reply_line(id: &serde_json::Value, code: i64, message: String) -> Option<String> {
    let value = serde_json::json!({ "id": id, "error": { "code": code, "message": message, "data": null } });
    serde_json::to_string(&value).ok()
}

fn dispatch_incoming_line(
    line: &str,
    pending: &PendingMap,
    events_tx: &mpsc::UnboundedSender<Event>,
    write_tx: &mpsc::UnboundedSender<String>,
) {
    let value: serde_json::Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(err) => {
            tracing::warn!(%line, error = %err, "app-server sent a line that is not valid JSON; ignoring");
            return;
        }
    };
    let Some(obj) = value.as_object() else {
        tracing::warn!(%line, "app-server sent a non-object JSON-RPC line; ignoring");
        return;
    };

    if obj.contains_key("method") {
        match obj.get("id") {
            Some(id_value) => {
                let id_value = id_value.clone();
                match serde_json::from_value::<ServerRequest>(value) {
                    Ok(request) => {
                        let (reply_tx, reply_rx) = oneshot::channel::<OutgoingReply>();
                        let write_tx = write_tx.clone();
                        tokio::spawn(async move {
                            if let Ok(reply) = reply_rx.await {
                                if let Ok(line) = reply.into_line() {
                                    let _ = write_tx.send(line);
                                }
                            }
                        });
                        let _ = events_tx
                            .send(Event::Request(PendingServerRequest { request, reply_tx }));
                    }
                    Err(err) => {
                        // The app-server expects a reply for every request it
                        // sends; if we can't understand this one (e.g. a
                        // method added in a newer app-server version than
                        // this crate's schema), silently dropping it leaves
                        // the app-server waiting forever. Fail it explicitly
                        // instead.
                        tracing::warn!(
                            %line, error = %err,
                            "could not decode a server->client request into any known method; \
                             sending back a JSON-RPC error instead of leaving the app-server waiting"
                        );
                        if let Some(reply) = error_reply_line(
                            &id_value,
                            -32601,
                            format!("codex-app-server-client: unrecognized or undecodable request: {err}"),
                        ) {
                            let _ = write_tx.send(reply);
                        }
                    }
                }
            }
            None => match serde_json::from_value::<ServerNotification>(value) {
                Ok(notification) => {
                    let _ = events_tx.send(Event::Notification(notification));
                }
                Err(err) => {
                    // Notifications never expect a reply, so there's nothing
                    // actionable to send back - just make the gap visible.
                    tracing::warn!(
                        %line, error = %err,
                        "could not decode a server notification into any known method; ignoring"
                    );
                }
            },
        }
        return;
    }

    let Some(id_value) = obj.get("id") else {
        tracing::warn!(%line, "app-server sent a message with neither method nor id; ignoring");
        return;
    };
    let Some(id) = id_value.as_i64() else {
        // We only ever mint i64 request ids ourselves, so a non-integer id on
        // a response can't correlate to anything we're waiting on.
        tracing::warn!(%line, "app-server response has a non-integer id we never issued; ignoring");
        return;
    };

    let sender = pending.lock().unwrap().remove(&id);
    let Some(sender) = sender else {
        tracing::debug!(
            id,
            "app-server response for an unknown or already-resolved request id"
        );
        return;
    };

    if let Some(result) = obj.get("result") {
        let _ = sender.send(Ok(result.clone()));
    } else if let Some(error) = obj.get("error") {
        let code = error
            .get("code")
            .and_then(serde_json::Value::as_i64)
            .unwrap_or(-1);
        let message = error
            .get("message")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string();
        let data = error.get("data").cloned();
        let _ = sender.send(Err(RpcErrorPayload {
            code,
            message,
            data,
        }));
    } else {
        tracing::warn!(%line, "app-server response has an id but neither result nor error");
    }
}

include!(concat!(env!("OUT_DIR"), "/methods_generated.rs"));

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, BufReader};

    /// Regression test: dropping every `CodexAppServerClient` clone must
    /// terminate the writer task (and, for a spawned child, kill it) even
    /// though the reader task independently holds its own `write_tx` clone
    /// for its whole lifetime. Verified indirectly via `connect_streams`: the
    /// writer task drops its write half on shutdown, which the peer observes
    /// as EOF.
    #[tokio::test]
    async fn dropping_the_last_client_terminates_the_writer_and_closes_the_transport() {
        let (client_io, mut server_io) = tokio::io::duplex(4096);
        let (client_read, client_write) = tokio::io::split(client_io);
        let (client, _events) =
            CodexAppServerClient::connect_streams(BufReader::new(client_read), client_write);

        drop(client);

        let mut buf = [0u8; 8];
        let result = tokio::time::timeout(Duration::from_secs(5), server_io.read(&mut buf)).await;
        assert!(
            matches!(result, Ok(Ok(0))),
            "expected EOF on the peer side after dropping the last client handle, got {result:?}"
        );
    }

    /// Regression test: cancelling an in-flight call (here, via an outer
    /// `tokio::time::timeout` that drops the `call_request` future before it
    /// resolves) must not leak its entry in the pending-request map.
    #[tokio::test]
    async fn a_cancelled_call_does_not_leak_its_pending_map_entry() {
        let (client_io, _server_io) = tokio::io::duplex(4096);
        // `_server_io` is kept alive but never reads or writes, so no response
        // ever arrives and `call_request`'s own (much longer) timeout won't
        // fire either - only the outer cancellation below will.
        let (client_read, client_write) = tokio::io::split(client_io);
        let (client, _events) =
            CodexAppServerClient::connect_streams(BufReader::new(client_read), client_write);

        let result = tokio::time::timeout(
            Duration::from_millis(50),
            client.call_request(|id| ClientRequest::MemoryReset { id, params: () }),
        )
        .await;
        assert!(
            result.is_err(),
            "expected the outer timeout to cancel call_request before any response arrives"
        );
        assert!(
            client.pending.lock().unwrap().is_empty(),
            "pending map must be empty once the caller gave up on the call"
        );
    }
}

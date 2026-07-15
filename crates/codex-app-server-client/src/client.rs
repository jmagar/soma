use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::process::Child;
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;

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

/// Upper bound on how long the forwarding task spawned for one incoming
/// server->client request (see [`PendingServerRequest`]) will wait for
/// `.respond()`/`.respond_error()` before giving up on its own. This exists
/// purely as a leak backstop: dropping a `PendingServerRequest` *without*
/// responding already resolves the forwarding task immediately (dropping the
/// paired `oneshot::Sender` wakes it with an error), so this timeout only
/// fires for the pathological case of a caller holding onto one indefinitely
/// without ever dropping or responding to it - at which point the task gives
/// up, sends a fallback error reply so the app-server isn't left waiting
/// forever either, and frees its resources (the spawned task itself and its
/// `write_tx` clone). Generous because these are often human-in-the-loop
/// approval/elicitation flows that can legitimately take a while.
const PENDING_SERVER_REQUEST_TIMEOUT: Duration = Duration::from_secs(600);

/// Capacity of the internal channel from the reader task to [`EventStream`].
/// Bounded (rather than unbounded) so a stalled or absent consumer grows
/// memory by a fixed amount, not without bound, if events keep arriving
/// faster than [`EventStream::recv`] is called - see that type's doc comment
/// for the drop policy once this fills up.
const EVENTS_CHANNEL_CAPACITY: usize = 1024;

type PendingSender = oneshot::Sender<std::result::Result<serde_json::Value, Error>>;
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
/// you only care about requests, not notifications). The channel between the
/// reader task and this stream is bounded (see [`EVENTS_CHANNEL_CAPACITY`]):
/// a slow consumer just grows a fixed-size backlog, but a consumer that stops
/// draining entirely will eventually cause the reader task to drop events
/// once that backlog fills. Drop policy when full:
/// - [`Event::Notification`]: dropped and logged - fire-and-forget by design.
/// - [`Event::Request`]: **not** silently dropped - this crate sends a
///   fallback error reply on the app-server's behalf first (so it isn't left
///   hanging), then drops the event.
/// - [`Event::Closed`]: dropped and logged, but harmless - once the reader
///   task ends it drops its sender, so [`Self::recv`] still observes the
///   connection closing (as `None`) even without the explicit event.
pub struct EventStream {
    rx: mpsc::Receiver<Event>,
}

impl EventStream {
    pub async fn recv(&mut self) -> Option<Event> {
        self.rx.recv().await
    }
}

/// A server->client request the app-server sent us (approvals, elicitation,
/// etc.), paired with a one-shot reply channel. Obtain these from
/// [`EventStream::recv`] as an [`Event::Request`].
///
/// Every `PendingServerRequest` you receive must eventually be resolved via
/// [`Self::respond`] or [`Self::respond_error`] - dropping one without
/// responding resolves its internal forwarding task immediately (no leak),
/// but the app-server then gets **no reply at all**, ever; the
/// `PENDING_SERVER_REQUEST_TIMEOUT` fallback error only covers the different
/// case of a caller holding onto one indefinitely without ever dropping or
/// responding to it. Always call [`Self::respond`] or [`Self::respond_error`]
/// - even to report "not handled" - rather than dropping.
pub struct PendingServerRequest {
    pub request: ServerRequest,
    pub(crate) reply_tx: oneshot::Sender<OutgoingReply>,
}

impl std::fmt::Debug for PendingServerRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PendingServerRequest")
            .field("request", &self.request)
            .finish_non_exhaustive()
    }
}

impl PendingServerRequest {
    /// The `RequestId` the app-server expects echoed back in the reply.
    pub fn id(&self) -> &RequestId {
        self.request.id()
    }

    /// The wire method name, e.g. `"execCommandApproval"`.
    pub fn method_name(&self) -> &'static str {
        self.request.method_name()
    }

    /// The name of the `crate::protocol` type [`Self::respond`]'s `result`
    /// must serialize to for this specific request, e.g.
    /// `"ExecCommandApprovalResponse"`. Prefer this over guessing from
    /// [`Self::method_name`] - the naming convention (`PascalCase(method) +
    /// "Response"`) doesn't hold for every method (e.g.
    /// `"item/tool/call"` expects `DynamicToolCallResponse`, not
    /// `ItemToolCallResponse`).
    pub fn expected_response_type_name(&self) -> &'static str {
        self.request.expected_response_type_name()
    }

    /// Send a successful reply. `result` must serialize to the response
    /// shape the app-server expects for this specific method - see
    /// [`Self::expected_response_type_name`] for exactly which
    /// `crate::protocol` type that is.
    pub fn respond(self, result: impl serde::Serialize) -> Result<()> {
        let id = self.id().clone();
        let value = serde_json::to_value(result)?;
        let _ = self
            .reply_tx
            .send(OutgoingReply::Result { id, result: value });
        Ok(())
    }

    /// Send an error reply. Unlike [`Self::respond`], this can't fail - it
    /// builds a fixed-shape error object rather than serializing an
    /// arbitrary caller-supplied value - so it deliberately returns `()`
    /// rather than a `Result` that could only ever be `Ok`.
    pub fn respond_error(
        self,
        code: i64,
        message: impl Into<String>,
        data: Option<serde_json::Value>,
    ) {
        let id = self.id().clone();
        let _ = self.reply_tx.send(OutgoingReply::Error {
            id,
            code,
            message: message.into(),
            data,
        });
    }
}

/// Drop signal decoupled from any internal task's channel clones. Only
/// [`CodexAppServerClient`] holds (a clone of) the `Arc` wrapping this; the
/// reader task and per-request reply-forwarding tasks intentionally do *not*
/// hold *this* handle (though the reader task holds its own [`CancellationToken`]
/// clone directly - see `connect` - so it can also trigger shutdown itself on
/// EOF/error, not just observe it), so the connection's lifetime tracks "the
/// caller still holds a client handle, or the connection is known to be dead"
/// rather than "some internal task still happens to hold a sender."
struct ShutdownOnDrop(CancellationToken);

impl Drop for ShutdownOnDrop {
    fn drop(&mut self) {
        self.0.cancel();
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
    /// also reaped promptly and automatically the moment the connection is
    /// otherwise known to be dead: a transport read error, clean EOF (e.g.
    /// the child crashed or exited), or a write that exceeds the internal
    /// stall timeout - none of these require the caller to drop the client or
    /// issue another call first.
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
        let (events_tx, events_rx) = mpsc::channel::<Event>(EVENTS_CHANNEL_CAPACITY);
        let pending: PendingMap = Arc::new(Mutex::new(HashMap::new()));
        let cancel = CancellationToken::new();

        // Writer task: owns the write half + (if spawned) the child process,
        // so the process stays alive exactly as long as this task is running.
        // Exits (and so kills the child) on: cancellation - triggered either by
        // the last CodexAppServerClient clone being dropped (`ShutdownOnDrop`)
        // *or* by the reader task detecting the connection is dead (EOF/error,
        // see below) - a write that exceeds WRITE_TIMEOUT (peer stalled), a
        // write I/O error, or every sender having been dropped (fallback -
        // should be unreachable in practice since the reader task and
        // forwarding tasks are careful not to hold long-lived clones past
        // their own task's natural lifetime, but a real fallback rather than
        // an assumption).
        let writer_cancel = cancel.clone();
        tokio::spawn(async move {
            let _child = child; // kept alive here; dropped (kill_on_drop) when this task ends
            loop {
                tokio::select! {
                    biased;
                    _ = writer_cancel.cancelled() => break,
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
        // and the `select!` above); it depends on `cancel`.
        let pending_reader = pending.clone();
        let write_tx_for_reader = write_tx.clone();
        let reader_cancel = cancel.clone();
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
            if let Err(err) = events_tx.try_send(Event::Closed) {
                // Harmless: dropping `events_tx` below (end of this task)
                // still makes `EventStream::recv` observe closure as `None`.
                tracing::debug!(
                    ?err,
                    "event channel full/closed while sending Event::Closed"
                );
            }
            // Proactively tear down the writer task (and reap a spawned child)
            // the moment we know the connection is dead, rather than waiting
            // for the caller to notice `Event::Closed` and drop the client, or
            // for the writer's own next write attempt to fail.
            reader_cancel.cancel();
        });

        let client = CodexAppServerClient {
            write_tx,
            pending,
            next_id: Arc::new(AtomicI64::new(0)),
            call_timeout: DEFAULT_CALL_TIMEOUT,
            _lifetime: Arc::new(ShutdownOnDrop(cancel)),
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
            Ok(Ok(result)) => result,
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

/// Number of characters of a raw wire line included in diagnostic logs. This
/// is a truncation, not real redaction - JSON-RPC payloads on this protocol
/// can carry sensitive content (auth tokens, command output, arbitrary
/// agent-generated text), and these log sites fire on *undecodable* messages,
/// which are expected to happen occasionally in normal operation (e.g. a
/// newer app-server version using a method this crate's vendored schema
/// doesn't know about) - not just on truly exceptional/attacker-controlled
/// input. Keeping full lines out of logs by default bounds that exposure; a
/// short preview is still enough to identify which method/shape was involved
/// for debugging, since JSON-RPC messages put `method`/`id` up front.
const LOG_LINE_PREVIEW_CHARS: usize = 200;

fn line_preview(line: &str) -> String {
    if line.chars().count() <= LOG_LINE_PREVIEW_CHARS {
        line.to_string()
    } else {
        let preview: String = line.chars().take(LOG_LINE_PREVIEW_CHARS).collect();
        format!(
            "{preview}... ({} bytes total, truncated for logging)",
            line.len()
        )
    }
}

fn dispatch_incoming_line(
    line: &str,
    pending: &PendingMap,
    events_tx: &mpsc::Sender<Event>,
    write_tx: &mpsc::UnboundedSender<String>,
) {
    let value: serde_json::Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(err) => {
            tracing::warn!(line_preview = %line_preview(line), error = %err, "app-server sent a line that is not valid JSON; ignoring");
            return;
        }
    };
    // Own the parsed object from here on (rather than borrowing via
    // `.as_object()`) so the response/error path below can `.remove()` the
    // `result`/`error` payload instead of cloning it - `result` in
    // particular can be an arbitrarily large value (e.g. a big `fs/readFile`
    // payload), and this is on the per-line hot path.
    let serde_json::Value::Object(mut map) = value else {
        tracing::warn!(line_preview = %line_preview(line), "app-server sent a non-object JSON-RPC line; ignoring");
        return;
    };

    if map.contains_key("method") {
        match map.get("id").cloned() {
            Some(id_value) => {
                let value = serde_json::Value::Object(map);
                match serde_json::from_value::<ServerRequest>(value) {
                    Ok(request) => {
                        let (reply_tx, reply_rx) = oneshot::channel::<OutgoingReply>();
                        let write_tx = write_tx.clone();
                        let timeout_id = id_value.clone();
                        tokio::spawn(async move {
                            match tokio::time::timeout(PENDING_SERVER_REQUEST_TIMEOUT, reply_rx)
                                .await
                            {
                                Ok(Ok(reply)) => {
                                    if let Ok(line) = reply.into_line() {
                                        let _ = write_tx.send(line);
                                    }
                                }
                                Ok(Err(_recv_error)) => {
                                    // The PendingServerRequest was dropped without a
                                    // response - nothing to forward and nothing left
                                    // to wait for; this is not the leak case.
                                }
                                Err(_elapsed) => {
                                    tracing::warn!(
                                        timeout = ?PENDING_SERVER_REQUEST_TIMEOUT,
                                        "no one responded to a server->client request within the \
                                         timeout; sending a fallback error reply instead of leaving \
                                         the app-server waiting forever"
                                    );
                                    if let Some(line) = error_reply_line(
                                        &timeout_id,
                                        -32000,
                                        format!(
                                            "codex-app-server-client: no reply within {PENDING_SERVER_REQUEST_TIMEOUT:?}"
                                        ),
                                    ) {
                                        let _ = write_tx.send(line);
                                    }
                                }
                            }
                        });
                        if let Err(err) = events_tx
                            .try_send(Event::Request(PendingServerRequest { request, reply_tx }))
                        {
                            // The consumer isn't draining EventStream fast enough (Full) or
                            // has dropped it entirely (Closed). Either way, letting `psr`
                            // fall out of scope here would silently drop `reply_tx`, which
                            // resolves the forwarding task immediately with *no* reply ever
                            // sent (see `PendingServerRequest`'s doc comment on bare drops) -
                            // leaving the app-server waiting forever. Reply explicitly
                            // instead of a silent hang.
                            let Event::Request(psr) = err.into_inner() else {
                                unreachable!("we always send Event::Request here")
                            };
                            tracing::warn!(
                                method = psr.method_name(),
                                "event channel unavailable (full or EventStream dropped); \
                                 replying with a fallback error instead of leaving this \
                                 server->client request unanswered"
                            );
                            psr.respond_error(
                                -32000,
                                "codex-app-server-client: event channel unavailable, request \
                                 could not be delivered",
                                None,
                            );
                        }
                    }
                    Err(err) => {
                        // The app-server expects a reply for every request it
                        // sends; if we can't understand this one (e.g. a
                        // method added in a newer app-server version than
                        // this crate's schema), silently dropping it leaves
                        // the app-server waiting forever. Fail it explicitly
                        // instead.
                        tracing::warn!(
                            line_preview = %line_preview(line), error = %err,
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
            None => {
                match serde_json::from_value::<ServerNotification>(serde_json::Value::Object(map)) {
                    Ok(notification) => {
                        // Fire-and-forget: unlike Event::Request, nothing on the other end
                        // is waiting for a reply, so it's safe to just drop this under
                        // backpressure rather than block the reader task. Deliberately not
                        // logging the notification payload itself (could be arbitrarily
                        // large, e.g. a big diff) - just that one was dropped.
                        if let Err(err) = events_tx.try_send(Event::Notification(notification)) {
                            let cause = match err {
                                mpsc::error::TrySendError::Full(_) => "channel full",
                                mpsc::error::TrySendError::Closed(_) => "EventStream dropped",
                            };
                            tracing::warn!(cause, "dropping a server notification");
                        }
                    }
                    Err(err) => {
                        // Notifications never expect a reply, so there's nothing
                        // actionable to send back - just make the gap visible.
                        tracing::warn!(
                            line_preview = %line_preview(line), error = %err,
                            "could not decode a server notification into any known method; ignoring"
                        );
                    }
                }
            }
        }
        return;
    }

    let Some(id) = map.get("id").and_then(serde_json::Value::as_i64) else {
        // Either there's no "id" at all, or it's not an integer - we only
        // ever mint i64 request ids ourselves, so a non-integer id on a
        // response can't correlate to anything we're waiting on either way.
        tracing::warn!(line_preview = %line_preview(line), "app-server sent a response with no id, or an id we never issued; ignoring");
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

    if let Some(result) = map.remove("result") {
        let _ = sender.send(Ok(result));
    } else if let Some(error) = map.remove("error") {
        let code = error
            .get("code")
            .and_then(serde_json::Value::as_i64)
            .unwrap_or(-1);
        let message = error
            .get("message")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string();
        let data = match error {
            serde_json::Value::Object(mut error_map) => error_map.remove("data"),
            _ => None,
        };
        let _ = sender.send(Err(Error::Rpc {
            code,
            message,
            data,
        }));
    } else {
        tracing::warn!(line_preview = %line_preview(line), "app-server response has an id but neither result nor error");
    }
}

include!(concat!(env!("OUT_DIR"), "/methods_generated.rs"));

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};

    /// Regression test: `expected_response_type_name()` must resolve to the
    /// *actual* response type even for methods where the naive
    /// `PascalCase(method) + "Response"` convention is wrong -
    /// `item/tool/call` is one of the 6 (of 11) server-request methods that
    /// need the `RESPONSE_OVERRIDES` table in
    /// `schema/build_combined_schema.py` (it expects `DynamicToolCallResponse`,
    /// not the naive `ItemToolCallResponse`).
    #[tokio::test]
    async fn expected_response_type_name_resolves_irregular_names_correctly() {
        let (reply_tx, _reply_rx) = oneshot::channel::<OutgoingReply>();
        let pending = PendingServerRequest {
            request: ServerRequest::ItemToolCall {
                id: RequestId::Int64(1),
                params: crate::protocol::DynamicToolCallParams {
                    arguments: serde_json::json!({}),
                    call_id: "call-1".into(),
                    namespace: None,
                    thread_id: "thr_test".into(),
                    tool: "some_tool".into(),
                    turn_id: "turn_1".into(),
                },
            },
            reply_tx,
        };
        assert_eq!(
            pending.expected_response_type_name(),
            "DynamicToolCallResponse"
        );
    }

    /// `PendingServerRequest::respond()` must serialize `result` and put it
    /// on the wire as `{"id": ..., "result": ...}` - the exact shape the
    /// app-server expects for a reply to one of its requests. Constructs the
    /// pair directly (bypassing the full dispatch/connection machinery,
    /// which is exercised by other tests) to check the wire format in
    /// isolation.
    #[tokio::test]
    async fn respond_puts_the_correct_shape_on_the_wire() {
        let (reply_tx, reply_rx) = oneshot::channel::<OutgoingReply>();
        let pending = PendingServerRequest {
            request: ServerRequest::CurrentTimeRead {
                id: RequestId::Int64(7),
                params: crate::protocol::CurrentTimeReadParams {
                    thread_id: "thr_test".into(),
                },
            },
            reply_tx,
        };
        assert_eq!(pending.method_name(), "currentTime/read");
        assert_eq!(pending.id(), &RequestId::Int64(7));

        pending
            .respond(serde_json::json!({ "currentTimeMs": 12345 }))
            .expect("respond should succeed for a plain serializable value");

        let reply = reply_rx
            .await
            .expect("forwarding channel should receive the reply");
        let line = reply
            .into_line()
            .expect("reply should serialize to a wire line");
        let parsed: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(
            parsed,
            serde_json::json!({ "id": 7, "result": { "currentTimeMs": 12345 } })
        );
    }

    /// Same as above but for the error-reply path.
    #[tokio::test]
    async fn respond_error_puts_the_correct_shape_on_the_wire() {
        let (reply_tx, reply_rx) = oneshot::channel::<OutgoingReply>();
        let pending = PendingServerRequest {
            request: ServerRequest::CurrentTimeRead {
                id: RequestId::String("req-1".into()),
                params: crate::protocol::CurrentTimeReadParams {
                    thread_id: "thr_test".into(),
                },
            },
            reply_tx,
        };

        pending.respond_error(-32000, "denied", None);

        let reply = reply_rx
            .await
            .expect("forwarding channel should receive the reply");
        let line = reply
            .into_line()
            .expect("reply should serialize to a wire line");
        let parsed: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(
            parsed,
            serde_json::json!({ "id": "req-1", "error": { "code": -32000, "message": "denied", "data": null } })
        );
    }

    /// `connect_unix` must complete a real handshake over an actual Unix
    /// domain socket - the only transport constructor not otherwise exercised
    /// by the `connect_streams`-based tests in this module (or by the live
    /// `tests/smoke.rs`, which only spawns a child over stdio).
    #[cfg(unix)]
    #[tokio::test]
    async fn connect_unix_completes_a_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let socket_path = dir.path().join("codex-app-server-client-test.sock");
        let listener = tokio::net::UnixListener::bind(&socket_path).unwrap();

        let accept_task = tokio::spawn(async move {
            let (stream, _addr) = listener.accept().await.unwrap();
            let (read_half, mut write_half) = stream.into_split();
            let mut reader = BufReader::new(read_half);
            let mut line = String::new();
            reader.read_line(&mut line).await.unwrap();
            let request: serde_json::Value = serde_json::from_str(&line).unwrap();
            assert_eq!(request["method"], "memory/reset");
            let id = request["id"].clone();
            // MemoryResetResponse wraps a JSON object (`serde_json::Map`), not `null`.
            let response = serde_json::json!({ "id": id, "result": {} });
            write_half
                .write_all(format!("{response}\n").as_bytes())
                .await
                .unwrap();
        });

        let (client, _events) = CodexAppServerClient::connect_unix(&socket_path)
            .await
            .expect("connect_unix should dial the listening socket");
        client
            .memory_reset()
            .await
            .expect("round trip over the Unix socket should succeed");

        accept_task.await.unwrap();
    }

    /// Regression test: dropping a `oneshot::Sender` wakes its paired
    /// `Receiver` immediately with an error - it does not require the
    /// `Receiver`'s side of a `tokio::time::timeout` to elapse. This is the
    /// exact mechanism `PendingServerRequest`'s forwarding task relies on to
    /// resolve promptly (not after `PENDING_SERVER_REQUEST_TIMEOUT`) when a
    /// caller drops a `PendingServerRequest` without responding.
    #[tokio::test]
    async fn dropping_a_oneshot_sender_resolves_the_receiver_immediately_not_via_timeout() {
        let (tx, rx) = oneshot::channel::<()>();
        drop(tx);
        let result = tokio::time::timeout(Duration::from_secs(600), rx).await;
        assert!(
            matches!(result, Ok(Err(_))),
            "dropping the sender should resolve the receiver immediately with an error, \
             not by waiting out the 600s timeout - got {result:?}"
        );
    }

    /// Regression test: dropping a `PendingServerRequest` without responding
    /// must not wedge the connection - the reader must keep dispatching
    /// subsequent server->client requests normally afterward. Uses a real
    /// `currentTime/read` wire line (the simplest `ServerRequest` variant -
    /// `CurrentTimeReadParams` has exactly one required field) written
    /// directly into the duplex to exercise the actual `dispatch_incoming_line`
    /// path, not a hand-rolled shortcut.
    #[tokio::test]
    async fn an_abandoned_pending_server_request_does_not_wedge_the_connection() {
        let (client_io, mut server_io) = tokio::io::duplex(4096);
        let (client_read, client_write) = tokio::io::split(client_io);
        let (_client, mut events) =
            CodexAppServerClient::connect_streams(BufReader::new(client_read), client_write);

        let send_current_time_read = |id: i64| {
            format!(
                r#"{{"method":"currentTime/read","id":{id},"params":{{"threadId":"thr_test"}}}}"#
            )
        };

        server_io
            .write_all(format!("{}\n", send_current_time_read(1)).as_bytes())
            .await
            .unwrap();
        let first = tokio::time::timeout(Duration::from_secs(5), events.recv())
            .await
            .expect("first event within timeout")
            .expect("event stream still open");
        let Event::Request(pending) = first else {
            panic!("expected Event::Request, got something else");
        };
        drop(pending); // abandoned without .respond()/.respond_error()

        server_io
            .write_all(format!("{}\n", send_current_time_read(2)).as_bytes())
            .await
            .unwrap();
        let second = tokio::time::timeout(Duration::from_secs(5), events.recv())
            .await
            .expect("second event within timeout - connection must not be wedged by the abandoned request")
            .expect("event stream still open");
        assert!(
            matches!(second, Event::Request(_)),
            "expected a second Event::Request after abandoning the first, got something else"
        );
    }

    /// Regression test: the reader task detecting a dead connection (EOF from
    /// the peer, here simulated by dropping the peer side entirely) must
    /// proactively reap the writer task - not just when the caller eventually
    /// drops the client. Observed via `send_initialized` failing with
    /// `TransportClosed` once the writer task's channel receiver has dropped,
    /// which only happens once that task has actually exited.
    #[tokio::test]
    async fn reader_detected_disconnect_promptly_reaps_the_writer_task() {
        let (client_io, server_io) = tokio::io::duplex(4096);
        let (client_read, client_write) = tokio::io::split(client_io);
        let (client, mut events) =
            CodexAppServerClient::connect_streams(BufReader::new(client_read), client_write);

        drop(server_io); // simulate the peer disconnecting - client never dropped

        let event = tokio::time::timeout(Duration::from_secs(5), events.recv()).await;
        assert!(
            matches!(event, Ok(Some(Event::Closed))),
            "expected Event::Closed promptly after peer disconnect, got {event:?}"
        );

        let reaped = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if matches!(client.send_initialized(), Err(Error::TransportClosed)) {
                    return;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await;
        assert!(
            reaped.is_ok(),
            "writer task should be reaped shortly after the reader detects disconnect, \
             without needing the client itself to be dropped"
        );
    }

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

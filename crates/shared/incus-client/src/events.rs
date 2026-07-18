//! WebSocket subscription to Incus's `/1.0/events` push-notification
//! stream. This is an *enhancement* over [`crate::operations::Client::wait_for_operation`],
//! not a replacement - that method works without this `events` feature at
//! all.
//!
//! `subscribe_events` directly re-exposes the underlying WebSocket stream
//! rather than buffering through an intermediate channel, so a slow
//! consumer simply leaves frames unread in the transport's own receive
//! buffer (natural backpressure) instead of risking unbounded in-process
//! buffering.

use futures::{Stream, StreamExt};
use serde::Deserialize;
use tokio::net::UnixStream;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::WebSocketStream;

use crate::error::{Error, Result};
use crate::operations::Operation;
use crate::transport::Client;

/// Which event types to subscribe to. All `true` by default (subscribe to
/// everything Incus emits).
#[derive(Debug, Clone, Copy)]
pub struct EventFilter {
    pub operations: bool,
    pub lifecycle: bool,
    pub logging: bool,
}

impl Default for EventFilter {
    fn default() -> Self {
        Self {
            operations: true,
            lifecycle: true,
            logging: true,
        }
    }
}

impl EventFilter {
    fn query_value(self) -> String {
        let mut types = Vec::new();
        if self.operations {
            types.push("operation");
        }
        if self.lifecycle {
            types.push("lifecycle");
        }
        if self.logging {
            types.push("logging");
        }
        types.join(",")
    }
}

/// One event from the `/1.0/events` stream. `Lifecycle` and `Logging`
/// payloads stay untyped (`serde_json::Value`) for v1 - only `Operation`
/// events are fully typed, since that's what operation-completion tracking
/// needs.
#[derive(Debug, Clone)]
pub enum Event {
    Operation(Operation),
    Lifecycle(serde_json::Value),
    Logging(serde_json::Value),
}

#[derive(Debug, Deserialize)]
struct RawFrame {
    #[serde(rename = "type")]
    kind: String,
    metadata: serde_json::Value,
}

fn parse_event(text: &str) -> Result<Event> {
    let frame: RawFrame = serde_json::from_str(text)?;
    match frame.kind.as_str() {
        "operation" => Ok(Event::Operation(serde_json::from_value(frame.metadata)?)),
        "lifecycle" => Ok(Event::Lifecycle(frame.metadata)),
        "logging" => Ok(Event::Logging(frame.metadata)),
        other => Err(Error::InvalidResponse(format!(
            "unknown event frame type {other:?}"
        ))),
    }
}

/// The event stream returned by [`Client::subscribe_events`]. Yields
/// `Result<Event>` - a malformed frame surfaces as one `Err` item rather
/// than terminating the whole stream, since one bad frame from a busy
/// daemon shouldn't take down an otherwise-healthy subscription.
///
/// This crate has no TLS transport in this epic (Incus's events endpoint is
/// reached over the same Unix socket as every other request), so the inner
/// stream is `WebSocketStream<UnixStream>` directly rather than wrapped in
/// `tokio_tungstenite::MaybeTlsStream`.
pub struct EventStream {
    inner: WebSocketStream<UnixStream>,
}

impl Stream for EventStream {
    type Item = Result<Event>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        loop {
            match self.inner.poll_next_unpin(cx) {
                std::task::Poll::Ready(Some(Ok(Message::Text(text)))) => {
                    return std::task::Poll::Ready(Some(parse_event(text.as_str())));
                }
                std::task::Poll::Ready(Some(Ok(Message::Ping(_) | Message::Pong(_)))) => {
                    // tokio-tungstenite answers pings automatically; nothing
                    // for the caller to see here - poll again.
                    continue;
                }
                std::task::Poll::Ready(Some(Ok(Message::Close(_))))
                | std::task::Poll::Ready(None) => {
                    return std::task::Poll::Ready(None);
                }
                std::task::Poll::Ready(Some(Ok(Message::Binary(_) | Message::Frame(_)))) => {
                    return std::task::Poll::Ready(Some(Err(Error::InvalidResponse(
                        "unexpected binary websocket frame from /1.0/events".to_owned(),
                    ))));
                }
                std::task::Poll::Ready(Some(Err(err))) => {
                    return std::task::Poll::Ready(Some(Err(Error::Transport(
                        std::io::Error::other(err.to_string()),
                    ))));
                }
                std::task::Poll::Pending => return std::task::Poll::Pending,
            }
        }
    }
}

impl Client {
    /// Subscribes to Incus's `/1.0/events` WebSocket stream, filtered per
    /// `filter`. The connection is made over the same Unix socket as every
    /// other request - see the module doc comment for why the resulting
    /// stream is exposed directly rather than through a buffering channel.
    pub async fn subscribe_events(&self, filter: EventFilter) -> Result<EventStream> {
        let socket_path = self.socket_path();
        let stream = UnixStream::connect(&socket_path)
            .await
            .map_err(Error::Transport)?;
        let request = format!("ws://localhost/1.0/events?type={}", filter.query_value());
        let (ws_stream, _response) = tokio_tungstenite::client_async(request, stream)
            .await
            .map_err(|err| Error::Transport(std::io::Error::other(err.to_string())))?;
        Ok(EventStream { inner: ws_stream })
    }
}

#[cfg(test)]
#[path = "events_tests.rs"]
mod tests;

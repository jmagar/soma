//! The one place resource/operation code reaches the Incus daemon through:
//! [`Client::request`]. Everything below this module is HTTP framing
//! (`transport::unix`); everything above it (operations, resources) works
//! only with [`IncusEnvelope`], never raw bytes.

pub(crate) mod unix;

use std::path::PathBuf;
use std::sync::Arc;

use crate::config::ClientConfig;
use crate::error::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Method {
    Get,
    Post,
    Put,
    Patch,
    Delete,
}

impl Method {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Method::Get => "GET",
            Method::Post => "POST",
            Method::Put => "PUT",
            Method::Patch => "PATCH",
            Method::Delete => "DELETE",
        }
    }
}

/// A parsed Incus response envelope - see
/// <https://linuxcontainers.org/incus/docs/main/rest-api/>. Error responses
/// (HTTP 4xx/5xx, or a `{"type":"error",...}` body) are turned into
/// `Err(Error::Api { .. })` by [`Client::request`] rather than represented
/// here, so callers never need to check for an `Error` envelope variant
/// themselves.
#[derive(Debug, Clone)]
pub(crate) enum IncusEnvelope {
    Sync {
        metadata: serde_json::Value,
        etag: Option<String>,
    },
    Async {
        /// The `Location`-style operation URL Incus returns (e.g.
        /// `/1.0/operations/<uuid>`). Preserved for wire-protocol fidelity
        /// with the documented envelope shape, but no code path in this
        /// crate currently reads it - `operation_from_envelope` derives the
        /// operation ID from `metadata.id` instead, so the two stay in sync
        /// without this crate needing to parse the URL. Kept (rather than
        /// dropped) so a future caller that wants the raw URL - e.g. for a
        /// diagnostic log line - doesn't require a wire-parsing change.
        #[allow(dead_code)]
        operation_url: String,
        /// The raw operation JSON object - `crate::operations` deserializes
        /// this into a typed `Operation`. Kept untyped here so this module
        /// has no dependency on `crate::operations`.
        metadata: serde_json::Value,
    },
}

/// A value paired with the ETag it was fetched with, for later use as an
/// `If-Match` precondition on an update.
#[derive(Debug, Clone)]
pub struct WithEtag<T> {
    pub value: T,
    pub etag: Option<String>,
}

#[derive(Debug, Clone)]
struct ClientInner {
    socket_path: PathBuf,
}

/// The Incus API client. Cheap to clone (`Arc`-backed) - share one instance
/// across tasks rather than constructing a new one per call.
#[derive(Debug, Clone)]
pub struct Client(Arc<ClientInner>);

impl Client {
    /// Builds a client from `config`. No I/O happens here - connection
    /// attempts happen lazily, once per request, when a method is called.
    #[must_use]
    pub fn new(config: ClientConfig) -> Self {
        Self(Arc::new(ClientInner {
            socket_path: config.socket_path,
        }))
    }

    /// Executes one Incus API request and returns its parsed envelope.
    /// Every resource and operation method in this crate is built on top of
    /// this one method - nothing else in the crate does its own HTTP
    /// framing or envelope parsing.
    pub(crate) async fn request(
        &self,
        method: Method,
        path: &str,
        query: &[(&str, &str)],
        body: Option<&serde_json::Value>,
        if_match: Option<&str>,
    ) -> Result<IncusEnvelope> {
        let body_bytes = body.map(serde_json::to_vec).transpose()?;
        let raw = unix::execute(
            &self.0.socket_path,
            method,
            path,
            query,
            body_bytes.as_deref(),
            if_match,
        )
        .await?;

        if raw.status >= 400 {
            let error_body: serde_json::Value = serde_json::from_slice(&raw.body)
                .unwrap_or_else(|_| serde_json::json!({"error": "unparseable error body"}));
            let message = error_body
                .get("error")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("unknown error")
                .to_owned();
            return Err(Error::Api {
                status_code: raw.status,
                message,
            });
        }

        let parsed: serde_json::Value = serde_json::from_slice(&raw.body)?;
        let envelope_type = parsed
            .get("type")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| {
                Error::InvalidResponse(format!("response body had no \"type\" field: {parsed}"))
            })?;

        match envelope_type {
            "sync" => {
                let metadata = parsed
                    .get("metadata")
                    .cloned()
                    .unwrap_or(serde_json::Value::Null);
                let etag = raw.header("etag").map(str::to_owned);
                Ok(IncusEnvelope::Sync { metadata, etag })
            }
            "async" => {
                let operation_url = parsed
                    .get("operation")
                    .and_then(serde_json::Value::as_str)
                    .ok_or_else(|| {
                        Error::InvalidResponse(
                            "async response had no \"operation\" field".to_owned(),
                        )
                    })?
                    .to_owned();
                let metadata = parsed.get("metadata").cloned().ok_or_else(|| {
                    Error::InvalidResponse("async response had no \"metadata\" field".to_owned())
                })?;
                Ok(IncusEnvelope::Async {
                    operation_url,
                    metadata,
                })
            }
            other => Err(Error::InvalidResponse(format!(
                "unknown envelope type {other:?}"
            ))),
        }
    }

    /// Only called from `events::subscribe_events`, which is gated behind
    /// the `events` feature - suppress the dead-code lint under default
    /// features rather than widen this method's visibility or duplicate the
    /// socket path lookup in `events.rs`.
    #[cfg_attr(not(feature = "events"), allow(dead_code))]
    pub(crate) fn socket_path(&self) -> &std::path::Path {
        &self.0.socket_path
    }
}

#[cfg(test)]
#[path = "transport_tests.rs"]
mod tests;

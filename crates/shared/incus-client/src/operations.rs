//! Async-operation lifecycle: every mutation Incus documents as
//! long-running returns one of these, which callers wait on via
//! [`Client::wait_for_operation`] rather than assuming synchronous
//! completion.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{Error, Result};
use crate::transport::{Client, IncusEnvelope, Method};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OperationClass {
    Task,
    Websocket,
    Token,
}

/// An Incus asynchronous operation - see
/// <https://linuxcontainers.org/incus/docs/main/rest-api/>. `resources` and
/// `metadata` stay untyped (`serde_json::Value`) because their shape varies
/// per operation kind; the well-known top-level fields are fully typed.
#[derive(Debug, Clone, Deserialize)]
pub struct Operation {
    pub id: Uuid,
    pub class: OperationClass,
    pub status: String,
    pub status_code: u16,
    #[serde(default)]
    pub resources: serde_json::Value,
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,
    pub may_cancel: bool,
    #[serde(default)]
    pub err: Option<String>,
}

impl Operation {
    fn is_terminal(&self) -> bool {
        // Per Incus's 3-digit status_code scheme: 100-199 are in-progress
        // states, 200-399 are positive (terminal) results, 400-599 are
        // negative (terminal) results.
        self.status_code >= 200
    }

    fn is_failure(&self) -> bool {
        self.status_code >= 400
    }
}

/// Converts an [`IncusEnvelope::Async`]'s untyped `metadata` into a typed
/// [`Operation`]. Also accepts [`IncusEnvelope::Sync`] (used by
/// `wait_for_operation`, whose `.../wait` endpoint returns the operation
/// object directly as sync metadata, not wrapped in another async envelope).
pub(crate) fn operation_from_envelope(envelope: IncusEnvelope) -> Result<Operation> {
    let metadata = match envelope {
        IncusEnvelope::Async { metadata, .. } => metadata,
        IncusEnvelope::Sync { metadata, .. } => metadata,
    };
    Ok(serde_json::from_value(metadata)?)
}

impl Client {
    /// Waits for operation `id` to reach a terminal status, using Incus's
    /// `.../wait?timeout=<seconds>` long-poll endpoint.
    ///
    /// - `timeout = Some(duration)`: bounds a *single* long-poll call. If
    ///   the operation is still in-progress when that window elapses, this
    ///   returns `Ok(Operation)` with the in-progress snapshot - it does
    ///   **not** re-poll, since the caller explicitly chose how long they're
    ///   willing to wait.
    /// - `timeout = None`: waits indefinitely by transparently re-issuing
    ///   the long-poll call as many times as needed until a terminal status
    ///   is reached. Each individual call is still bounded server-side; this
    ///   just means the method as a whole doesn't return until completion.
    ///
    /// A terminal status in the 400-599 (failure) range returns
    /// `Err(Error::OperationFailed { .. })`, not `Ok(Operation)` - callers
    /// don't need to inspect `status_code` themselves to detect failure.
    pub async fn wait_for_operation(
        &self,
        id: Uuid,
        timeout: Option<Duration>,
    ) -> Result<Operation> {
        loop {
            let query_value;
            let query: &[(&str, &str)] = if let Some(duration) = timeout {
                query_value = duration.as_secs().to_string();
                &[("timeout", query_value.as_str())]
            } else {
                &[]
            };
            let path = format!("/1.0/operations/{id}/wait");
            let envelope = self.request(Method::Get, &path, query, None, None).await?;
            let operation = operation_from_envelope(envelope)?;

            if !operation.is_terminal() {
                if timeout.is_some() {
                    // Caller set an explicit bound on one long-poll call;
                    // honor it rather than looping past it.
                    return Ok(operation);
                }
                // No caller-set bound: this window elapsed without a
                // terminal status, so re-issue the wait call and keep
                // waiting.
                continue;
            }

            if operation.is_failure() {
                return Err(Error::OperationFailed {
                    id: operation.id,
                    status_code: operation.status_code,
                    err: operation.err,
                });
            }

            return Ok(operation);
        }
    }

    /// Cancels operation `op` if it's cancellable. Short-circuits with
    /// `Error::NotCancellable` (no network call) when `op.may_cancel` is
    /// false, since the server would reject it anyway and there's no reason
    /// to round-trip to find that out.
    pub async fn cancel_operation(&self, op: &Operation) -> Result<()> {
        if !op.may_cancel {
            return Err(Error::NotCancellable);
        }
        let path = format!("/1.0/operations/{}", op.id);
        self.request(Method::Delete, &path, &[], None, None).await?;
        Ok(())
    }
}

#[cfg(test)]
#[path = "operations_tests.rs"]
mod tests;

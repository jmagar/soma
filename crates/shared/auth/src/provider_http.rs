use std::time::Instant;

use reqwest::Url;
use reqwest::header;
use serde::de::DeserializeOwned;
use tracing::{info, warn};

use crate::error::AuthError;

pub(crate) struct RequestTrace<'a> {
    provider_id: &'static str,
    operation: &'static str,
    method: &'static str,
    endpoint: &'a Url,
    started: Instant,
}

impl<'a> RequestTrace<'a> {
    pub(crate) fn start(
        provider_id: &'static str,
        operation: &'static str,
        method: &'static str,
        endpoint: &'a Url,
    ) -> Self {
        info!(
            provider = provider_id,
            operation,
            method,
            host = endpoint.host_str().unwrap_or_default(),
            path = endpoint.path(),
            "request.start"
        );
        Self {
            provider_id,
            operation,
            method,
            endpoint,
            started: Instant::now(),
        }
    }

    fn finish(&self, status: reqwest::StatusCode) {
        info!(
            provider = self.provider_id,
            operation = self.operation,
            method = self.method,
            host = self.endpoint.host_str().unwrap_or_default(),
            path = self.endpoint.path(),
            status = status.as_u16(),
            elapsed_ms = self.started.elapsed().as_millis(),
            "request.finish"
        );
    }

    fn error(&self, status: Option<reqwest::StatusCode>, error: &reqwest::Error) {
        if let Some(status) = status {
            warn!(
                provider = self.provider_id,
                operation = self.operation,
                method = self.method,
                host = self.endpoint.host_str().unwrap_or_default(),
                path = self.endpoint.path(),
                status = status.as_u16(),
                elapsed_ms = self.started.elapsed().as_millis(),
                error = %error,
                "request.error"
            );
        } else {
            warn!(
                provider = self.provider_id,
                operation = self.operation,
                method = self.method,
                host = self.endpoint.host_str().unwrap_or_default(),
                path = self.endpoint.path(),
                elapsed_ms = self.started.elapsed().as_millis(),
                error = %error,
                "request.error"
            );
        }
    }
}

pub(crate) struct RequestErrors {
    provider_id: &'static str,
    transport_context: &'static str,
    status_context: &'static str,
    decode_context: &'static str,
}

impl RequestErrors {
    pub(crate) fn new(
        provider_id: &'static str,
        transport_context: &'static str,
        status_context: &'static str,
        decode_context: &'static str,
    ) -> Self {
        Self {
            provider_id,
            transport_context,
            status_context,
            decode_context,
        }
    }
}

pub(crate) async fn read_json_response<T: DeserializeOwned>(
    trace: RequestTrace<'_>,
    request: reqwest::RequestBuilder,
    errors: RequestErrors,
) -> Result<T, AuthError> {
    let response = request.send().await.map_err(|error| {
        let auth_error = AuthError::Network(format!("{}: {error}", errors.transport_context));
        trace.error(None, &error);
        warn!(
            provider = errors.provider_id,
            error = %error,
            kind = auth_error.kind(),
            "{}",
            errors.transport_context
        );
        auth_error
    })?;
    let status = response.status();
    let retry_after_ms = response
        .headers()
        .get(header::RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .map(|seconds| seconds.saturating_mul(1_000));
    let response = response.error_for_status().map_err(|error| {
        let auth_error = if retry_after_ms.is_some() {
            // GitHub's secondary rate limit (abuse detection) responds with
            // 403, not 429, but does carry `Retry-After` — trust the header's
            // presence over the exact status code so we don't miss it.
            AuthError::RateLimited {
                message: format!("{}: {}", errors.status_context, status),
                retry_after_ms: retry_after_ms.unwrap_or(1_000),
            }
        } else if status.is_server_error() {
            AuthError::Server(format!("{}: {error}", errors.status_context))
        } else {
            AuthError::AuthFailed(format!("{}: {error}", errors.status_context))
        };
        trace.error(Some(status), &error);
        warn!(
            provider = errors.provider_id,
            error = %error,
            kind = auth_error.kind(),
            "{}",
            errors.status_context
        );
        auth_error
    })?;
    trace.finish(status);
    response.json::<T>().await.map_err(|error| {
        let auth_error = AuthError::Decode(format!("{}: {error}", errors.decode_context));
        warn!(
            provider = errors.provider_id,
            error = %error,
            kind = auth_error.kind(),
            "{}",
            errors.decode_context
        );
        auth_error
    })
}

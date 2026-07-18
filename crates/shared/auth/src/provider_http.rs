use std::time::Instant;

use reqwest::Url;
use reqwest::header;
use serde::de::DeserializeOwned;
use tracing::{info, warn};

use crate::error::AuthError;

/// Installs the process-wide rustls crypto provider, if one isn't already
/// installed.
///
/// rmcp's HTTP transport (and, transitively, reqwest) requires a rustls
/// crypto provider to be installed before the first TLS-capable client is
/// built. The real binary installs one at startup; test binaries never go
/// through that path, so every `OAuthProvider::new` (Google, Authelia,
/// GitHub) also calls this before building its `reqwest::Client`. The
/// `Result` is intentionally discarded — `Err` only means a provider was
/// already installed elsewhere (e.g. by an earlier-constructed provider in
/// the same process), which is safe to ignore.
pub(crate) fn install_rustls_default_once() {
    drop(rustls::crypto::ring::default_provider().install_default());
}

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

    pub(crate) fn finish(&self, status: reqwest::StatusCode) {
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

    pub(crate) fn error(&self, status: Option<reqwest::StatusCode>, error: &reqwest::Error) {
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

/// Cap on how much of an upstream error response body gets echoed into an
/// `AuthError` message / log line — bounds the blast radius of a malicious
/// or buggy upstream sending back an oversized body.
const ERROR_BODY_SNIPPET_MAX_CHARS: usize = 500;

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

    // Check the status via `error_for_status_ref` (borrows, doesn't consume)
    // so a non-2xx response's body can still be read afterward via
    // `.text()` — `error_for_status()` on the owned `Response` would consume
    // it, and the resulting `reqwest::Error` carries only the status line,
    // leaving operators debugging a 400 from a token endpoint with no
    // indication of *why* (e.g. `{"error":"invalid_grant",...}`).
    if let Err(status_error) = response.error_for_status_ref() {
        let body = response.text().await.unwrap_or_default();
        let body_snippet = error_body_snippet(&body);
        let auth_error = if let Some(retry_after_ms) = retry_after_ms {
            // GitHub's secondary rate limit (abuse detection) responds with
            // 403, not 429, but does carry `Retry-After` — trust the header's
            // presence over the exact status code so we don't miss it.
            AuthError::RateLimited {
                message: format!("{}: {status}{body_snippet}", errors.status_context),
                retry_after_ms,
            }
        } else if status.is_server_error() {
            AuthError::Server(format!(
                "{}: {status_error}{body_snippet}",
                errors.status_context
            ))
        } else {
            AuthError::AuthFailed(format!(
                "{}: {status_error}{body_snippet}",
                errors.status_context
            ))
        };
        trace.error(Some(status), &status_error);
        warn!(
            provider = errors.provider_id,
            error = %status_error,
            kind = auth_error.kind(),
            "{}",
            errors.status_context
        );
        return Err(auth_error);
    }

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

/// Bounded-length ` - <body>` suffix for an error message/log line. Returns
/// an empty string (so callers can append it directly with no dangling
/// separator) when the body is empty or unreadable — `.text()` already
/// degrades invalid UTF-8 losslessly-to-lossy rather than erroring, so
/// "unreadable" here means the read itself failed (I/O error), handled by
/// the caller's `.unwrap_or_default()`.
fn error_body_snippet(body: &str) -> String {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let snippet: String = trimmed.chars().take(ERROR_BODY_SNIPPET_MAX_CHARS).collect();
    format!(" - {snippet}")
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::{RequestErrors, RequestTrace, read_json_response};
    use crate::error::AuthError;

    #[derive(Debug, Deserialize)]
    struct TestPayload {
        #[allow(dead_code)]
        value: String,
    }

    fn test_errors() -> RequestErrors {
        RequestErrors::new(
            "test-provider",
            "transport failed",
            "status error",
            "decode failed",
        )
    }

    /// GitHub's secondary rate limit responds with 403 (not 429) but carries
    /// `Retry-After` — this must classify as `AuthError::RateLimited` with
    /// the header value converted to milliseconds, not `AuthFailed`.
    #[tokio::test]
    async fn a_403_with_retry_after_classifies_as_rate_limited() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/rate-limited"))
            .respond_with(
                ResponseTemplate::new(403)
                    .insert_header("Retry-After", "5")
                    .set_body_json(serde_json::json!({"message": "rate limited"})),
            )
            .mount(&server)
            .await;
        let url = server
            .uri()
            .parse::<reqwest::Url>()
            .unwrap()
            .join("/rate-limited")
            .unwrap();
        let client = reqwest::Client::new();
        let trace = RequestTrace::start("test-provider", "op", "GET", &url);

        let error =
            read_json_response::<TestPayload>(trace, client.get(url.clone()), test_errors())
                .await
                .unwrap_err();
        match error {
            AuthError::RateLimited { retry_after_ms, .. } => {
                assert_eq!(retry_after_ms, 5_000);
            }
            other => panic!("expected RateLimited, got {other:?}"),
        }
    }

    /// A generic 4xx with no `Retry-After` header must classify as
    /// `AuthError::AuthFailed`, not `RateLimited` or `Server`.
    #[tokio::test]
    async fn a_generic_4xx_without_retry_after_classifies_as_auth_failed() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/bad-request"))
            .respond_with(
                ResponseTemplate::new(400)
                    .set_body_json(serde_json::json!({"message": "bad request"})),
            )
            .mount(&server)
            .await;
        let url = server
            .uri()
            .parse::<reqwest::Url>()
            .unwrap()
            .join("/bad-request")
            .unwrap();
        let client = reqwest::Client::new();
        let trace = RequestTrace::start("test-provider", "op", "GET", &url);

        let error =
            read_json_response::<TestPayload>(trace, client.get(url.clone()), test_errors())
                .await
                .unwrap_err();
        assert!(
            matches!(error, AuthError::AuthFailed(_)),
            "expected AuthFailed, got {error:?}"
        );
    }

    /// A transport-level failure (connection refused) must classify as
    /// `AuthError::Network`, not surface as a raw `reqwest::Error`.
    #[tokio::test]
    async fn a_transport_failure_classifies_as_network_error() {
        // Bind a listener to grab a free port, then drop it immediately so
        // the port is guaranteed closed — connecting to it fails fast with
        // "connection refused" instead of relying on an arbitrary unused
        // port number that might coincidentally be listened on.
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);
        let url = reqwest::Url::parse(&format!("http://{addr}/unreachable")).unwrap();
        let client = reqwest::Client::new();
        let trace = RequestTrace::start("test-provider", "op", "GET", &url);

        let error =
            read_json_response::<TestPayload>(trace, client.get(url.clone()), test_errors())
                .await
                .unwrap_err();
        assert!(
            matches!(error, AuthError::Network(_)),
            "expected Network, got {error:?}"
        );
    }

    /// A 200 response whose body doesn't match the target type must
    /// classify as `AuthError::Decode`, not panic or surface a raw
    /// deserialization error.
    #[tokio::test]
    async fn a_200_response_with_an_unexpected_body_shape_classifies_as_decode_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/ok"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({"unexpected": "shape"})),
            )
            .mount(&server)
            .await;
        let url = server
            .uri()
            .parse::<reqwest::Url>()
            .unwrap()
            .join("/ok")
            .unwrap();
        let client = reqwest::Client::new();
        let trace = RequestTrace::start("test-provider", "op", "GET", &url);

        let error =
            read_json_response::<TestPayload>(trace, client.get(url.clone()), test_errors())
                .await
                .unwrap_err();
        assert!(
            matches!(error, AuthError::Decode(_)),
            "expected Decode, got {error:?}"
        );
    }
}

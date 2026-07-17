//! Reusable JSON response envelope and error-body helpers.

use axum::{
    extract::rejection::JsonRejection,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;

use crate::problem::ErrorBody;

/// Render a JSON body extraction failure as a response.
///
/// `413 Payload Too Large` when the body exceeded the configured limit,
/// `400 Bad Request` for every other rejection (missing/invalid content
/// type, malformed JSON, etc.).
pub fn json_rejection_response(error: JsonRejection) -> Response {
    let status = if error.status() == StatusCode::PAYLOAD_TOO_LARGE {
        StatusCode::PAYLOAD_TOO_LARGE
    } else {
        StatusCode::BAD_REQUEST
    };
    ErrorBody::new(error.to_string()).into_response_with_status(status)
}

/// Render a `400 Bad Request` with a generic validation error body.
pub fn validation_error_response(message: impl Into<String>) -> Response {
    ErrorBody::new("validation_error")
        .with_message(message)
        .into_response_with_status(StatusCode::BAD_REQUEST)
}

/// Render any `Serialize` payload as a JSON response with the given status.
pub fn json_response(status: StatusCode, body: impl Serialize) -> Response {
    (status, Json(body)).into_response()
}

#[cfg(test)]
mod tests {
    use axum::body::to_bytes;

    use super::*;

    #[tokio::test]
    async fn validation_error_response_is_bad_request_with_message() {
        let response = validation_error_response("name is required");
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(value["error"], "validation_error");
        assert_eq!(value["message"], "name is required");
    }

    #[tokio::test]
    async fn json_response_serializes_status_and_body() {
        let response = json_response(StatusCode::CREATED, serde_json::json!({"ok": true}));
        assert_eq!(response.status(), StatusCode::CREATED);
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(value["ok"], true);
    }
}

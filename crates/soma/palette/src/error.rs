//! Product error mapping for Palette UI responses.
//!
//! The Palette frontend renders `code`/`message`/`remediation` directly, so
//! the JSON body is `ApplicationError` itself (already `Serialize`); this
//! module only owns the HTTP status mapping, which mirrors `soma-api`'s but
//! lives here independently — `product-surface` packages must not depend on
//! one another (see `xtask check-architecture`).

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use soma_application::ApplicationError;

pub fn palette_error_response(error: ApplicationError) -> Response {
    let status = palette_error_status(&error);
    tracing::warn!(code = %error.code, "palette request failed");
    (status, Json(error)).into_response()
}

pub fn palette_error_status(error: &ApplicationError) -> StatusCode {
    match error.code.as_str() {
        "unknown_action" | "surface_not_exposed" | "upstream_missing" | "unknown_upstream" => {
            StatusCode::NOT_FOUND
        }
        "insufficient_scope" | "capability_denied" | "admin_required" | "not_exposed" => {
            StatusCode::FORBIDDEN
        }
        "input_too_large" | "response_too_large" => StatusCode::PAYLOAD_TOO_LARGE,
        "input_schema_failed"
        | "confirmation_required"
        | "invalid_param"
        | "spawn_validation_failed"
        | "upstream_exists"
        | "invalid_config" => StatusCode::BAD_REQUEST,
        "unsupported_transport" => StatusCode::NOT_IMPLEMENTED,
        "gateway_reloading"
        | "store_not_mounted"
        | "oauth_runtime_error"
        | "not_routable"
        | "upstream_connect_failed"
        | "upstream_call_failed"
        | "engine_unavailable" => StatusCode::SERVICE_UNAVAILABLE,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

/// `404` body for a launcher id that doesn't resolve to any palette-exposed
/// tool. Kept distinct from `ApplicationError` mapping because catalog/schema
/// lookups happen in this crate, before any call reaches `SomaApplication`.
pub fn launcher_not_found(id: &str) -> Response {
    (
        StatusCode::NOT_FOUND,
        Json(serde_json::json!({
            "code": "launcher_not_found",
            "message": format!("no palette-exposed launcher entry `{id}`"),
            "remediation": "Refresh the catalog and use a known launcher id.",
        })),
    )
        .into_response()
}

#[cfg(test)]
#[path = "error_tests.rs"]
mod tests;

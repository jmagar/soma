//! REST API handlers — `POST /v1/example`, `GET /health`, `GET /status`, `GET /openapi.json`.
//!
//! All handlers are thin: parse the request, call the service, return JSON.
//! Business logic lives in `app.rs`.

use axum::{
    extract::{Extension, State},
    http::{StatusCode, header},
    response::{IntoResponse, Json},
};
use lab_auth::AuthContext;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::actions::{execute_service_action, required_scope_for_action, ExampleAction};
use crate::server::{AppState, AuthPolicy};

/// Request body for `POST /v1/example`.
///
/// REST uses an explicit `{ action, params }` envelope. MCP uses a flat
/// argument object such as `{ action, message }`. Both convert into the same
/// typed `ExampleAction` before calling `ExampleService`.
#[derive(Deserialize)]
pub struct ActionRequest {
    #[serde(default)]
    pub action: String,
    #[serde(default)]
    pub params: Value,
}

/// `POST /v1/example` — dispatches an action by name.
///
/// Request:  `{"action": "greet", "params": {"name": "Alice"}}`
/// Response: `{"greeting": "Hello, Alice!", ...}`
pub async fn api_dispatch(
    State(state): State<AppState>,
    auth: Option<Extension<AuthContext>>,
    Json(body): Json<ActionRequest>,
) -> impl IntoResponse {
    let result = match ExampleAction::from_rest(&body.action, &body.params) {
        Ok(action) => {
            if let Some(response) = enforce_rest_scope(
                &state,
                auth.as_ref().map(|Extension(auth)| auth),
                &body.action,
            ) {
                return response;
            }
            execute_service_action(&state.service, &action).await
        }
        Err(error) => Err(error),
    };

    match result {
        Ok(value) => Json(value).into_response(),
        Err(e) if crate::actions::is_validation_error(&e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
        Err(e) => {
            tracing::error!(error = %e, action = %body.action, "REST action execution failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response()
        }
    }
}

fn enforce_rest_scope(
    state: &AppState,
    auth: Option<&AuthContext>,
    action: &str,
) -> Option<axum::response::Response> {
    if !matches!(&state.auth_policy, AuthPolicy::Mounted { .. }) {
        return None;
    }
    let required_scope = required_scope_for_action(action)?;
    let Some(auth) = auth else {
        tracing::warn!(action = %action, "REST action denied: missing auth context");
        return Some(
            (
                StatusCode::FORBIDDEN,
                Json(json!({"error": "forbidden: missing auth context"})),
            )
                .into_response(),
        );
    };
    let satisfied = crate::actions::scopes_satisfy(&auth.scopes, required_scope);
    if satisfied {
        return None;
    }
    tracing::warn!(
        subject = %auth.sub,
        action = %action,
        required_scope = %required_scope,
        "REST action denied: insufficient scope"
    );
    Some(
        (
            StatusCode::FORBIDDEN,
            Json(json!({"error": format!("forbidden: requires scope: {required_scope}")})),
        )
            .into_response(),
    )
}

/// `GET /health` — liveness probe (unauthenticated).
pub async fn health() -> impl IntoResponse {
    tracing::debug!("health probe");
    Json(json!({ "status": "ok" }))
}

/// `GET /openapi.json` — generated OpenAPI schema for the REST surface.
pub async fn openapi_json() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
        include_str!("../docs/generated/openapi.json"),
    )
}

/// `GET /status` — local runtime status (unauthenticated, redacts secrets).
pub async fn status(State(state): State<AppState>) -> impl IntoResponse {
    Json(json!({
        "status": "ok",
        "server": state.config.server_name,
        "version": env!("CARGO_PKG_VERSION"),
        "transport": "http",
    }))
    .into_response()
}

#[cfg(test)]
#[path = "api_tests.rs"]
mod tests;

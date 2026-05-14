//! REST API handlers — `POST /v1/example`, `GET /health`, `GET /status`.
//!
//! All handlers are thin: parse the request, call the service, return JSON.
//! Business logic lives in `app.rs`.

use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Json},
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::actions::{execute_service_action, ExampleAction};
use crate::server::AppState;

/// Request body for `POST /v1/example`.
///
/// Same `action` + `params` shape as the MCP tool interface — all three surfaces
/// (MCP, REST, CLI) call the same `ExampleService` methods.
#[derive(Deserialize)]
pub struct ActionRequest {
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
    Json(body): Json<ActionRequest>,
) -> impl IntoResponse {
    let result = match ExampleAction::from_rest(&body.action, &body.params) {
        Ok(action) => execute_service_action(&state.service, &action).await,
        Err(error) => Err(error),
    };

    match result {
        Ok(value) => Json(value).into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// `GET /health` — liveness probe (unauthenticated).
pub async fn health() -> impl IntoResponse {
    Json(json!({ "status": "ok" }))
}

/// `GET /status` — runtime status (unauthenticated, redacts secrets).
pub async fn status(State(state): State<AppState>) -> impl IntoResponse {
    Json(json!({
        "status": "ok",
        "server": state.config.server_name,
        "version": env!("CARGO_PKG_VERSION"),
        "transport": "http",
    }))
    .into_response()
}

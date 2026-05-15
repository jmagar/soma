//! REST API handlers — `POST /v1/example`, `GET /health`, `GET /status`.
//!
//! All handlers are thin: parse the request, call the service, return JSON.
//! Business logic lives in `app.rs`.

use axum::{
    extract::State,
    http::{header, StatusCode},
    response::{IntoResponse, Json},
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::actions::{execute_service_action, ExampleAction};
use crate::server::AppState;

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
    Json(body): Json<ActionRequest>,
) -> impl IntoResponse {
    let result = match ExampleAction::from_rest(&body.action, &body.params) {
        Ok(action) => execute_service_action(&state.service, &action).await,
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

/// `GET /health` — liveness probe (unauthenticated).
pub async fn health() -> impl IntoResponse {
    Json(json!({ "status": "ok" }))
}

/// `GET /openapi.json` — generated OpenAPI schema for the REST surface.
pub async fn openapi_json() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
        include_str!("../docs/generated/openapi.json"),
    )
}

/// `GET /status` — runtime status (unauthenticated, redacts secrets).
pub async fn status(State(state): State<AppState>) -> impl IntoResponse {
    match state.service.status().await {
        Ok(mut value) => {
            if let Some(object) = value.as_object_mut() {
                object.insert("server".into(), json!(state.config.server_name));
                object.insert("version".into(), json!(env!("CARGO_PKG_VERSION")));
                object.insert("transport".into(), json!("http"));
            }
            Json(value).into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "runtime status check failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "status check failed"})),
            )
                .into_response()
        }
    }
}

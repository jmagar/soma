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
    let result = match body.action.as_str() {
        "greet" => {
            let name = body.params["name"].as_str();
            state.service.greet(name).await
        }
        "echo" => {
            let msg = body.params["message"].as_str().unwrap_or("");
            if msg.is_empty() {
                Err(anyhow::anyhow!(
                    "`message` param is required for action=echo"
                ))
            } else {
                state.service.echo(msg).await
            }
        }
        "status" => state.service.status().await,
        "help" => Ok(json!({
            "actions": ["greet", "echo", "status", "help"],
            "usage": "POST /v1/example with {\"action\": \"<action>\", \"params\": {...}}",
            "examples": {
                "greet":  {"action": "greet",  "params": {"name": "Alice"}},
                "echo":   {"action": "echo",   "params": {"message": "Hello!"}},
                "status": {"action": "status", "params": {}},
            }
        })),
        other => Err(anyhow::anyhow!(
            "unknown action: {other}. POST {{\"action\":\"help\"}} for documentation."
        )),
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
    match state.service.status().await {
        Ok(v) => Json(v).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

//! Axum router — wires HTTP endpoints to MCP service and auth middleware.
//!
//! Endpoints:
//!   `POST /mcp`         — MCP Streamable HTTP transport (tools, resources, prompts)
//!   `GET  /health`      — Health check (unauthenticated)
//!   `GET  /status`      — Runtime status (unauthenticated, redacts secrets)
//!   `POST /v1/example`  — REST API action dispatch (mirrors MCP tool interface)
//!   `/*`                — SPA fallback for embedded web UI (when web feature enabled)
//!
//! **Template**: extend `health()` if you want richer health data, or add
//! application-specific routes alongside `/mcp` and `/v1/example`.

use std::sync::Arc;

use axum::{
    extract::State,
    http::{HeaderValue, Method, StatusCode},
    response::{IntoResponse, Json},
    routing::{get, post},
    Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use tower_http::{
    cors::{Any, CorsLayer},
    limit::RequestBodyLimitLayer,
};

use super::rmcp_server::{allowed_origins, streamable_http_config, streamable_http_service};
use super::{build_auth_layer, AppState, AuthPolicy};

const MCP_BODY_LIMIT_BYTES: usize = 65_536;

pub fn router(state: AppState) -> Router {
    let rmcp_config = streamable_http_config(&state.config);

    let resource_url = match &state.auth_policy {
        AuthPolicy::Mounted { .. } => state
            .config
            .auth
            .public_url
            .as_deref()
            .map(|u| Arc::<str>::from(format!("{}/mcp", u.trim_end_matches('/')))),
        AuthPolicy::LoopbackDev => None,
    };

    // Build auth layer — applied to BOTH /mcp and /v1/example (same policy).
    let auth_layer = build_auth_layer(
        &state.auth_policy,
        state.config.api_token.as_deref().map(Arc::<str>::from),
        resource_url,
    );

    // Build state-dependent routes before resolving state.
    // Then combine MCP + REST API, wrap with auth, and merge public routes.
    let api_and_mcp: Router<AppState> = Router::new()
        .nest_service("/mcp", streamable_http_service(state.clone(), rmcp_config))
        .route("/v1/example", post(api_dispatch));

    let api_and_mcp_resolved: Router<()> = api_and_mcp.with_state(state.clone());

    let authenticated = if let Some(layer) = auth_layer {
        api_and_mcp_resolved.layer(layer)
    } else {
        api_and_mcp_resolved
    };

    let oauth_router: Option<Router> = if let AuthPolicy::Mounted {
        auth_state: Some(ref state_arc),
    } = state.auth_policy
    {
        let auth_state = state_arc.as_ref().clone();
        let path_based_discovery = Router::new()
            .route(
                "/mcp/.well-known/oauth-authorization-server",
                get(lab_auth::metadata::authorization_server_metadata),
            )
            .route(
                "/mcp/.well-known/openid-configuration",
                get(lab_auth::metadata::authorization_server_metadata),
            )
            .route(
                "/mcp/.well-known/oauth-protected-resource",
                get(lab_auth::metadata::protected_resource_metadata),
            )
            .with_state(auth_state.clone());
        Some(lab_auth::routes::router(auth_state).merge(path_based_discovery))
    } else {
        None
    };

    // Public routes (no auth) + authenticated routes
    let public: Router<()> = Router::new()
        .route("/health", get(health))
        .route("/status", get(status))
        .with_state(state.clone());

    let mut base: Router<()> = Router::new().merge(authenticated).merge(public);

    if let Some(oauth) = oauth_router {
        base = base.merge(oauth);
    }

    // SPA fallback — LAST (catches anything not matched above)
    let base = if crate::web::web_assets_available() {
        base.fallback(crate::web::serve_web_assets)
    } else {
        base.fallback(|| async { (StatusCode::NOT_FOUND, Json(json!({"error": "not_found"}))) })
    };

    base.layer(RequestBodyLimitLayer::new(MCP_BODY_LIMIT_BYTES))
        .layer(cors_layer(&state.config))
}

fn cors_layer(config: &crate::config::McpConfig) -> CorsLayer {
    let origins: Vec<HeaderValue> = allowed_origins(config)
        .into_iter()
        .filter_map(|o| o.parse::<HeaderValue>().ok())
        .collect();
    CorsLayer::new()
        .allow_origin(origins)
        .allow_methods([Method::POST, Method::GET])
        .allow_headers(Any)
}

// ── REST API ──────────────────────────────────────────────────────────────────

/// Request body for `POST /v1/example`.
///
/// Mirrors the MCP tool interface: same `action` + `params` shape.
/// Agents can use whichever surface is available — all three (MCP, REST, CLI)
/// call the same `ExampleService` methods.
#[derive(Deserialize)]
struct ActionRequest {
    action: String,
    #[serde(default)]
    params: Value,
}

/// `POST /v1/example` — action dispatch that mirrors the MCP tool.
///
/// Request:  `{"action": "greet", "params": {"name": "Alice"}}`
/// Response: `{"greeting": "Hello, Alice!", ...}`
async fn api_dispatch(
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

// ── health / status ───────────────────────────────────────────────────────────

async fn health() -> impl IntoResponse {
    Json(json!({ "status": "ok" }))
}

async fn status(State(state): State<AppState>) -> impl IntoResponse {
    match state.service.status().await {
        Ok(v) => Json(v).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

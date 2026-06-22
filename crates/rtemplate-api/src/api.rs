//! REST API handlers — direct `/v1/*` routes plus public health/status docs.
//!
//! All handlers are thin: parse the request, call the service, return JSON.
//! Business logic lives in `app.rs`.

use anyhow::Result;
use axum::{
    extract::{rejection::JsonRejection, Extension, State},
    http::{header, StatusCode},
    response::{IntoResponse, Json},
};
#[cfg(feature = "auth")]
use rtemplate_auth::AuthContext;
#[cfg(not(feature = "auth"))]
struct AuthContext {
    sub: String,
    scopes: Vec<String>,
}
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use rtemplate_contracts::actions::{required_scope_for_action, ExampleAction};
use rtemplate_contracts::token_limit::MAX_RESPONSE_BYTES;
use rtemplate_runtime::server::{AppState, AuthPolicy};
use rtemplate_service::{classify_service_error, dispatch_action};

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub struct RestRoute {
    pub method: &'static str,
    pub path: &'static str,
    pub action: Option<&'static str>,
    pub auth: &'static str,
    pub description: &'static str,
}

pub const REST_ROUTES: &[RestRoute] = &[
    RestRoute {
        method: "GET",
        path: "/health",
        action: None,
        auth: "public",
        description: "Fast liveness probe.",
    },
    RestRoute {
        method: "GET",
        path: "/readyz",
        action: None,
        auth: "public",
        description: "Readiness probe; 503 when the upstream dependency is unreachable.",
    },
    RestRoute {
        method: "GET",
        path: "/metrics",
        action: None,
        auth: "public",
        description:
            "Prometheus metrics (text exposition format; requires the observability feature).",
    },
    RestRoute {
        method: "GET",
        path: "/status",
        action: None,
        auth: "public",
        description: "Local redacted runtime status.",
    },
    RestRoute {
        method: "GET",
        path: "/openapi.json",
        action: None,
        auth: "public",
        description: "Generated OpenAPI schema.",
    },
    RestRoute {
        method: "GET",
        path: "/v1/capabilities",
        action: None,
        auth: "mounted auth policy",
        description: "Direct REST route inventory and server metadata.",
    },
    RestRoute {
        method: "POST",
        path: "/v1/greet",
        action: Some("greet"),
        auth: "mounted auth policy; requires example:read when scoped",
        description: "Return a greeting.",
    },
    RestRoute {
        method: "POST",
        path: "/v1/echo",
        action: Some("echo"),
        auth: "mounted auth policy; requires example:read when scoped",
        description: "Echo a message back unchanged.",
    },
    RestRoute {
        method: "GET",
        path: "/v1/status",
        action: Some("status"),
        auth: "mounted auth policy; requires example:read when scoped",
        description: "Return authenticated service status.",
    },
    RestRoute {
        method: "GET",
        path: "/v1/help",
        action: Some("help"),
        auth: "mounted auth policy",
        description: "Return the action catalog and route help.",
    },
    RestRoute {
        method: "POST",
        path: "/v1/example",
        action: None,
        auth: "mounted auth policy",
        description: "Deprecated compatibility action envelope.",
    },
];

#[derive(Debug, Serialize)]
pub struct CapabilitiesResponse {
    pub server: &'static str,
    pub version: &'static str,
    pub preferred_rest_style: &'static str,
    pub supported_routes: Vec<String>,
    pub routes: &'static [RestRoute],
}

/// Request body for deprecated `POST /v1/example`.
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

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GreetRequest {
    #[serde(default)]
    pub name: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EchoRequest {
    pub message: String,
}

/// Deprecated compatibility route. New platform servers should prefer direct
/// product routes such as `POST /v1/echo` over an action envelope.
pub async fn api_dispatch(
    State(state): State<AppState>,
    auth: Option<Extension<AuthContext>>,
    Json(body): Json<ActionRequest>,
) -> impl IntoResponse {
    // Destructive actions require an explicit "confirm": true. No-op for the
    // template's current (non-destructive) actions; gates any future one.
    if let Err(tool_error) = rtemplate_contracts::actions::require_confirmation_if_destructive(
        &body.action,
        &body.params,
    ) {
        return (
            StatusCode::from_u16(tool_error.http_status_code()).unwrap_or(StatusCode::BAD_REQUEST),
            Json(tool_error.to_rest_payload()),
        )
            .into_response();
    }

    run_rest_action_request(
        state,
        auth.as_ref().map(|Extension(auth)| auth),
        ExampleAction::from_rest(&body.action, &body.params),
        &body.action,
    )
    .await
    .into_response()
}

pub async fn v1_capabilities() -> impl IntoResponse {
    Json(CapabilitiesResponse {
        server: "rtemplate-mcp",
        version: env!("CARGO_PKG_VERSION"),
        preferred_rest_style: "direct_routes",
        supported_routes: REST_ROUTES
            .iter()
            .map(|route| format!("{} {}", route.method, route.path))
            .collect(),
        routes: REST_ROUTES,
    })
}

pub async fn v1_greet(
    State(state): State<AppState>,
    auth: Option<Extension<AuthContext>>,
    body: Result<Json<GreetRequest>, JsonRejection>,
) -> axum::response::Response {
    let Json(body) = match body {
        Ok(body) => body,
        Err(error) => return rest_json_rejection_response(error),
    };
    run_rest_action_request(
        state,
        auth.as_ref().map(|Extension(auth)| auth),
        ExampleAction::from_rest("greet", &optional_name_params(body.name)),
        "greet",
    )
    .await
}

pub async fn v1_echo(
    State(state): State<AppState>,
    auth: Option<Extension<AuthContext>>,
    body: Result<Json<EchoRequest>, JsonRejection>,
) -> axum::response::Response {
    let Json(body) = match body {
        Ok(body) => body,
        Err(error) => return rest_json_rejection_response(error),
    };
    run_rest_action_request(
        state,
        auth.as_ref().map(|Extension(auth)| auth),
        ExampleAction::from_rest("echo", &json!({ "message": body.message })),
        "echo",
    )
    .await
}

pub async fn v1_service_status(
    State(state): State<AppState>,
    auth: Option<Extension<AuthContext>>,
) -> axum::response::Response {
    run_rest_action_request(
        state,
        auth.as_ref().map(|Extension(auth)| auth),
        Ok(ExampleAction::Status),
        "status",
    )
    .await
}

pub async fn v1_help(
    State(state): State<AppState>,
    auth: Option<Extension<AuthContext>>,
) -> axum::response::Response {
    run_rest_action_request(
        state,
        auth.as_ref().map(|Extension(auth)| auth),
        Ok(ExampleAction::Help),
        "help",
    )
    .await
}

async fn run_rest_action_request(
    state: AppState,
    auth: Option<&AuthContext>,
    action: Result<ExampleAction>,
    action_name: &str,
) -> axum::response::Response {
    match action {
        Ok(action) => run_rest_action(state, auth, action).await,
        Err(error) => rest_error_response(error, action_name),
    }
}

async fn run_rest_action(
    state: AppState,
    auth: Option<&AuthContext>,
    action: ExampleAction,
) -> axum::response::Response {
    let action_name = action.name();
    if let Some(response) = enforce_rest_scope(&state, auth, action_name) {
        return response;
    }

    match dispatch_action(&state.service, &action, "rest").await {
        Ok(value) => match cap_rest_response(value) {
            Ok(value) => Json(value).into_response(),
            Err(e) => {
                tracing::error!(error = %e, action = %action_name, "REST response serialization failed");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": "internal server error"})),
                )
                    .into_response()
            }
        },
        Err(e) => rest_error_response(e, action_name),
    }
}

fn rest_error_response(error: anyhow::Error, action: &str) -> axum::response::Response {
    let tool_error = classify_service_error(&error);
    if tool_error.kind == rtemplate_contracts::errors::ServiceErrorKind::Validation {
        tracing::warn!(
            action = %action,
            code = %tool_error.code,
            "REST action rejected invalid params"
        );
    } else {
        tracing::error!(
            error = %error,
            action = %action,
            service_error_kind = %tool_error.kind.as_str(),
            "REST action execution failed"
        );
    }
    (
        StatusCode::from_u16(tool_error.http_status_code())
            .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
        Json(tool_error.to_rest_payload()),
    )
        .into_response()
}

fn rest_json_rejection_response(error: JsonRejection) -> axum::response::Response {
    let status = if error.status() == StatusCode::PAYLOAD_TOO_LARGE {
        StatusCode::PAYLOAD_TOO_LARGE
    } else {
        StatusCode::BAD_REQUEST
    };
    (status, Json(json!({"error": error.to_string()}))).into_response()
}

fn optional_name_params(name: Option<String>) -> Value {
    match name {
        Some(name) => json!({ "name": name }),
        None => json!({}),
    }
}

fn cap_rest_response(value: Value) -> Result<Value> {
    let serialized = serde_json::to_vec(&value)?;
    if serialized.len() <= MAX_RESPONSE_BYTES {
        return Ok(value);
    }
    Ok(json!({
        "truncated": true,
        "error": "response exceeded REST response size limit",
        "max_response_bytes": MAX_RESPONSE_BYTES,
        "hint": "Use limit/offset parameters or more specific filters to get a smaller result.",
    }))
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
    let satisfied = rtemplate_contracts::actions::scopes_satisfy(&auth.scopes, required_scope);
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

/// `GET /readyz` — readiness probe (unauthenticated).
///
/// Unlike `/health` (pure liveness: "the process is up"), this probes the
/// upstream dependency and returns `503 Service Unavailable` when it is
/// unreachable, so orchestrators (Kubernetes, compose healthchecks, load
/// balancers) only route traffic once the server can actually serve it.
pub async fn readyz(State(state): State<AppState>) -> impl IntoResponse {
    match state.service.ready().await {
        Ok(()) => (StatusCode::OK, Json(json!({ "status": "ready" }))).into_response(),
        Err(error) => {
            tracing::warn!(%error, "readiness probe failed");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({ "status": "not_ready", "reason": error.to_string() })),
            )
                .into_response()
        }
    }
}

/// `GET /openapi.json` — generated OpenAPI schema for the REST surface.
pub async fn openapi_json() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
        include_str!("../../../docs/generated/openapi.json"),
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

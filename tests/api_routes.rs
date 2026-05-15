//! Route-level tests for REST dispatch, status, and mounted auth behavior.

use axum::{
    body::{to_bytes, Body},
    http::{header, Method, Request, StatusCode},
};
use rmcp_template::{
    server::{self, AuthPolicy},
    testing::{bearer_state, loopback_state},
};
use serde_json::{json, Value};
use tower::ServiceExt;

async fn request_json(
    app: axum::Router,
    method: Method,
    path: &str,
    auth: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder().method(method).uri(path);
    if let Some(token) = auth {
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    let request = if let Some(body) = body {
        builder
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_string()))
            .expect("request should build")
    } else {
        builder.body(Body::empty()).expect("request should build")
    };

    let response = app.oneshot(request).await.expect("route should respond");
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body should read");
    let value = serde_json::from_slice(&bytes).expect("response should be JSON");
    (status, value)
}

#[tokio::test]
async fn rest_echo_accepts_nested_params() {
    let app = server::router(loopback_state());
    let (status, body) = request_json(
        app,
        Method::POST,
        "/v1/example",
        None,
        Some(json!({"action": "echo", "params": {"message": "hello"}})),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["echo"], "hello");
}

#[tokio::test]
async fn rest_validation_errors_are_bad_requests() {
    let app = server::router(loopback_state());
    for body in [
        json!({"action": "echo", "params": {}}),
        json!({"action": "echo", "params": {"message": ""}}),
        json!({"action": "echo", "params": {"message": 42}}),
        json!({"action": "missing", "params": {}}),
        json!({"params": {}}),
    ] {
        let (status, response) =
            request_json(app.clone(), Method::POST, "/v1/example", None, Some(body)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{response}");
        assert!(response.get("error").is_some(), "{response}");
    }
}

#[tokio::test]
async fn rest_help_excludes_mcp_only_actions_from_rest_actions() {
    let app = server::router(loopback_state());
    let (status, body) = request_json(
        app,
        Method::POST,
        "/v1/example",
        None,
        Some(json!({"action": "help", "params": {}})),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["actions"], json!(["greet", "echo", "status", "help"]));
    assert_eq!(
        body["mcp_only_actions"],
        json!(["elicit_name", "scaffold_intent"])
    );
}

#[tokio::test]
async fn status_uses_service_status_and_local_metadata() {
    let app = server::router(loopback_state());
    let (status, body) = request_json(app, Method::GET, "/status", None, None).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "ok");
    assert_eq!(body["server"], "example-mcp");
    assert_eq!(body["transport"], "http");
    assert!(body.get("version").is_some());
}

#[tokio::test]
async fn mounted_bearer_auth_protects_rest_endpoint() {
    let app = server::router(bearer_state("secret"));
    let body = json!({"action": "status", "params": {}});

    let (missing_status, _) = request_json(
        app.clone(),
        Method::POST,
        "/v1/example",
        None,
        Some(body.clone()),
    )
    .await;
    assert_eq!(missing_status, StatusCode::UNAUTHORIZED);

    let (valid_status, valid_body) =
        request_json(app, Method::POST, "/v1/example", Some("secret"), Some(body)).await;
    assert_eq!(valid_status, StatusCode::OK);
    assert_eq!(valid_body["status"], "ok");
}

#[tokio::test]
async fn trusted_gateway_unscoped_bypasses_local_auth() {
    let mut state = loopback_state();
    state.auth_policy = AuthPolicy::TrustedGatewayUnscoped;
    let app = server::router(state);
    let (status, body) = request_json(
        app,
        Method::POST,
        "/v1/example",
        None,
        Some(json!({"action": "status", "params": {}})),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "ok");
}

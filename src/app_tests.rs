//! Unit tests for ExampleService — sidecar file for src/app.rs
//!
//! Declared in app.rs as:
//! ```rust
//! #[cfg(test)]
//! #[path = "app_tests.rs"]
//! mod tests;
//! ```
//!
//! The service layer tests verify that ExampleService correctly delegates to
//! ExampleClient and that any transformations or caching are correct.
//!
//! **Template**: These tests use real ExampleClient instances (pointing at a
//! stub URL), which means they test the full delegation chain without mocking.
//! For services with complex business logic, consider adding mock clients.

use super::*;
use crate::{config::ExampleConfig, example::ExampleClient};

/// Build a stub ExampleService for testing without real credentials.
fn stub_service() -> ExampleService {
    let client = ExampleClient::new(&ExampleConfig {
        api_url: "http://localhost:1/stub".to_string(),
        api_key: "test-key".to_string(),
    })
    .expect("stub client should always build");
    ExampleService::new(client)
}

#[tokio::test]
async fn test_service_greet_delegates_to_client() {
    // TEMPLATE: Test that the service correctly passes parameters through to the client.
    //           If your service adds transformation logic, test that here.
    let service = stub_service();
    let result = service.greet(None).await.expect("greet should succeed");

    assert!(
        result.get("greeting").is_some(),
        "service greet should return greeting field"
    );
}

#[tokio::test]
async fn test_service_greet_with_name_passes_name_through() {
    // TEMPLATE: Verify the service passes parameters through unchanged.
    //           If your service transforms inputs, test the transformation here.
    let service = stub_service();
    let result = service
        .greet(Some("Bob"))
        .await
        .expect("greet Bob should succeed");

    let greeting = result
        .get("greeting")
        .and_then(|v| v.as_str())
        .expect("greeting field should be present");

    assert!(
        greeting.contains("Bob"),
        "service should pass name through to client; got: {greeting}"
    );
}

#[tokio::test]
async fn test_service_echo_returns_exact_message() {
    // TEMPLATE: Test round-trip fidelity at the service layer.
    let service = stub_service();
    let msg = "service layer echo test";
    let result = service.echo(msg).await.expect("echo should succeed");

    let echo = result
        .get("echo")
        .and_then(|v| v.as_str())
        .expect("echo field should be present");

    assert_eq!(
        echo, msg,
        "service echo should return the input message unchanged"
    );
}

#[tokio::test]
async fn test_service_status_returns_ok() {
    // TEMPLATE: Status checks at the service layer should pass through correctly.
    let service = stub_service();
    let result = service.status().await.expect("status should succeed");

    assert_eq!(
        result.get("status").and_then(|v| v.as_str()),
        Some("ok"),
        "service status should return ok"
    );
}

#[test]
fn test_scaffold_intent_transformation_lives_in_service() {
    let service = stub_service();
    let result = service.scaffold_intent(ScaffoldIntent {
        display_name: "Lab Gateway".into(),
        crate_name: "lab-gateway-mcp".into(),
        binary_name: "lab-gateway".into(),
        server_category: "application platform".into(),
        env_prefix: "lab".into(),
        auth_kind: "api key".into(),
        host: "".into(),
        port: 3100,
        mcp_transport: "streamable-http".into(),
        mcp_primitives: "tools, resources, tools".into(),
        deployment: "containers".into(),
        plugins: "claude, gemini, none".into(),
        publish_mcp: true,
        crawl_urls: "https://docs.example.test, https://api.example.test".into(),
        crawl_repos: "".into(),
        crawl_search_topics: "Lab API".into(),
    });

    assert_eq!(result["server_category"], "application-platform");
    assert_eq!(result["project"]["service_name"], "lab_gateway");
    assert_eq!(result["project"]["env_prefix"], "LAB");
    assert_eq!(result["upstream"]["base_url_env"], "LAB_API_URL");
    assert_eq!(result["upstream"]["auth_kind"], "api-key");
    assert_eq!(result["runtime"]["host"], "127.0.0.1");
    assert_eq!(result["runtime"]["mcp_transport"], "http");
    assert_eq!(result["deployment"], "docker");
    assert_eq!(result["plugins"], serde_json::json!(["claude", "gemini"]));
    assert_eq!(
        result["mcp_primitives"],
        serde_json::json!(["tools", "resources"])
    );
}

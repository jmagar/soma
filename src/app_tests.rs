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

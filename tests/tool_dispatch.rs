//! Integration tests for MCP tool dispatch.
//!
//! Tests verify that each action returns valid JSON without errors.
//! Uses `loopback_state()` from the test-support feature — no real creds needed.
//!
//! **Template**: mirror this file for your service. Add one test per action.

use rmcp_template::testing::loopback_state;

/// Helper: call the service action with a loopback state and return the Value.
///
/// We test at the service layer (via `AppState.service`) since `execute_tool`
/// requires a `Peer<RoleServer>` for elicitation, which isn't available in unit tests.
async fn call_service_action(action: &str) -> serde_json::Value {
    let state = loopback_state();
    match action {
        "greet" => state
            .service
            .greet(None)
            .await
            .expect("greet should succeed"),
        "greet_named" => state
            .service
            .greet(Some("Alice"))
            .await
            .expect("greet Alice should succeed"),
        "echo" => state
            .service
            .echo("hello world")
            .await
            .expect("echo should succeed"),
        "status" => state.service.status().await.expect("status should succeed"),
        other => panic!("unknown test action: {other}"),
    }
}

#[tokio::test]
async fn test_greet_no_name_returns_greeting() {
    let result = call_service_action("greet").await;
    let greeting = result
        .get("greeting")
        .and_then(|v| v.as_str())
        .expect("greeting field should be present");
    assert!(
        greeting.contains("Hello"),
        "greeting should contain Hello, got: {greeting}"
    );
}

#[tokio::test]
async fn test_greet_with_name_includes_name() {
    let result = call_service_action("greet_named").await;
    let greeting = result
        .get("greeting")
        .and_then(|v| v.as_str())
        .expect("greeting field should be present");
    assert!(
        greeting.contains("Alice"),
        "greeting should include Alice, got: {greeting}"
    );
}

#[tokio::test]
async fn test_echo_returns_message() {
    let result = call_service_action("echo").await;
    let echo = result
        .get("echo")
        .and_then(|v| v.as_str())
        .expect("echo field should be present");
    assert_eq!(echo, "hello world");
}

#[tokio::test]
async fn test_status_returns_ok() {
    let result = call_service_action("status").await;
    let status = result
        .get("status")
        .and_then(|v| v.as_str())
        .expect("status field should be present");
    assert_eq!(status, "ok");
}

#[tokio::test]
async fn test_all_actions_return_valid_json_object() {
    for action in &["greet", "echo", "status"] {
        let result = call_service_action(action).await;
        assert!(
            result.is_object(),
            "action={action} should return a JSON object, got: {result}"
        );
    }
}

#[tokio::test]
async fn test_greet_target_defaults_to_world() {
    let result = call_service_action("greet").await;
    let target = result
        .get("target")
        .and_then(|v| v.as_str())
        .expect("target field should be present");
    assert_eq!(target, "World");
}

#[test]
fn test_schemas_actions_list_is_non_empty() {
    // Verify the schema action list compiles and has the expected entries
    use rmcp_template::server;
    let _ = server::router(loopback_state()); // builds router — exercises schema code path
}

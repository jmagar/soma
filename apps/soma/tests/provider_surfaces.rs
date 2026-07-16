use serde_json::json;
use soma::testing::loopback_state;
use soma_application::{ExecuteActionRequest, ExecutionContext};
use soma_domain::{AuthorizationMode, RequestId, Surface};

fn context(authorization_mode: AuthorizationMode, surface: Surface, id: &str) -> ExecutionContext {
    let mut context = ExecutionContext::loopback(surface, RequestId::new(id).unwrap());
    context.authorization_mode = authorization_mode;
    context
}

#[tokio::test]
async fn static_provider_preserves_builtin_service_outputs() {
    let state = loopback_state();
    let output = state
        .application()
        .execute_action(
            ExecuteActionRequest {
                action: "greet".to_owned(),
                params: json!({"name": "Alice"}),
            },
            context(
                AuthorizationMode::LoopbackDev,
                Surface::Mcp,
                "provider-greet",
            ),
        )
        .await
        .expect("greet dispatch");

    assert_eq!(output.output["greeting"], "Hello, Alice!");
}

#[tokio::test]
async fn static_provider_help_is_public_and_rest_exposed() {
    let state = loopback_state();
    let output = state
        .application()
        .execute_action(
            ExecuteActionRequest {
                action: "help".to_owned(),
                params: json!({}),
            },
            context(AuthorizationMode::Mounted, Surface::Rest, "provider-help"),
        )
        .await
        .expect("help dispatch");

    assert_eq!(output.output["preferred_rest_style"], "direct_routes");
}

#[tokio::test]
async fn static_provider_mcp_interactive_actions_are_not_rest_exposed() {
    let state = loopback_state();
    let error = state
        .application()
        .execute_action(
            ExecuteActionRequest {
                action: "elicit_name".to_owned(),
                params: json!({}),
            },
            context(
                AuthorizationMode::LoopbackDev,
                Surface::Rest,
                "provider-elicit",
            ),
        )
        .await
        .expect_err("elicitation should remain MCP-only");

    assert_eq!(error.code, "surface_not_exposed");
}

use async_trait::async_trait;
use rmcp::model::{
    CallToolRequestParams, GetPromptRequestParams, ReadResourceRequestParams, ResourceContents,
};
use rmcp::ServiceExt;
use serde_json::{json, Map, Value};
use soma_application::{
    ExecutionContext, GatewayExecuteRequest, GatewayPort, GatewayPromptRoute, GatewayReloadRequest,
    GatewayResourceRoute, GatewayRouteScope, GatewayToolRoute, PortError,
};
use soma_test_support::{tracing_test_lock, SharedBuf};

use crate::{rmcp_server, testing::loopback_state_with_gateway};

struct RecordingGateway;

#[async_trait]
impl GatewayPort for RecordingGateway {
    async fn status(&self, _context: &ExecutionContext) -> Result<Value, PortError> {
        Ok(json!({}))
    }

    async fn reload(
        &self,
        _request: GatewayReloadRequest,
        _context: &ExecutionContext,
    ) -> Result<Value, PortError> {
        Ok(json!({}))
    }

    async fn execute(
        &self,
        _request: GatewayExecuteRequest,
        _context: &ExecutionContext,
    ) -> Result<Value, PortError> {
        Ok(json!({}))
    }

    async fn list_mcp_tools(
        &self,
        _scope: Option<&GatewayRouteScope>,
        _context: &ExecutionContext,
    ) -> Result<Vec<GatewayToolRoute>, PortError> {
        Ok([("echo", "echoes a message"), ("fail", "always fails")]
            .into_iter()
            .map(|(name, description)| GatewayToolRoute {
                name: name.to_owned(),
                description: Some(description.to_owned()),
                input_schema: Some(json!({"type": "object"})),
                output_schema: None,
                destructive: false,
            })
            .collect())
    }

    async fn call_mcp_tool(
        &self,
        name: &str,
        params: Value,
        _scope: Option<&GatewayRouteScope>,
        _context: &ExecutionContext,
    ) -> Result<Option<Value>, PortError> {
        if name == "fail" {
            return Err(PortError::new("upstream_failed", "synthetic failure"));
        }
        Ok((name == "echo").then(|| json!({"echo": params["message"]})))
    }

    async fn list_mcp_resources(
        &self,
        _scope: Option<&GatewayRouteScope>,
        _context: &ExecutionContext,
    ) -> Result<Vec<GatewayResourceRoute>, PortError> {
        Ok(vec![GatewayResourceRoute {
            uri: "mcp-gateway://upstream/mock/test%3A%2F%2Fone".to_owned(),
            native_uri: "test://one".to_owned(),
            name: Some("one".to_owned()),
        }])
    }

    async fn read_mcp_resource(
        &self,
        uri: &str,
        _scope: Option<&GatewayRouteScope>,
        _context: &ExecutionContext,
    ) -> Result<Option<Value>, PortError> {
        Ok(Some(
            json!({"contents": [{"uri": uri, "mimeType": "text/plain", "text": "hello"}]}),
        ))
    }

    async fn list_mcp_prompts(
        &self,
        _scope: Option<&GatewayRouteScope>,
        _context: &ExecutionContext,
    ) -> Result<Vec<GatewayPromptRoute>, PortError> {
        Ok(vec![GatewayPromptRoute {
            name: "hello".to_owned(),
            description: Some("hello prompt".to_owned()),
        }])
    }

    async fn get_mcp_prompt(
        &self,
        name: &str,
        _arguments: Option<Map<String, Value>>,
        _scope: Option<&GatewayRouteScope>,
        _context: &ExecutionContext,
    ) -> Result<Option<Value>, PortError> {
        Ok((name == "hello").then(|| json!({"messages": []})))
    }
}

#[allow(clippy::await_holding_lock)]
#[tokio::test(flavor = "current_thread")]
async fn mcp_server_exposes_application_gateway_tools_resources_and_prompts() -> anyhow::Result<()>
{
    let _lock = tracing_test_lock();
    let buf = SharedBuf::new();
    let subscriber = tracing_subscriber::fmt()
        .with_writer(buf.writer())
        .with_ansi(false)
        .without_time()
        .with_max_level(tracing::Level::INFO)
        .finish();
    let guard = tracing::subscriber::set_default(subscriber);
    let state = loopback_state_with_gateway(std::sync::Arc::new(RecordingGateway));
    let (server_transport, client_transport) = tokio::io::duplex(16 * 1024);
    let server_handle = tokio::spawn(async move {
        rmcp_server(state)
            .serve(server_transport)
            .await?
            .waiting()
            .await?;
        anyhow::Ok(())
    });
    let client = ().serve(client_transport).await?;

    let tools = client.list_tools(Default::default()).await?;
    assert!(tools.tools.iter().any(|tool| tool.name == "soma"));
    assert!(tools.tools.iter().any(|tool| tool.name == "echo"));
    let echo = client
        .call_tool(
            CallToolRequestParams::new("echo").with_arguments(
                json!({"message": "through-soma"})
                    .as_object()
                    .expect("object")
                    .clone(),
            ),
        )
        .await?;
    assert_eq!(
        echo.structured_content,
        Some(json!({"echo": "through-soma"}))
    );
    let failed = client.call_tool(CallToolRequestParams::new("fail")).await?;
    assert_eq!(failed.is_error, Some(true));

    let resources = client.list_resources(Default::default()).await?;
    let uri = resources
        .resources
        .iter()
        .find(|resource| resource.uri.starts_with("mcp-gateway://"))
        .expect("gateway resource")
        .uri
        .clone();
    let resource = client
        .read_resource(ReadResourceRequestParams::new(uri))
        .await?;
    match &resource.contents[0] {
        ResourceContents::TextResourceContents { text, .. } => assert_eq!(text, "hello"),
        other => panic!("unexpected resource contents: {other:?}"),
    }

    let prompts = client.list_prompts(Default::default()).await?;
    assert!(prompts.prompts.iter().any(|prompt| prompt.name == "hello"));
    let prompt = client
        .get_prompt(GetPromptRequestParams::new("hello"))
        .await?;
    assert!(prompt.messages.is_empty());

    client.cancel().await?;
    server_handle.await??;
    drop(guard);

    let logs = buf.contents();
    assert!(
        logs.contains("MCP gateway tool execution completed"),
        "logs were: {logs}"
    );
    assert!(logs.contains("tool=echo"), "logs were: {logs}");
    assert!(
        logs.contains("MCP gateway tool execution failed"),
        "logs were: {logs}"
    );
    assert!(logs.contains("tool=fail"), "logs were: {logs}");
    assert_eq!(
        logs.matches("MCP gateway tool execution completed").count(),
        1,
        "failed gateway calls must not be logged as completed: {logs}"
    );
    Ok(())
}

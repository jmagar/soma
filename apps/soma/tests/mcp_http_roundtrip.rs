//! Real Streamable HTTP MCP protocol round-trip: `initialize` -> `tools/list`
//! -> `tools/call` against the actual `POST /mcp` endpoint, over a real
//! loopback TCP connection — not stdio, not `tower::ServiceExt::oneshot`.
//!
//! Every existing `list_tools()`/`call_tool()` test in this suite
//! (`drop_provider_probe.rs`, `mcp_provider.rs`, `stdio_remote_api.rs`,
//! `stdio_mcp.rs`) drives a stdio-spawned client.
//! `routes_tests.rs::cors_preflight_allows_mcp_protocol_headers` only checks
//! CORS response headers for `POST /mcp`; `transport_tests.rs` only
//! unit-tests `allowed_hosts`/`allowed_origins` string computation; and
//! `soma_serve.rs::soma_serve_starts_http_runtime` only polls `GET /health`.
//! No test previously performed an actual JSON-RPC exchange against the real
//! Streamable HTTP transport.
//!
//! This binds the real Axum router built from `soma::testing::loopback_state()`
//! to a loopback TCP port with `axum::serve` (mirroring `soma_serve.rs`'s
//! server-spawn pattern, but in-process rather than subprocess) and drives it
//! with rmcp's real `StreamableHttpClientTransport` (reqwest-backed) client —
//! a genuine network round trip through `apps/soma/src/routes.rs`'s
//! `mcp_state_for_state`/`streamable_http_service` wiring, which PR 12/13
//! will change as `SomaApplication` and `Config` move to split crates.
#![cfg(feature = "mcp-http")]

use std::net::TcpListener as StdTcpListener;

use rmcp::{
    model::CallToolRequestParams, service::ServiceExt, transport::StreamableHttpClientTransport,
};
use serde_json::json;

async fn spawn_http_mcp_server() -> anyhow::Result<(u16, tokio::task::JoinHandle<()>)> {
    let std_listener = StdTcpListener::bind("127.0.0.1:0")?;
    let port = std_listener.local_addr()?.port();
    std_listener.set_nonblocking(true)?;
    let listener = tokio::net::TcpListener::from_std(std_listener)?;

    let app = soma::server::router(soma::testing::loopback_state());
    let handle = tokio::spawn(async move {
        let _ = axum::serve(listener, app.into_make_service()).await;
    });
    Ok((port, handle))
}

fn structured_result_json(result: &rmcp::model::CallToolResult) -> serde_json::Value {
    if let Some(value) = result.structured_content.clone() {
        return value;
    }
    let value = serde_json::to_value(result).expect("tool result should serialize");
    let text = value["content"][0]["text"]
        .as_str()
        .expect("tool result should contain text content");
    serde_json::from_str(text).expect("tool text content should be JSON")
}

#[tokio::test]
async fn streamable_http_round_trip_lists_tools_and_calls_actions() -> anyhow::Result<()> {
    let (port, server_handle) = spawn_http_mcp_server().await?;
    let url = format!("http://127.0.0.1:{port}/mcp");

    // Real rmcp client, real reqwest-backed Streamable HTTP transport, real
    // TCP connection to the router built by apps/soma/src/routes.rs — this
    // performs the actual initialize -> notifications/initialized ->
    // tools/list -> tools/call JSON-RPC exchange over HTTP.
    let transport = StreamableHttpClientTransport::from_uri(url);
    let service = ().serve(transport).await?;

    let tools = service.list_tools(Default::default()).await?;
    let names: Vec<&str> = tools.tools.iter().map(|tool| tool.name.as_ref()).collect();
    assert_eq!(names, vec!["soma"]);
    assert_eq!(
        tools.tools[0]
            .output_schema
            .as_ref()
            .expect("soma should advertise structured output")["type"],
        "object"
    );

    let status = service
        .call_tool(
            CallToolRequestParams::new("soma")
                .with_arguments(json!({"action": "status"}).as_object().unwrap().clone()),
        )
        .await?;
    let status = structured_result_json(&status);
    assert_eq!(status["status"], "ok", "status payload was {status}");

    let echo = service
        .call_tool(
            CallToolRequestParams::new("soma").with_arguments(
                json!({"action": "echo", "message": "http round trip works"})
                    .as_object()
                    .unwrap()
                    .clone(),
            ),
        )
        .await?;
    let echo = structured_result_json(&echo);
    assert_eq!(echo["echo"], "http round trip works");

    let missing_message = service
        .call_tool(
            CallToolRequestParams::new("soma")
                .with_arguments(json!({"action": "echo"}).as_object().unwrap().clone()),
        )
        .await?;
    assert_eq!(missing_message.is_error, Some(true));
    let error = structured_result_json(&missing_message);
    assert_eq!(error["kind"], "mcp_tool_error");
    assert_eq!(error["code"], "input_schema_failed");

    service.cancel().await?;
    server_handle.abort();
    Ok(())
}

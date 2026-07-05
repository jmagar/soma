use std::{fs, process::Stdio};

use rmcp::{
    model::CallToolRequestParams,
    transport::{ConfigureCommandExt, TokioChildProcess},
    ServiceExt,
};
use serde_json::{json, Map, Value};
use tokio::process::Command;

#[tokio::test]
async fn hot_dropped_mcp_provider_proxies_upstream_tool_call() -> anyhow::Result<()> {
    let temp = tempfile::tempdir()?;
    let providers = temp.path().join("providers");
    let upstream = temp.path().join("upstream");
    fs::create_dir(&providers)?;
    fs::create_dir(&upstream)?;

    let service = stdio_client_in(temp.path()).await?;
    let before = service.list_tools(Default::default()).await?;
    assert!(!action_enum(&before.tools[0].input_schema).contains(&"upstream_echo".to_owned()));

    fs::write(
        providers.join("upstream-mcp.json"),
        serde_json::to_string_pretty(&json!({
            "schema_version": 1,
            "provider": {
                "name": "upstream-mcp",
                "kind": "mcp"
            },
            "tools": [{
                "name": "upstream_echo",
                "description": "Proxy echo through an upstream MCP server.",
                "input_schema": {
                    "type": "object",
                    "required": ["message"],
                    "additionalProperties": false,
                    "properties": {
                        "message": { "type": "string", "minLength": 1 }
                    }
                },
                "meta": {
                    "mcp": {
                        "upstream_tool": "example",
                        "static_args": { "action": "echo" }
                    }
                }
            }],
            "meta": {
                "mcp": {
                    "stdio": {
                        "command": env!("CARGO_BIN_EXE_rtemplate"),
                        "args": ["mcp"],
                        "cwd": upstream.display().to_string()
                    },
                    "timeout_ms": 10000
                }
            }
        }))?,
    )?;

    let after = service.list_tools(Default::default()).await?;
    assert!(action_enum(&after.tools[0].input_schema).contains(&"upstream_echo".to_owned()));

    let result = service
        .call_tool(
            CallToolRequestParams::new("example").with_arguments(
                json!({"action": "upstream_echo", "message": "hello"})
                    .as_object()
                    .unwrap()
                    .clone(),
            ),
        )
        .await?;
    assert_eq!(result.structured_content.unwrap()["echo"], "hello");

    service.cancel().await?;
    Ok(())
}

async fn stdio_client_in(
    cwd: &std::path::Path,
) -> anyhow::Result<rmcp::service::RunningService<rmcp::RoleClient, ()>> {
    let binary = env!("CARGO_BIN_EXE_rtemplate");
    let (transport, _stderr) = TokioChildProcess::builder(Command::new(binary).configure(|cmd| {
        cmd.arg("mcp")
            .current_dir(cwd)
            .env("RUST_LOG", "warn")
            .env_remove("RTEMPLATE_API_URL")
            .env_remove("RTEMPLATE_API_KEY")
            .env_remove("RTEMPLATE_MCP_TOKEN")
            .env_remove("RTEMPLATE_PROVIDER_DIR");
    }))
    .stderr(Stdio::piped())
    .spawn()?;
    Ok(().serve(transport).await?)
}

fn action_enum(schema: &Map<String, Value>) -> Vec<String> {
    schema["properties"]["action"]["enum"]
        .as_array()
        .expect("action enum should exist")
        .iter()
        .map(|value| value.as_str().expect("enum value").to_owned())
        .collect()
}

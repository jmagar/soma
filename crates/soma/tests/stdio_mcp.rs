use std::process::Stdio;

use rmcp::{
    model::CallToolRequestParams,
    service::ServiceExt,
    transport::{ConfigureCommandExt, TokioChildProcess},
};
use serde_json::json;
use tokio::{io::AsyncReadExt, process::Command};

async fn stdio_client() -> anyhow::Result<(
    rmcp::service::RunningService<rmcp::RoleClient, ()>,
    Option<tokio::process::ChildStderr>,
)> {
    let binary = env!("CARGO_BIN_EXE_soma");
    let (transport, stderr) = TokioChildProcess::builder(Command::new(binary).configure(|cmd| {
        cmd.arg("mcp")
            .env("RUST_LOG", "warn")
            .env_remove("SOMA_API_URL")
            .env_remove("SOMA_API_KEY")
            .env_remove("SOMA_MCP_TOKEN");
    }))
    .stderr(Stdio::piped())
    .spawn()?;
    let service = ().serve(transport).await?;
    Ok((service, stderr))
}

fn text_content_json(result: &rmcp::model::CallToolResult) -> serde_json::Value {
    let value = serde_json::to_value(result).expect("tool result should serialize");
    let text = value["content"][0]["text"]
        .as_str()
        .expect("tool result should contain text content");
    serde_json::from_str(text).expect("tool text content should be JSON")
}

#[tokio::test]
async fn stdio_child_process_lists_tools_and_calls_actions() {
    let (service, stderr) = stdio_client().await.unwrap();

    let tools = service.list_tools(Default::default()).await.unwrap();
    let names: Vec<&str> = tools.tools.iter().map(|tool| tool.name.as_ref()).collect();
    assert_eq!(names, vec!["soma"]);

    let status = service
        .call_tool(
            CallToolRequestParams::new("soma")
                .with_arguments(json!({"action": "status"}).as_object().unwrap().clone()),
        )
        .await
        .unwrap();
    let status = text_content_json(&status);
    assert_eq!(status["status"], "ok");

    let echo = service
        .call_tool(
            CallToolRequestParams::new("soma").with_arguments(
                json!({"action": "echo", "message": "stdio works"})
                    .as_object()
                    .unwrap()
                    .clone(),
            ),
        )
        .await
        .unwrap();
    let echo = text_content_json(&echo);
    assert_eq!(echo["echo"], "stdio works");

    service.cancel().await.unwrap();

    if let Some(mut stderr) = stderr {
        let mut logs = String::new();
        match tokio::time::timeout(
            std::time::Duration::from_secs(1),
            stderr.read_to_string(&mut logs),
        )
        .await
        {
            Ok(Ok(_)) => {}
            Ok(Err(error)) => panic!("failed to read stdio child stderr: {error}"),
            Err(_) => panic!("stdio child stderr did not close after cancellation"),
        }
        assert!(
            !logs.contains("MCP server listening") && !logs.contains("HTTP server"),
            "stdio mode must not start network services; stderr was: {logs}"
        );
    }
}

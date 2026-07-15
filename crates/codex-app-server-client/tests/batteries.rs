use std::time::Duration;

use codex_app_server_client::protocol::{
    AgentMessageDeltaNotification, ClientInfo, ConfigReadParams, CurrentTimeReadParams,
    InitializeParams, ServerNotification, ServerRequest, ThreadStartParams, TurnStartParams,
    UserInput,
};
use codex_app_server_client::{
    ApprovalHandler, CodexDaemon, CodexSession, CompatibilityReport, DenyAllApprovalHandler,
    EventCollector, ServerRequestReply, SessionOptions, SurfaceSummary, CODEX_SCHEMA_VERSION,
};
use tokio::io::{AsyncBufReadExt as _, AsyncWriteExt as _, BufReader};

#[test]
fn builders_cover_the_common_no_json_path() {
    let info = ClientInfo::new("demo-client", "0.2.0").with_title("Demo Client");
    assert_eq!(info.name, "demo-client");
    assert_eq!(info.title.as_deref(), Some("Demo Client"));

    let init = InitializeParams::for_client("demo-client", "0.2.0");
    assert_eq!(init.client_info.name, "demo-client");
    assert!(init.capabilities.is_none());

    let thread = ThreadStartParams::new()
        .model("gpt-5")
        .cwd("/workspace")
        .ephemeral(true);
    assert_eq!(thread.model.as_deref(), Some("gpt-5"));
    assert_eq!(thread.cwd.as_deref(), Some("/workspace"));
    assert_eq!(thread.ephemeral, Some(true));

    let turn = TurnStartParams::text("thread-1", "hello");
    assert_eq!(turn.thread_id, "thread-1");
    assert_eq!(turn.input.len(), 1);
    assert!(matches!(&turn.input[0], UserInput::Text { text, .. } if text == "hello"));

    let config = ConfigReadParams::for_cwd("/workspace").include_layers(true);
    assert_eq!(config.cwd.as_deref(), Some("/workspace"));
    assert_eq!(config.include_layers, Some(true));
}

#[test]
fn compatibility_report_summarizes_schema_and_installed_version() {
    let report = CompatibilityReport::from_installed_version(Some(CODEX_SCHEMA_VERSION.to_owned()));
    assert_eq!(report.schema_codex_version, CODEX_SCHEMA_VERSION);
    assert_eq!(
        report.surface,
        SurfaceSummary {
            client_requests: 122,
            server_requests: 11,
            server_notifications: 68,
            client_notifications: 1,
        }
    );
    assert_eq!(
        report.installed_codex_version.as_deref(),
        Some(CODEX_SCHEMA_VERSION)
    );
    assert!(report.schema_matches_installed());

    let stale = CompatibilityReport::from_installed_version(Some("codex-cli 999.0.0".to_owned()));
    assert!(!stale.schema_matches_installed());
}

#[test]
fn event_collector_aggregates_text_diff_completion_and_errors() {
    let mut collector = EventCollector::for_turn("thread-1", "turn-1");

    collector.observe_notification(&ServerNotification::ItemAgentMessageDelta(
        AgentMessageDeltaNotification {
            delta: "hello ".to_owned(),
            item_id: "item-1".to_owned(),
            thread_id: "thread-1".to_owned(),
            turn_id: "turn-1".to_owned(),
        },
    ));
    collector.observe_notification(&ServerNotification::ItemAgentMessageDelta(
        AgentMessageDeltaNotification {
            delta: "world".to_owned(),
            item_id: "item-1".to_owned(),
            thread_id: "thread-1".to_owned(),
            turn_id: "turn-1".to_owned(),
        },
    ));

    let diff = serde_json::json!({
        "method": "turn/diff/updated",
        "params": {
            "threadId": "thread-1",
            "turnId": "turn-1",
            "diff": "diff --git a/file b/file"
        }
    });
    let diff = serde_json::from_value::<ServerNotification>(diff).unwrap();
    collector.observe_notification(&diff);

    let completed = serde_json::json!({
        "method": "turn/completed",
        "params": {
            "threadId": "thread-1",
            "turn": {
                "id": "turn-1",
                "items": [],
                "status": "completed"
            }
        }
    });
    let completed = serde_json::from_value::<ServerNotification>(completed).unwrap();
    collector.observe_notification(&completed);

    assert_eq!(collector.agent_message(), "hello world");
    assert_eq!(collector.latest_diff(), Some("diff --git a/file b/file"));
    assert!(collector.is_complete());
    assert_eq!(collector.errors().len(), 0);
}

#[test]
fn deny_all_approval_handler_replies_with_structured_error() {
    let request = ServerRequest::CurrentTimeRead {
        id: codex_app_server_client::protocol::RequestId::Int64(7),
        params: CurrentTimeReadParams {
            thread_id: "thread-1".to_owned(),
        },
    };
    let handler = DenyAllApprovalHandler::default();

    let reply = handler.handle(&request);
    assert!(matches!(
        reply,
        ServerRequestReply::Error { code: -32000, .. }
    ));
}

#[test]
fn daemon_helper_builds_real_app_server_listen_args() {
    let daemon = CodexDaemon::new("/tmp/codex.sock").with_config("model", "\"gpt-5\"");
    assert_eq!(
        daemon.app_server_args(),
        vec![
            "app-server",
            "--listen",
            "unix:///tmp/codex.sock",
            "-c",
            "model=\"gpt-5\""
        ]
    );
    assert_eq!(
        daemon.start_args(),
        vec!["app-server", "daemon", "-c", "model=\"gpt-5\"", "start"]
    );
}

#[tokio::test]
async fn session_connect_streams_handshakes_before_returning() {
    let (client_io, server_io) = tokio::io::duplex(4096);
    let (client_read, client_write) = tokio::io::split(client_io);
    let (server_read, mut server_write) = tokio::io::split(server_io);

    let server = tokio::spawn(async move {
        let mut reader = BufReader::new(server_read);
        let mut line = String::new();
        reader.read_line(&mut line).await.unwrap();
        let init: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(init["method"], "initialize");
        let id = init["id"].clone();
        let response = serde_json::json!({
            "id": id,
            "result": {
                "codexHome": "/tmp/codex-home",
                "platformFamily": "unix",
                "platformOs": "linux",
                "userAgent": "fake-codex"
            }
        });
        server_write
            .write_all(format!("{response}\n").as_bytes())
            .await
            .unwrap();

        line.clear();
        reader.read_line(&mut line).await.unwrap();
        let initialized: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(initialized["method"], "initialized");
    });

    let options =
        SessionOptions::new("test-client", "0.1.0").with_call_timeout(Duration::from_secs(5));
    let session = CodexSession::connect_streams(BufReader::new(client_read), client_write, options)
        .await
        .unwrap();

    assert_eq!(session.initialize_response().platform_os, "linux");
    server.await.unwrap();
}

use super::*;

#[test]
fn local_and_stdio_modes_default_to_quiet_logging() {
    assert_eq!(default_log_level(true, false), "warn");
    assert_eq!(default_log_level(false, false), "warn");
}

#[test]
fn http_server_mode_defaults_to_info_logging() {
    assert_eq!(default_log_level(false, true), "info");
}

#[cfg(feature = "cli")]
#[tokio::test]
async fn local_cli_composition_builds_the_application_catalog() {
    let providers = tempfile::tempdir().unwrap();
    std::fs::write(
        providers.path().join("fixture.json"),
        r#"{
          "schema_version": 1,
          "provider": { "name": "fixture", "kind": "static-rust" },
          "tools": [{
            "name": "fixture_action",
            "description": "Composition refresh fixture",
            "input_schema": { "type": "object", "properties": {}, "additionalProperties": false },
            "output_schema": { "type": "object", "properties": {}, "additionalProperties": true },
            "cli": { "enabled": true, "command": "fixture" }
          }]
        }"#,
    )
    .unwrap();
    let application =
        build_cli_application_with_provider_dir(&Config::default(), Some(providers.path()))
            .await
            .unwrap();

    assert_eq!(application.resolve_cli_action("status").unwrap(), "status");
    assert_eq!(
        application.provider_for_action("status").as_deref(),
        Some("static-rust")
    );
    assert_eq!(
        application.resolve_cli_action("fixture").unwrap(),
        "fixture_action"
    );
}

#[cfg(feature = "auth")]
#[test]
fn soma_auth_config_builder_supports_upstream_oauth_without_inbound_oauth() {
    let cfg = soma_mcp_auth_config_builder()
        .build_from_sources([
            (
                "SOMA_MCP_PUBLIC_URL".to_string(),
                "https://mcp.example.com".to_string(),
            ),
            (
                "SOMA_MCP_AUTH_SQLITE_PATH".to_string(),
                "/tmp/soma-auth.db".to_string(),
            ),
        ])
        .unwrap();

    assert!(matches!(cfg.mode, soma_auth::config::AuthMode::Bearer));
    assert_eq!(cfg.resource_path, "/mcp");
    assert!(cfg.scopes_supported.contains(&"soma:admin".to_string()));
}

use std::{fs, process::Command};

use serde_json::Value;
use tempfile::tempdir;

fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_rtemplate")
}

const HELLO_PROVIDER_JSON: &str = r#"{
  "schema_version": 1,
  "provider": { "name": "hello", "kind": "static-rust", "version": "0.1.0" },
  "tools": [
    {
      "name": "hello",
      "description": "Hello probe",
      "input_schema": { "type": "object", "properties": {}, "additionalProperties": false },
      "output_schema": { "type": "object", "properties": {}, "additionalProperties": true }
    }
  ]
}"#;

fn provider_cli() -> Command {
    let mut command = Command::new(binary());
    command
        .env("RTEMPLATE_API_URL", "")
        .env_remove("RTEMPLATE_API_KEY")
        .env_remove("RTEMPLATE_MCP_TOKEN");
    command
}

#[test]
fn providers_list_json_reports_dropped_provider() {
    let temp = tempdir().expect("tempdir");
    let providers = temp.path().join("providers");
    fs::create_dir(&providers).expect("create providers dir");
    fs::write(providers.join("hello.json"), HELLO_PROVIDER_JSON).expect("write provider");

    let output = provider_cli()
        .args(["providers", "list", "--dir"])
        .arg(&providers)
        .arg("--json")
        .output()
        .expect("run providers list");

    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(value["summary"]["loaded"], 1);
    assert_eq!(value["files"][0]["provider_id"], "hello");
    assert_eq!(value["files"][0]["actions"][0], "hello");
}

#[test]
fn providers_validate_fails_for_invalid_provider_file() {
    let temp = tempdir().expect("tempdir");
    let providers = temp.path().join("providers");
    fs::create_dir(&providers).expect("create providers dir");
    fs::write(providers.join("broken.json"), "{").expect("write invalid provider");

    let output = provider_cli()
        .args(["providers", "validate", "--dir"])
        .arg(&providers)
        .output()
        .expect("run providers validate");

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("invalid"));
}

#[test]
fn providers_validate_fails_for_semantically_invalid_provider_file() {
    let temp = tempdir().expect("tempdir");
    let providers = temp.path().join("providers");
    fs::create_dir(&providers).expect("create providers dir");
    fs::write(
        providers.join("duplicate-tools.json"),
        r#"{
          "schema_version": 1,
          "provider": { "name": "duplicate-tools", "kind": "static-rust", "version": "0.1.0" },
          "tools": [
            {
              "name": "dup",
              "description": "Duplicate probe one",
              "input_schema": { "type": "object", "properties": {}, "additionalProperties": false }
            },
            {
              "name": "dup",
              "description": "Duplicate probe two",
              "input_schema": { "type": "object", "properties": {}, "additionalProperties": false }
            }
          ]
        }"#,
    )
    .expect("write semantically invalid provider");

    let output = provider_cli()
        .args(["providers", "validate", "--dir"])
        .arg(&providers)
        .arg("--json")
        .output()
        .expect("run providers validate");

    assert!(!output.status.success());
    let value: Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(value["valid"], false);
    assert_eq!(value["files"][0]["status"], "invalid");
    assert!(value["files"][0]["error"]
        .as_str()
        .unwrap_or_default()
        .contains("duplicate_tool_name"));
}

#[test]
fn providers_status_uses_rtemplate_provider_dir_environment() {
    let temp = tempdir().expect("tempdir");
    let providers = temp.path().join("custom-providers");
    fs::create_dir(&providers).expect("create providers dir");

    let output = provider_cli()
        .args(["providers", "status"])
        .env("RTEMPLATE_PROVIDER_DIR", &providers)
        .output()
        .expect("run providers status");

    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains(&providers.display().to_string()));
}

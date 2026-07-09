use std::{fs, path::PathBuf, process::Command};

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

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("workspace root")
        .to_path_buf()
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
fn providers_validate_fails_for_explicit_missing_provider_directory() {
    let temp = tempdir().expect("tempdir");
    let missing = temp.path().join("missing-providers");

    let output = provider_cli()
        .args(["providers", "validate", "--dir"])
        .arg(&missing)
        .arg("--json")
        .output()
        .expect("run providers validate");

    assert!(!output.status.success());
    let value: Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(value["exists"], false);
    assert_eq!(value["valid"], false);
}

#[test]
fn providers_validate_accepts_documented_examples_directory() {
    let examples = workspace_root().join("examples/providers");

    let output = provider_cli()
        .args(["providers", "validate", "--dir"])
        .arg(&examples)
        .arg("--json")
        .output()
        .expect("run providers validate");

    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(value["exists"], true);
    assert_eq!(value["valid"], true);
    assert_eq!(value["summary"]["loaded"], 3);
}

#[test]
fn documented_ai_sdk_example_reads_cli_params_envelope() {
    if !ai_sdk_runtime_available() {
        eprintln!("skipping documented AI SDK provider execution test because node is unavailable or unhealthy");
        return;
    }

    let examples = workspace_root().join("examples/providers");

    let output = provider_cli()
        .args(["hello_ai_sdk", "--json", r#"{"message":"hello"}"#])
        .env("RTEMPLATE_PROVIDER_DIR", &examples)
        .output()
        .expect("run AI SDK example provider");

    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(value["ok"], true);
    assert_eq!(value["echoed"], "hello");
}

fn ai_sdk_runtime_available() -> bool {
    if cfg!(windows) {
        return false;
    }

    Command::new("node")
        .args([
            "-e",
            "require('node:crypto').randomBytes(1); console.log('ok')",
        ])
        .status()
        .is_ok_and(|status| status.success())
}

#[test]
fn dynamic_provider_yes_confirms_destructive_provider_cli_dispatch() {
    let temp = tempdir().expect("tempdir");
    let providers = temp.path().join("providers");
    fs::create_dir(&providers).expect("create providers dir");
    fs::write(
        providers.join("danger.json"),
        r#"{
          "schema_version": 1,
          "provider": { "name": "danger", "kind": "static-rust", "version": "0.1.0" },
          "tools": [
            {
              "name": "danger_delete",
              "description": "Delete probe",
              "destructive": true,
              "input_schema": { "type": "object", "properties": {}, "additionalProperties": false },
              "output_schema": { "type": "object", "additionalProperties": true },
              "cli": { "enabled": true, "command": "danger-delete" },
              "meta": { "result": { "ok": true, "deleted": true } }
            }
          ]
        }"#,
    )
    .expect("write destructive provider");

    let rejected = provider_cli()
        .arg("danger-delete")
        .env("RTEMPLATE_PROVIDER_DIR", &providers)
        .output()
        .expect("run destructive provider without yes");

    assert!(!rejected.status.success());
    assert!(String::from_utf8_lossy(&rejected.stderr).contains("--yes"));

    let accepted = provider_cli()
        .args(["danger-delete", "--yes"])
        .env("RTEMPLATE_PROVIDER_DIR", &providers)
        .output()
        .expect("run destructive provider with yes");

    assert!(
        accepted.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&accepted.stderr)
    );
    let value: Value = serde_json::from_slice(&accepted.stdout).expect("json output");
    assert_eq!(value["ok"], true);
    assert_eq!(value["deleted"], true);
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

use serde_json::json;

use crate::{
    provider_validation::validate_provider_manifest_value,
    providers::{EnvRequirement, ProviderManifest},
};

fn valid_manifest() -> serde_json::Value {
    json!({
        "schema_version": 1,
        "provider": { "name": "demo", "kind": "static-rust" },
        "tools": [{
            "name": "weather",
            "description": "Fetch weather.",
            "input_schema": { "type": "object", "properties": {} },
            "rest": { "enabled": true, "method": "POST", "path": "/v1/weather" },
            "cli": { "enabled": true, "command": "weather", "aliases": ["wx"] }
        }],
        "env": [{ "name": "API_KEY" }]
    })
}

#[test]
fn validates_manifest_and_server_prefixed_env() {
    let manifest = validate_provider_manifest_value(&valid_manifest()).expect("valid manifest");
    assert_eq!(manifest.env[0].runtime_name("LAB"), "LAB_API_KEY");
}

#[test]
fn rejects_mcp_default_rest_exposure() {
    let mut value = valid_manifest();
    value["provider"]["kind"] = json!("mcp");
    let error = validate_provider_manifest_value(&value).expect_err("MCP REST default fails");
    assert_eq!(error.code(), "mcp_rest_requires_explicit_opt_in");
}

#[test]
fn rejects_duplicate_tool_names() {
    let mut value = valid_manifest();
    value["tools"] = json!([
        {"name":"dupe","description":"one","input_schema":{"type":"object"}},
        {"name":"dupe","description":"two","input_schema":{"type":"object"}}
    ]);
    let error = validate_provider_manifest_value(&value).expect_err("duplicate fails");
    assert_eq!(error.code(), "duplicate_tool_name");
}

#[test]
fn rejects_duplicate_routes_and_cli_aliases() {
    let mut value = valid_manifest();
    value["tools"] = json!([
        {"name":"one","description":"one","input_schema":{"type":"object"},"rest":{"enabled":true,"method":"POST","path":"/v1/shared"},"cli":{"enabled":true,"command":"one","aliases":["same"]}},
        {"name":"two","description":"two","input_schema":{"type":"object"},"rest":{"enabled":true,"method":"POST","path":"/v1/shared"},"cli":{"enabled":true,"command":"two"}}
    ]);
    let error = validate_provider_manifest_value(&value).expect_err("route duplicate fails");
    assert_eq!(error.code(), "duplicate_rest_route");

    let mut value = valid_manifest();
    value["tools"] = json!([
        {"name":"one","description":"one","input_schema":{"type":"object"},"cli":{"enabled":true,"command":"one","aliases":["same"]}},
        {"name":"two","description":"two","input_schema":{"type":"object"},"cli":{"enabled":true,"command":"two","aliases":["same"]}}
    ]);
    let error = validate_provider_manifest_value(&value).expect_err("cli duplicate fails");
    assert_eq!(error.code(), "duplicate_cli_command");
}

#[test]
fn rejects_reserved_cli_names_denied_capabilities_and_prefixed_env() {
    let mut value = valid_manifest();
    value["tools"][0]["cli"] = json!({"enabled":true,"command":"doctor"});
    let error = validate_provider_manifest_value(&value).expect_err("reserved name fails");
    assert_eq!(error.code(), "reserved_cli_command");

    let mut value = valid_manifest();
    value["capabilities"] = json!({"filesystem":{"enabled":true,"read_roots":["/tmp"]}});
    let error = validate_provider_manifest_value(&value).expect_err("capability fails");
    assert_eq!(error.code(), "denied_capability");

    let mut value = valid_manifest();
    value["env"] = json!([{"name":"LAB_API_KEY"}]);
    let error = validate_provider_manifest_value(&value).expect_err("prefixed env fails");
    assert_eq!(error.code(), "invalid_env_declaration");
}

#[test]
fn contracts_do_not_expose_execution_types() {
    let _manifest: Option<ProviderManifest> = None;
    let _env: Option<EnvRequirement> = None;
}

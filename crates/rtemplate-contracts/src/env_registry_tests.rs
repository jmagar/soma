use super::*;

#[test]
fn registry_contains_core_runtime_keys() {
    let keys: Vec<&str> = all_specs().iter().map(|spec| spec.key).collect();
    for expected in [
        "RTEMPLATE_API_URL",
        "RTEMPLATE_API_KEY",
        "RTEMPLATE_MCP_TOKEN",
        "RTEMPLATE_MCP_HOST",
        "RTEMPLATE_MCP_PORT",
    ] {
        assert!(keys.contains(&expected), "missing {expected}");
    }
}

#[test]
fn secret_keys_are_marked_secret() {
    assert!(spec_for("RTEMPLATE_API_KEY").unwrap().secret);
    assert!(spec_for("RTEMPLATE_MCP_TOKEN").unwrap().secret);
    assert!(!spec_for("RTEMPLATE_API_URL").unwrap().secret);
}

#[test]
fn plugin_option_mapping_is_derived_from_specs() {
    let mappings: Vec<_> = plugin_option_mappings().collect();
    assert!(mappings.contains(&("CLAUDE_PLUGIN_OPTION_SOMA_API_URL", "RTEMPLATE_API_URL")));
    assert!(mappings.contains(&("CLAUDE_PLUGIN_OPTION_API_TOKEN", "RTEMPLATE_MCP_TOKEN")));
    assert!(mappings.contains(&(
        "CLAUDE_PLUGIN_OPTION_GOOGLE_CLIENT_SECRET",
        "RTEMPLATE_MCP_GOOGLE_CLIENT_SECRET"
    )));
}

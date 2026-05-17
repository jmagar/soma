use crate::actions::mcp_action_names;

use super::tool_definitions;

#[test]
fn schema_action_enum_comes_from_mcp_action_metadata() {
    let tools = tool_definitions();
    let enum_values = tools[0]["inputSchema"]["properties"]["action"]["enum"]
        .as_array()
        .expect("action enum should be an array")
        .iter()
        .map(|value| value.as_str().expect("action enum values are strings"))
        .collect::<Vec<_>>();

    // Only MCP-enabled actions appear in the tool schema.
    assert_eq!(enum_values, mcp_action_names());
}

#[test]
fn echo_message_schema_requires_non_empty_string() {
    let tools = tool_definitions();
    assert_eq!(
        tools[0]["inputSchema"]["properties"]["message"]["minLength"],
        1
    );
}

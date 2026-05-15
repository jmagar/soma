//! Tool JSON schemas for the MCP example tool.
//!
//! This file defines the action list and input schema for the `example` tool.
//! MCP clients inspect this schema to know what arguments are valid.
//!
//! **Template**: rename `example` to your tool name. Add/remove actions and
//! parameters to match your service. Use `"required": [...]` for mandatory args.

use std::sync::OnceLock;

use serde_json::{json, Value};

use crate::actions::action_names;

/// Cached JSON schema definitions (static data, built once at first call).
static TOOL_DEFINITIONS: OnceLock<Vec<Value>> = OnceLock::new();

/// Return the JSON schema definitions for all tools (cached after first call).
///
/// Returns a `Vec<Value>` where each item is a tool definition object matching
/// the MCP `Tool` schema: `{ name, description, inputSchema }`.
///
/// This is also used by the schema resource (`example://schema/mcp-tool`).
pub(super) fn tool_definitions() -> &'static Vec<Value> {
    TOOL_DEFINITIONS.get_or_init(build_tool_definitions)
}

fn build_tool_definitions() -> Vec<Value> {
    vec![json!({
        "name": "example",
        "description": "Example MCP tool demonstrating the action-based dispatch pattern. Use action=help for full documentation.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "description": "The operation to perform.",
                    "enum": action_names()
                },
                "name": {
                    "type": "string",
                    "description": "Name to greet (optional, action=greet only). Omit to greet the world."
                },
                "message": {
                    "type": "string",
                    "minLength": 1,
                    "description": "Message to echo back (required for action=echo)."
                }
            },
            "required": ["action"]
        }
    })]
}

#[cfg(test)]
mod tests {
    use crate::actions::action_names;

    use super::tool_definitions;

    #[test]
    fn schema_action_enum_comes_from_action_metadata() {
        let tools = tool_definitions();
        let enum_values = tools[0]["inputSchema"]["properties"]["action"]["enum"]
            .as_array()
            .expect("action enum should be an array")
            .iter()
            .map(|value| value.as_str().expect("action enum values are strings"))
            .collect::<Vec<_>>();

        assert_eq!(enum_values, action_names());
    }

    #[test]
    fn echo_message_schema_requires_non_empty_string() {
        let tools = tool_definitions();
        assert_eq!(
            tools[0]["inputSchema"]["properties"]["message"]["minLength"],
            1
        );
    }
}

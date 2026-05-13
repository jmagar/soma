//! Tool JSON schemas for the MCP example tool.
//!
//! This file defines the action list and input schema for the `example` tool.
//! MCP clients inspect this schema to know what arguments are valid.
//!
//! **Template**: rename `example` to your tool name. Add/remove actions and
//! parameters to match your service. Use `"required": [...]` for mandatory args.

use serde_json::{json, Value};

/// All valid actions for the `example` tool.
pub(super) const EXAMPLE_ACTIONS: &[&str] = &["greet", "echo", "status", "elicit_name", "help"];

/// Generate the JSON schema definitions for all tools exposed by this server.
///
/// Returns a `Vec<Value>` where each item is a tool definition object matching
/// the MCP `Tool` schema: `{ name, description, inputSchema }`.
///
/// This is also used by the schema resource (`example://schema/mcp-tool`).
pub(super) fn tool_definitions() -> Vec<Value> {
    vec![json!({
        "name": "example",
        "description": "Example MCP tool demonstrating the action-based dispatch pattern. Use action=help for full documentation.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "description": "The operation to perform.",
                    "enum": EXAMPLE_ACTIONS
                },
                "name": {
                    "type": "string",
                    "description": "Name to greet (optional, action=greet only). Omit to greet the world."
                },
                "message": {
                    "type": "string",
                    "description": "Message to echo back (required for action=echo)."
                }
            },
            "required": ["action"]
        }
    })]
}

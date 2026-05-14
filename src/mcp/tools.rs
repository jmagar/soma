//! MCP tool dispatch — thin shims only.
//!
//! **Rule**: no business logic here. Parse args → call service → return Value.
//! All logic belongs in `app.rs` (or `example.rs` for transport concerns).
//!
//! The `peer` parameter is threaded through so that elicitation actions can
//! ask the MCP client for user input mid-call. For non-elicitation actions
//! it is unused.

use rmcp::{
    service::{ElicitationError, Peer},
    RoleServer,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::server::AppState;

/// Dispatch an incoming MCP tool call to the appropriate handler.
///
/// `name`   — tool name (matches schema, currently only "example")
/// `args`   — parsed JSON arguments from the MCP client
/// `peer`   — connection to the MCP client; used for elicitation
pub(super) async fn execute_tool(
    state: &AppState,
    name: &str,
    args: Value,
    peer: &Peer<RoleServer>,
) -> anyhow::Result<Value> {
    match name {
        "example" => dispatch_example(state, args, peer).await,
        _ => Err(anyhow::anyhow!("unknown tool: {name}")),
    }
}

async fn dispatch_example(
    state: &AppState,
    args: Value,
    peer: &Peer<RoleServer>,
) -> anyhow::Result<Value> {
    let action =
        string_arg(&args, "action").ok_or_else(|| anyhow::anyhow!("action is required"))?;

    match action.as_str() {
        "greet" => {
            let name = string_arg(&args, "name");
            state.service.greet(name.as_deref()).await
        }
        "echo" => {
            let message = string_arg(&args, "message")
                .ok_or_else(|| anyhow::anyhow!("`message` is required for action=echo"))?;
            state.service.echo(&message).await
        }
        "status" => state.service.status().await,
        "elicit_name" => elicit_name(peer).await,
        "help" => Ok(json!({ "help": HELP_TEXT })),
        other => Err(anyhow::anyhow!(
            "unknown example action: {other}; use action=help for documentation"
        )),
    }
}

// ── elicitation ───────────────────────────────────────────────────────────────

/// Input schema for the elicit_name elicitation request.
///
/// `ElicitationSafe` requires this to be a struct (object schema) — not a primitive.
/// The MCP client renders this as a form for the user to fill in.
///
/// Add `#[schemars(description = "...")]` on fields to provide field-level hints
/// in the client's UI.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct NameInput {
    /// Your first name (or whatever you'd like to be called)
    name: String,
}

// Mark as safe for elicitation — proves this type generates an "object" JSON schema.
rmcp::elicit_safe!(NameInput);

/// Ask the MCP client to elicit the user's name, then return a personalised greeting.
///
/// # How MCP elicitation works
///
/// Elicitation is an MCP protocol feature (spec 2025-06-18) where the *server*
/// requests input from the *user* (via the MCP client) mid-call:
///
/// 1. Server sends `elicitation/create` with a message and a JSON Schema
/// 2. Client displays a form to the user
/// 3. User fills in the form and submits (accept), refuses (decline), or closes (cancel)
/// 4. Client returns the user's choice + their data back to the server
/// 5. Server processes the response and continues the tool call
///
/// `peer.elicit::<T>()` handles the schema generation and response parsing automatically.
///
/// # Client compatibility
///
/// Only clients that declared the `elicitation` capability during the MCP initialisation
/// handshake will respond. If the client doesn't support it, this returns a graceful
/// fallback message rather than an error.
async fn elicit_name(peer: &Peer<RoleServer>) -> anyhow::Result<Value> {
    match peer
        .elicit::<NameInput>("What is your name? I'll use it to give you a personalised greeting.")
        .await
    {
        Ok(Some(input)) => {
            let name = input.name.trim().to_string();
            if name.is_empty() {
                Ok(json!({
                    "greeting": "Hello, mysterious stranger!",
                    "note": "You submitted an empty name — that's perfectly fine!",
                }))
            } else {
                Ok(json!({
                    "greeting": format!("Hello, {name}! Welcome to the example MCP server."),
                    "name": name,
                }))
            }
        }
        Ok(None) => Ok(json!({
            "greeting": "Hello! (you provided no name — that's okay)",
        })),
        Err(ElicitationError::UserDeclined) => Ok(json!({
            "message": "No problem — you chose not to share your name.",
            "greeting": "Hello, anonymous user!",
        })),
        Err(ElicitationError::UserCancelled) => Ok(json!({
            "message": "Elicitation was cancelled.",
            "greeting": "Hello there!",
        })),
        Err(ElicitationError::CapabilityNotSupported) => {
            tracing::warn!("elicitation requested but client does not support it");
            Ok(json!({
                "message": "Elicitation is not supported by this MCP client.",
                "hint": "Try a client like Claude.app that supports MCP elicitation (spec 2025-06-18).",
                "fallback_greeting": "Hello, World! (elicitation unavailable)",
            }))
        }
        Err(e) => {
            tracing::warn!(error = %e, "elicitation failed");
            Ok(json!({
                "message": "Elicitation failed — see server logs for details.",
                "error": e.to_string(),
                "fallback_greeting": "Hello, World!",
            }))
        }
    }
}

// ── arg helpers ───────────────────────────────────────────────────────────────

fn string_arg(args: &Value, name: &str) -> Option<String> {
    args.get(name).and_then(|v| v.as_str()).map(String::from)
}

// ── help text ─────────────────────────────────────────────────────────────────

const HELP_TEXT: &str = r#"# example MCP Tool

A template demonstrating the action-based dispatch pattern for MCP servers.
Set the `action` argument to select an operation.

## Actions

### greet
Return a greeting. Optional `name` parameter (string).
Example: `{ "action": "greet", "name": "Alice" }`

### echo
Echo a message back unchanged. Required `message` parameter (string).
Example: `{ "action": "echo", "message": "Hello!" }`

### status
Return the server status and configuration info.
Example: `{ "action": "status" }`

### elicit_name
Demonstrates MCP elicitation — the server asks the user for their name
via the MCP client UI, then returns a personalised greeting.
Requires a client that supports the MCP elicitation capability (spec 2025-06-18).
Example: `{ "action": "elicit_name" }`

### help
Show this documentation.
Example: `{ "action": "help" }`

## Adding a new action

1. Add the action name to `EXAMPLE_ACTIONS` in `mcp/schemas.rs`.
2. Add any new parameters to the `inputSchema` in `mcp/schemas.rs`.
3. Add a method to `ExampleClient` in `example.rs` (transport).
4. Add a method to `ExampleService` in `app.rs` (business logic).
5. Add a match arm in `dispatch_example()` in `mcp/tools.rs`.
6. Add the action to `READ_ONLY_ACTIONS` in `mcp/rmcp_server.rs`.
7. Add a test in `tests/tool_dispatch.rs`.
"#;

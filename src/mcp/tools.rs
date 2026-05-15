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

use crate::actions::{execute_service_action, ExampleAction};
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
    let action = ExampleAction::from_mcp_args(&args)?;

    match action {
        ExampleAction::ElicitName => elicit_name(peer).await,
        ExampleAction::ScaffoldIntent => scaffold_intent(peer).await,
        ExampleAction::Help => Ok(json!({ "help": HELP_TEXT })),
        other => execute_service_action(&state.service, &other).await,
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

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct ScaffoldIntentInput {
    /// Human-readable project name, e.g. "Unraid MCP" or "Lab Gateway"
    display_name: String,
    /// Cargo package name, e.g. "unraid-mcp"
    crate_name: String,
    /// Binary/tool name, e.g. "unraid"
    binary_name: String,
    /// Server category: "upstream-client" or "application-platform"
    server_category: String,
    /// Environment variable prefix, e.g. "UNRAID" or "LAB"
    env_prefix: String,
    /// Upstream auth kind: "none", "api-key", "bearer", "oauth", or "other"
    auth_kind: String,
    /// Upstream resource groups, comma-separated, e.g. "vms, shares, docker"
    resource_groups: String,
    /// Read actions, comma-separated, e.g. "list_vms, get_status"
    read_actions: String,
    /// Write/destructive actions, comma-separated. Leave blank if none.
    write_actions: String,
    /// MCP-only actions, comma-separated. Leave blank if none.
    mcp_only_actions: String,
}

rmcp::elicit_safe!(ScaffoldIntentInput);

/// Ask the MCP client to collect scaffold requirements, then return JSON for a skill handoff.
///
/// This function intentionally does not mutate files. The returned JSON is consumed by
/// the `scaffold-project` skill, which drafts an approval-first plan for the user.
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
async fn scaffold_intent(peer: &Peer<RoleServer>) -> anyhow::Result<Value> {
    match peer
        .elicit::<ScaffoldIntentInput>(
            "Tell me what kind of project you are scaffolding. I will return JSON only; the scaffold-project skill will turn it into an approval-first plan.",
        )
        .await
    {
        Ok(Some(input)) => Ok(scaffold_intent_json(input)),
        Ok(None) => Ok(json!({
            "kind": "rmcp_template_scaffold_intent",
            "schema_version": 1,
            "status": "no_input",
            "message": "No scaffold intent was provided.",
        })),
        Err(ElicitationError::UserDeclined) => Ok(json!({
            "kind": "rmcp_template_scaffold_intent",
            "schema_version": 1,
            "status": "declined",
            "message": "User declined to provide scaffold intent.",
        })),
        Err(ElicitationError::UserCancelled) => Ok(json!({
            "kind": "rmcp_template_scaffold_intent",
            "schema_version": 1,
            "status": "cancelled",
            "message": "Scaffold intent elicitation was cancelled.",
        })),
        Err(ElicitationError::CapabilityNotSupported) => Ok(json!({
            "kind": "rmcp_template_scaffold_intent",
            "schema_version": 1,
            "status": "elicitation_not_supported",
            "message": "This MCP client does not support elicitation.",
            "fallback": {
                "recommended_skill": "scaffold-project",
                "instructions": "Ask the user for the scaffold fields manually, then create the same JSON shape documented by the scaffold-project skill. Do not mutate files until the user approves the plan."
            }
        })),
        Err(e) => {
            tracing::error!(error = %e, "scaffold intent elicitation failed unexpectedly");
            Err(anyhow::anyhow!("scaffold intent elicitation failed unexpectedly: {e}"))
        }
    }
}

fn scaffold_intent_json(input: ScaffoldIntentInput) -> Value {
    let category = normalize_category(&input.server_category);
    let required_surfaces = if category == "application-platform" {
        vec!["api", "cli", "mcp", "web"]
    } else {
        vec!["mcp", "cli"]
    };
    let service_name = input.binary_name.trim().replace('-', "_");
    let env_prefix = input.env_prefix.trim().to_ascii_uppercase();

    json!({
        "kind": "rmcp_template_scaffold_intent",
        "schema_version": 1,
        "server_category": category,
        "required_surfaces": required_surfaces,
        "project": {
            "display_name": input.display_name.trim(),
            "crate_name": input.crate_name.trim(),
            "binary_name": input.binary_name.trim(),
            "service_name": service_name,
            "env_prefix": env_prefix,
        },
        "upstream": {
            "base_url_env": format!("{env_prefix}_API_URL"),
            "auth_kind": input.auth_kind.trim(),
            "resource_groups": split_csv(&input.resource_groups),
        },
        "actions": {
            "read": split_csv(&input.read_actions),
            "write": split_csv(&input.write_actions),
            "mcp_only": split_csv(&input.mcp_only_actions),
            "cli_only_operational": ["serve", "mcp", "doctor", "watch", "setup"],
        },
        "handoff": {
            "recommended_skill": "scaffold-project",
            "instructions": "Create an approval-first scaffold plan from this JSON. Do not mutate files until the user approves the plan.",
        },
        "policy": {
            "business_action_minimum_surfaces": ["mcp", "cli"],
            "upstream_client_surfaces": ["mcp", "cli"],
            "application_platform_surfaces": ["api", "cli", "mcp", "web"],
        }
    })
}

fn normalize_category(category: &str) -> &'static str {
    let normalized = category.trim().to_ascii_lowercase();
    if normalized.contains("application") || normalized.contains("platform") {
        "application-platform"
    } else {
        "upstream-client"
    }
}

fn split_csv(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

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
            tracing::error!(error = %e, "elicitation failed unexpectedly");
            Err(anyhow::anyhow!("elicitation failed unexpectedly: {e}"))
        }
    }
}

// ── arg helpers ───────────────────────────────────────────────────────────────

// ── help text ─────────────────────────────────────────────────────────────────

const HELP_TEXT: &str = r#"# example MCP Tool

A template demonstrating the action-based dispatch pattern for MCP servers.
Set the `action` argument to select an operation.

## Actions

### greet
Return a greeting. Optional `name` parameter (string).
Example: `{ "action": "greet", "name": "Alice" }`

### echo
Echo a message back unchanged. Required `message` parameter (non-empty string).
Example: `{ "action": "echo", "message": "Hello!" }`

### status
Return the server status and configuration info.
Example: `{ "action": "status" }`

### elicit_name
Demonstrates MCP elicitation — the server asks the user for their name
via the MCP client UI, then returns a personalised greeting.
Requires a client that supports the MCP elicitation capability (spec 2025-06-18).
Example: `{ "action": "elicit_name" }`

### scaffold_intent
Uses MCP elicitation to collect project scaffold intent and returns JSON for the
`scaffold-project` skill. This action does not mutate files; the skill creates an
approval-first plan that the user can accept, edit, or reject.
Example: `{ "action": "scaffold_intent" }`

### help
Show this documentation.
Example: `{ "action": "help" }`

## Adding a new action

1. Add the action metadata to `ACTION_SPECS` in `actions.rs`.
2. Add any new parameters to the `inputSchema` in `mcp/schemas.rs`.
3. Add a method to `ExampleClient` in `example.rs` (transport).
4. Add a method to `ExampleService` in `app.rs` (business logic).
5. Add a match arm in `dispatch_example()` in `mcp/tools.rs`.
6. Add a test covering parser, schema, and dispatch behavior.
"#;

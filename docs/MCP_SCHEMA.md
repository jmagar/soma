---
title: "MCP Schema Contract"
doc_type: "guide"
status: "active"
owner: "rmcp-template"
audience:
  - "contributors"
  - "agents"
scope: "template"
source_of_truth: true
upstream_refs:
  - "src/actions.rs"
last_reviewed: "2026-05-15"
---

# MCP Schema Contract

Generated from `src/actions.rs` and checked against the schema, README, skill docs, help text, and scope routing.

Run:

```bash
python3 scripts/check-schema-docs.py --write
python3 scripts/check-schema-docs.py --check
```

## Tool

| Field | Value |
|---|---|
| Tool name | `example` |
| Schema resource | `example://schema/mcp-tool` |
| Dispatch parameter | `action` |

## Single-tool pattern

All servers expose **one MCP tool** with an `action` parameter that dispatches to sub-functions:

```rust
// mcp/tools.rs
pub(super) async fn execute_tool(state: &AppState, name: &str, args: Value) -> anyhow::Result<Value> {
    match name {
        "example" => dispatch(state, args).await,
        _ => Err(anyhow::anyhow!("unknown tool: {name}")),
    }
}

async fn dispatch(state: &AppState, args: Value) -> anyhow::Result<Value> {
    let action = string_arg(&args, "action")
        .ok_or_else(|| anyhow::anyhow!("action is required"))?;
    match action.as_str() {
        "greet"   => { ... state.service.greet(name).await }
        "echo"    => { ... state.service.echo(msg).await }
        "status"  => state.service.status().await,
        "help"    => Ok(json!({ "help": HELP_TEXT })),
        other     => Err(anyhow::anyhow!("unknown action: {other}; use action=help")),
    }
}
```

## Actions

| Action | Scope | Description |
|---|---|---|
| `greet` | `example:read` | Return a greeting. Optional `name` string. |
| `echo` | `example:read` | Echo a required `message` string. |
| `status` | `example:read` | Return server status and configuration summary. |
| `elicit_name` | `example:read` | Ask the MCP client to elicit a name and return a personalized greeting. |
| `scaffold_intent` | `example:read` | Elicit scaffold requirements and return JSON for the scaffold-project skill. |
| `help` | public | Return the in-tool action reference. Public; no scope required. |

## Scope enforcement

```rust
// mcp/rmcp_server.rs
const READ_SCOPE:  &str = "example:read";
const WRITE_SCOPE: &str = "example:write";
const DENY_SCOPE:  &str = "example:__deny__";  // sentinel — never granted

fn required_scope_for(action: &str) -> Option<&'static str> {
    required_scope_for_action(action)
}
```

Scopes: `example:read` and `example:write`. Write satisfies read. `help` requires no scope. Unknown actions get `DENY_SCOPE`.

## Schema resource

The tool definition is exposed as an MCP resource at `example://schema/mcp-tool`:

```rust
const SCHEMA_RESOURCE_URI: &str = "example://schema/mcp-tool";

async fn read_resource(uri: &str) -> Result<ResourceContents> {
    if uri == SCHEMA_RESOURCE_URI {
        Ok(ResourceContents::text(
            serde_json::to_string_pretty(&tool_definitions())?,
            uri,
        ))
    } else {
        Err(anyhow::anyhow!("unknown resource: {uri}"))
    }
}
```

## JSON schema

```rust
pub(super) fn tool_definitions() -> Vec<Value> {
    vec![json!({
        "name": "example",
        "description": "Query and manage Example service. Use action=help for documentation.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "action": { "type": "string", "enum": action_names() },
                "name":   { "type": "string", "description": "Optional name (greet)" },
                "message":{ "type": "string", "description": "Required message (echo)" }
            },
            "required": ["action"]
        }
    })]
}
```

## Drift rules

- `ACTION_SPECS` in `src/actions.rs` is the canonical action and scope list.
- `src/mcp/schemas.rs` must derive its enum from `ACTION_SPECS`.
- `help` is intentionally public and must have no required scope.
- `src/mcp/tools.rs`, `README.md`, and `plugins/example/skills/example/SKILL.md` must mention every action.
- `cargo xtask patterns` checks these invariants and fails CI if they drift.

See `docs/PATTERNS.md` §8 and §9 for the full MCP tool dispatch and resource patterns.

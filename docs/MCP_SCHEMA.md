# MCP Schema Contract

Generated from `src/mcp/schemas.rs` and checked against README, skill docs, help text, and scope routing.

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

## Actions

| Action | Scope | Description |
|---|---|---|
| `greet` | `example:read` | Return a greeting. Optional `name` string. |
| `echo` | `example:read` | Echo a required `message` string. |
| `status` | `example:read` | Return server status and configuration summary. |
| `elicit_name` | `example:read` | Ask the MCP client to elicit a name and return a personalized greeting. |
| `help` | public | Return the in-tool action reference. Public; no scope required. |

## Drift Rules

- `EXAMPLE_ACTIONS` in `src/mcp/schemas.rs` is the canonical action list.
- `READ_ONLY_ACTIONS` in `src/mcp/rmcp_server.rs` must include every scoped read action.
- `help` is intentionally public and must not appear in `READ_ONLY_ACTIONS`.
- `src/mcp/tools.rs`, `README.md`, and `plugins/example/skills/example/SKILL.md` must mention every action.

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

## Actions

| Action | Scope | Description |
|---|---|---|
| `greet` | `example:read` | Return a greeting. Optional `name` string. |
| `echo` | `example:read` | Echo a required `message` string. |
| `status` | `example:read` | Return server status and configuration summary. |
| `elicit_name` | `example:read` | Ask the MCP client to elicit a name and return a personalized greeting. |
| `scaffold_intent` | `example:read` | Elicit scaffold requirements and return JSON for the scaffold-project skill. Does not mutate files. |
| `help` | public | Return the in-tool action reference. Public; no scope required. |
| `config_list` | `example:write` | TEMPLATE: document this action. |
| `config_get` | `example:write` | TEMPLATE: document this action. |
| `config_set` | `example:write` | TEMPLATE: document this action. |
| `config_unset` | `example:write` | TEMPLATE: document this action. |
| `config_path` | `example:write` | TEMPLATE: document this action. |

## Drift Rules

- `ACTION_SPECS` in `src/actions.rs` is the canonical action and scope list.
- `src/mcp/schemas.rs` must derive its enum from `ACTION_SPECS`.
- `help` is intentionally public and must have no required scope.
- `src/mcp/tools.rs`, `README.md`, and `plugins/example/skills/example/SKILL.md` must mention every action.

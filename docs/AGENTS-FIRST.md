# Agents-first design

This template is optimized for AI agents as primary operators and consumers.

## Design rules

- Return stable JSON objects with predictable keys.
- Keep responses compact; cap large outputs and summarize by default.
- Include actionable error messages with remediation hints.
- Make all actions discoverable through `action="help"` and `docs/MCP_SCHEMA.md`.
- Prefer semantic test assertions so agents can trust examples.

## Transport surfaces

Agents may use:

1. MCP tool calls through `/mcp` or stdio.
2. CLI commands for local shell workflows.
3. REST `/v1/example` when MCP tooling is unavailable.
4. Plugin skills as human/agent guidance.

The action metadata in `src/actions.rs` keeps these surfaces aligned.

## Documentation contract

When adding an action, update:

- `src/actions.rs`
- `src/app.rs`
- `src/mcp/tools.rs`
- `src/mcp/schemas.rs`
- `src/cli.rs` when not MCP-only
- `tests/tool_dispatch.rs`
- `docs/MCP_SCHEMA.md`
- plugin skill docs

## Security for agents

Never place secrets in skill text, generated docs, or examples. Sensitive plugin settings must be marked `sensitive: true` and passed through environment variables or headers.

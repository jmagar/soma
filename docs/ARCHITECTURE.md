# Architecture

`rmcp-template` is a Rust template for MCP servers built on `rmcp`. The architecture is intentionally layered so transports stay thin and business logic stays testable.

```text
ExampleClient  (src/example.rs)  -> network/upstream calls only
ExampleService (src/app.rs)      -> business logic, validation, transformations
MCP shim       (src/mcp/tools.rs)-> parse JSON args, delegate, return Value
CLI shim       (src/cli.rs)      -> parse argv, delegate, print
REST shim      (src/api.rs)      -> parse HTTP JSON, delegate, return JSON
```

## Core files

| File | Responsibility |
|---|---|
| `src/example.rs` | Upstream/client transport stub. Replace this with your service API client. |
| `src/app.rs` | Service layer. Put business rules here. |
| `src/actions.rs` | Canonical action metadata, action parsing, REST dispatch helpers. |
| `src/mcp/tools.rs` | MCP tool dispatch and elicitation-only actions. |
| `src/mcp/schemas.rs` | Tool input schema generated from action metadata. |
| `src/mcp/rmcp_server.rs` | `ServerHandler`, scope enforcement, tools/resources/prompts. |
| `src/server.rs` | Axum server startup, auth policy resolution, app state. |
| `src/server/routes.rs` | HTTP routes for MCP, health, status, REST API, and web assets. |
| `src/config.rs` | Environment/config loading and safe defaults. |
| `src/main.rs` | Mode dispatch: HTTP server, stdio MCP, CLI, setup commands. |

## Invariants

- Shims do not contain business logic.
- All action metadata starts in `src/actions.rs`.
- Read actions require `example:read`; write actions require `example:write`; `help` is public.
- Stdio is local trusted transport; HTTP is protected unless in loopback or explicit trusted-gateway mode.
- Plugin setup is binary-owned: hook scripts delegate to `example setup plugin-hook`.

See `docs/PATTERNS.md` for the detailed pattern catalog.

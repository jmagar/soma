# Observability

The template exposes fast, redacted status surfaces for humans, agents, and deployment automation.

## HTTP endpoints

| Endpoint | Auth | Purpose |
|---|---|---|
| `/health` | Public | Fast liveness check. Returns minimal status. |
| `/status` | Public | Redacted runtime/config metadata. |
| `/mcp` | Protected unless dev/trusted gateway | MCP Streamable HTTP endpoint. |
| `/v1/example` | Same policy as REST layer | REST action dispatch. |

## MCP status action

`action="status"` returns service-level status from `ExampleService`. It is useful for MCP clients that cannot call HTTP endpoints separately.

## Logging

Use `RUST_LOG` to control tracing:

```bash
RUST_LOG=info,rmcp=warn example serve
```

Prefer structured, actionable logs that include action names, elapsed time, and failure context without leaking secrets.

## Runtime freshness

`just runtime-current` checks whether the running Docker/systemd instance matches the current artifact. Use it after deploys and when debugging stale behavior.

## Agent-first diagnostics

Errors should say what failed, what was expected, and the next command to run. Avoid opaque `internal error` responses when validation can provide a precise message.

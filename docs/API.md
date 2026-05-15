# API

The server exposes HTTP endpoints alongside MCP.

## Endpoints

| Endpoint | Method | Auth | Purpose |
|---|---|---|---|
| `/health` | GET | Public | Fast liveness. |
| `/status` | GET | Public | Redacted runtime status. |
| `/mcp` | POST/stream | Auth policy dependent | Streamable HTTP MCP endpoint. |
| `/v1/example` | POST | Auth policy dependent | REST action dispatch. |

## REST action request

```json
{
  "action": "echo",
  "params": {
    "message": "hello"
  }
}
```

`params` may also be omitted or empty for no-argument actions.

## REST action response

Responses are JSON values produced by `ExampleService` via `src/actions.rs::execute_service_action`.

Examples:

```json
{"status":"ok"}
```

```json
{"echo":"hello"}
```

## MCP-only actions

Some actions require MCP client capabilities and are excluded from REST action lists, such as elicitation-based actions. REST `help` returns both `actions` and `mcp_only_actions` so clients can discover the split.

## Errors

Validation errors return HTTP 400 with an `error` field. Keep errors actionable and avoid leaking secrets.

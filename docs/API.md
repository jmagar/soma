# API

The server exposes HTTP endpoints alongside MCP. All surfaces (MCP, REST, CLI) call the same `ExampleService` methods — no logic is duplicated.

## Endpoints

| Endpoint | Method | Auth | Purpose |
|---|---|---|---|
| `/health` | GET | Public | Fast liveness. Returns minimal status. |
| `/status` | GET | Public | Redacted runtime status. |
| `/mcp` | POST/stream | Auth policy | Streamable HTTP MCP endpoint. |
| `/v1/example` | POST | Auth policy | REST action dispatch. |

## REST action request

The REST API uses the same `action` + `params` pattern as MCP tools:

```json
{
  "action": "echo",
  "params": {
    "message": "hello"
  }
}
```

`params` may be omitted or empty for no-argument actions.

## REST handler

```rust
// src/api.rs
#[derive(Deserialize)]
pub struct ActionRequest {
    pub action: String,
    #[serde(default)]
    pub params: serde_json::Value,
}

async fn api_dispatch(
    State(state): State<AppState>,
    Json(body): Json<ActionRequest>,
) -> impl IntoResponse {
    let result = match body.action.as_str() {
        "greet" => {
            let name = body.params["name"].as_str();
            state.service.greet(name).await
        }
        "echo" => {
            let msg = body.params["message"].as_str().unwrap_or("");
            state.service.echo(msg).await
        }
        "status" => state.service.status().await,
        other => Err(anyhow::anyhow!(
            "unknown action: {other}. POST to /v1/example with action=help"
        )),
    };

    match result {
        Ok(value) => Json(value).into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": e.to_string()})),
        ).into_response(),
    }
}
```

## Surface parity

| Surface | Call pattern |
|---|---|
| MCP | `example(action="greet", name="Alice")` |
| REST | `POST /v1/example {"action":"greet","params":{"name":"Alice"}}` |
| CLI | `example greet --name Alice` |

All three call `state.service.greet(Some("Alice"))`.

## Response shapes

```json
{"status":"ok"}
```

```json
{"echo":"hello"}
```

Responses are JSON values produced by `ExampleService` via `src/actions.rs`.

## MCP-only actions

Some actions require MCP client capabilities and are excluded from REST action lists. Elicitation-based actions require a live MCP client interaction. REST `help` returns both `actions` and `mcp_only_actions` so clients can discover the split.

## Agent-first output rules

- No single response may return more than ~10,000 tokens (~40 KB). Truncate with a clear message.
- List actions MUST support `limit` and `offset` (or `cursor`).
- List actions that return heterogeneous data MUST support `filter` and `state` parameters.
- Every CLI command that outputs data MUST support `--json`.

```rust
const MAX_RESPONSE_BYTES: usize = 40_000; // ~10K tokens

fn truncate_response(text: &str) -> String {
    if text.len() <= MAX_RESPONSE_BYTES {
        return text.to_string();
    }
    let truncated = &text[..MAX_RESPONSE_BYTES];
    format!("{truncated}\n\n[TRUNCATED: response exceeded 10K token limit. Use limit/offset or more specific filters.]")
}
```

## Error messages

Errors must be actionable. Every error must say what failed, the bad value, why it failed, and how to fix it:

```rust
Ok(CallToolResult::error(vec![Content::text(format!(
    "ERROR: {action} failed\n\
     Reason: {reason}\n\
     Hint: {how_to_fix}\n\
     See: action=help for full documentation"
))]))
```

Validation errors return HTTP 400 with an `error` field. Never leak secrets in error text.

Common error shapes:
- Missing required arg: `` "`id` is required for docker_logs — pass id=<container_id>" ``
- Unknown action: `"unknown action: \"florp\" — valid actions: greet, echo, status, help"`
- API unreachable: `"EXAMPLE_URL unreachable: connection refused — is the service running?"`

See `docs/PATTERNS.md` §A2, §39, §40 for the full REST pattern, error structure, and token discipline.

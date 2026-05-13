# rmcp-template — Claude Code instructions

## What this project is

A reusable Rust template for building MCP servers with the rmcp crate. The binary is named `example`. All stub identifiers (`Example*`, `EXAMPLE_*`) are renamed when the template is used for a real service.

## Module map

| File | Role |
|------|------|
| `src/example.rs` | `ExampleClient` — HTTP/API transport stub; one method per remote operation |
| `src/app.rs` | `ExampleService` — business layer; all logic lives here, never in shims |
| `src/mcp/tools.rs` | MCP shim: parse JSON args → call service → return `Value` |
| `src/mcp/schemas.rs` | Tool JSON schema (`EXAMPLE_ACTIONS`, `tool_definitions()`) |
| `src/mcp/rmcp_server.rs` | `ServerHandler` impl: tools, resources, prompts, scope checks |
| `src/mcp/routes.rs` | Axum router: `/mcp`, `/health`, OAuth discovery routes |
| `src/mcp/prompts.rs` | MCP prompts (`quick_start`) |
| `src/mcp.rs` | `AppState`, `AuthPolicy`, `build_auth_layer` |
| `src/config.rs` | `Config`, `ExampleConfig`, `McpConfig`, `AuthConfig`, env loading |
| `src/cli.rs` | CLI shim: parse args → call service → print |
| `src/main.rs` | Mode dispatch: HTTP server / stdio / CLI |
| `src/lib.rs` | Public API + `testing` helpers for integration tests |
| `tests/cli_parse.rs` | CLI argument parsing tests |
| `tests/tool_dispatch.rs` | MCP tool dispatch tests (service-layer, no real credentials) |

## The thin-shim rule — enforce this hard

`src/mcp/tools.rs` and `src/cli.rs` contain **zero business logic**. They only:
1. Parse their input format (JSON args or CLI flags)
2. Call the corresponding `ExampleService` method
3. Return the result

If you find yourself computing, filtering, transforming, or validating data in `tools.rs` or `cli.rs`, stop and move it to `app.rs`.

## How to add an action (4-file checklist)

1. **`src/example.rs`** — add `pub async fn your_action(&self, ...) -> Result<Value>` with the actual HTTP/API call (or stub).

2. **`src/app.rs`** — add a delegating method: `pub async fn your_action(&self, ...) -> Result<Value> { self.client.your_action(...).await }`.

3. **`src/mcp/schemas.rs`** — add `"your_action"` to `EXAMPLE_ACTIONS` and any new parameters to `tool_definitions()`.

4. **`src/mcp/tools.rs`** — add a match arm in `dispatch_example()`: `"your_action" => { ... state.service.your_action(...).await }`. Also add to `HELP_TEXT`.

5. **`src/mcp/rmcp_server.rs`** — add to `READ_ONLY_ACTIONS` (or the appropriate scope list).

6. **`src/cli.rs`** — add a `Command` variant, a parse arm in `parse_args()`, and a dispatch arm in `run()`.

7. **`tests/tool_dispatch.rs`** — add a test.

For actions with parameters, extract them with `string_arg(&args, "param_name")` in `tools.rs`.

## Auth model

`AuthPolicy` is an enum with three states:

| Variant | When | Effect |
|---------|------|--------|
| `AuthPolicy::LoopbackDev` | `no_auth=true` or host starts with `127.` | No auth middleware; scope checks bypassed |
| `AuthPolicy::Mounted { auth_state: None }` | Default non-loopback | Static bearer token required |
| `AuthPolicy::Mounted { auth_state: Some(_) }` | `auth_mode = "oauth"` | Full Google OAuth + RS256 JWT issuance |

Auth is selected in `build_auth_policy()` in `main.rs`. Scopes are `example:read` (read ops) and `example:admin` (satisfies read). `help` requires no scope. Unknown actions get `DENY_SCOPE`.

## Environment variables

| Variable | Default | Description |
|----------|---------|-------------|
| `EXAMPLE_API_URL` | — | Upstream service base URL |
| `EXAMPLE_API_KEY` | — | Upstream service API key |
| `EXAMPLE_MCP_HOST` | `0.0.0.0` | Bind host |
| `EXAMPLE_MCP_PORT` | `3100` | Bind port |
| `EXAMPLE_MCP_NO_AUTH` | `false` | Disable auth (loopback only) |
| `EXAMPLE_MCP_TOKEN` | — | Static bearer token |
| `EXAMPLE_MCP_ALLOWED_HOSTS` | — | Extra comma-separated Host header values |
| `EXAMPLE_MCP_ALLOWED_ORIGINS` | — | Extra comma-separated CORS origins |
| `EXAMPLE_MCP_PUBLIC_URL` | — | Public URL for OAuth metadata endpoints |
| `EXAMPLE_MCP_AUTH_MODE` | `bearer` | `bearer` or `oauth` |
| `EXAMPLE_MCP_GOOGLE_CLIENT_ID` | — | Google OAuth client ID |
| `EXAMPLE_MCP_GOOGLE_CLIENT_SECRET` | — | Google OAuth client secret |
| `EXAMPLE_MCP_AUTH_ADMIN_EMAIL` | — | OAuth admin email |
| `RUST_LOG` | `info` | Log filter |

## Elicitation

The `elicit_name` action demonstrates MCP elicitation (spec 2025-06-18). The server calls `peer.elicit::<T>()` to ask the MCP client for user input mid-call. The type `T` must:
- Derive `JsonSchema`, `Serialize`, `Deserialize`
- Be an object (struct), not a primitive
- Be registered with `rmcp::elicit_safe!(T)`

`ElicitationError::CapabilityNotSupported` is handled gracefully — clients that don't support it get a fallback message instead of an error.

## Build commands

```bash
cargo build --release     # produces target/release/example
cargo test                # all tests
cargo clippy -- -D warnings  # lint (must pass)
cargo fmt                 # format

just dev                  # cargo run -- serve mcp
just test                 # cargo test
just lint                 # cargo clippy -- -D warnings
just fmt                  # cargo fmt
just gen-token            # openssl rand -hex 32
just health               # curl http://localhost:3100/health | jq .
```

## Test helpers

`src/lib.rs` exports `testing::loopback_state()` and `testing::bearer_state(token)` (behind `features = ["test-support"]` or `cfg(test)`). Use these in integration tests — they build `AppState` without real credentials.

## CLI ↔ MCP action parity

Every action in the MCP tool must also be reachable from the CLI, and vice versa.
Both shims call the same `ExampleService` methods, so parity is automatic when the
shims are complete.

**Exception — MCP-only features:** `elicit_name` and MCP resources/prompts have no
CLI equivalent. Elicitation requires a live MCP client interaction (the server asks
the user for input mid-call via `peer.elicit()`); that interaction model does not
translate to a one-shot CLI call. Resources and prompts are MCP protocol concepts
with no CLI analogue.

| Service Method | MCP Action | CLI Command | Notes |
|---|---|---|---|
| `service.greet(name)` | `example(action="greet", name="...")` | `example greet [--name N]` | `name` optional in both |
| `service.echo(message)` | `example(action="echo", message="...")` | `example echo --message <msg>` | `message` required in both |
| `service.status()` | `example(action="status")` | `example status` | |
| _(MCP client interaction)_ | `example(action="elicit_name")` | _(MCP-only — no CLI equivalent)_ | Requires elicitation-capable client |
| _(built-in)_ | `example(action="help")` | `example --help` | MCP returns structured JSON; CLI prints usage |

**TEMPLATE:** Replace this table with your service's actual actions when you adapt
the template. The rule is: one row per service method, with both the MCP action name
and the CLI subcommand/flag documented.

## Common gotchas

- **Stdio mode suppresses logs** — `main.rs` sets log level to `warn` in stdio mode so JSON-RPC is not corrupted by log lines on stdout.
- **`config.toml` is a template file** — it still contains `unraid-mcp` values; update it when adapting this template.
- **Scope checks run in `rmcp_server.rs`**, not in `tools.rs`. `tools.rs` only dispatches.
- **`help` action is public** — `required_scope_for("help")` returns `None`. All other actions require at least `example:read`.
- **Default port is 3100** — set in `default_mcp_port()` in `config.rs`. Override with `EXAMPLE_MCP_PORT`.
- **`elicit_name` is MCP-only** — elicitation requires a live client connection; it cannot be invoked from the CLI. This is the one intentional parity exception.

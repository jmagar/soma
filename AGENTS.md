# rmcp-template — Agent instructions

## What this project is

A Rust template for building MCP servers with the rmcp crate. The stub binary is named `example`. All `Example*` / `EXAMPLE_*` identifiers are renamed when the template is adapted for a real service.

## Key files

| File | Role |
|------|------|
| `src/example.rs` | `ExampleClient` — transport stub; replace with your HTTP/API client |
| `src/app.rs` | `ExampleService` — ALL business logic lives here |
| `src/mcp/tools.rs` | MCP dispatch shim — parse args, call service, return Value |
| `src/mcp/schemas.rs` | Tool JSON schema and action list |
| `src/mcp/rmcp_server.rs` | `ServerHandler` impl: tools, resources, prompts, scope enforcement |
| `src/mcp/routes.rs` | Axum router (`/mcp`, `/health`, OAuth routes) |
| `src/mcp/prompts.rs` | MCP prompts |
| `src/mcp.rs` | `AppState`, `AuthPolicy`, auth layer builder |
| `src/config.rs` | Config structs and env loading |
| `src/cli.rs` | CLI dispatch shim |
| `src/main.rs` | Mode dispatch: HTTP / stdio / CLI |
| `src/lib.rs` | Public API and test helpers |
| `tests/` | Integration tests (`cli_parse.rs`, `tool_dispatch.rs`) |

## Architecture

```
ExampleClient  (example.rs)    ← network calls only
      ↓
ExampleService (app.rs)        ← all business logic
      ↓
  ┌─────────────────────────────┐
  │  MCP shim (mcp/tools.rs)   │  JSON args → service → Value
  │  CLI shim (cli.rs)         │  CLI args  → service → print
  └─────────────────────────────┘
```

## Invariant: zero logic in shims

`mcp/tools.rs` and `cli.rs` must not contain business logic. They parse inputs and delegate to `ExampleService`. All computation, validation, and transformation belongs in `app.rs`.

## How to add an action

1. `src/example.rs` — add transport method returning `Result<Value>`
2. `src/app.rs` — add service method delegating to client
3. `src/mcp/schemas.rs` — add action name to `EXAMPLE_ACTIONS`; add parameters to `tool_definitions()`
4. `src/mcp/tools.rs` — add match arm in `dispatch_example()`; update `HELP_TEXT`
5. `src/mcp/rmcp_server.rs` — add to `READ_ONLY_ACTIONS`
6. `src/cli.rs` — add `Command` variant, parse arm, dispatch arm
7. `tests/tool_dispatch.rs` — add a test

## Auth policy

| State | Condition | Behavior |
|-------|-----------|----------|
| `LoopbackDev` | `no_auth=true` or host starts with `127.` | No auth, no scope checks |
| `Mounted { auth_state: None }` | Default non-loopback | Static bearer token required |
| `Mounted { auth_state: Some(_) }` | `EXAMPLE_MCP_AUTH_MODE=oauth` | Google OAuth + RS256 JWT |

`help` action requires no scope. All other actions require `example:read` (or `example:admin` which satisfies read).

## Environment variables

```
EXAMPLE_API_URL              Upstream service base URL
EXAMPLE_API_KEY              Upstream service API key
EXAMPLE_MCP_HOST             Bind host (default 0.0.0.0)
EXAMPLE_MCP_PORT             Bind port (default 3100)
EXAMPLE_MCP_NO_AUTH          Disable auth — loopback only (1/true/yes)
EXAMPLE_MCP_TOKEN            Static bearer token
EXAMPLE_MCP_ALLOWED_HOSTS    Comma-separated extra Host header values
EXAMPLE_MCP_ALLOWED_ORIGINS  Comma-separated extra CORS origins
EXAMPLE_MCP_PUBLIC_URL       Public URL for OAuth metadata
EXAMPLE_MCP_AUTH_MODE        bearer (default) or oauth
EXAMPLE_MCP_GOOGLE_CLIENT_ID     Google OAuth client ID (OAuth mode)
EXAMPLE_MCP_GOOGLE_CLIENT_SECRET  Google OAuth client secret (OAuth mode)
EXAMPLE_MCP_AUTH_ADMIN_EMAIL  OAuth admin email (OAuth mode)
RUST_LOG                     Log filter (e.g. info,rmcp=warn)
```

## Transports

- `example serve` (or no args) — Streamable HTTP on `EXAMPLE_MCP_PORT` (default 3100)
- `example mcp` — stdio transport for child-process MCP clients
- `example greet / echo / status` — direct CLI

## MCP tool actions

Single tool `example`, dispatched by `action` parameter:

| Action | Scope | Description |
|--------|-------|-------------|
| `greet` | `example:read` | Greeting; optional `name` string |
| `echo` | `example:read` | Echo; required `message` string |
| `status` | `example:read` | Server status |
| `elicit_name` | `example:read` | Elicitation demo — asks user for name mid-call |
| `help` | none (public) | Full action reference |

## MCP features implemented

- **Tools** — `example` tool with action dispatch
- **Resources** — `example://schema/mcp-tool` (JSON schema for the tool)
- **Prompts** — `quick_start` prompt
- **Elicitation** — `elicit_name` action uses `peer.elicit::<NameInput>(...)` (spec 2025-06-18)

## Plugin versioning

Plugin manifests (`.claude-plugin/plugin.json`, `.codex-plugin/plugin.json`, `gemini-extension.json`) do **not** contain a `version` field. The marketplace derives version from the git commit SHA — an explicit version causes every push to create a duplicate entry. Never add `version` to a plugin manifest.

## Build and test

```bash
cargo build --release
cargo test
cargo clippy -- -D warnings
cargo fmt
```

## Test helpers

`rmcp_template::testing::loopback_state()` builds `AppState` with no auth — use in all integration tests. `bearer_state(token)` builds a bearer-only state.

<!-- BEGIN BEADS INTEGRATION v:1 profile:minimal hash:ca08a54f -->
## Beads Issue Tracker

This project uses **bd (beads)** for issue tracking. Run `bd prime` to see full workflow context and commands.

### Quick Reference

```bash
bd ready              # Find available work
bd show <id>          # View issue details
bd update <id> --claim  # Claim work
bd close <id>         # Complete work
```

### Rules

- Use `bd` for ALL task tracking — do NOT use TodoWrite, TaskCreate, or markdown TODO lists
- Run `bd prime` for detailed command reference and session close protocol
- Use `bd remember` for persistent knowledge — do NOT use MEMORY.md files

## Session Completion

**When ending a work session**, you MUST complete ALL steps below. Work is NOT complete until `git push` succeeds.

**MANDATORY WORKFLOW:**

1. **File issues for remaining work** - Create issues for anything that needs follow-up
2. **Run quality gates** (if code changed) - Tests, linters, builds
3. **Update issue status** - Close finished work, update in-progress items
4. **PUSH TO REMOTE** - This is MANDATORY:
   ```bash
   git pull --rebase
   bd dolt push
   git push
   git status  # MUST show "up to date with origin"
   ```
5. **Clean up** - Clear stashes, prune remote branches
6. **Verify** - All changes committed AND pushed
7. **Hand off** - Provide context for next session

**CRITICAL RULES:**
- Work is NOT complete until `git push` succeeds
- NEVER stop before pushing - that leaves work stranded locally
- NEVER say "ready to push when you are" - YOU must push
- If push fails, resolve and retry until it succeeds
<!-- END BEADS INTEGRATION -->

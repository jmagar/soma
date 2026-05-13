# rmcp-template

A reusable Rust template for building MCP servers using the [rmcp](https://crates.io/crates/rmcp) crate. Clone this, rename a handful of identifiers, drop in your API client, and you have a working MCP server with both stdio and Streamable HTTP transports, bearer token or Google OAuth authentication, elicitation support, resources, and prompts.

## What this template gives you

- **Layered architecture** — transport client → service → MCP/CLI shims, enforced by convention
- **Action-based dispatch** — one MCP tool with an `action` parameter routes to any number of operations
- **Both transports** — `example serve` (Streamable HTTP) and `example mcp` (stdio)
- **Both auth modes** — static bearer token or full Google OAuth with RS256 JWT issuance
- **MCP elicitation** — server-asks-user mid-call (spec 2025-06-18), with graceful fallback
- **MCP resources** — exposes the tool schema as a readable resource
- **MCP prompts** — pre-canned `quick_start` prompt for clients that support them
- **CLI** — same service layer, human-readable output, no duplication
- **Test helpers** — `loopback_state()` and `bearer_state()` for tests without real credentials

## Architecture

```
ExampleClient  (src/example.rs)    ← HTTP/GraphQL/gRPC calls to upstream
      ↓
ExampleService (src/app.rs)        ← all business logic lives here
      ↓
  ┌──────────────────────────────────┐
  │  MCP shim (src/mcp/tools.rs)    │  parse JSON args → call service → return Value
  │  CLI shim (src/cli.rs)          │  parse CLI args  → call service → print
  └──────────────────────────────────┘
```

The rule: **zero business logic in `tools.rs` or `cli.rs`**. Both are pure shims. All logic belongs in `app.rs` (or `example.rs` for transport concerns).

## Quickstart — run the stub

```bash
git clone https://github.com/jmagar/rmcp-template
cd rmcp-template
cargo run -- serve          # Streamable HTTP on :3100
# or
cargo run -- mcp            # stdio transport
# or
cargo run -- greet --name Alice
```

Health check:

```bash
curl http://localhost:3100/health
# {"status":"ok"}
```

Call the MCP tool directly:

```bash
curl -s -X POST http://localhost:3100/mcp \
  -H "Content-Type: application/json" \
  -H "Accept: application/json, text/event-stream" \
  -d '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"example","arguments":{"action":"greet","name":"Alice"}}}'
```

## Step-by-step: build your own MCP server from this template

### 1. Clone and rename

```bash
git clone https://github.com/jmagar/rmcp-template myservice-mcp
cd myservice-mcp
```

Find and replace these identifiers across the project:

| Find | Replace with |
|------|-------------|
| `rmcp-template` | `myservice-mcp` (Cargo.toml package name) |
| `example` (binary name) | `myservice` (Cargo.toml `[[bin]] name`) |
| `ExampleClient` | `MyServiceClient` |
| `ExampleService` | `MyServiceService` |
| `ExampleConfig` | `MyServiceConfig` |
| `ExampleRmcpServer` | `MyServiceRmcpServer` |
| `EXAMPLE_API_URL` | `MYSERVICE_API_URL` |
| `EXAMPLE_MCP_*` | `MYSERVICE_MCP_*` |
| `example:read` | `myservice:read` |
| `example://schema/mcp-tool` | `myservice://schema/mcp-tool` |

### 2. Replace ExampleClient with your API client

Edit `src/example.rs`. This is the only file that makes network calls.

```rust
pub struct MyServiceClient {
    client: reqwest::Client,
    base_url: String,
    api_key: String,
}

impl MyServiceClient {
    pub fn new(cfg: &MyServiceConfig) -> Result<Self> {
        if cfg.api_url.is_empty() { anyhow::bail!("MYSERVICE_API_URL is not set"); }
        let client = reqwest::ClientBuilder::new()
            .timeout(std::time::Duration::from_secs(30))
            .build()?;
        Ok(Self { client, base_url: cfg.api_url.clone(), api_key: cfg.api_key.clone() })
    }

    pub async fn get_things(&self) -> Result<Value> {
        let resp = self.client
            .get(format!("{}/things", self.base_url))
            .bearer_auth(&self.api_key)
            .send().await?
            .json::<Value>().await?;
        Ok(resp)
    }
}
```

### 3. Add service methods

Edit `src/app.rs`. Delegate to the client; add caching, retries, or transformation here:

```rust
pub async fn get_things(&self) -> Result<Value> {
    self.client.get_things().await
}
```

### 4. Add tool actions

For each new action:

**a. `src/mcp/schemas.rs`** — add to `EXAMPLE_ACTIONS`:

```rust
pub(super) const EXAMPLE_ACTIONS: &[&str] = &["greet", "echo", "status", "get_things", "help"];
```

Add any new parameters to `tool_definitions()`.

**b. `src/mcp/tools.rs`** — add a match arm in `dispatch_example()`:

```rust
"get_things" => state.service.get_things().await,
```

Also add the action to `READ_ONLY_ACTIONS` in `src/mcp/rmcp_server.rs`.

**c. `src/cli.rs`** — add a `Command` variant and dispatch arm:

```rust
pub enum Command { ..., GetThings }

// in parse_args():
"get-things" => Some(Command::GetThings),

// in run():
Command::GetThings => service.get_things().await?,
```

**d. Add a test** in `tests/tool_dispatch.rs`.

### 5. Update config

Edit `src/config.rs` to rename `ExampleConfig` fields and env var names. Edit `config.toml` and `.env.example`.

## Command modes

```
example [serve]          Start Streamable HTTP MCP server (default)
example mcp              Start stdio MCP transport
example greet [--name]   CLI: greet
example echo --message   CLI: echo
example status           CLI: server status
example --help           Usage
example --version        Version
```

## MCP tool actions

The single `example` tool dispatches on the `action` parameter:

| Action | Description | Parameters |
|--------|-------------|------------|
| `greet` | Return a greeting | `name` (optional string) |
| `echo` | Echo a message back | `message` (required string) |
| `status` | Server status info | none |
| `elicit_name` | Ask user for name via elicitation, return greeting | none |
| `help` | Full action reference | none |

## Authentication

### Bearer token (default)

Set `EXAMPLE_MCP_TOKEN`. All `/mcp` requests must include `Authorization: Bearer <token>`.

### No auth (loopback only)

Set `EXAMPLE_MCP_NO_AUTH=true` or bind to `127.*`. Only legal for local development.

### OAuth (Google)

Set `EXAMPLE_MCP_AUTH_MODE=oauth` and the OAuth env vars below. The server issues RS256 JWTs after Google authentication. OAuth and bearer can coexist (OAuth mode disables the static token by default; set `disable_static_token_with_oauth = false` to keep both active).

`/health` is always unauthenticated.

## Environment variables

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `EXAMPLE_API_URL` | no | — | Upstream service base URL |
| `EXAMPLE_API_KEY` | no | — | Upstream service API key |
| `EXAMPLE_MCP_HOST` | no | `0.0.0.0` | Bind host |
| `EXAMPLE_MCP_PORT` | no | `3100` | Bind port |
| `EXAMPLE_MCP_NO_AUTH` | no | `false` | Disable auth (loopback only; 1/true/yes) |
| `EXAMPLE_MCP_TOKEN` | no | — | Static bearer token for `/mcp` |
| `EXAMPLE_MCP_ALLOWED_HOSTS` | no | — | Extra comma-separated Host header values |
| `EXAMPLE_MCP_ALLOWED_ORIGINS` | no | — | Extra comma-separated CORS origins |
| `EXAMPLE_MCP_PUBLIC_URL` | OAuth | — | Public URL (e.g. `https://myservice.example.com`) |
| `EXAMPLE_MCP_AUTH_MODE` | no | `bearer` | `bearer` or `oauth` |
| `EXAMPLE_MCP_GOOGLE_CLIENT_ID` | OAuth | — | Google OAuth client ID |
| `EXAMPLE_MCP_GOOGLE_CLIENT_SECRET` | OAuth | — | Google OAuth client secret |
| `EXAMPLE_MCP_AUTH_ADMIN_EMAIL` | OAuth | — | Admin email address |
| `RUST_LOG` | no | `info` | Log filter (e.g. `info,rmcp=warn`) |

## Development commands

```bash
cargo build           # debug build
cargo build --release # release build
cargo test            # run tests
cargo clippy -- -D warnings  # lint
cargo fmt             # format

just dev              # cargo run -- serve mcp
just test             # cargo test
just lint             # cargo clippy -- -D warnings
just fmt              # cargo fmt
just build            # cargo build
just release          # cargo build --release
just gen-token        # openssl rand -hex 32
just health           # curl http://localhost:3100/health | jq .
```

## MCP client configuration

### Streamable HTTP (Claude.app, mcpx, etc.)

```json
{
  "mcpServers": {
    "example": {
      "url": "http://localhost:3000/mcp",
      "headers": { "Authorization": "Bearer YOUR_TOKEN" }
    }
  }
}
```

### stdio (Claude Desktop, local clients)

```json
{
  "mcpServers": {
    "example": {
      "command": "/path/to/example",
      "args": ["mcp"],
      "env": { "RUST_LOG": "warn" }
    }
  }
}
```

## Using this template

This checklist covers everything you need to adapt rmcp-template for a real service. Work through it top-to-bottom; each step is independent.

### Checklist

#### Core: rename and implement

1. **Replace all occurrences of `example`/`Example`/`EXAMPLE` with your service name**

   Global search-replace across the entire project:

   | Find | Replace with |
   |------|-------------|
   | `rmcp-template` | `myservice-mcp` (Cargo.toml package name) |
   | `example` (binary name) | `myservice` (Cargo.toml `[[bin]] name`) |
   | `ExampleClient` | `MyServiceClient` |
   | `ExampleService` | `MyServiceService` |
   | `ExampleConfig` | `MyServiceConfig` |
   | `ExampleRmcpServer` | `MyServiceRmcpServer` |
   | `EXAMPLE_API_URL` | `MYSERVICE_API_URL` |
   | `EXAMPLE_MCP_*` | `MYSERVICE_MCP_*` |
   | `EXAMPLE_NOAUTH` | `MYSERVICE_NOAUTH` |
   | `example:read` | `myservice:read` |
   | `example://schema/mcp-tool` | `myservice://schema/mcp-tool` |
   | `.example` (data dir) | `.myservice` (in `config.rs` and `docker-compose.yml`) |

2. **Implement your API client in `src/example.rs`**

   Replace the stub methods with real HTTP/GraphQL/gRPC calls. See the inline comments for the `reqwest::Client` pattern.

3. **Add service methods to `src/app.rs`**

   Each public method on `ExampleService` corresponds to one MCP action. Business logic, caching, and retries go here — not in `tools.rs`.

4. **Add MCP actions to `src/mcp/tools.rs` and `src/mcp/schemas.rs`**

   - `schemas.rs`: add action names to `EXAMPLE_ACTIONS` slice
   - `tools.rs`: add match arms in `dispatch_example()`

5. **Add CLI commands to `src/cli.rs`**

   One `Command` enum variant and one `fmt_*` formatter per action. Keep CLI output human-readable; the MCP layer handles machine-readable JSON.

6. **Update `src/config.rs`** with service-specific config fields

   Rename `ExampleConfig` and add any fields your service needs. Update env prefixes throughout.

7. **Add required env vars to `check-env` in `xtask/src/main.rs`**

   Uncomment the `REQUIRED_VARS` entries (or add your own) so `cargo xtask check-env` catches missing credentials.

#### Docker and deployment

8. **Update `config/Dockerfile` binary name, port, and cache IDs**

   Replace every occurrence of `example` (binary copy, cache IDs, CMD, LABEL) with your binary name. Update `EXPOSE` to your port.

9. **Update `docker-compose.yml`**

   - Change `3000` to your service's port (must match `config.toml [mcp] port`)
   - The `${HOME}/.example:/data` volume is already set; rename `.example` to your service

10. **Update `entrypoint.sh`**

    Uncomment the `REQUIRED_VARS` check block and add your service's required env vars. Replace `EXAMPLE_API_KEY` references with your prefix.

11. **Update `config/Dockerfile` to use `entrypoint.sh`**

    Already wired in the template (ENTRYPOINT + CMD split). Verify the gosu/su-exec choice matches your base image.

#### Infrastructure

12. **Configure Git LFS** (if not already done)

    ```bash
    git lfs install    # one-time per machine
    ```

    `.gitattributes` already tracks `bin/*`, `*.tar.gz`, `*.zip`. Distribute your binary via `cargo xtask dist`.

13. **Run `just symlink-docs`** after any new CLAUDE.md

    Creates `AGENTS.md` + `GEMINI.md` symlinks next to every `CLAUDE.md` in the repo.

14. **Update GitHub workflow files** (`.github/workflows/`)

    In all three workflows, replace:
    - `rmcp-template` → your repo name (cache keys)
    - `example-mcp` → your Docker image name
    - `example` → your binary name
    - `jmagar` → your GitHub org/username (image registry path)

15. **Update `.env.example`** with your service's actual variable names and descriptions

16. **Update `config.example.toml`** with your service's actual config fields

#### Plugin and skills

17. **Update plugin.json userConfig for your service's credentials**

    Edit `plugins/example/.claude-plugin/plugin.json`. Replace the `example_api_url` / `example_api_key` fields with your service's actual credential names and descriptions.

18. **Update `plugins/example/hooks/plugin-setup.sh`**

    Replace `EXAMPLE_*` env var names, `example-mcp` service references, and add any service-specific credentials your binary needs.

19. **Update `plugins/example/skills/example/SKILL.md`**

    Replace the action table with your actual actions and documented response shapes. Good skill docs drive better AI tool use.

20. **Update `plugins/example/.codex-plugin/plugin.json`** for Codex plugin registry

    Every field marked `TEMPLATE:` must be replaced. Key fields:
    - `name` — `<your-service>-mcp`
    - `interface.displayName` — human-readable name
    - `interface.shortDescription` — 50-char tagline
    - `interface.capabilities` — `["Read"]` or `["Read", "Write"]` based on your server
    - `interface.defaultPrompt` — 3 sample prompts demonstrating your actions
    - `interface.brandColor` — hex color matching your service's brand

    See `plugins/example/.codex-plugin/README.md` for the full field reference.

21. **Write `server.json`** for MCP registry publishing

    Update every `TEMPLATE:` field in `server.json` at the repo root:
    - `name` — your reverse-DNS namespace (e.g. `yourdomain.com/myservice-mcp`)
    - `description` — one-sentence description
    - `repository.url` — your GitHub repo URL
    - `packages[0].identifier` — your OCI image ref
    - `environmentVariables` — your service's actual env vars

    See `docs/server-json-guide.md` for step-by-step publishing instructions.

#### Tests

22. **Update `tests/mcporter/test-tools.sh`**

    Add semantic checks for your actions. Validate actual field values, not just key existence.

21. **Run all checks**

    ```bash
    cargo check               # must compile clean
    cargo nextest run         # all tests pass
    taplo check               # TOML format valid
    cargo xtask check-env     # required env vars set
    ```

### After renaming

```bash
# Verify it compiles
cargo check

# Run tests with nextest
cargo nextest run

# Check environment variables
cargo xtask check-env

# Start the server in dev mode
just dev       # no-auth mode on :3000

# Symlink docs for all AI systems
just symlink-docs

# In another terminal, run integration tests
just test-mcporter
```

## License

MIT

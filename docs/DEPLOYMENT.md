# Deployment

This template supports three deployment modes:

1. **Local development** with `just dev`.
2. **Docker Compose** with `just docker-up`.
3. **User systemd** with an installed release binary.

## Binary command surface

Every server binary exposes exactly two server modes and a CLI:

| Command | Mode | Description |
|---|---|---|
| `example mcp` | stdio MCP | For Claude Code `~/.claude/settings.json` stdio servers |
| `example serve` | Streamable HTTP MCP | For Docker/remote deployment |
| `example [subcommand]` | CLI | Direct API access; all subcommands support `--json` |
| `example doctor` | Pre-flight check | Validates environment and config |
| `example --help` | Help | Print usage |
| `example --version` | Version | Print version |

## Deployment checklist

1. Build and test locally:
   ```bash
   just verify
   scripts/pre-release-check.sh
   ```
2. Create a `.env` from `.env.example` and set real credentials.
3. Generate a bearer token:
   ```bash
   just gen-token
   ```
4. Choose Docker or systemd.
5. Verify runtime freshness:
   ```bash
   just runtime-current
   ```
6. Smoke-test auth:
   ```bash
   EXAMPLE_MCP_TOKEN=<token> just auth-smoke
   ```
7. Run MCP integration tests:
   ```bash
   just test-mcporter
   ```

## Binary environment awareness

The binary normalizes paths, bind hosts, and ports based on its deployment context:

```rust
fn is_containerized() -> bool {
    std::path::Path::new("/.dockerenv").exists()
        || std::env::var("RUNNING_IN_CONTAINER").is_ok()
        || std::env::var("container").is_ok()
}

fn resolve_data_dir(config_path: Option<&str>) -> PathBuf {
    if let Some(p) = config_path { return PathBuf::from(p); }
    if is_containerized() { return PathBuf::from("/data"); }
    dirs::home_dir().unwrap_or_default().join(".example")
}

fn resolve_bind_host(configured: &str) -> &str {
    if is_containerized() { "0.0.0.0" } else { configured }
}
```

## Appdata convention

All deployments share `~/.<service>` as the logical data root:

| Deployment | Data directory |
|---|---|
| Local binary | `~/.example/` |
| Docker | `/data/` in container, mounted from `~/.example/` on host |
| Plugin | `$CLAUDE_PLUGIN_DATA` (symlinked to `~/.example/`) |

## Auth expectations

Non-loopback HTTP deployments must use bearer auth or OAuth. The server refuses to bind to a non-loopback address without authentication unless explicitly configured:

- Loopback bind or `EXAMPLE_MCP_NO_AUTH=true` → `LoopbackDev` (no auth)
- Non-loopback + bearer token → mounted bearer auth
- Non-loopback + `auth_mode=oauth` → mounted OAuth auth
- Non-loopback + `EXAMPLE_NOAUTH=true` → `TrustedGatewayUnscoped` (trusted gateway, explicit opt-out)
- Non-loopback + no credentials + no gateway acknowledgment → startup error

## Claude Code stdio config

```json
{
  "mcpServers": {
    "example": {
      "type": "stdio",
      "command": "example",
      "args": ["mcp"]
    }
  }
}
```

The binary must be in `$PATH`. The plugin's `plugin-setup.sh` symlinks it to `~/.local/bin/` on SessionStart.

## Public endpoints

- `/health` is public and fast.
- `/status` is public but redacted.
- `/mcp` is the Streamable HTTP MCP endpoint.
- `/v1/example` is the REST action endpoint.

See `docs/DOCKER.md`, `docs/SYSTEMD.md`, `docs/ENV.md`, and `docs/CONFIG.md` for deployment-specific details. See `docs/PATTERNS.md` §27, §28, §46, §47 for security, environment awareness, and binary installation patterns.

---
title: "Configuration"
doc_type: "guide"
status: "active"
owner: "rmcp-template"
audience:
  - "contributors"
  - "agents"
scope: "template"
source_of_truth: false
upstream_refs:
  - "docs/PATTERNS.md"
last_reviewed: "2026-05-15"
---

# Configuration

Configuration is split between non-secret settings (`config.toml`) and secrets (`  .env`). Env vars always override `config.toml`.

## Files

| File | Purpose |
|---|---|
| `.env.example` | Documented environment variable template. Safe to commit. |
| `.env` | Local secrets and deployment settings. Never commit. |
| `config.example.toml` | Optional structured config example for derived services. |
| `src/config.rs` | Loads env/config into typed Rust structs. |

## What goes where

| Goes in `.env` | Goes in `config.toml` |
|---|---|
| API keys, tokens, passwords | bind host, port, server_name |
| Service URLs | TLS skip, site, tailnet |
| Google OAuth credentials | auth_mode, auth TTLs |
| MCP bearer token | allowed_hosts, allowed_origins |
| Docker runtime vars (PUID, PGID) | retention settings, batch sizes |
| RUST_LOG | resource limits |

## config.toml structure

```toml
# config.toml — non-secret settings only
# Env vars override everything here.

[service]
skip_tls_verify = false
site = "default"

[mcp]
host = "0.0.0.0"
port = 3000
server_name = "example-mcp"

[mcp.auth]
mode = "bearer"           # or "oauth"
admin_email = ""
sqlite_path = "/data/auth.db"
key_path = "/data/auth-jwt.pem"
access_token_ttl_secs = 3600
refresh_token_ttl_secs = 2592000
auth_code_ttl_secs = 300
```

## .env structure

```bash
# .env — secrets and URLs ONLY
EXAMPLE_API_URL=https://example.internal/api
EXAMPLE_API_KEY=your_api_key_here

# MCP auth
EXAMPLE_MCP_TOKEN=your_bearer_token_here

# OAuth (only when auth_mode=oauth in config.toml)
# EXAMPLE_MCP_GOOGLE_CLIENT_ID=...
# EXAMPLE_MCP_GOOGLE_CLIENT_SECRET=...

# Docker runtime
PUID=1000
PGID=1000
DOCKER_NETWORK=jakenet
RUST_LOG=info
```

## Config loading pattern

```rust
impl Config {
    pub fn load() -> anyhow::Result<Self> {
        let mut config = Config::default();

        // 1. Load config.toml (non-secret settings)
        match std::fs::read_to_string("config.toml") {
            Ok(contents) => { config = toml::from_str(&contents)?; }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(anyhow::anyhow!("config.toml: {e}")),
        }

        // 2. Env overrides (secrets + any setting the user wants to override)
        env_str("EXAMPLE_MCP_HOST", &mut config.mcp.host);
        env_parse("EXAMPLE_MCP_PORT", &mut config.mcp.port)?;
        env_opt_str("EXAMPLE_MCP_TOKEN", &mut config.mcp.api_token);
        env_str("EXAMPLE_API_URL", &mut config.example.url);
        env_str("EXAMPLE_API_KEY", &mut config.example.api_key);
        Ok(config)
    }
}
```

## Auth policy summary

| Situation | Policy |
|---|---|
| Stdio transport | `LoopbackDev` |
| Loopback bind or `EXAMPLE_MCP_NO_AUTH=true` | `LoopbackDev` |
| Non-loopback with bearer token | `Mounted { auth_state: None }` |
| OAuth mode (`auth_mode=oauth`) | `Mounted { auth_state: Some(_) }` |
| Explicit trusted gateway (`EXAMPLE_NOAUTH=true`) | `TrustedGatewayUnscoped` |

Non-loopback no-auth should only be used when an upstream gateway enforces authorization.

```rust
pub enum AuthPolicy {
    /// No auth — only legal when bound to loopback (127.x).
    LoopbackDev,
    /// Auth active. auth_state=Some → OAuth+JWKS; auth_state=None → bearer-only.
    Mounted { auth_state: Option<Arc<lab_auth::state::AuthState>> },
}
```

## Defaults

- Host defaults to `0.0.0.0` for HTTP serving.
- Port defaults to `3100` in config (some family services use `40060`).
- Appdata defaults to `~/.<service>` locally, `/data` in Docker.

## Validation

```bash
just doctor
cargo xtask check-env
scripts/check-version-sync.sh
```

See `docs/ENV.md` for variable-by-variable reference. See `docs/PATTERNS.md` §4 and §5 for the full config split and auth policy patterns.

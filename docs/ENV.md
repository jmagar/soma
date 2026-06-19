---
title: "Environment Variables"
doc_type: "guide"
status: "active"
owner: "rmcp-template"
audience:
  - "contributors"
  - "agents"
scope: "template"
source_of_truth: false
upstream_refs:
  - "crates/rtemplate-contracts/src/config.rs"
last_reviewed: "2026-05-15"
---

# Environment variables

The template uses `RTEMPLATE_*` variables. Rename the prefix when adapting the template.

## API target

| Variable | Purpose |
|---|---|
| `RTEMPLATE_API_URL` | Deployed platform API or upstream API base URL used by `ExampleClient`. Empty selects offline template stub mode. |
| `RTEMPLATE_API_KEY` | Bearer token or upstream API key. Keep secret. Required when the deployed API requires auth. |

## MCP HTTP server

| Variable | Default | Purpose |
|---|---:|---|
| `RTEMPLATE_MCP_HOST` | `127.0.0.1` | Bind host for HTTP transport. Set `0.0.0.0` only with bearer, OAuth, or trusted-gateway auth configured. |
| `RTEMPLATE_MCP_PORT` | `40060` | Bind port for HTTP transport. |
| `RTEMPLATE_MCP_NO_AUTH` | `false` | Disable local auth for loopback development only. |
| `RTEMPLATE_NOAUTH` | `false` | Trusted-gateway no-auth mode for non-loopback deployments. |
| `RTEMPLATE_MCP_TOKEN` | unset | Static bearer token. Required for bearer-only mounted HTTP. |
| `RTEMPLATE_MCP_ALLOWED_HOSTS` | unset | Extra accepted Host header values (comma-separated). |
| `RTEMPLATE_MCP_ALLOWED_ORIGINS` | unset | Extra CORS origins (comma-separated). |
| `RTEMPLATE_MCP_PUBLIC_URL` | unset | Public URL used for OAuth metadata endpoints. |
| `RTEMPLATE_MCP_AUTH_MODE` | `bearer` | `bearer` or `oauth`. |

## OAuth mode

Only required when `RTEMPLATE_MCP_AUTH_MODE=oauth`:

| Variable | Purpose |
|---|---|
| `RTEMPLATE_MCP_GOOGLE_CLIENT_ID` | Google OAuth client ID. |
| `RTEMPLATE_MCP_GOOGLE_CLIENT_SECRET` | Google OAuth client secret. |
| `RTEMPLATE_MCP_AUTH_ADMIN_EMAIL` | Initial/admin email allowed by the OAuth flow. |
| `RTEMPLATE_MCP_AUTH_SQLITE_PATH` | OAuth session/client database path. Defaults to `/data/auth.db`. |
| `RTEMPLATE_MCP_AUTH_KEY_PATH` | RS256 signing key path. Defaults to `/data/auth-jwt.pem`. |

## Docker runtime

| Variable | Purpose |
|---|---|
| `PUID` | UID to run the container as (default: 1000). |
| `PGID` | GID to run the container as (default: 1000). |
| `DOCKER_NETWORK` | Docker network name (default: `mcp`). |
| `VERSION` | Image tag to pull (default: `latest`). |

## Logging

| Variable | Example | Purpose |
|---|---|---|
| `RUST_LOG` | `info,rmcp=warn` | Tracing filter. |
| `NO_COLOR` | `1` | Disable ANSI color in console logs. |
| `FORCE_COLOR` | `1` | Force ANSI color even when stderr is not a TTY. |

## `.env` file structure

```bash
# .env — secrets and URLs ONLY
RTEMPLATE_API_URL=https://example.internal/api
RTEMPLATE_API_KEY=your_api_key_here

# MCP auth
RTEMPLATE_MCP_TOKEN=your_bearer_token_here

# OAuth (only when auth_mode=oauth in config.toml)
# RTEMPLATE_MCP_GOOGLE_CLIENT_ID=...
# RTEMPLATE_MCP_GOOGLE_CLIENT_SECRET=...

# Docker runtime
PUID=1000
PGID=1000
DOCKER_NETWORK=mcp
RUST_LOG=info
```

## Safety

`.env` and `.env.*` are ignored by `.gitignore` and blocked by `scripts/block-env-commits.sh`. Only `.env.example` belongs in git.

Non-secret settings (host, port, auth mode, TTLs) go in `config.toml`, not `.env`. See `docs/CONFIG.md` for the full split.

Generate a bearer token:

```bash
just gen-token
# or: openssl rand -hex 32
```

See `docs/CONFIG.md` for the config loading pattern and auth policy details.

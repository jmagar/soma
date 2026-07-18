# Authentication

This server supports two authentication mechanisms simultaneously: **static bearer tokens** and **OAuth 2.0**. They serve different audiences and can be active at the same time.

---

## Why two mechanisms?

**Bearer tokens** are for agents and automation. An agent sets `Authorization: Bearer <token>` and makes calls. No browser, no redirect flow, no session cookie — just a shared secret. Tokens are fast to issue (`just gen-token`) and easy to rotate.

**OAuth** is for humans. It runs a full browser-based Google OAuth flow, issues short-lived JWTs, and maintains refresh tokens. This is the right choice when a human user needs to grant access through a UI without ever seeing a raw token.

When both are configured, each request is accepted if it satisfies either mechanism. A human signs in via OAuth; an agent uses a token. They share the same server.

---

## Scopes

All non-trivial actions require at least `soma:read`. Mutating actions require `soma:write`, which also satisfies read checks. The `help` action is always public.

Static bearer tokens default to `soma:read` only. OAuth tokens carry whatever scopes the OAuth flow issued.

---

## Configuring bearer token auth

```bash
# Generate a token
export SOMA_MCP_TOKEN=$(openssl rand -hex 32)

# Or: just gen-token
```

Set `SOMA_MCP_TOKEN` in your environment or `.env` file. Clients authenticate with:

```
Authorization: Bearer <token>
```

That's all. The server validates the header on every mounted MCP and REST route,
including `/mcp`, direct native `/v1/*` business routes, and provider-backed
dynamic `/v1/*` routes.

---

## Configuring OAuth

Set the following environment variables:

```bash
SOMA_MCP_AUTH_MODE=oauth
SOMA_MCP_PUBLIC_URL=https://your-server.example.com   # public URL for OAuth callbacks
SOMA_MCP_GOOGLE_CLIENT_ID=...
SOMA_MCP_GOOGLE_CLIENT_SECRET=...
SOMA_MCP_AUTH_ADMIN_EMAIL=you@example.com
```

The server exposes standard OAuth discovery endpoints under `/mcp/.well-known/` that MCP clients can use for dynamic registration. Session cookies are disabled — all auth is via `Authorization` headers.

OAuth and bearer token can coexist: set both `SOMA_MCP_TOKEN` and the OAuth variables. When `SOMA_MCP_TOKEN` is unset, OAuth mode accepts only OAuth-issued bearer JWTs.

---

## Multi-provider OAuth (crate capability)

`soma-auth` (`crates/shared/auth`) supports more than one upstream OAuth/OIDC
identity provider at once: Google, Authelia (a real OIDC Provider with a
configurable issuer), and GitHub (plain OAuth2, no ID token). A consuming
deployment enables any subset simultaneously via `AuthConfig.google` /
`.authelia` / `.github` and `AuthConfig.default_provider`. When 2+ providers
are configured, `GET /auth/login` renders a plain HTML picker unless the
request already specifies `?provider=`, and `GET /authorize` accepts the same
optional `?provider=` query parameter for headless MCP clients.

The `soma` binary's own CLI/config/setup-wizard/doctor surface does not yet
expose Authelia/GitHub configuration — that wiring is a separate, dependent
change. This section documents the underlying crate capability so downstream
consumers of `soma-auth` (any Rust MCP server in this family) can wire it in
directly today.

**Security trade-off — read before enabling 2+ providers.** The email
allowlist (`admin_email` plus the `allowed_users` SQLite table) is a single
flat list shared across *every* configured provider. Being on the allowlist
grants full admin scope regardless of which provider authenticated the user
— this is pre-existing, unchanged behavior. The consequence of enabling more
than one provider is that the deployment's effective admin-gate strength
becomes that of its *weakest* configured provider's identity-verification
signal:

- Google and Authelia both re-verify `email_verified` on every login, live,
  via the signed ID token returned in that login's token exchange.
- GitHub has no ID token. Its `email_verified` signal is derived from the
  `primary && verified` flag on a `GET /user/emails` entry — a point-in-time
  claim that GitHub does not re-check on every subsequent login.

Full per-provider allowlist scoping (a schema change to `allowed_users`) was
considered and rejected as disproportionate for this crate's actual
deployment shape (single-operator homelab/small-fleet, not multi-tenant
SaaS). Instead, `AuthState::new` logs a `tracing::warn!` at startup whenever
2+ providers are configured — that log line is the visible signal operators
should watch for, not a silent trade-off. (The check is purely
`providers.len() > 1`, with no allowlist-emptiness condition — the allowlist
can never actually be empty in OAuth mode anyway, since `admin_email` is
required by `AuthConfig::validate`.)

Practical guidance: if you need strict per-identity isolation between
providers, run separate deployments (separate `soma-auth` SQLite databases)
per provider instead of enabling several providers with one shared allowlist
in a single deployment.

Subject identifiers are namespaced per provider (`{provider_id}:{raw_subject}`,
e.g. `github:9182310`) to avoid collisions across providers sharing one
database — except Google, whose subject format is left bare for backward
compatibility with already-issued sessions and refresh tokens.

---

## The startup guard

**The HTTP server will refuse to start if it is binding to a non-loopback address with no authentication configured.**

This is enforced by `server::resolve_auth_policy_kind()`. The exact error:

```
Refusing to bind MCP server to 0.0.0.0 without authentication.

Choose one of:
1. Bind to loopback:    SOMA_MCP_HOST=127.0.0.1
2. Set a bearer token:  SOMA_MCP_TOKEN=$(openssl rand -hex 32)
3. Enable OAuth:        SOMA_MCP_AUTH_MODE=oauth (+ OAuth credentials)
4. Disable auth:        SOMA_MCP_HOST=127.0.0.1 SOMA_MCP_NO_AUTH=true
5. Upstream gateway:    SOMA_NOAUTH=true  (if a proxy handles auth)
```

The guard passes when any of the following is true:

| Condition | Variable | Notes |
|---|---|---|
| Loopback bind | `SOMA_MCP_HOST=127.0.0.1` | Trust boundary is the network address |
| Bearer token set | `SOMA_MCP_TOKEN=<token>` | Auth middleware enforces it |
| OAuth enabled | `SOMA_MCP_AUTH_MODE=oauth` | Auth middleware enforces it |
| Auth disabled | `SOMA_MCP_HOST=127.0.0.1` + `SOMA_MCP_NO_AUTH=true` | Local dev — see below |
| Gateway override | `SOMA_NOAUTH=true` | Upstream handles auth — see below |

---

## Local development (no auth)

For local development, disable auth entirely:

```bash
just dev
# equivalent to: SOMA_MCP_HOST=127.0.0.1 SOMA_MCP_NO_AUTH=true cargo run --bin soma -- serve
```

`SOMA_MCP_NO_AUTH=true` is accepted only on a loopback bind. It sets the auth policy to `LoopbackDev`, removes the auth middleware, and requires no token for local calls.

**Do not use this in production.**

---

## Upstream gateway / MCP proxy (no server-level auth)

If you deploy behind a gateway that handles authentication for all services (e.g. an MCP proxy that validates tokens before routing to this server), you can disable auth at the server level:

```bash
SOMA_NOAUTH=true         # acknowledge the startup guard that an upstream gateway handles auth
```

`SOMA_NOAUTH=true` selects the explicit `TrustedGatewayUnscoped` policy. It removes the local auth middleware and scope checks, so only use it when a trusted upstream gateway enforces both authentication and authorization before traffic reaches this server.

---

## Stdio transport

The stdio transport (`soma mcp`) bypasses all HTTP auth entirely. It is always `LoopbackDev` — the trust boundary is the OS pipe between parent and child process. Scope checks are not enforced in stdio mode. This matches the MCP spec: stdio servers are local, trusted, subprocess connections.

---

## Auth policy reference

The `AuthPolicy` enum in `crates/soma/runtime/src/server.rs` controls what the router does:

| Policy | When | Auth enforced? | Scope checks? |
|---|---|---|---|
| `LoopbackDev` | Loopback bind, or stdio mode. `SOMA_MCP_NO_AUTH=true` also enables this policy for loopback development. | No | No |
| `TrustedGatewayUnscoped` | Non-loopback no-auth deployment with `SOMA_NOAUTH=true` | No | No |
| `Mounted { auth_state: None }` | Bearer-only mode | Yes (token) | Yes |
| `Mounted { auth_state: Some(_) }` | OAuth mode (+ optional token) | Yes (OAuth / token) | Yes |

Public endpoints (`/health`, `/status`) are never gated by auth, regardless of policy. `/status` returns only local redacted runtime metadata.

---

## CUSTOMIZE

When you adapt Soma, replace all `SOMA_` prefixes with your service's prefix
throughout `crates/soma/config/src/config.rs`,
`apps/soma/src/bin/soma.rs`, and this document.

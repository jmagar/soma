# Environment variables

The template uses `EXAMPLE_*` variables. Rename the prefix when adapting the template.

## Upstream service

| Variable | Purpose |
|---|---|
| `EXAMPLE_API_URL` | Upstream API base URL used by `ExampleClient`. |
| `EXAMPLE_API_KEY` | Upstream API key or token. Keep secret. |

## MCP HTTP server

| Variable | Default | Purpose |
|---|---:|---|
| `EXAMPLE_MCP_HOST` | `0.0.0.0` | Bind host for HTTP transport. |
| `EXAMPLE_MCP_PORT` | `3100` | Bind port for HTTP transport. |
| `EXAMPLE_MCP_NO_AUTH` | false | Disable local auth for loopback development only. |
| `EXAMPLE_NOAUTH` | false | Trusted-gateway no-auth mode for non-loopback deployments. |
| `EXAMPLE_MCP_TOKEN` | unset | Static bearer token. Required for bearer-only mounted HTTP. |
| `EXAMPLE_MCP_ALLOWED_HOSTS` | unset | Extra accepted Host header values. |
| `EXAMPLE_MCP_ALLOWED_ORIGINS` | unset | Extra CORS origins. |
| `EXAMPLE_MCP_PUBLIC_URL` | unset | Public URL used for OAuth metadata. |
| `EXAMPLE_MCP_AUTH_MODE` | `bearer` | `bearer` or `oauth`. |

## OAuth mode

| Variable | Purpose |
|---|---|
| `EXAMPLE_MCP_GOOGLE_CLIENT_ID` | Google OAuth client ID. |
| `EXAMPLE_MCP_GOOGLE_CLIENT_SECRET` | Google OAuth client secret. |
| `EXAMPLE_MCP_AUTH_ADMIN_EMAIL` | Initial/admin email allowed by OAuth flow. |

## Logging

| Variable | Purpose |
|---|---|
| `RUST_LOG` | Tracing filter, e.g. `info,rmcp=warn`. |

## Safety

`.env` and `.env.*` are ignored and blocked by `scripts/block-env-commits.sh`. Only `.env.example` belongs in git.

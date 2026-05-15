# Configuration

Configuration is split between non-secret settings and secret environment variables.

## Files

| File | Purpose |
|---|---|
| `.env.example` | Documented environment variable template. Safe to commit. |
| `.env` | Local secrets and deployment settings. Never commit. |
| `config.example.toml` | Optional structured config example for derived services. |
| `src/config.rs` | Loads env/config into typed Rust structs. |

## Auth policy summary

| Situation | Policy |
|---|---|
| Stdio transport | `LoopbackDev` |
| Loopback bind | `LoopbackDev` |
| Non-loopback with bearer token | `Mounted { auth_state: None }` |
| OAuth mode | `Mounted { auth_state: Some(_) }` |
| Explicit trusted gateway no-auth | `TrustedGatewayUnscoped` |

Non-loopback no-auth should only be used when an upstream gateway enforces auth.

## Defaults

- Host defaults to `0.0.0.0` for HTTP serving.
- Port defaults to `3100` in config, while some dev recipes use service-family ports such as `40060`.
- Appdata defaults to a hidden service directory under the user home.

## Validation

Use:

```bash
just doctor
cargo xtask check-env
scripts/check-version-sync.sh
```

See `docs/ENV.md` for variable-by-variable reference.

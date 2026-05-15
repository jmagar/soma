# Deployment

This template supports three deployment modes:

1. **Local development** with `just dev`.
2. **Docker Compose** with `just docker-up`.
3. **User systemd** with an installed release binary.

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

## Auth expectations

Non-loopback HTTP deployments should use bearer auth or OAuth. No-auth on non-loopback is only for an explicitly trusted gateway deployment where another layer enforces authorization.

## Public endpoints

- `/health` is public and fast.
- `/status` is public but redacted.
- `/mcp` is the Streamable HTTP MCP endpoint.
- `/v1/example` is the REST action endpoint.

See `docs/DOCKER.md`, `docs/SYSTEMD.md`, `docs/ENV.md`, and `docs/CONFIG.md` for deployment-specific details.

# Justfile

`Justfile` is the operator command surface for local development, CI parity, Docker, plugin packaging, and diagnostics.

## Core recipes

| Recipe | Purpose |
|---|---|
| `just dev` | Run HTTP MCP server on loopback in no-auth dev mode. |
| `just mcp` | Run stdio MCP transport. |
| `just greet` | Quick CLI smoke test. |
| `just doctor` | Pre-flight environment/connectivity checks. |
| `just build` / `just build-release` | Debug/release Rust builds. |
| `just build-web` | Build static Next.js web assets. |
| `just build-full` | Build web assets then release binary. |
| `just verify` | `fmt-check`, `lint`, `check`, `test`. |
| `just template-check` | Pattern/plugin/schema/template checks. |
| `just pre-release` | Full release-readiness gate. |

## Deployment recipes

| Recipe | Purpose |
|---|---|
| `just docker-up` / `just docker-down` | Start/stop compose stack. |
| `just docker-rebuild` | Rebuild and recreate Docker service. |
| `just runtime-current` | Detect stale running runtime. |
| `just auth-smoke` | Test bearer auth path. |
| `just test-mcporter` | Run live MCP integration tests. |

## Plugin recipes

| Recipe | Purpose |
|---|---|
| `just build-plugin` | Copy release binary into `bin/` and plugin `bin/`. |
| `just validate-plugin` | Validate Claude/Codex/Gemini plugin manifests and skills. |
| `just repair` | Rebuild and restart via systemd or Docker when available. |

Run `just --list` for the complete current list.

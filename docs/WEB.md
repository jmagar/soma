# Web UI

The optional web UI lives under `apps/web/` and is built as a static Next.js export.

## Commands

```bash
just build-web       # build apps/web/out
just web-watch       # rebuild on changes
just build-full      # build web then release binary
```

## Runtime configuration

`apps/web/lib/template.ts` defines the service display name, endpoints, and optional API base URL. `NEXT_PUBLIC_EXAMPLE_API_BASE_URL` should be empty by default so the UI uses same-origin API calls when served by the Rust binary.

Use `apps/web/.env.example` for local web development overrides only.

## Static export

`apps/web/out/.gitkeep` is tracked so Docker COPY paths exist, but generated files under `apps/web/out/*` are ignored. Build assets locally before embedding them in release builds.

## API surfaces

The UI calls:

- `/health`
- `/status`
- `/v1/example`
- `/mcp` for MCP clients rather than browser UI calls

## Validation

```bash
pnpm -C apps/web check
pnpm -C apps/web typecheck
pnpm -C apps/web build
```

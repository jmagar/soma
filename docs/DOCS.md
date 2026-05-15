# Documentation

This repo keeps documentation close to the automation it describes.

## Primary docs

| File | Purpose |
|---|---|
| `README.md` | User-facing overview and template adaptation checklist. |
| `AGENTS.md` / `CLAUDE.md` / `GEMINI.md` | Agent instructions. `CLAUDE.md` is the source; others are symlinks. |
| `docs/PATTERNS.md` | Canonical pattern catalog for rmcp server repos. |
| `docs/MCP_SCHEMA.md` | Generated MCP action/schema contract. |
| `docs/generated/openapi.json` | Generated REST OpenAPI schema. |
| `docs/AUTH.md` | Auth policy and deployment security guidance. |
| `docs/PLUGINS.md` | Plugin packaging notes. |
| `docs/QUICKSTART.md` | Fast start path. |

## Generated references

`docs/references/` is produced by `scripts/refresh-docs.sh` and ignored by git. It includes crawled MCP/Claude docs and Repomix packs of upstream repositories.

```bash
scripts/refresh-docs.sh --dry-run
scripts/refresh-docs.sh
```

## Schema docs

Regenerate and check MCP schema docs with:

```bash
just schema-docs
just schema-docs-check
```

The checker treats `src/actions.rs::ACTION_SPECS` as canonical.

## OpenAPI docs

Regenerate and check REST OpenAPI docs with:

```bash
just openapi
just openapi-check
```

The generator derives REST-capable actions from `src/actions.rs::ACTION_SPECS` and excludes MCP-only actions.

## Agent docs symlinks

After adding any nested `CLAUDE.md`, run:

```bash
just symlink-docs
```

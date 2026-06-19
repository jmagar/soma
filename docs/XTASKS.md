---
title: "xtasks"
doc_type: "guide"
status: "active"
owner: "rmcp-template"
audience:
  - "contributors"
  - "agents"
scope: "template"
source_of_truth: false
last_reviewed: "2026-05-15"
---

# xtasks

The `xtask/` crate contains typed repo automation invoked as `cargo xtask <command>`. Shell scripts are good for glue; xtasks are better when the check needs structured parsing, walking the repo, or cross-platform behavior.

## Repository layout

```
xtask/
  Cargo.toml    # name = "xtask", path dep on main crate
  src/
    main.rs     # cargo xtask <command>
```

## Commands

| Command | Purpose |
|---|---|
| `cargo xtask dist` | Build the release binary into the Cargo target directory. |
| `cargo xtask ci` | Run local CI sequence: fmt, clippy, tests, taplo, patterns, audit when tools exist. |
| `cargo xtask symlink-docs` | Create `AGENTS.md` and `GEMINI.md` symlinks next to each `CLAUDE.md`. |
| `cargo xtask check-env` | Validate required environment before server start. |
| `cargo xtask patterns` | Check static contracts derived from `docs/PATTERNS.md`. |
| `cargo xtask sync-web-source` | Copy editable `apps/web` source into `crates/rtemplate-web/assets/source` with generated artifacts excluded. |
| `cargo xtask check-web-source-sync` | Fail if the bundled web source has drifted from `apps/web`. |
| `cargo xtask update-aurora-web` | Refresh the known Aurora registry components, validate `apps/web`, then sync the bundle. |

## Justfile delegates to xtask

```just
dist:
    cargo xtask dist
symlink-docs:
    cargo xtask symlink-docs
```

## Pattern checks

`cargo xtask patterns` verifies important architecture contracts:

- required template files exist
- no `mod.rs` files
- file size warnings and hard limits
- MCP/CLI shims remain thin (no business logic)
- action surfaces stay represented in schemas, help text, tests, and CLI
- routes, plugin manifests, auth config, and tooling hooks exist

`cargo xtask patterns --strict` treats warnings as failures.

### What the pattern checker catches

```
WARN  crates/rtemplate-mcp/src/tools.rs  line 42: potential business logic in MCP shim
WARN  crates/rtemplate-cli/src/lib.rs  line 87: potential business logic in CLI shim
ERROR crates/rtemplate-service/src/app/mod.rs: mod.rs files are banned
ERROR crates/rtemplate-mcp/src/tools.rs: action "new_action" in ACTION_SPECS missing from dispatch
ERROR crates/rmcp-template/tests/tool_dispatch.rs: action "new_action" has no test
```

## Web Source Sync

`rtemplate-web` bundles editable Aurora frontend source for generated projects.
The source of truth is `apps/web`; the bundled copy lives at
`crates/rtemplate-web/assets/source`.

```bash
cargo xtask sync-web-source
cargo xtask check-web-source-sync
```

The sync excludes generated artifacts: `.next`, `node_modules`, `out`,
`tsconfig.tsbuildinfo`, and `.DS_Store`. `cargo xtask ci` runs
`check-web-source-sync` so drift is caught before merge.

To pull the current Aurora registry versions into the template web app:

```bash
cargo xtask update-aurora-web
```

That command refreshes the Aurora tokens plus the Aurora UI components currently
used by the template, runs `pnpm --dir apps/web validate`, then syncs the bundled
source.

## symlink-docs

`cargo xtask symlink-docs` keeps `AGENTS.md` and `GEMINI.md` in sync with `CLAUDE.md` across every directory that has a `CLAUDE.md`:

```bash
find . -name "CLAUDE.md" -not -path "./.git/*" -not -path "./target/*" | while read f; do
    dir=$(dirname "$f")
    ln -sf "CLAUDE.md" "${dir}/AGENTS.md"
    ln -sf "CLAUDE.md" "${dir}/GEMINI.md"
done
```

Run `just symlink-docs` after adding any new `CLAUDE.md` file.

## check-env

`cargo xtask check-env` reports missing or misconfigured environment before startup:

```
✓ RTEMPLATE_API_URL:   https://example.internal/api (set)
✗ RTEMPLATE_API_KEY:   not set
  → Set RTEMPLATE_API_KEY in ~/.example/.env or your environment
```

See `docs/PATTERNS.md` §24 and §48 for the xtask and doctor patterns.

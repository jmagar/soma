# xtasks

The `xtask/` crate contains typed repo automation invoked as `cargo xtask <command>`.

## Commands

| Command | Purpose |
|---|---|
| `cargo xtask dist` | Build release binary and copy it to `bin/example`. |
| `cargo xtask ci` | Run local CI sequence: fmt, clippy, tests, taplo, patterns, audit when tools exist. |
| `cargo xtask symlink-docs` | Create `AGENTS.md` and `GEMINI.md` symlinks next to each `CLAUDE.md`. |
| `cargo xtask check-env` | Validate required environment before server start. |
| `cargo xtask patterns` | Check static contracts derived from `docs/PATTERNS.md`. |

## Pattern checks

`cargo xtask patterns` verifies important architecture contracts:

- required template files exist
- no `mod.rs` files
- file size warnings/hard limits
- MCP/CLI shims remain thin
- action surfaces stay represented in schemas/help/tests/CLI
- routes, plugin manifests, auth config, and tooling hooks exist

`cargo xtask patterns --strict` treats warnings as failures.

## Why xtask?

Shell scripts are good for glue; xtasks are better when the check needs structured parsing, walking the repo, or cross-platform behavior.

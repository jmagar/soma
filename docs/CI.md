---
title: "CI"
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

# CI

CI mirrors local quality gates so failures are reproducible before pushing.

## Local CI commands

```bash
just verify
just template-check
scripts/pre-release-check.sh
```

`just ci` delegates to `cargo xtask ci`, which runs formatting, clippy, tests, TOML checks, pattern checks, and audit when supporting tools are installed.

## GitHub workflows

Three workflows cover CI, Docker publishing, and releases:

### `.github/workflows/ci.yml`

Runs on push/PR to main:
- `fmt`: `cargo fmt -- --check`
- `clippy`: `cargo clippy -- -D warnings`
- `test`: `cargo nextest run --profile ci`
- `web`: `pnpm install --frozen-lockfile`, `pnpm audit`, `pnpm lint`, `pnpm build`
- `toml`: `taplo check`
- `deny`: `cargo deny check`
- `gitleaks`: secret scanning

### `.github/workflows/docker-publish.yml`

Runs on push to main + tags:
- Multi-platform build (linux/amd64, linux/arm64)
- Push to `ghcr.io/jmagar/<repo>:latest` on main, `:<version>` on tags
- Trivy vulnerability scan
- SBOM generation
- MCP registry publish on version tags

### `.github/workflows/release.yml`

Runs on version tags (`v*`):
- Build release binaries for linux/amd64 and linux/arm64
- Create GitHub Release with binary assets
- Update `install.sh` download URLs

## nextest configuration

CI uses `cargo nextest` with a dedicated profile in `.config/nextest.toml`:

```toml
[profile.default]
fail-fast = false

[profile.ci]
fail-fast = true
retries = 2
```

## Release gate

`scripts/pre-release-check.sh` runs:

1. `cargo xtask patterns`
2. plugin layout validation
3. schema docs validation
4. template feature smoke tests
5. version sync
6. blob-size check
7. ASCII hygiene
8. `just verify`
9. `just build-plugin`

Use `--mcporter` when a server is running and live MCP integration should be included.

## TOML formatting

All repos require `taplo` for TOML formatting:

```bash
taplo format     # format
taplo check      # CI check
```

Install: `cargo install taplo-cli` or `mise use taplo`.

`taplo.toml`:
```toml
[formatting]
align_entries = false
array_trailing_comma = true
array_auto_expand = true
array_auto_collapse = true
compact_arrays = true
compact_inline_tables = false
column_width = 100
indent_string = "  "
trailing_newline = true
allowed_blank_lines = 1
```

## Blob policy

Large artifacts are blocked unless allowlisted in `scripts/blob-size-allowlist.txt`. Plugin binaries are expected artifacts and are allowlisted.

See `docs/PATTERNS.md` §24, §29, §31 for nextest, taplo, and GitHub workflow patterns.

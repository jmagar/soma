# scripts

Maintenance and automation scripts. All use `set -euo pipefail`.

## Scripts

### `check-dependency-updates.sh`

Read-only dependency drift report for Rust workspaces.

```bash
scripts/check-dependency-updates.sh
scripts/check-dependency-updates.sh --skip-search
scripts/check-dependency-updates.sh --fail-on-updates
```

Reports lockfile-compatible updates from `cargo update --dry-run`, then checks
direct root dependencies against crates.io with `cargo search`. Git/path
dependencies are skipped. Use `--skip-search` for offline runs.

---

### `check-file-size.sh`

Fast pre-commit guard for staged source files.

```bash
scripts/check-file-size.sh
MAX_RS=450 MAX_TS=350 scripts/check-file-size.sh
```

Checks staged `.rs`, `.ts`, and `.tsx` files for effective production lines.
Test files are exempt, and Rust inline `#[cfg(test)]` modules are excluded from
the count. Defaults are `MAX_RS=350` and `MAX_TS=300`.

---

### `asciicheck.py`

Checks files for unexpected non-ASCII characters and can replace common smart
punctuation with ASCII equivalents.

```bash
python3 scripts/asciicheck.py README.md Justfile
python3 scripts/asciicheck.py --fix README.md
just ascii-check
just ascii-fix
```

The template intentionally allows a small set of existing documentation
characters used in comments and headings: section signs, em dashes, arrows, and
box-drawing divider lines.

---

### `check-blob-size.py`

Checks changed git blobs against a size budget.

```bash
python3 scripts/check-blob-size.py
python3 scripts/check-blob-size.py --base origin/main --head HEAD --max-bytes 512000
just blob-size-check
```

Defaults to `origin/main` as the base when available, then `main`, then
`HEAD~1`. Allow unavoidable large checked-in artifacts with
`scripts/blob-size-allowlist.txt`; patterns are repo-relative globs.

---

### `check-runtime-current.sh`

Detects stale deployed runtimes.

```bash
scripts/check-runtime-current.sh
scripts/check-runtime-current.sh --mode systemd --expected-binary target/release/example
scripts/check-runtime-current.sh --mode docker --pull --compose-dir .
just runtime-current
```

Systemd mode compares the running process hash from `/proc/<pid>/exe` to the
unit `ExecStart` binary, and optionally to an expected binary path. Docker mode
compares the running container image ID with the local Compose image ID.

**TEMPLATE**: Rename `example-mcp`, `example`, and `EXAMPLE_MCP_*` defaults when
adapting this template to a real service.

---

### `test-mcp-auth.sh`

Smoke-tests the protected MCP HTTP auth path against a running server.

```bash
EXAMPLE_MCP_TOKEN=... scripts/test-mcp-auth.sh
scripts/test-mcp-auth.sh --url http://localhost:3000/mcp --token ...
```

Checks that `/health` is public, `/mcp` rejects missing and bad bearer tokens
with `401`, and `/mcp` accepts `Authorization: Bearer <token>`.

`x-api-key` support is intentionally not assumed because this template's pinned
auth layer accepts bearer tokens. For derived services that add `x-api-key`, run:

```bash
scripts/test-mcp-auth.sh --check-x-api-key
```

---

### `check-schema-docs.py`

Generates and verifies the MCP action schema contract.

```bash
python3 scripts/check-schema-docs.py --write
python3 scripts/check-schema-docs.py --check
just schema-docs
just schema-docs-check
```

The checker treats `EXAMPLE_ACTIONS` in `src/mcp/schemas.rs` as canonical, then
verifies `READ_ONLY_ACTIONS`, `src/mcp/tools.rs` help text, `README.md`, and the
shared skill mention every action. Generated output lives in
`docs/MCP_SCHEMA.md`.

---

### `test-template-features.sh`

Runs fast shell smoke tests for template invariants that are not worth modeling
as full integration tests.

```bash
bash scripts/test-template-features.sh
just template-features
```

Current checks cover `.env` commit blocking, `CLAUDE.md` sibling symlink
creation, plugin layout validation, schema-doc validation, and ASCII hygiene.

---

### `pre-release-check.sh`

Runs the release-readiness gate.

```bash
scripts/pre-release-check.sh
scripts/pre-release-check.sh --skip-verify --skip-build-plugin
scripts/pre-release-check.sh --mcporter
just pre-release
```

The default gate runs plugin validation, schema-doc validation, template feature
smoke tests, version sync, blob-size check, ASCII hygiene, `just verify`, and
`just build-plugin`. `--mcporter` adds the live MCP integration harness.

---

### `check-coupled-files.sh`

CI-oriented guard for files that should usually change together.

```bash
scripts/check-coupled-files.sh origin/main HEAD
just coupled-files-check
```

Examples: `Justfile` with `lefthook.yml`, scripts with `scripts/README.md`, and
schema changes with `docs/MCP_SCHEMA.md`.

---

### `validate-plugin-layout.sh`

Validates the shipped plugin package.

```bash
scripts/validate-plugin-layout.sh
PLUGIN_ROOT=plugins/example scripts/validate-plugin-layout.sh
just validate-plugin
```

Checks Claude, Codex, and Gemini manifests, shared MCP config, hook config, and
skill frontmatter. It also enforces the template rule that plugin manifests do
not carry a `version` field; Cargo/Git tags are the release version source.

---

### `refresh-docs.sh`

Fetch and refresh local reference documentation from external sources. Crawls MCP protocol docs and Claude Code docs via Axon, packs GitHub repos via Repomix, and updates `docs/references/`.

```bash
scripts/refresh-docs.sh              # full crawl + repomix packs
scripts/refresh-docs.sh --dry-run    # print plan, write nothing
scripts/refresh-docs.sh --skip-crawl # repomix packs only
scripts/refresh-docs.sh --skip-repomix # axon crawls only
```

**Produces:**

```
docs/references/
├── mcp/
│   ├── docs/          # Crawled modelcontextprotocol.io (markdown)
│   └── repos/         # Repomix packs: rust-sdk, spec, registry
├── claude-code/       # Crawled code.claude.com (markdown)
├── mcporter/
│   ├── docs/          # Sparse-cloned mcporter docs
│   └── repos/         # Repomix pack of mcporter source
├── INDEX.md           # File counts and key references
└── CHANGES.md         # Before/after diff summary
```

**Environment variables:**

| Variable | Default | Description |
|---|---|---|
| `AXON_OUTPUT_DIR` | `~/.axon/output` | Axon host output directory |
| `REPOMIX_BIN` | auto-detected | Path to repomix (falls back to `npx --yes repomix`) |

Uses atomic directory replacement (temp dir + `mv`) so an interrupted run never leaves a partial state.

**TEMPLATE**: Add your service's crawl targets and repos in the clearly marked `TEMPLATE:` section near the bottom of the script.

---

### `bump-version.sh`

Atomically update the version number across all config files in the project.

```bash
scripts/bump-version.sh 1.3.5    # explicit version
scripts/bump-version.sh patch    # 1.2.3 → 1.2.4
scripts/bump-version.sh minor    # 1.2.3 → 1.3.0
scripts/bump-version.sh major    # 1.2.3 → 2.0.0
```

Reads the current version from `.claude-plugin/plugin.json` (single source of truth) and updates:

- `Cargo.toml`
- `pyproject.toml`
- `.claude-plugin/plugin.json`
- `.codex-plugin/plugin.json`
- `.gemini-extension.json` / `gemini-extension.json`

Skips files that don't exist. Prints a summary and reminds you to update `CHANGELOG.md`.

---

### `check-version-sync.sh`

Pre-commit hook that validates all version-bearing files agree and that `CHANGELOG.md` has an entry for the current version.

```bash
scripts/check-version-sync.sh           # check current directory
scripts/check-version-sync.sh /path/to  # check specific directory
```

Checks `Cargo.toml`, `package.json`, `pyproject.toml`, `.claude-plugin/plugin.json`, `.codex-plugin/plugin.json`, `gemini-extension.json`. Exits non-zero if any versions differ. Missing `CHANGELOG.md` entry is a warning, not a failure.

---

### `block-env-commits.sh`

Pre-commit guard that rejects commits containing `.env*` files (except `.env.example`).

```bash
# Called automatically by lefthook; can also be run manually:
bash scripts/block-env-commits.sh
```

Matches any `.env`, `.env.local`, `.env.prod`, `.env.staging`, etc. at any directory depth. Exits 0 (allow) or 1 (block) with a list of the offending files.

No configuration needed — copy unchanged to any new server repo.

---

### `sync-cargo.sh`

Sync `Cargo.lock` from the repo root into a plugin data directory. Used for plugin isolation and containerized environments where the lockfile must live outside the source tree.

```bash
bash scripts/sync-cargo.sh
```

**Environment variables:**

| Variable | Default | Description |
|---|---|---|
| `CLAUDE_PLUGIN_ROOT` | script's parent dir | Repository root |
| `CLAUDE_PLUGIN_DATA` | `CLAUDE_PLUGIN_ROOT` | Destination data directory |

Compares lockfiles with `diff` before copying to avoid unnecessary writes. Falls back to `cargo fetch` if the copy fails. Cleans up partial copies on error.

---

## Hook integration (lefthook)

`block-env-commits.sh`, `check-version-sync.sh`, and `check-file-size.sh` are
designed as lefthook pre-commit hooks. Wire them in `lefthook.yml`:

```yaml
pre-commit:
  commands:
    env_guard:
      run: bash scripts/block-env-commits.sh
    version_sync:
      run: bash scripts/check-version-sync.sh
    file_size:
      glob: "*.{rs,ts,tsx}"
      run: bash scripts/check-file-size.sh
```

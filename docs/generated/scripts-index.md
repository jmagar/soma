# Scripts Index

Generated from script header comments.

| File | Summary |
|---|---|
| `scripts/asciicheck.py` | Check files for non-ASCII characters. |
| `scripts/block-env-commits.sh` | scripts/block-env-commits.sh — Pre-commit guard: block .env secrets |
| `scripts/build-web.sh` | Build the Next.js web UI static export. |
| `scripts/bump-version.sh` | bump-version.sh — update version in all version-bearing files atomically. |
| `scripts/check-blob-size.py` | Fail if changed git blobs exceed the configured size budget. |
| `scripts/check-cargo-generate.py` | Compatibility wrapper for the xtask-owned cargo-generate smoke test. |
| `scripts/check-coupled-files.sh` | Fail when common coupled files are changed without their companion updates. |
| `scripts/check-dependency-updates.sh` | Report lockfile-compatible and latest direct dependency updates. |
| `scripts/check-file-size.sh` | Prevent monolithic staged source files from being committed. |
| `scripts/check-openapi.py` | Generate and verify OpenAPI docs for the template REST API. |
| `scripts/check-plugin-hook-contract.py` | Audit binary-owned plugin hook setup contracts across Rust MCP servers. |
| `scripts/check-plugin-stdio-smoke.sh` | Smoke-test the installed stdio MCP binary used by plugin manifests. |
| `scripts/check-runtime-current.sh` | Check whether the running systemd unit or Docker container uses the current artifact. |
| `scripts/check-scaffold-intent-contract.py` | Validate scaffold intent contract JSON and checked-in examples. |
| `scripts/check-schema-docs.py` | Generate and verify MCP schema/action documentation drift. |
| `scripts/check-stale-claims.py` | Fail on stale template claims that should never come back. |
| `scripts/check-version-sync.sh` | check-version-sync.sh — compatibility wrapper for the manifest-backed gate. |
| `scripts/generate-cli.sh` | Generate a standalone CLI for this server via mcporter. |
| `scripts/generate-docs.py` | Generate volatile docs and metadata from canonical template specs. |
| `scripts/pre-release-check.sh` | Run the template release-readiness gate. |
| `scripts/refresh-docs.sh` | refresh-docs.sh — Refresh reference docs for rmcp-template |
| `scripts/repair.sh` | Stop, rebuild, and restart the rtemplate-mcp service. |
| `scripts/run-ascii-check.sh` | Check (or fix) tracked source/config/docs files for non-ASCII characters. |
| `scripts/sync-cargo.sh` | Copy Cargo.lock into plugin data directories for reproducible plugin builds. |
| `scripts/test-mcp-auth.sh` | Smoke-test the HTTP MCP static bearer-token gate. |
| `scripts/test-template-features.sh` | Smoke-test template invariants that are awkward to express as unit tests. |
| `scripts/validate-plugin-layout.sh` | Validate the template plugin artifacts shipped by this repository. |
| `scripts/web-watch.sh` | Watch apps/web for changes and rebuild on save. |

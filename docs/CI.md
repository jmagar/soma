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

The repo includes workflows for Rust CI, CodeQL, MSRV, cargo-deny, Docker publishing, and releases. Keep workflow changes coupled with Justfile or script changes when they alter local/CI parity.

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

## Blob policy

Large artifacts are blocked unless intentionally allowlisted in `scripts/blob-size-allowlist.txt`. Plugin binaries are expected artifacts and are allowlisted.

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
last_reviewed: "2026-06-27"
---

# CI

CI mirrors local quality gates so failures are reproducible before pushing.

## Local CI commands

```bash
just verify
just template-check
cargo xtask pre-release-check
```

`just ci` delegates to `cargo xtask ci`, which runs formatting, clippy, tests, TOML checks, pattern checks, and audit when supporting tools are installed.

## GitHub workflows

The repository keeps separate workflows for fast PR feedback, release
automation, long-lived marketplace-variant maintenance, and scheduled drift
checks. Use the smallest workflow that proves the thing you changed; do not
turn release or sync workflows into general PR CI.

### `.github/workflows/ci.yml`

Use for: every PR, every push to `main`, and manual full verification.

Do not use for: tag-only packaging or marketplace-no-mcp branch maintenance.

The jobs run on self-hosted runners: Linux on the TOOTIE Docker runner
(`runs-on: [self-hosted, tootie, rmcp-template]`, see `docs/LINUX-RUNNER.md`)
and Windows on steamy (`runs-on: [self-hosted, Windows, rmcp-template, steamy]`,
see `docs/WINDOWS-RUNNER.md`). The Rust jobs force `RUSTC_WRAPPER=sccache`,
`CARGO_BUILD_RUSTC_WRAPPER=sccache`, and `CARGO_INCREMENTAL=0`; the local
`.github/actions/setup-rust-sccache` action installs Rust plus sccache and prints
the effective cache configuration. This keeps CI on sccache while bypassing the
repo's developer-only `scripts/cargo-rustc-wrapper`.

Self-hosted jobs use a same-repository job guard. Pushes, schedules, manual
runs, and same-repo PRs can use the TOOTIE and steamy runners; fork PRs do not
allocate self-hosted runners. Add a GitHub-hosted fork fallback before accepting
outside PRs that need CI feedback.

Jobs:
- `actionlint`: validates workflow syntax and self-hosted labels
- `fmt`: `cargo fmt -- --check`
- `clippy`: `cargo clippy -- -D warnings`
- `test`: builds the stdio binary, runs `cargo nextest run --profile ci`, and uploads the JUnit report
- `frontend-assets`: `pnpm install --frozen-lockfile`, `pnpm audit`, `pnpm lint`, `pnpm typecheck`, `pnpm build`
- `build-linux`: native Linux release build, uploads `rtemplate-linux-x86_64`
- `build-windows`: native Windows release build and test on steamy, uploads `rtemplate-windows-x86_64`
- `mcp-smoke`: starts the HTTP MCP server and runs the mcporter smoke suite
- `container-smoke`: validates compose files and builds the Docker image
- `toml`: `taplo check`
- `lefthook-speed`: keeps pre-commit hooks staged-only and fast
- `template`: generated docs, plugin layout, scaffold, release-version, blob, coupled-file, and ASCII gates
- `deny`: `cargo deny check`
- `gitleaks`: secret scanning
- `ci-gate`: single aggregate status for branch protection

The Linux and Windows build jobs are PR-time artifact checks. They prove the
binary compiles natively before a release tag exists and give reviewers a
downloadable artifact for manual smoke testing.

The Windows job follows the Axon CI pattern: it builds on native Windows and
sets explicit portable CPU flags so self-hosted runner config cannot leak
`target-cpu=native` into published artifacts. See `docs/WINDOWS-RUNNER.md` for
the full runner setup and audit process.

### `.github/workflows/msrv.yml`

Use for: proving the declared `rust-version` remains honest.

Do not use for: full behavior testing; it only checks that the workspace still
builds on the minimum supported toolchain.

Runs on PRs and pushes to `main` with Rust 1.96.0 and sccache.

### `.github/workflows/auto-tag.yml`

Use for: automatic component tag creation after a successful push to `main`.

Do not use for: manually forcing a release. If the release manifest says no
component changed, this workflow intentionally does nothing.

It runs `cargo xtask release-plan --head HEAD --mode main --json`, waits for CI
on the exact push SHA, and creates the candidate tag for each changed component.

### `.github/workflows/release.yml`

Use for: tag-time binary packaging and GitHub Release creation.

Do not use for: PR validation. PRs should use `ci.yml`; release only runs on
`v*` tags.

It builds Linux and Windows release artifacts, writes SHA256 sums, and creates
the GitHub Release. Release Cargo builds use sccache through the same wrapper
environment as CI. The LFS write-back job is intentionally isolated here because
it can push to `main`; audit that behavior before reusing it in a derived repo.

### `.github/workflows/docker-publish.yml`

Use for: publishing container images after code has landed.

Do not use for: PR smoke tests. `ci.yml` has a non-pushing `container-smoke` job
for that.

Runs on push to `main` and tags:
- Multi-platform build (linux/amd64, linux/arm64)
- Push to `ghcr.io/jmagar/<repo>:latest` on main, `:<version>` on tags
- Trivy vulnerability scan
- SBOM generation
- MCP registry publish on version tags

### `.github/workflows/scheduled.yml`

Use for: surfacing new RUSTSEC advisories after code has already merged.

Do not use for: replacing the PR-time `audit` job. This is a periodic safety
net, not the merge gate.

Scheduled runs check advisories only; manual dispatch can run the full
`cargo-deny` suite.

### `.github/workflows/check-no-mcp-drift.yml`

Use for: detecting drift between `main` and the protected `marketplace-no-mcp`
variant.

Do not use for: syncing or modifying the branch. This workflow is read-only.

### `.github/workflows/sync-marketplace-no-mcp.yml`

Use for: keeping the protected `marketplace-no-mcp` branch current with `main`
while applying the no-MCP variant rules.

Do not use for: branch cleanup. `marketplace-no-mcp` is a long-lived protected
variant branch and must not be deleted, squashed away, or folded into `main`
unless Jacob explicitly asks for that exact branch to be retired.

### `.github/workflows/dependabot-auto-merge.yml`

Use for: auto-merging eligible Dependabot updates after required checks pass.

Do not use for: human-authored dependency migrations, major upgrades, or changes
that alter public behavior. Those need normal review.

### `.github/workflows/rmcp-release-monitor.yml`

Use for: watching upstream `rmcp`, MCP schema, and conformance movement and
opening/updating an issue when there is drift.

Do not use for: automatically bumping protocol dependencies. It reports; humans
decide the migration.

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

`cargo xtask pre-release-check` runs:

1. `cargo xtask patterns`
2. plugin layout validation
3. schema docs validation
4. template feature smoke tests
5. release version gate
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

## Release artifact distribution

`release/components.toml` is the source of truth for release components, version-bearing files, tag prefixes, release workflows, and shipping paths. PR CI runs `cargo xtask check-release-versions --base origin/main --head HEAD --mode pr`, using the merge-base of the PR branch so base-only changes do not force a false bump. Pushes to `main` run `.github/workflows/auto-tag.yml`, which consumes `cargo xtask release-plan --head HEAD --mode main --json`, waits for CI on the exact push SHA, and creates the candidate tag for changed components.

Version tags (`v*`) trigger the release workflow, which builds release binaries and attaches them to the GitHub Release. The release workflow must **not** push generated binaries back to `main`. Local `just dist` / `cargo xtask dist` recipes are operator conveniences for preparing artifacts — they are not a CI write-back path.

CI artifact naming convention:

- `rtemplate-linux-x86_64`
- `rtemplate-windows-x86_64`

Release tarball naming convention:

- `rtemplate-x86_64.tar.gz`
- `rtemplate-windows-x86_64.tar.gz`

## CHANGELOG.md

Every repo keeps a `CHANGELOG.md` following [Keep a Changelog](https://keepachangelog.com/):

```markdown
# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] — 2026-05-13

### Added
- Initial release
- MCP server with action-based tool dispatch
- CLI thin shim
- Bearer token + Google OAuth authentication
- Streamable HTTP + stdio transport
- Thin plugin setup hook plus binary-owned setup/repair
- Claude Code plugin with userConfig
```

Update `[Unreleased]` with every meaningful change. The release workflow promotes it to a versioned section on tag.

See `docs/PATTERNS.md` §21, §24, §29, §31, §34 for release artifacts, nextest, taplo, GitHub workflow, and changelog patterns.

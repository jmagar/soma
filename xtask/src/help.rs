pub(crate) fn print_help() {
    eprintln!(
        "cargo xtask - repo automation for soma

USAGE:
  cargo xtask <command>

COMMANDS:
  dist                  Build release binary into Cargo target dir
  ci                    Run all CI checks: fmt, clippy, nextest, taplo, audit
  symlink-docs          Create AGENTS.md + GEMINI.md symlinks next to every CLAUDE.md
  check-env             Validate required environment variables are set
  check-test-siblings   Verify every src/*.rs has a sibling *_tests.rs
  check-architecture    Validate workspace dependency-layer boundaries
  patterns              Check static contracts from docs/PATTERNS.md (--strict, --json)
  contract-audit        Run local static/spec checks without live upstream calls
  scaffold              Plan/apply/verify a generated project from Soma
  codex-schema          Rebuild/bisect the vendored codex-app-server-client schema
  cargo-generate        Smoke-test real cargo-generate output (--no-cargo-check)
  cargo-generate-post   Internal generated-project rewrite command
  generate-docs         Generate volatile docs and metadata from canonical specs
  check-docs            Validate generated docs and metadata are current
  check-mcp-registry    Validate server.json against docs/contracts/mcp-server.schema.json
  check-stale-claims    Fail on stale hardcoded Soma claims
  check-cargo-generate  Validate cargo-generate output
  sync-web-source       Copy apps/web into crates/soma/web/assets/source
  check-web-source-sync Validate bundled web source matches apps/web
  update-aurora-web     Refresh Aurora registry components, validate, then sync
  build-web             Build optional apps/web static export
  web-watch             Watch apps/web and rebuild on changes
  generate-cli          Generate dist/soma-cli through mcporter
  repair                Rebuild and restart local soma-mcp runtime
  test-mcp-auth         Smoke-test HTTP MCP bearer auth
  asciicheck            Check/fix explicit files for non-ASCII characters
  check-blob-size       Check changed git blobs against size budget
  block-env-commits     Prevent staged .env secrets from being committed
  check-coupled-files   Check common companion-file drift in a diff
  check-dependency-updates
                        Report Cargo dependency updates
  check-file-size       Check staged source files against size budgets
  check-openapi         Generate/check docs/generated/openapi.json
  check-plugin-hook-contract
                        Audit cross-repo plugin hook contracts
  run-ascii-check       Check or fix tracked source/config/docs ASCII hygiene
  check-plugin-stdio-smoke
                        Smoke-test installed plugin stdio binary
  check-runtime-current Check systemd/Docker runtime artifact freshness
  check-schema-docs     Generate/check docs/MCP_SCHEMA.md
  check-scaffold-intent-contract
                        Validate scaffold intent schema/examples
  apply-no-mcp-marketplace
                        Remove bundled MCP registrations for the no-MCP branch
  check-no-mcp-drift    Validate marketplace-no-MCP invariants and branch drift
  sync-cargo            Copy Cargo.lock into plugin data directories
  pre-release-check     Run release-readiness gate
  refresh-docs          Refresh ignored reference docs
  test-soma-features    Run Soma invariant smoke tests
  validate-plugin-layout
                        Validate Claude/Codex/Gemini plugin package layout
  check-version-sync    Validate release manifest version-file parity
  check-release-versions [--base REF] [--head REF] [--mode pr|main] [--json]
                        Validate changed release components have fresh versions/tags
  release-plan          Print changed release components and candidate tags
  sync-release-please-version
                        Sync version files to .release-please-manifest.json
  bump-version          Bump a component: cargo xtask bump-version soma patch
  bump-soma-version     Bump Soma component: cargo xtask bump-soma-version patch
  changed-paths         Classify changed files into CI routing categories
  help                  Show this help

CUSTOMIZE:
  Add commands by extending the match block in xtask/src/main.rs.
  Keep dependencies minimal - xtask should compile in seconds."
    );
}

#[cfg(test)]
#[path = "help_tests.rs"]
mod tests;

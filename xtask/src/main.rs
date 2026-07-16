//! xtask — Repo automation for soma.
//!
//! Invoked via: `cargo xtask <command>`
//!
//! Commands:
//!   dist         Build release binary into Cargo target dir
//!   ci           Run all CI checks: fmt, clippy, nextest, taplo, audit
//!   symlink-docs Create AGENTS.md and GEMINI.md symlinks next to every CLAUDE.md
//!   check-env    Validate required environment variables are set
//!   patterns     Check static contracts from docs/PATTERNS.md
//!   contract-audit Run local static/spec checks for REST-client MCP servers
//!   scaffold     Plan, generate, or verify a new project from Soma
//!   codex-schema Rebuild/bisect the vendored codex-app-server-client schema
//!   cargo-generate Smoke-test cargo-generate output
//!   cargo-generate-post Apply cargo-generate post-processing rewrites
//!   generate-docs Generate volatile docs and metadata from canonical specs
//!   generate-provider-surfaces Generate provider docs and marketplace catalogs
//!   check-docs    Validate generated docs and metadata are current
//!   check-architecture Validate workspace dependency-layer boundaries
//!   check-mcp-registry Validate server.json against the MCP registry schema
//!   check-stale-claims Fail on stale hardcoded Soma claims
//!   sync-web-source Copy apps/web into the bundled soma-web scaffold source
//!   check-web-source-sync Validate bundled web source matches apps/web
//!   update-aurora-web Refresh Aurora components, validate apps/web, then sync bundle
//!   block-env-commits Prevent staged .env secrets from being committed
//!   check-coupled-files Check common companion-file drift in a diff
//!   check-file-size Check staged source files against size budgets
//!   run-ascii-check Check or fix tracked source/config/docs ASCII hygiene
//!   check-plugin-stdio-smoke Smoke-test installed plugin stdio binary
//!   apply-no-mcp-marketplace Apply deterministic no-MCP marketplace branch transform
//!   check-no-mcp-drift Validate marketplace-no-mcp branch invariants and drift
//!   sync-cargo   Copy Cargo.lock into plugin data directories
//!   check-release-versions Validate release component version policy
//!   release-plan Print changed release components and candidate tags
//!   sync-release-please-version Sync release files to .release-please-manifest.json
//!   bump-version Bump a release component version
//!   changed-paths Classify changed files into CI routing categories
//!
//! CUSTOMIZE: Add your own commands by adding arms to the match block below.
//!           Keep each command as a separate `fn` for readability.
//!
//! Philosophy: xtask replaces ad-hoc shell scripts. It gets type-checked by the
//! compiler, works cross-platform, and is easy to extend. Keep functions small
//! and use `std::process::Command` to shell out to existing tools rather than
//! reimplementing them in Rust.

use anyhow::{bail, Context, Result};
use std::process::{Command, Stdio};

mod architecture;
mod architecture_graph;
mod cargo_generate;
mod cargo_generate_post;
mod ci_paths;
mod codex_schema;
mod generated_surfaces;
mod help;
mod mcp_registry;
mod no_mcp;
mod patterns;
mod provider_manifest;
mod release_commands;
mod release_versions;
mod repo_checks;
mod rmcp_release_monitor;
mod scaffold;
mod scripts;
mod scripts_lane_a;
mod scripts_lane_b;
mod scripts_lane_c;
mod scripts_lane_d;
mod web_source;

fn main() -> Result<()> {
    // Cargo sets CARGO_MANIFEST_DIR for the workspace root when invoked as
    // `cargo xtask`. Change into the workspace root so all commands work
    // regardless of the cwd from which the user invoked cargo.
    //
    // CUSTOMIZE: This path navigation assumes xtask/ is a direct child of the
    //           workspace root. If you restructure, adjust the `..` accordingly.
    let workspace_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask/Cargo.toml must have a parent directory");
    std::env::set_current_dir(workspace_root).context("Failed to change into workspace root")?;

    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("dist") => dist(),
        Some("ci") => ci(),
        Some("symlink-docs") => repo_checks::symlink_docs(),
        Some("check-env") => repo_checks::check_env(),
        Some("patterns") => patterns_cmd(&args[1..]),
        Some("contract-audit") => contract_audit(),
        Some("scaffold") => scaffold::run(&args[1..]),
        Some("codex-schema") => codex_schema::run(&args[1..]),
        Some("cargo-generate") => cargo_generate(&args[1..]),
        Some("cargo-generate-post") => cargo_generate_post::run(&args[1..]),
        Some("generate-docs") => generate_docs(),
        Some("generate-provider-surfaces") => generated_surfaces::provider_surfaces(&args[1..]),
        Some("check-docs") => check_docs(),
        Some("check-architecture") => architecture::check(workspace_root),
        Some("check-mcp-registry") => mcp_registry::check_cmd(workspace_root, &args[1..]),
        Some("check-stale-claims") => check_stale_claims(),
        Some("check-cargo-generate") => scripts_lane_d::check_cargo_generate(&args[1..]),
        Some("sync-web-source") => web_source::sync(),
        Some("check-web-source-sync") => web_source::check(),
        Some("update-aurora-web") => web_source::update_aurora(),
        Some("build-web") => scripts_lane_a::build_web(),
        Some("web-watch") => scripts_lane_a::web_watch(),
        Some("generate-cli") => scripts_lane_a::generate_cli(),
        Some("repair") => scripts_lane_a::repair(),
        Some("test-mcp-auth") => scripts_lane_a::test_mcp_auth(&args[1..]),
        Some("block-env-commits") => scripts::block_env_commits(),
        Some("asciicheck") => scripts_lane_d::asciicheck(&args[1..]),
        Some("check-blob-size") => scripts_lane_c::check_blob_size(&args[1..]),
        Some("check-coupled-files") => scripts::check_coupled_files(&args[1..]),
        Some("check-dependency-updates") => scripts_lane_c::check_dependency_updates(&args[1..]),
        Some("check-file-size") => scripts::check_file_size(),
        Some("check-openapi") => scripts_lane_d::check_openapi(&args[1..]),
        Some("check-openapi-drift") => scripts_lane_d::check_openapi(&args[1..]),
        Some("check-palette-manifest") => generated_surfaces::check_palette_manifest(&args[1..]),
        Some("check-provider-manifest-contract") => provider_manifest::check(),
        Some("check-plugin-hook-contract") => {
            scripts_lane_c::check_plugin_hook_contract(&args[1..])
        }
        Some("run-ascii-check") => scripts::run_ascii_check(&args[1..]),
        Some("check-plugin-stdio-smoke") => scripts::check_plugin_stdio_smoke(),
        Some("check-runtime-current") => scripts_lane_c::check_runtime_current(&args[1..]),
        Some("check-schema-docs") => scripts_lane_d::check_schema_docs(&args[1..]),
        Some("check-scaffold-intent-contract") => scripts_lane_d::check_scaffold_intent_contract(),
        Some("apply-no-mcp-marketplace") => no_mcp::apply_cmd(),
        Some("check-no-mcp-drift") => no_mcp::check_cmd(&args[1..]),
        Some("sync-cargo") => scripts::sync_cargo(),
        Some("pre-release-check") => scripts_lane_b::pre_release_check(&args[1..]),
        Some("refresh-docs") => scripts_lane_c::refresh_docs(&args[1..]),
        Some("test-soma-features") => scripts_lane_b::test_soma_features(workspace_root),
        Some("validate-plugin-layout") => {
            let plugin_root = std::env::var_os("PLUGIN_ROOT").map(std::path::PathBuf::from);
            scripts_lane_b::validate_plugin_layout(workspace_root, plugin_root.as_deref())
        }
        Some("check-test-siblings") => repo_checks::check_test_siblings(),
        Some("check-version-sync") => {
            scripts_lane_b::check_version_sync(workspace_root, &args[1..])
        }
        Some("check-release-versions") => {
            release_commands::check_release_versions(workspace_root, &args[1..])
        }
        Some("release-plan") => release_commands::release_plan(workspace_root, &args[1..]),
        Some("sync-release-please-version") => {
            release_versions::sync_release_please_version(workspace_root, "soma")
        }
        Some("rmcp-release-monitor") => rmcp_release_monitor::run(&args[1..]),
        Some("bump-version") => release_commands::bump_version(workspace_root, &args[1..]),
        Some("bump-soma-version") => scripts_lane_b::bump_version(workspace_root, &args[1..]),
        Some("changed-paths") => ci_paths::run(&args[1..]),
        Some("--help") | Some("-h") | Some("help") | None => {
            help::print_help();
            Ok(())
        }
        Some(unknown) => {
            bail!("Unknown xtask command: {unknown:?}\nRun `cargo xtask --help` for usage.")
        }
    }
}

// =============================================================================
// cargo-generate — Smoke-test generated scaffold output
// =============================================================================

fn cargo_generate(args: &[String]) -> Result<()> {
    cargo_generate::run(args)
}

// =============================================================================
// contract-audit — Safe local contract/spec checks for REST-client MCP servers
// =============================================================================

/// Run the local, non-destructive audit suite for the Soma contract.
///
/// This command intentionally avoids live upstream services. REST-client
/// behavior belongs in per-server mock-upstream tests; this command verifies
/// the static contract surfaces that every derived server should keep current.
fn contract_audit() -> Result<()> {
    println!("==> contract-audit: local static/spec checks only");
    println!("==> [1/13] cargo xtask check-architecture");
    architecture::check(std::path::Path::new(".")).context("architecture check failed")?;

    println!("==> [2/13] cargo xtask patterns");
    patterns::run(patterns::PatternOptions::default()).context("patterns contract check failed")?;

    println!("==> [3/13] cargo xtask check-test-siblings");
    repo_checks::check_test_siblings().context("test sibling check failed")?;

    println!("==> [4/13] cargo xtask check-docs");
    check_docs().context("generated docs check failed")?;

    println!("==> [5/13] cargo xtask check-stale-claims");
    check_stale_claims().context("stale claim check failed")?;

    println!("==> [6/13] cargo xtask check-schema-docs --check");
    scripts_lane_d::check_schema_docs(&["--check".to_owned()])
        .context("schema docs check failed")?;

    println!("==> [7/13] cargo xtask check-openapi --check");
    scripts_lane_d::check_openapi(&["--check".to_owned()]).context("OpenAPI docs check failed")?;

    println!("==> [8/13] cargo xtask check-mcp-registry");
    mcp_registry::check_default(std::path::Path::new("."))
        .context("MCP registry manifest check failed")?;

    println!("==> [9/13] cargo xtask check-provider-manifest-contract");
    provider_manifest::check().context("provider manifest contract check failed")?;

    println!("==> [10/13] cargo xtask check-palette-manifest --check");
    generated_surfaces::check_palette_manifest(&["--check".to_owned()])
        .context("Palette manifest check failed")?;

    println!("==> [11/13] cargo xtask generate-provider-surfaces --check");
    generated_surfaces::provider_surfaces(&["--check".to_owned()])
        .context("provider surfaces check failed")?;

    println!("==> [12/13] cargo xtask check-scaffold-intent-contract");
    scripts_lane_d::check_scaffold_intent_contract()
        .context("scaffold intent contract check failed")?;

    println!("==> [13/13] cargo xtask test-soma-features");
    scripts_lane_b::test_soma_features(std::path::Path::new("."))
        .context("Soma feature smoke failed")?;

    println!("==> contract-audit: passed; no live upstream services were contacted");
    Ok(())
}

// =============================================================================
// generated docs — Render/check volatile docs and metadata
// =============================================================================

fn generate_docs() -> Result<()> {
    run_cmd("python3", &["scripts/generate-docs.py", "--write"])
        .context("generated docs update failed")
}

fn check_docs() -> Result<()> {
    run_cmd("python3", &["scripts/generate-docs.py", "--check"])
        .context("generated docs are stale; run `cargo xtask generate-docs`")
}

fn check_stale_claims() -> Result<()> {
    run_cmd("python3", &["scripts/check-stale-claims.py"]).context("stale claim check failed")
}

// =============================================================================
// dist — Build release binary
// =============================================================================

/// Build the release binary. Distribution is handled by package/release tooling;
/// plugins reference an installed PATH binary and do not bundle artifacts.
///
/// CUSTOMIZE: Replace "soma" with your binary name throughout this function.
///           The binary name must match Cargo.toml `[[bin]] name = "..."`.
fn dist() -> Result<()> {
    // CUSTOMIZE: Replace "soma" with your binary name.
    const BINARY_NAME: &str = "soma";

    println!("==> Building release binary: {BINARY_NAME}");
    run_cargo(&["build", "--release", "--locked", "--bin", BINARY_NAME])?;

    let target_dir = std::env::var("CARGO_TARGET_DIR").unwrap_or_else(|_| "target".into());
    let artifact = std::path::Path::new(&target_dir)
        .join("release")
        .join(BINARY_NAME);
    if !artifact.exists() {
        bail!("Release binary not found at {artifact:?} — build must have failed");
    }

    println!("==> Built {artifact:?}");
    println!("==> Run `just install-local` to install it to ~/.local/bin for plugin use");
    Ok(())
}

// =============================================================================
// ci — Run all CI checks locally
// =============================================================================

/// Run all CI checks in sequence: fmt, clippy, nextest, taplo, audit.
///
/// This mirrors what `.github/workflows/ci.yml` runs. Use it to catch failures
/// before pushing.
///
/// CUSTOMIZE: Add or remove steps to match your CI pipeline.
fn ci() -> Result<()> {
    println!("==> [1/14] cargo fmt --check");
    run_cargo(&["fmt", "--all", "--", "--check"]).context("fmt failed — run `cargo fmt` to fix")?;

    println!("==> [2/14] cargo xtask check-architecture");
    architecture::check(std::path::Path::new(".")).context("architecture check failed")?;

    println!("==> [3/14] cargo clippy");
    run_cargo(&["clippy", "--all-targets", "--", "-D", "warnings"]).context("clippy failed")?;

    println!("==> [4/14] cargo nextest run --profile ci");
    // Falls back to cargo test if nextest isn't installed.
    // CUSTOMIZE: Remove the fallback once nextest is in your CI environment.
    if command_exists("cargo-nextest") {
        run_cargo(&["nextest", "run", "--profile", "ci"]).context("nextest failed")?;
    } else {
        eprintln!("  (nextest not installed — falling back to cargo test)");
        run_cargo(&["test"]).context("cargo test failed")?;
    }

    println!("==> [5/14] taplo check");
    // CUSTOMIZE: Remove this step if you don't use taplo.
    if command_exists("taplo") {
        run_cmd("taplo", &["check"]).context("taplo check failed — run `taplo format` to fix")?;
    } else {
        eprintln!("  (taplo not installed — skipping TOML format check)");
    }

    println!("==> [6/14] cargo xtask patterns");
    patterns::run(patterns::PatternOptions::default())
        .context("PATTERNS.md contract check failed")?;

    println!("==> [7/14] cargo xtask check-test-siblings");
    repo_checks::check_test_siblings().context("test sibling check failed")?;

    println!("==> [8/14] cargo xtask check-docs");
    check_docs().context("generated docs check failed")?;

    println!("==> [9/14] cargo xtask check-stale-claims");
    check_stale_claims().context("stale claim check failed")?;

    println!("==> [10/14] cargo xtask check-mcp-registry");
    mcp_registry::check_default(std::path::Path::new("."))
        .context("MCP registry manifest check failed")?;

    println!("==> [11/14] cargo xtask check-provider-manifest-contract");
    provider_manifest::check().context("provider manifest contract check failed")?;

    println!("==> [12/14] cargo xtask check-palette-manifest --check");
    generated_surfaces::check_palette_manifest(&["--check".to_owned()])
        .context("Palette manifest check failed")?;

    println!("==> [13/14] cargo xtask check-web-source-sync");
    web_source::check().context("web source bundle drifted from apps/web")?;

    println!("==> [14/14] cargo audit");
    // CUSTOMIZE: Remove if you don't want advisory audits in local CI.
    if command_exists("cargo-audit") {
        run_cargo(&["audit"]).context("cargo audit found vulnerabilities")?;
    } else {
        eprintln!(
            "  (cargo-audit not installed — skipping; install with `cargo install cargo-audit`)"
        );
    }

    println!("==> All CI checks passed!");
    Ok(())
}

// =============================================================================
// patterns — Check docs/PATTERNS.md contracts
// =============================================================================

fn patterns_cmd(args: &[String]) -> Result<()> {
    let mut options = patterns::PatternOptions::default();
    for arg in args {
        match arg.as_str() {
            "--strict" => options.strict = true,
            "--json" => options.json = true,
            "--help" | "-h" => {
                println!("Usage: cargo xtask patterns [--strict] [--json]");
                return Ok(());
            }
            unknown => bail!("Unknown patterns option: {unknown}"),
        }
    }
    patterns::run(options)
}

// =============================================================================
// Helpers
// =============================================================================

/// Run a `cargo` subcommand, forwarding stdout/stderr.
pub(crate) fn run_cargo(args: &[&str]) -> Result<()> {
    run_cmd("cargo", args)
}

/// Run an arbitrary command, forwarding stdout/stderr. Fails if exit code != 0.
pub(crate) fn run_cmd(program: &str, args: &[&str]) -> Result<()> {
    let status = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .status()
        .with_context(|| format!("Failed to spawn `{program}`"))?;
    if !status.success() {
        bail!("`{program} {}` exited with status {status}", args.join(" "));
    }
    Ok(())
}

/// Run an arbitrary command and return stdout. Fails if exit code != 0.
pub(crate) fn run_cmd_output(program: &str, args: &[&str]) -> Result<String> {
    let output = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .with_context(|| format!("Failed to spawn `{program}`"))?;
    if !output.status.success() {
        bail!(
            "`{program} {}` exited with status {}",
            args.join(" "),
            output.status
        );
    }
    String::from_utf8(output.stdout)
        .with_context(|| format!("`{program}` emitted non-UTF-8 stdout"))
}

/// Check whether a cargo subcommand (or standalone binary) is installed.
///
/// Checks for both `cargo-nextest` (cargo subcommand) and `nextest` in PATH.
pub(crate) fn command_exists(name: &str) -> bool {
    Command::new(name)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

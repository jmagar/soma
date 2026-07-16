//! `cargo xtask check-ts-client [--write|--check]` - keeps
//! `crates/shared/codex-app-server-client/clients/typescript/src/generated/openapi-types.ts`
//! (a checked-in, `openapi-typescript`-generated TypeScript client for that
//! crate's `rest` feature - see that directory's own README.md) in sync with
//! the crate's checked-in `openapi.json`, and (in `--check` mode) verifies
//! the package still type-checks.
//!
//! This is the TypeScript-side analogue of [`super::scripts_lane_d::check_openapi`]
//! (which keeps `docs/generated/openapi.json` in sync with the main `soma`
//! binary's own OpenAPI surface) - same `--write` regenerates /
//! `--check` verifies shape, different source of truth and a different
//! generator (`openapi-typescript`'s JS API, not a Rust builder function).
//!
//! # Why this shells out to `pnpm`, not a Rust HTTP-schema crate
//!
//! The TypeScript client's whole point (see bead `rmcp-template-g0qf.5`) is
//! proving `openapi.json` is consumable by a real, independent TypeScript
//! toolchain - reimplementing `openapi-typescript`'s logic in Rust here would
//! defeat that. `xtask` itself has no Node/pnpm dependency; this module only
//! *drives* the package's own `package.json` scripts
//! (`clients/typescript/scripts/generate.mjs` and `.../check-sync` /
//! `.../typecheck`), the same way a human contributor would.
//!
//! # Why a graceful skip, not a hard failure, when `node`/`pnpm` are missing
//!
//! Mirrors [`super::codex_schema::drift`]'s posture on a missing `codex` CLI:
//! this repo's self-hosted CI runners are not guaranteed to have a
//! Node/pnpm toolchain provisioned (see docs/CI.md), and a drift check that
//! hard-fails whenever the runner's toolchain lags reads as CI flakiness, not
//! a real problem with this PR's diff. A skip is loud (printed, not
//! swallowed) and still exits `0`, so the check is "yes if we can tell,
//! silent-not-lying if we can't" rather than a false failure.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};

const TS_CLIENT_DIR: &str = "crates/shared/codex-app-server-client/clients/typescript";

const USAGE: &str = "Usage: cargo xtask check-ts-client [--write|--check]

  --write  Regenerate src/generated/openapi-types.ts from ../../openapi.json
           (via `pnpm run generate`) and overwrite the checked-in file.
  --check  (default) Verify src/generated/openapi-types.ts is byte-identical
           to what `pnpm run generate` would produce, then run `pnpm run
           typecheck` (`tsc --noEmit`) over the whole package.

Requires `node` and `pnpm` on PATH. If either is missing, this prints a skip
message and exits 0 rather than failing - see xtask/src/ts_client.rs's module
docs for why.";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mode {
    Check,
    Write,
}

pub fn run(args: &[String]) -> Result<()> {
    let mode = parse_mode(args)?;
    let root = current_dir()?;
    let package_dir = root.join(TS_CLIENT_DIR);
    if !package_dir.join("package.json").is_file() {
        bail!(
            "{} is missing package.json - expected the checked-in TypeScript REST client \
             package (see crates/shared/codex-app-server-client/clients/typescript/README.md)",
            package_dir.display()
        );
    }

    if let Some(reason) = missing_toolchain_reason() {
        println!(
            "check-ts-client: skipped: {reason}. Cannot verify \
             {TS_CLIENT_DIR}/src/generated/openapi-types.ts is in sync with openapi.json. \
             Install Node.js and pnpm (e.g. via mise) to run this check locally or in CI."
        );
        return Ok(());
    }

    run_pnpm(&package_dir, &["install", "--frozen-lockfile"]).context(
        "`pnpm install --frozen-lockfile` failed for the TypeScript REST client package",
    )?;

    match mode {
        Mode::Write => {
            run_pnpm(&package_dir, &["run", "generate"]).context("`pnpm run generate` failed")?;
            println!("check-ts-client: wrote {TS_CLIENT_DIR}/src/generated/openapi-types.ts");
        }
        Mode::Check => {
            run_pnpm(&package_dir, &["run", "check-sync"]).context(
                "TypeScript REST client types are out of sync with openapi.json - regenerate \
                 with `cargo xtask check-ts-client --write` and commit the diff",
            )?;
            run_pnpm(&package_dir, &["run", "typecheck"])
                .context("TypeScript REST client failed to typecheck (`pnpm run typecheck`)")?;
            println!(
                "check-ts-client: {TS_CLIENT_DIR} is in sync with openapi.json and type-checks"
            );
        }
    }
    Ok(())
}

fn parse_mode(args: &[String]) -> Result<Mode> {
    let mut write = false;
    let mut check = false;
    for arg in args {
        match arg.as_str() {
            "--write" => write = true,
            "--check" => check = true,
            "--help" | "-h" => {
                println!("{USAGE}");
                std::process::exit(0);
            }
            unknown => bail!("unknown option: {unknown}\n\n{USAGE}"),
        }
    }
    if write && check {
        bail!("--write and --check are mutually exclusive\n\n{USAGE}");
    }
    Ok(if write { Mode::Write } else { Mode::Check })
}

/// `None` when both `node` and `pnpm` are usable; otherwise a human-readable
/// reason naming the first missing tool.
fn missing_toolchain_reason() -> Option<String> {
    for bin in ["node", "pnpm"] {
        if !crate::command_exists(bin) {
            return Some(format!("`{bin}` not found on PATH"));
        }
    }
    None
}

fn run_pnpm(package_dir: &Path, args: &[&str]) -> Result<()> {
    let status = Command::new("pnpm")
        .args(args)
        .current_dir(package_dir)
        .status()
        .with_context(|| format!("failed to run `pnpm {}`", args.join(" ")))?;
    if !status.success() {
        bail!("`pnpm {}` exited with status {status}", args.join(" "));
    }
    Ok(())
}

fn current_dir() -> Result<PathBuf> {
    std::env::current_dir().context("failed to read current directory")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_mode_defaults_to_check() {
        assert_eq!(parse_mode(&[]).unwrap(), Mode::Check);
    }

    #[test]
    fn parse_mode_accepts_check_flag() {
        assert_eq!(parse_mode(&["--check".to_owned()]).unwrap(), Mode::Check);
    }

    #[test]
    fn parse_mode_accepts_write_flag() {
        assert_eq!(parse_mode(&["--write".to_owned()]).unwrap(), Mode::Write);
    }

    #[test]
    fn parse_mode_rejects_both_flags() {
        let error = parse_mode(&["--write".to_owned(), "--check".to_owned()]).unwrap_err();
        assert!(error.to_string().contains("mutually exclusive"));
    }

    #[test]
    fn parse_mode_rejects_unknown_flag() {
        let error = parse_mode(&["--bogus".to_owned()]).unwrap_err();
        assert!(error.to_string().contains("unknown option"));
    }

    #[test]
    fn ts_client_dir_points_at_the_checked_in_package() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("xtask/Cargo.toml must have a parent directory");
        assert!(
            root.join(TS_CLIENT_DIR).join("package.json").is_file(),
            "{TS_CLIENT_DIR}/package.json must exist"
        );
        assert!(
            root.join(TS_CLIENT_DIR)
                .join("src/generated/openapi-types.ts")
                .is_file(),
            "{TS_CLIENT_DIR}/src/generated/openapi-types.ts must exist (checked in)"
        );
    }
}

//! `cargo xtask codex-schema <subcommand>` — Rust port of the vendored
//! `codex-app-server-client` crate's former `schema/build_combined_schema.py`,
//! plus staleness stamping and typify-panic bisection tooling.
//!
//! Subcommands:
//!   regen   Rebuild schema/protocol.schema.json + schema/methods.json from a
//!           fresh `codex app-server generate-json-schema` dump, and stamp the
//!           `codex` version the vendored schema was generated from.
//!   bisect  Binary-search a fresh schema dump for the minimal definition(s)
//!           that panic typify's schema-merge logic, the same failure mode
//!           documented for `McpServerElicitationRequestParams` in the
//!           crate's README.
//!
//! See `crates/codex-app-server-client/README.md`'s "Regenerating the
//! schema" section for the end-to-end workflow this drives.

mod bisect;
mod merge;
mod naming;
mod regen;
mod typify_probe;

use anyhow::{bail, Context, Result};
use std::path::Path;

/// Workspace-relative path to the vendored combined protocol schema.
pub(crate) const PROTOCOL_SCHEMA_PATH: &str =
    "crates/codex-app-server-client/schema/protocol.schema.json";
/// Workspace-relative path to the vendored per-method manifest.
pub(crate) const METHODS_JSON_PATH: &str = "crates/codex-app-server-client/schema/methods.json";
/// Workspace-relative path to the staleness-tracking codex version stamp
/// (see `regen::stamp_codex_version` and `build.rs`'s staleness check).
pub(crate) const CODEX_VERSION_PATH: &str =
    "crates/codex-app-server-client/schema/CODEX_VERSION.txt";

pub fn run(args: &[String]) -> Result<()> {
    match args.first().map(String::as_str) {
        Some("regen") => regen::run(&args[1..]),
        Some("bisect") => bisect::run(&args[1..]),
        Some("--help") | Some("-h") | None => {
            print_help();
            Ok(())
        }
        Some(unknown) => bail!(
            "Unknown codex-schema subcommand: {unknown:?}\nRun `cargo xtask codex-schema --help` for usage."
        ),
    }
}

fn print_help() {
    eprintln!(
        "cargo xtask codex-schema — codex-app-server-client schema tooling

USAGE:
  cargo xtask codex-schema <subcommand> <path-to-codex-generate-json-schema-output-dir>

SUBCOMMANDS:
  regen <dir>   Rebuild schema/protocol.schema.json + schema/methods.json from
                a `codex app-server generate-json-schema --out <dir> --experimental`
                dump, and stamp schema/CODEX_VERSION.txt with `codex --version`.
  bisect <dir>  Binary-search a fresh schema dump for the definition(s) that
                panic typify's schema-merge logic (see README.md).
  help          Show this help

Typical workflow after upgrading the installed `codex` CLI:
  codex app-server generate-json-schema --out /tmp/codex-schema --experimental
  cargo xtask codex-schema regen /tmp/codex-schema
  cargo build -p codex-app-server-client --all-targets
  cargo clippy -p codex-app-server-client --all-targets -- -D warnings
  cargo test -p codex-app-server-client

If that panics inside typify, bisect it:
  cargo xtask codex-schema bisect /tmp/codex-schema"
    );
}

/// Reads and parses a JSON file, with file-path context on failure.
pub(crate) fn read_json(path: &Path) -> Result<serde_json::Value> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    serde_json::from_str(&text)
        .with_context(|| format!("failed to parse {} as JSON", path.display()))
}

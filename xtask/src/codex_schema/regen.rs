//! `cargo xtask codex-schema regen <dir>` - rebuilds the vendored schema
//! assets from a fresh `codex app-server generate-json-schema` dump, and
//! stamps the `codex` version they were generated from for staleness
//! detection (see `crates/codex-app-server-client/build.rs`).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};
use serde_json::Value;

use super::{merge, read_json, CODEX_VERSION_PATH, METHODS_JSON_PATH, PROTOCOL_SCHEMA_PATH};

const MASTER_BUNDLE_FILE: &str = "codex_app_server_protocol.schemas.json";
const V2_BUNDLE_FILE: &str = "codex_app_server_protocol.v2.schemas.json";

pub fn run(args: &[String]) -> Result<()> {
    let gen_dir = parse_args(args)?;
    regen(&gen_dir)
}

fn parse_args(args: &[String]) -> Result<PathBuf> {
    let mut gen_dir = None;
    for arg in args {
        match arg.as_str() {
            "--help" | "-h" => {
                println!(
                    "Usage: cargo xtask codex-schema regen <path-to-codex-generate-json-schema-output-dir>"
                );
                std::process::exit(0);
            }
            other if gen_dir.is_none() => gen_dir = Some(PathBuf::from(other)),
            other => bail!("unexpected argument: {other}"),
        }
    }
    gen_dir.context(
        "Usage: cargo xtask codex-schema regen <path-to-codex-generate-json-schema-output-dir>\n\
         Generate that directory first with:\n  \
         codex app-server generate-json-schema --out <dir> --experimental",
    )
}

pub fn regen(gen_dir: &Path) -> Result<()> {
    let master = read_json(&gen_dir.join(MASTER_BUNDLE_FILE))?;
    let v2 = read_json(&gen_dir.join(V2_BUNDLE_FILE))?;

    let combined = merge::build_combined(&master, &v2)?;
    let combined_defs = combined
        .get("definitions")
        .and_then(Value::as_object)
        .context("combined schema missing \"definitions\"")?;

    let protocol_text =
        serde_json::to_string_pretty(&combined).context("serialize combined schema")?;
    assert_no_v2_refs(&protocol_text)?;

    let manifest = merge::build_methods_manifest(combined_defs)?;
    let methods_value = serde_json::to_value(&manifest).context("serialize methods manifest")?;
    let methods_text =
        serde_json::to_string_pretty(&methods_value).context("serialize methods manifest")?;

    fs::write(PROTOCOL_SCHEMA_PATH, &protocol_text)
        .with_context(|| format!("write {PROTOCOL_SCHEMA_PATH}"))?;
    fs::write(METHODS_JSON_PATH, &methods_text)
        .with_context(|| format!("write {METHODS_JSON_PATH}"))?;

    eprintln!("total definitions: {}", combined_defs.len());
    eprintln!("wrote {PROTOCOL_SCHEMA_PATH}");

    let missing_response: Vec<&str> = manifest
        .client_requests
        .iter()
        .chain(&manifest.server_requests)
        .filter(|e| e.response_type.is_none())
        .map(|e| e.method.as_str())
        .collect();
    eprintln!(
        "client_requests={} server_requests={} server_notifications={} client_notifications={}",
        manifest.client_requests.len(),
        manifest.server_requests.len(),
        manifest.server_notifications.len(),
        manifest.client_notifications.len(),
    );
    eprintln!("methods with no resolvable response type: {missing_response:?}");
    eprintln!("wrote {METHODS_JSON_PATH}");

    stamp_codex_version()?;

    Ok(())
}

/// Mirrors the Python original's final sanity assertion: every
/// `#/definitions/v2/X` ref must have been rewritten to `#/definitions/X`
/// before writing - a leftover match means the ref-rewrite pass missed
/// something and the output would fail to resolve.
fn assert_no_v2_refs(serialized_protocol_schema: &str) -> Result<()> {
    let count = serialized_protocol_schema
        .matches("#/definitions/v2/")
        .count();
    eprintln!("remaining v2-prefixed refs (must be 0): {count}");
    if count != 0 {
        bail!("ref rewrite incomplete: found {count} remaining \"#/definitions/v2/\" ref(s)");
    }
    Ok(())
}

fn stamp_codex_version() -> Result<()> {
    let version = capture_codex_version()?;
    fs::write(CODEX_VERSION_PATH, format!("{version}\n"))
        .with_context(|| format!("write {CODEX_VERSION_PATH}"))?;
    eprintln!("stamped {CODEX_VERSION_PATH}: {version:?}");
    Ok(())
}

fn capture_codex_version() -> Result<String> {
    let output = Command::new("codex")
        .arg("--version")
        .output()
        .context("failed to run `codex --version` - is the codex CLI installed and on PATH?")?;
    if !output.status.success() {
        bail!(
            "`codex --version` exited with status {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let raw =
        String::from_utf8(output.stdout).context("`codex --version` emitted non-UTF-8 output")?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        bail!("`codex --version` produced no output");
    }
    Ok(trimmed.to_string())
}

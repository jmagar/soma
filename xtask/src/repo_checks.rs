use anyhow::{bail, Context, Result};
use std::path::PathBuf;
use walkdir::WalkDir;

pub(crate) fn check_test_siblings() -> Result<()> {
    const EXEMPT: &[&str] = &["main.rs", "lib.rs"];
    const ORPHAN_EXEMPT: &[&str] = &["cli_tests.rs", "mcp_tests.rs"];

    let mut missing = Vec::new();
    let mut orphans = Vec::new();

    for root in crate_src_roots() {
        collect_missing_siblings(&root, EXEMPT, &mut missing);
        collect_orphan_tests(&root, ORPHAN_EXEMPT, &mut orphans);
    }

    if missing.is_empty() && orphans.is_empty() {
        println!("==> check-test-siblings: all source files have a _tests.rs sibling");
        return Ok(());
    }
    print_sibling_failures(&missing, &orphans);
    bail!("{} missing, {} orphaned", missing.len(), orphans.len())
}

pub(crate) fn symlink_docs() -> Result<()> {
    let mut created = 0usize;
    let mut skipped = 0usize;

    for entry in WalkDir::new(".")
        .into_iter()
        .filter_entry(|entry| {
            let name = entry.file_name().to_string_lossy();
            !matches!(name.as_ref(), ".git" | "target")
        })
        .filter_map(|entry| entry.ok())
    {
        if entry.file_name() != "CLAUDE.md" {
            continue;
        }
        let dir = entry
            .path()
            .parent()
            .expect("CLAUDE.md must be inside a directory");
        for link_name in ["AGENTS.md", "GEMINI.md"] {
            let link_path = dir.join(link_name);
            if link_path.exists() || link_path.symlink_metadata().is_ok() {
                println!("  skip  {}", link_path.display());
                skipped += 1;
                continue;
            }
            symlink_claude(&link_path)?;
            println!("  link  {} -> CLAUDE.md", link_path.display());
            created += 1;
        }
    }

    println!("==> symlink-docs: {created} created, {skipped} already present");
    Ok(())
}

pub(crate) fn check_env() -> Result<()> {
    const REQUIRED_VARS: &[(&str, &str)] = &[];
    const OPTIONAL_VARS: &[(&str, &str)] = &[
        (
            "SOMA_MCP_TOKEN",
            "Static bearer token for /mcp (required in production; omit only in loopback dev mode)",
        ),
        (
            "SOMA_MCP_HOST",
            "Bind host (default 127.0.0.1; set to 0.0.0.0 only with auth or trusted gateway)",
        ),
        ("SOMA_MCP_PORT", "Bind port (default 40060)"),
        (
            "RUST_LOG",
            "Log filter (for example info,rmcp=warn; default varies by mode)",
        ),
    ];

    let mut missing = Vec::new();
    println!("==> Checking required environment variables:");
    for &(var, desc) in REQUIRED_VARS {
        match std::env::var(var) {
            Ok(value) if !value.is_empty() => println!("  OK  {var}"),
            _ => {
                println!("  MISSING  {var}");
                println!("           {desc}");
                missing.push(var);
            }
        }
    }

    println!("\n==> Optional variables (missing = feature degraded, not error):");
    for &(var, desc) in OPTIONAL_VARS {
        match std::env::var(var) {
            Ok(value) if !value.is_empty() => println!("  set      {var} = {value}"),
            _ => println!("  unset    {var}  ({desc})"),
        }
    }

    if missing.is_empty() {
        println!("\n==> All required environment variables are set.");
        return Ok(());
    }
    bail!(
        "\nMissing required environment variables: {}\nCopy .env.example to .env and fill in the values.",
        missing.join(", ")
    )
}

fn collect_missing_siblings(root: &PathBuf, exempt: &[&str], missing: &mut Vec<PathBuf>) {
    for path in rust_files(root) {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        if name.ends_with("_tests.rs") || exempt.contains(&name) {
            continue;
        }
        let stem = name.strip_suffix(".rs").expect("rust file has .rs suffix");
        if !path
            .parent()
            .unwrap()
            .join(format!("{stem}_tests.rs"))
            .exists()
        {
            missing.push(path);
        }
    }
}

fn collect_orphan_tests(root: &PathBuf, exempt: &[&str], orphans: &mut Vec<PathBuf>) {
    for path in rust_files(root) {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        if !name.ends_with("_tests.rs") || exempt.contains(&name) {
            continue;
        }
        let stem = name.strip_suffix("_tests.rs").unwrap();
        if !path.parent().unwrap().join(format!("{stem}.rs")).exists() {
            orphans.push(path);
        }
    }
}

fn rust_files(root: &PathBuf) -> Vec<PathBuf> {
    WalkDir::new(root)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_file())
        .map(|entry| entry.into_path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("rs"))
        .collect()
}

fn print_sibling_failures(missing: &[PathBuf], orphans: &[PathBuf]) {
    if !missing.is_empty() {
        println!(
            "==> check-test-siblings: missing _tests.rs siblings ({}):",
            missing.len()
        );
        for path in missing {
            let stem = path.file_stem().unwrap().to_string_lossy();
            println!(
                "  MISSING  {}  (expected {}_tests.rs)",
                path.display(),
                stem
            );
        }
    }
    if !orphans.is_empty() {
        println!(
            "==> check-test-siblings: orphaned _tests.rs files ({}):",
            orphans.len()
        );
        for path in orphans {
            println!("  ORPHAN   {}  (no matching source file)", path.display());
        }
    }
}

fn crate_src_roots() -> Vec<PathBuf> {
    [
        "apps/soma/src",
        "crates/soma/api/src",
        "crates/soma/cli/src",
        "crates/soma/contracts/src",
        "crates/shared/codemode/src",
        "crates/shared/mcp/client/src",
        "crates/shared/mcp/gateway/src",
        "crates/shared/mcp/proxy/src",
        "crates/shared/mcp/server/src",
        "crates/soma/mcp/src",
        "crates/shared/observability/src",
        "crates/shared/openapi/src",
        "crates/soma/runtime/src",
        "crates/soma/service/src",
        "crates/soma/web/src",
    ]
    .into_iter()
    .map(PathBuf::from)
    .collect()
}

fn symlink_claude(link_path: &std::path::Path) -> Result<()> {
    #[cfg(unix)]
    std::os::unix::fs::symlink("CLAUDE.md", link_path)
        .with_context(|| format!("Failed to create symlink at {}", link_path.display()))?;

    #[cfg(windows)]
    std::os::windows::fs::symlink_file("CLAUDE.md", link_path).with_context(|| {
        format!(
            "Failed to create symlink at {} (may need developer mode on Windows)",
            link_path.display()
        )
    })?;

    Ok(())
}

#[cfg(test)]
#[path = "repo_checks_tests.rs"]
mod tests;

//! Rust implementations for small scripts that still have compatibility wrappers
//! in `scripts/`.

use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::{run_cargo, run_cmd_output};

pub fn block_env_commits() -> Result<()> {
    let staged = git_output(&["diff", "--cached", "--name-only"])?;
    let blocked: Vec<&str> = staged
        .lines()
        .filter(|path| is_blocked_env_path(path))
        .collect();

    if blocked.is_empty() {
        return Ok(());
    }

    eprintln!("block-env-commits: BLOCKED - .env file(s) staged for commit:");
    for path in &blocked {
        eprintln!("  {path}");
    }
    eprintln!();
    eprintln!("Only .env.example is allowed to be committed.");
    eprintln!("Remove the staged file(s) with: git restore --staged <file>");
    eprintln!("Then add them to .gitignore if they aren't already.");
    bail!(".env file(s) staged for commit")
}

pub fn check_coupled_files(args: &[String]) -> Result<()> {
    if matches!(args.first().map(String::as_str), Some("--help" | "-h")) {
        println!("Usage: cargo xtask check-coupled-files [BASE] [HEAD]");
        return Ok(());
    }
    if args.len() > 2 {
        bail!("Usage: cargo xtask check-coupled-files [BASE] [HEAD]");
    }

    let mut base = args.first().map(String::as_str).unwrap_or("origin/main");
    let head = args.get(1).map(String::as_str).unwrap_or("HEAD");
    if !git_ref_exists(base) {
        base = "HEAD~1";
    }

    let changed = git_output(&["diff", "--name-only", base, head])?;
    let changed: Vec<&str> = changed.lines().collect();
    let mut issues = Vec::new();

    if changed_path(&changed, "Justfile") && !changed_path(&changed, "lefthook.yml") {
        issues.push("Justfile changed but lefthook.yml did not; confirm hook/recipe parity.");
    }
    if changed_path(&changed, "lefthook.yml") && !changed_path(&changed, "Justfile") {
        issues.push(
            "lefthook.yml changed but Justfile did not; confirm matching manual recipe exists.",
        );
    }
    if changed_path(&changed, "scripts/*") && !changed_path(&changed, "scripts/README.md") {
        issues.push("scripts changed but scripts/README.md did not; document new or changed script behavior.");
    }
    if changed_path(&changed, "crates/rtemplate-mcp/src/schemas.rs")
        && !changed_path(&changed, "docs/MCP_SCHEMA.md")
    {
        issues.push("crates/rtemplate-mcp/src/schemas.rs changed but docs/MCP_SCHEMA.md did not; run scripts/check-schema-docs.py --write.");
    }
    if changed_path(&changed, "plugins/rtemplate/*") && !changed_path(&changed, "docs/PLUGINS.md") {
        issues.push("plugin package changed but docs/PLUGINS.md did not; confirm plugin docs are still current.");
    }

    if !issues.is_empty() {
        eprintln!("Coupled-file check failed:");
        for issue in &issues {
            eprintln!("  - {issue}");
        }
        bail!("coupled-file check failed");
    }

    println!("Coupled-file check passed ({base}..{head}).");
    Ok(())
}

pub fn sync_cargo() -> Result<()> {
    let repo_root = env_path("CLAUDE_PLUGIN_ROOT").unwrap_or_else(current_dir);
    let data_root = env_path("CLAUDE_PLUGIN_DATA").unwrap_or_else(|| repo_root.clone());
    let src_lock = repo_root.join("Cargo.lock");
    let dst_lock = data_root.join("Cargo.lock");

    if !src_lock.is_file() {
        bail!("sync-cargo.sh: missing lockfile at {}", src_lock.display());
    }

    if same_file_bytes(&src_lock, &dst_lock)? {
        return Ok(());
    }

    std::fs::create_dir_all(&data_root)
        .with_context(|| format!("failed to create {}", data_root.display()))?;

    if let Err(copy_error) = std::fs::copy(&src_lock, &dst_lock) {
        eprintln!(
            "sync-cargo: failed to copy {} to {}: {copy_error}; falling back to cargo fetch",
            src_lock.display(),
            dst_lock.display()
        );
        if let Err(fetch_error) = run_cargo_fetch(&repo_root) {
            let _ = std::fs::remove_file(&dst_lock);
            return Err(fetch_error);
        }
    }

    Ok(())
}

fn current_dir() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

fn env_path(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

fn git_output(args: &[&str]) -> Result<String> {
    run_cmd_output("git", args)
}

fn git_ref_exists(ref_name: &str) -> bool {
    Command::new("git")
        .args(["rev-parse", "--verify", ref_name])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn is_blocked_env_path(path: &str) -> bool {
    if path.ends_with(".env.example") {
        return false;
    }
    path.rsplit('/')
        .next()
        .is_some_and(|name| name.contains(".env"))
}

fn changed_path(paths: &[&str], pattern: &str) -> bool {
    paths.iter().any(|path| glob_match(pattern, path))
}

fn glob_match(pattern: &str, path: &str) -> bool {
    if let Some((prefix, suffix)) = pattern.split_once('*') {
        path.starts_with(prefix) && path.ends_with(suffix)
    } else {
        path == pattern
    }
}

fn same_file_bytes(left: &Path, right: &Path) -> Result<bool> {
    if !right.exists() {
        return Ok(false);
    }
    let left = std::fs::read(left).with_context(|| format!("failed to read {}", left.display()))?;
    let right =
        std::fs::read(right).with_context(|| format!("failed to read {}", right.display()))?;
    Ok(left == right)
}

fn run_cargo_fetch(repo_root: &Path) -> Result<()> {
    let manifest = repo_root.join("Cargo.toml");
    let manifest = manifest
        .to_str()
        .with_context(|| format!("non-UTF-8 manifest path: {}", manifest.display()))?;
    run_cargo(&["fetch", "--manifest-path", manifest])
}

#[cfg(test)]
mod tests {
    use super::{changed_path, glob_match, is_blocked_env_path};

    #[test]
    fn blocks_env_files_except_examples() {
        assert!(is_blocked_env_path(".env"));
        assert!(is_blocked_env_path("config/.env.local"));
        assert!(is_blocked_env_path("services/foo.env.prod"));
        assert!(!is_blocked_env_path(".env.example"));
        assert!(!is_blocked_env_path("docs/env.example.md"));
    }

    #[test]
    fn matches_bash_style_single_star_patterns_used_by_coupled_check() {
        assert!(glob_match("scripts/*", "scripts/check-coupled-files.sh"));
        assert!(glob_match(
            "plugins/rtemplate/*",
            "plugins/rtemplate/hooks/setup.sh"
        ));
        assert!(glob_match("Justfile", "Justfile"));
        assert!(!glob_match("Justfile", "docs/Justfile"));
    }

    #[test]
    fn changed_path_checks_any_changed_path() {
        let paths = ["README.md", "scripts/check-coupled-files.sh"];
        assert!(changed_path(&paths, "scripts/*"));
        assert!(!changed_path(&paths, "lefthook.yml"));
    }
}

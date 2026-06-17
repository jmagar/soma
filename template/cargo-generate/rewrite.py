#!/usr/bin/env python3
"""Post-process a generated rmcp-template checkout.

The source repo intentionally remains valid Rust/TOML, so we do not place
Liquid placeholders in Cargo files. cargo-generate calls this hook in its
temporary template directory before moving the generated project into place.
"""

from __future__ import annotations

import os
import shutil
import sys
from pathlib import Path


TEXT_SUFFIXES = {
    ".css",
    ".html",
    ".json",
    ".lock",
    ".md",
    ".mjs",
    ".py",
    ".rs",
    ".rhai",
    ".sh",
    ".toml",
    ".ts",
    ".tsx",
    ".txt",
    ".yml",
    ".yaml",
}

SKIP_DIRS = {
    ".git",
    ".beads",
    ".cache",
    ".dolt",
    ".full-review",
    ".lavra",
    ".next",
    ".serena",
    ".superpowers",
    ".worktrees",
    "node_modules",
    "target",
    "dist",
}

SKIP_FILES = {
    "cargo-generate.toml",
    "Cargo.lock",
}


def snake(value: str) -> str:
    return value.replace("-", "_")


def shouty_snake(value: str) -> str:
    return snake(value).upper()


def title_words(value: str) -> str:
    return value.replace("-", " ").replace("_", " ").title()


def replacements(args: list[str]) -> list[tuple[str, str]]:
    (
        crate_name,
        crate_prefix,
        binary_name,
        server_binary_name,
        service_slug,
        type_prefix,
        env_prefix,
        scope_prefix,
        default_port,
        github_owner,
        github_repo,
    ) = args

    crate_prefix_snake = snake(crate_prefix)
    mcp_surface_crate = f"{crate_prefix}-mcp-surface"
    mcp_surface_crate_snake = snake(mcp_surface_crate)
    service_slug_snake = snake(service_slug)
    binary_snake = snake(binary_name)

    return [
        ("rtemplate-server", server_binary_name),
        ("rmcp_template", snake(crate_name)),
        ("rtemplate_mcp", mcp_surface_crate_snake),
        ("rtemplate-mcp", mcp_surface_crate),
        ("rtemplate_", f"{crate_prefix_snake}_"),
        ("rtemplate-", f"{crate_prefix}-"),
        ("rtemplate", binary_name),
        ("rmcp-template", crate_name),
        ("rtemplate-mcp", crate_name),
        ("RTEMPLATE", env_prefix),
        ("ExampleRmcpServer", f"{type_prefix}RmcpServer"),
        ("ExampleService", f"{type_prefix}Service"),
        ("ExampleClient", f"{type_prefix}Client"),
        ("ExampleConfig", f"{type_prefix}Config"),
        ("ExampleAction", f"{type_prefix}Action"),
        ("example-server", server_binary_name),
        ("example_mcp_session", f"{service_slug_snake}_mcp_session"),
        ("example:read", f"{scope_prefix}:read"),
        ("example:write", f"{scope_prefix}:write"),
        ("example:__deny__", f"{scope_prefix}:__deny__"),
        ("example_mcp", f"{service_slug_snake}_mcp"),
        ("example-mcp", f"{service_slug}-mcp"),
        ("example", service_slug),
        ("Example", type_prefix),
        ("40060", default_port),
        ("40000", default_port),
        ("jmagar/rtemplate-mcp", f"{github_owner}/{github_repo}"),
        ("jmagar/rmcp-template", f"{github_owner}/{github_repo}"),
        ("github.com:jmagar/rtemplate-mcp.git", f"github.com:{github_owner}/{github_repo}.git"),
        ("github.com/jmagar/rmcp-template", f"github.com/{github_owner}/{github_repo}"),
        ("github.com/jmagar/rtemplate-mcp", f"github.com/{github_owner}/{github_repo}"),
        ("MyService", type_prefix),
        ("myservice-server", server_binary_name),
        ("myservice-mcp", crate_name),
        ("myservice", service_slug),
        ("MYSERVICE", env_prefix),
    ]


def should_skip_dir(path: Path) -> bool:
    return any(part in SKIP_DIRS for part in path.parts)


def should_rewrite(path: Path) -> bool:
    if path.name in SKIP_FILES:
        return False
    if path.suffix in TEXT_SUFFIXES:
        return True
    return path.name in {
        "Dockerfile",
        "Justfile",
        "LICENSE",
        "README",
        "CLAUDE.md",
        "AGENTS.md",
        "GEMINI.md",
    }


def rewrite_tree(root: Path, pairs: list[tuple[str, str]]) -> None:
    for path in sorted(root.rglob("*")):
        rel = path.relative_to(root)
        if should_skip_dir(rel):
            continue
        if not path.is_file() or not should_rewrite(path):
            continue
        try:
            original = path.read_text()
        except UnicodeDecodeError:
            continue
        updated = original
        for old, new in pairs:
            updated = updated.replace(old, new)
        if updated != original:
            path.write_text(updated)


def rename_paths(root: Path, crate_prefix: str, crate_name: str, binary_name: str) -> None:
    renames = []
    for path in sorted(root.rglob("*"), key=lambda p: len(p.parts), reverse=True):
        rel = path.relative_to(root)
        if should_skip_dir(rel):
            continue
        name = path.name
        new_name = name
        new_name = new_name.replace("rtemplate-mcp", f"{crate_prefix}-mcp-surface")
        new_name = new_name.replace("rtemplate", crate_prefix)
        new_name = new_name.replace("rmcp-template", crate_name)
        new_name = new_name.replace("example", binary_name)
        if new_name != name:
            renames.append((path, path.with_name(new_name)))
    for src, dst in renames:
        if src.exists() and not dst.exists():
            src.rename(dst)


def cleanup_template_files(root: Path) -> None:
    for path in [root / "cargo-generate.toml", root / "template"]:
        if path.is_dir():
            shutil.rmtree(path)
        elif path.exists():
            path.unlink()


def main() -> int:
    if len(sys.argv) != 12:
        print("expected 11 cargo-generate arguments", file=sys.stderr)
        return 2
    root = Path(os.getcwd())
    args = sys.argv[1:]
    pairs = replacements(args)
    rewrite_tree(root, pairs)
    rename_paths(root, crate_prefix=args[1], crate_name=args[0], binary_name=args[2])
    cleanup_template_files(root)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

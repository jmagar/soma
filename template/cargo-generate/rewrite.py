#!/usr/bin/env python3
"""Post-process a generated rmcp-template checkout.

The source repo intentionally remains valid Rust/TOML, so we do not place
Liquid placeholders in Cargo files. cargo-generate calls this hook in its
temporary template directory before moving the generated project into place.
"""

from __future__ import annotations

import os
import re
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


def validate_identifier(name: str, value: str) -> None:
    if not re.fullmatch(r"[a-z][a-z0-9_]*", value):
        raise ValueError(f"{name} must be Rust identifier-safe: {value!r}")


def validate_port(value: str) -> None:
    try:
        port = int(value)
    except ValueError as exc:
        raise ValueError(f"default_port must be an integer: {value!r}") from exc
    if not 1 <= port <= 65535:
        raise ValueError(f"default_port must be between 1 and 65535: {value!r}")


def validate_default_features(value: str) -> list[str]:
    allowed = {
        "cli",
        "mcp",
        "mcp-stdio",
        "api",
        "auth",
        "oauth",
        "observability",
        "plugin",
        "mcp-http",
        "web",
        "local-adapter",
        "server",
        "full",
        "test-support",
    }
    features = [feature.strip() for feature in value.split(",") if feature.strip()]
    if not features:
        raise ValueError("default_features must include at least one feature")
    unknown = [feature for feature in features if feature not in allowed]
    if unknown:
        raise ValueError(
            "default_features contains unknown feature(s): " + ", ".join(sorted(unknown))
        )
    return features


def cargo_feature_array(features: list[str]) -> str:
    return ", ".join(f'"{feature}"' for feature in features)


def parse_args(args: list[str]) -> dict[str, str]:
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
        default_features,
    ) = args

    validate_identifier("service_slug", service_slug)
    validate_port(default_port)
    features = validate_default_features(default_features)

    return {
        "crate_name": crate_name,
        "crate_name_snake": snake(crate_name),
        "crate_prefix": crate_prefix,
        "crate_prefix_snake": snake(crate_prefix),
        "binary_name": binary_name,
        "server_binary_name": server_binary_name,
        "service_slug": service_slug,
        "type_prefix": type_prefix,
        "env_prefix": env_prefix,
        "scope_prefix": scope_prefix,
        "default_port": default_port,
        "github_owner": github_owner,
        "github_repo": github_repo,
        "default_features": default_features,
        "default_feature_array": cargo_feature_array(features),
        "github_slug": f"{github_owner}/{github_repo}",
        "github_url": f"https://github.com/{github_owner}/{github_repo}",
        "github_ssh": f"github.com:{github_owner}/{github_repo}.git",
        "mcp_surface_crate": f"{crate_prefix}-mcp-surface",
        "mcp_surface_crate_snake": snake(f"{crate_prefix}-mcp-surface"),
    }


def replacements(values: dict[str, str]) -> list[tuple[str, str]]:
    crate_name = values["crate_name"]
    crate_name_snake = values["crate_name_snake"]
    crate_prefix = values["crate_prefix"]
    crate_prefix_snake = values["crate_prefix_snake"]
    binary_name = values["binary_name"]
    server_binary_name = values["server_binary_name"]
    service_slug = values["service_slug"]
    type_prefix = values["type_prefix"]
    env_prefix = values["env_prefix"]
    scope_prefix = values["scope_prefix"]
    default_port = values["default_port"]
    default_feature_array = values["default_feature_array"]
    github_slug = values["github_slug"]
    github_url = values["github_url"]
    github_ssh = values["github_ssh"]
    mcp_surface_crate = values["mcp_surface_crate"]
    mcp_surface_crate_snake = values["mcp_surface_crate_snake"]

    return [
        ("https://github.com/your-org/rtemplate-mcp", github_url),
        ("https://github.com/jmagar/rmcp-template", github_url),
        ("https://github.com/jmagar/rtemplate-mcp", github_url),
        ("github.com:jmagar/rtemplate-mcp.git", github_ssh),
        ("jmagar/rmcp-template", github_slug),
        ("jmagar/rtemplate-mcp", github_slug),
        ('"name": "rtemplate-mcp"', f'"name": "{crate_name}"'),
        ("rtemplate-server", server_binary_name),
        ('default = ["full"]', f"default = [{default_feature_array}]"),
        ("rmcp_template", crate_name_snake),
        ("rtemplate_mcp", mcp_surface_crate_snake),
        ("rtemplate-mcp", mcp_surface_crate),
        ("rtemplate_", f"{crate_prefix_snake}_"),
        ("rtemplate-", f"{crate_prefix}-"),
        ("rtemplate", binary_name),
        ("rmcp-template", crate_name),
        ("RTEMPLATE", env_prefix),
        ("ExampleRmcpServer", f"{type_prefix}RmcpServer"),
        ("ExampleService", f"{type_prefix}Service"),
        ("ExampleClient", f"{type_prefix}Client"),
        ("ExampleConfig", f"{type_prefix}Config"),
        ("ExampleAction", f"{type_prefix}Action"),
        ("example-server", server_binary_name),
        (
            "crates/rmcp-template/src/bin/example.rs",
            f"crates/{crate_name}/src/bin/{binary_name}.rs",
        ),
        ("example_mcp_session", f"{service_slug}_mcp_session"),
        ("example:read", f"{scope_prefix}:read"),
        ("example:write", f"{scope_prefix}:write"),
        ("example:__deny__", f"{scope_prefix}:__deny__"),
        ("example_mcp", f"{service_slug}_mcp"),
        ("example-mcp", f"{service_slug}-mcp"),
        ("example", service_slug),
        ("Example", type_prefix),
        ("40060", default_port),
        ("40000", default_port),
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


def rename_paths(
    root: Path, crate_prefix: str, crate_name: str, binary_name: str, service_slug: str
) -> None:
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
        new_name = new_name.replace("example", service_slug)
        if new_name != name:
            renames.append((path, path.with_name(new_name)))
    for src, dst in renames:
        if src.exists() and not dst.exists():
            src.rename(dst)


def cleanup_template_files(root: Path) -> None:
    for path in [root / "cargo-generate.toml", root / "template", root / "docs/CARGO_GENERATE.md"]:
        if path.is_dir():
            shutil.rmtree(path)
        elif path.exists():
            path.unlink()


def cleanup_generated_readme(root: Path) -> None:
    readme = root / "README.md"
    if not readme.exists():
        return
    text = readme.read_text()
    text = re.sub(
        r"\n## Generate a New Server\n.*?(?=\n## )",
        "\n",
        text,
        count=1,
        flags=re.S,
    )
    readme.write_text(text)


def main() -> int:
    if len(sys.argv) != 14:
        print("expected generated root plus 12 cargo-generate arguments", file=sys.stderr)
        return 2
    root = Path(sys.argv[1])
    try:
        values = parse_args(sys.argv[2:])
        pairs = replacements(values)
        rewrite_tree(root, pairs)
        rename_paths(
            root,
            crate_prefix=values["crate_prefix"],
            crate_name=values["crate_name"],
            binary_name=values["binary_name"],
            service_slug=values["service_slug"],
        )
        cleanup_template_files(root)
        cleanup_generated_readme(root)
    except ValueError as exc:
        print(f"cargo-generate rewrite failed: {exc}", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

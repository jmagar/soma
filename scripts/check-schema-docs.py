#!/usr/bin/env python3
"""Generate and verify MCP schema/action documentation drift."""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SCHEMAS_RS = ROOT / "src/mcp/schemas.rs"
TOOLS_RS = ROOT / "src/mcp/tools.rs"
RMCP_SERVER_RS = ROOT / "src/mcp/rmcp_server.rs"
README = ROOT / "README.md"
SKILL = ROOT / "plugins/example/skills/example/SKILL.md"
DOC = ROOT / "docs/MCP_SCHEMA.md"


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def extract_actions() -> list[str]:
    text = read(SCHEMAS_RS)
    match = re.search(r"EXAMPLE_ACTIONS:\s*&\[&str\]\s*=\s*&\[(.*?)\];", text, re.S)
    if not match:
        raise SystemExit("could not find EXAMPLE_ACTIONS in src/mcp/schemas.rs")
    return re.findall(r'"([^"]+)"', match.group(1))


def extract_read_only_actions() -> list[str]:
    text = read(RMCP_SERVER_RS)
    match = re.search(r"READ_ONLY_ACTIONS:\s*&\[&str\]\s*=\s*&\[(.*?)\];", text, re.S)
    if not match:
        raise SystemExit("could not find READ_ONLY_ACTIONS in src/mcp/rmcp_server.rs")
    return re.findall(r'"([^"]+)"', match.group(1))


def action_description(action: str) -> str:
    descriptions = {
        "greet": "Return a greeting. Optional `name` string.",
        "echo": "Echo a required `message` string.",
        "status": "Return server status and configuration summary.",
        "elicit_name": "Ask the MCP client to elicit a name and return a personalized greeting.",
        "help": "Return the in-tool action reference. Public; no scope required.",
    }
    return descriptions.get(action, "TEMPLATE: document this action.")


def render() -> str:
    actions = extract_actions()
    read_only = set(extract_read_only_actions())
    lines = [
        "# MCP Schema Contract",
        "",
        "Generated from `src/mcp/schemas.rs` and checked against README, skill docs, help text, and scope routing.",
        "",
        "Run:",
        "",
        "```bash",
        "python3 scripts/check-schema-docs.py --write",
        "python3 scripts/check-schema-docs.py --check",
        "```",
        "",
        "## Tool",
        "",
        "| Field | Value |",
        "|---|---|",
        "| Tool name | `example` |",
        "| Schema resource | `example://schema/mcp-tool` |",
        "| Dispatch parameter | `action` |",
        "",
        "## Actions",
        "",
        "| Action | Scope | Description |",
        "|---|---|---|",
    ]
    for action in actions:
        scope = "public" if action == "help" else "`example:read`" if action in read_only else "`example:__deny__`"
        lines.append(f"| `{action}` | {scope} | {action_description(action)} |")
    lines.extend(
        [
            "",
            "## Drift Rules",
            "",
            "- `EXAMPLE_ACTIONS` in `src/mcp/schemas.rs` is the canonical action list.",
            "- `READ_ONLY_ACTIONS` in `src/mcp/rmcp_server.rs` must include every scoped read action.",
            "- `help` is intentionally public and must not appear in `READ_ONLY_ACTIONS`.",
            "- `src/mcp/tools.rs`, `README.md`, and `plugins/example/skills/example/SKILL.md` must mention every action.",
            "",
        ]
    )
    return "\n".join(lines)


def check_mentions(actions: list[str]) -> list[str]:
    failures: list[str] = []
    surfaces = {
        "README.md": read(README),
        "plugins/example/skills/example/SKILL.md": read(SKILL),
        "src/mcp/tools.rs HELP_TEXT": read(TOOLS_RS),
    }
    for label, text in surfaces.items():
        for action in actions:
            if action not in text:
                failures.append(f"{label} does not mention action `{action}`")
    return failures


def check_scope(actions: list[str]) -> list[str]:
    failures: list[str] = []
    read_only = set(extract_read_only_actions())
    action_set = set(actions)
    if "help" in read_only:
        failures.append("help must be public and must not be in READ_ONLY_ACTIONS")
    for action in sorted(read_only - action_set):
        failures.append(f"READ_ONLY_ACTIONS contains unknown action `{action}`")
    for action in action_set - {"help"}:
        if action not in read_only:
            failures.append(f"action `{action}` is missing from READ_ONLY_ACTIONS")
    return failures


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--write", action="store_true", help="Rewrite docs/MCP_SCHEMA.md.")
    parser.add_argument("--check", action="store_true", help="Fail if docs or action surfaces drift.")
    args = parser.parse_args()
    if not args.write and not args.check:
        args.check = True

    rendered = render()
    if args.write:
        DOC.write_text(rendered, encoding="utf-8")
        print(f"wrote {DOC.relative_to(ROOT)}")

    failures: list[str] = []
    if args.check:
        if not DOC.exists():
            failures.append("docs/MCP_SCHEMA.md is missing; run --write")
        elif read(DOC) != rendered:
            failures.append("docs/MCP_SCHEMA.md is stale; run --write")
        actions = extract_actions()
        failures.extend(check_mentions(actions))
        failures.extend(check_scope(actions))

    if failures:
        for failure in failures:
            print(f"FAIL: {failure}", file=sys.stderr)
        return 1
    if args.check:
        print("schema docs are current")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

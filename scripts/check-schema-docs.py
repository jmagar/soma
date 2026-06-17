#!/usr/bin/env python3
"""Generate and verify MCP schema/action documentation drift."""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SCHEMAS_RS = ROOT / "src/mcp/schemas.rs"
ACTION_RS = ROOT / "src/actions.rs"
TOOLS_RS = ROOT / "src/mcp/tools.rs"
PROMPTS_RS = ROOT / "src/mcp/prompts.rs"
RMCP_SERVER_RS = ROOT / "src/mcp/rmcp_server.rs"
README = ROOT / "README.md"
SKILL = ROOT / "plugins/rtemplate/skills/rtemplate/SKILL.md"
DOC = ROOT / "docs/MCP_SCHEMA.md"


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def extract_actions() -> list[str]:
    text = read(ACTION_RS)
    actions: list[str] = []
    for entry in re.findall(r"ActionSpec\s*\{(.*?)\}", text, re.S):
        name_match = re.search(r'name:\s*"([^"]+)"', entry)
        if name_match:
            actions.append(name_match.group(1))
    return actions


def extract_scope_for_actions() -> dict[str, str]:
    text = read(ACTION_RS)
    entries = re.findall(r"ActionSpec\s*\{(.*?)\}", text, re.S)
    scopes: dict[str, str] = {}
    for entry in entries:
        name_match = re.search(r'name:\s*"([^"]+)"', entry)
        scope_match = re.search(r"required_scope:\s*([^,\n]+)", entry)
        if not name_match or not scope_match:
            continue
        name = name_match.group(1)
        scope_expr = scope_match.group(1).strip()
        if scope_expr == "None":
            scopes[name] = "public"
        elif scope_expr == "Some(READ_SCOPE)":
            scopes[name] = "`example:read`"
        elif scope_expr == "Some(WRITE_SCOPE)":
            scopes[name] = "`example:write`"
        else:
            scopes[name] = "`example:__deny__`"
    return scopes


def extract_cost_for_actions() -> dict[str, str]:
    text = read(ACTION_RS)
    entries = re.findall(r"ActionSpec\s*\{(.*?)\}", text, re.S)
    costs: dict[str, str] = {}
    for entry in entries:
        name_match = re.search(r'name:\s*"([^"]+)"', entry)
        cost_match = re.search(r"cost:\s*ActionCost::([A-Za-z]+)", entry)
        if name_match and cost_match:
            costs[name_match.group(1)] = cost_match.group(1).lower()
    return costs


def action_description(action: str) -> str:
    descriptions = {
        "greet": "Return a greeting. Optional `name` string.",
        "echo": "Echo a required `message` string.",
        "status": "Return server status and configuration summary.",
        "elicit_name": "Ask the MCP client to elicit a name and return a personalized greeting.",
        "scaffold_intent": "Elicit scaffold requirements and return JSON for the scaffold-project skill. Does not mutate files.",
        "help": "Return the in-tool action reference. Public; no scope required.",
    }
    return descriptions.get(action, "TEMPLATE: document this action.")


def render() -> str:
    actions = extract_actions()
    scopes = extract_scope_for_actions()
    costs = extract_cost_for_actions()
    lines = [
        "# MCP Schema Contract",
        "",
        "Generated from `src/actions.rs` and checked against the schema, README, skill docs, help text, and scope routing.",
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
        "| Action | Scope | Cost | Description |",
        "|---|---|---|---|",
    ]
    for action in actions:
        scope = scopes[action]
        cost = costs[action]
        lines.append(f"| `{action}` | {scope} | `{cost}` | {action_description(action)} |")
    lines.extend(
        [
            "",
            "## Drift Rules",
            "",
            "- `ACTION_SPECS` in `src/actions.rs` is the canonical action and scope list.",
            "- Action cost is planner metadata. Use `cheap` for first-pass reads, `moderate` for bounded workflow setup, `expensive` for broad scans or long-running work, and `write` for mutating operations.",
            "- `src/mcp/schemas.rs` must derive its enum from `ACTION_SPECS`.",
            "- The MCP tool schema must reject unknown top-level parameters except reserved `_response_*` continuation fields, and encode action-specific requirements that fit the single-tool dispatch model.",
            "- `help` is intentionally public and must have no required scope.",
            "- `src/mcp/tools.rs`, `README.md`, and `plugins/rtemplate/skills/example/SKILL.md` must mention every action.",
            "- `src/mcp/rmcp_server.rs` owns stable resources and must keep `example://schema/mcp-tool` wired to `tool_definitions()`.",
            "- `src/mcp/prompts.rs` owns stable prompts and must keep `quick_start` covered by prompt tests.",
            "",
            "## Resources",
            "",
            "| URI | Source | Contract |",
            "|---|---|---|",
            "| `example://schema/mcp-tool` | `src/mcp/rmcp_server.rs` | Returns `tool_definitions()` as `application/json`. |",
            "",
            "## Prompts",
            "",
            "| Prompt | Source | Contract |",
            "|---|---|---|",
            "| `quick_start` | `src/mcp/prompts.rs` | Guides a client to call `status` and `greet`. |",
            "",
            "## Input Validation",
            "",
            "- `action` is always required.",
            "- `echo` conditionally requires non-empty `message`.",
            "- `greet` accepts optional `name` and defaults to World.",
            "- `elicit_name` and `scaffold_intent` collect their extra fields through MCP elicitation, not direct tool-call arguments.",
            "- Unknown top-level parameters are rejected by the schema except reserved MCP adapter continuation fields.",
            "",
            "## Reserved Adapter Parameters",
            "",
            "Oversized MCP responses are returned as `kind=mcp_response_page` envelopes. Continuation calls reuse the same tool and original arguments, plus these reserved fields:",
            "",
            "| Parameter | Type | Purpose |",
            "|---|---|---|",
            "| `_response_cursor` | string | Cursor for cached serialized response data. Required with `_response_offset`. |",
            "| `_response_offset` | integer | Byte offset into the cached serialized response. |",
            "| `_response_page_bytes` | integer | Page size in bytes, from 1 to 16000. |",
            "",
            "The adapter strips these fields before dispatching to the service layer.",
            "",
        ]
    )
    return "\n".join(lines)


def check_mentions(actions: list[str]) -> list[str]:
    failures: list[str] = []
    surfaces = {
        "README.md": read(README),
        "plugins/rtemplate/skills/rtemplate/SKILL.md": read(SKILL),
    }
    for label, text in surfaces.items():
        for action in actions:
            if action not in text:
                failures.append(f"{label} does not mention action `{action}`")
    tools_text = read(TOOLS_RS)
    if "ACTION_SPECS" not in tools_text or "build_help_text" not in tools_text:
        failures.append("src/mcp/tools.rs HELP_TEXT must be derived from ACTION_SPECS")
    return failures


def check_scope(actions: list[str]) -> list[str]:
    failures: list[str] = []
    scopes = extract_scope_for_actions()
    costs = extract_cost_for_actions()
    if set(scopes) != set(actions):
        failures.append("ACTION_SPECS action names and scope entries are out of sync")
    if set(costs) != set(actions):
        failures.append("ACTION_SPECS action names and cost entries are out of sync")
    if scopes.get("help") != "public":
        failures.append("help must be public")
    for action in set(actions) - {"help"}:
        if scopes.get(action) == "public":
            failures.append(f"action `{action}` must declare a required scope")
    schema_text = read(SCHEMAS_RS)
    if "action_names()" not in schema_text:
        failures.append("src/mcp/schemas.rs must derive action enum from action_names()")
    if '"additionalProperties": false' not in schema_text:
        failures.append("src/mcp/schemas.rs must reject unknown top-level properties")
    if "required_param_conditionals()" not in schema_text or '"then": { "required": required }' not in schema_text:
        failures.append("src/mcp/schemas.rs must derive required action parameters from ACTION_SPECS")
    rmcp_server_text = read(RMCP_SERVER_RS)
    if "example://schema/mcp-tool" not in rmcp_server_text or "tool_definitions()" not in rmcp_server_text:
        failures.append("src/mcp/rmcp_server.rs must expose the schema resource from tool_definitions()")
    prompts_text = read(PROMPTS_RS)
    if "quick_start" not in prompts_text:
        failures.append("src/mcp/prompts.rs must expose quick_start prompt")
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

#!/usr/bin/env python3
"""Audit RMCP README files against the structural guide invariants."""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path


REQUIRED_HEADINGS = [
    "Contents",
    "Naming",
    "Capabilities And Boundaries",
    "Install",
    "Quickstart",
    "Client Configuration",
    "Runtime Surfaces",
    "MCP Tool Reference",
    "CLI Reference",
    "Configuration",
    "Authentication",
    "Safety And Trust Model",
    "Architecture",
    "Distribution Contract",
    "Development",
    "Verification",
    "Deployment",
    "Troubleshooting",
    "Related Servers",
    "Documentation",
    "License",
]

REQUIRED_PHRASES = [
    ("30-second path", "include a first-screen 30-second path"),
    ("Not for:", "state the product boundary with a Not for line"),
    ("npx -y", "show an npm/npx install or launcher path"),
    ("tools/call", "show a raw or client-equivalent MCP tools/call example"),
    ("MCP callers never provide", "forbid credentials in MCP tool arguments"),
    ("source of truth", "separate curated README content from source-of-truth docs"),
    ("Distribution Contract", "document distribution/version invariants"),
]

SECRET_ARGUMENT_WORDS = [
    "api_key",
    "apikey",
    "app_token",
    "bearer",
    "client_token",
    "oauth",
    "password",
    "secret",
    "ssh_key",
    "token",
]


def normalize_heading(value: str) -> str:
    return re.sub(r"[^a-z0-9]+", " ", value.lower()).strip()


def collect_headings(text: str) -> set[str]:
    headings: set[str] = set()
    for match in re.finditer(r"^##\s+(.+?)\s*$", text, flags=re.MULTILINE):
        heading = match.group(1).strip()
        if not heading.startswith("#"):
            headings.add(normalize_heading(heading))
    return headings


def line_number(text: str, offset: int) -> int:
    return text.count("\n", 0, offset) + 1


def check_readme(path: Path) -> list[str]:
    try:
        text = path.read_text(encoding="utf-8")
    except OSError as exc:
        return [f"could not read file: {exc}"]

    failures: list[str] = []
    headings = collect_headings(text)

    if not re.search(r"^#\s+\S+", text, flags=re.MULTILINE):
        failures.append("missing top-level H1")

    for heading in REQUIRED_HEADINGS:
        if normalize_heading(heading) not in headings:
            failures.append(f"missing heading: ## {heading}")

    lower_text = text.lower()
    for phrase, message in REQUIRED_PHRASES:
        if phrase.lower() not in lower_text:
            failures.append(message)

    if "generated" not in lower_text and "curated" not in lower_text:
        failures.append("name which docs are generated or curated")

    for match in re.finditer(r'"arguments"\s*:\s*\{(?P<body>.*?)\}', text, re.IGNORECASE | re.DOTALL):
        body = match.group("body").lower()
        found = [word for word in SECRET_ARGUMENT_WORDS if word in body]
        if found:
            words = ", ".join(sorted(set(found)))
            failures.append(
                f"line {line_number(text, match.start())}: MCP arguments appear to include secrets ({words})"
            )

    return failures


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Audit README files against docs/RMCP_README_GUIDE.md structural invariants."
    )
    parser.add_argument(
        "paths",
        nargs="*",
        default=["README.md"],
        help="README paths to audit. Defaults to README.md.",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    failed = False

    for raw_path in args.paths:
        path = Path(raw_path)
        failures = check_readme(path)
        if failures:
            failed = True
            print(f"FAIL: {path}")
            for failure in failures:
                print(f"  - {failure}")
        else:
            print(f"PASS: {path}")

    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())

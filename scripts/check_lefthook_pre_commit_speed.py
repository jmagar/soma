#!/usr/bin/env python3
"""Fail if lefthook.yml pre-commit grows workspace-scale commands."""

from __future__ import annotations

import re
import sys
from pathlib import Path

FORBIDDEN_SUBSTRINGS = (
    "cargo clippy --workspace",
    "cargo clippy --all",
    "cargo test --workspace",
    "cargo nextest run --workspace",
    "cargo build --workspace",
    "cargo check --workspace",
    "cargo test --all",
    "cargo nextest run --all",
    "cargo build --release",
    "pnpm -r test",
    "npm run test",
    "pytest",
)

EXPECTED_SENTINELS = {"diff_check", "toml_fmt", "env_guard", "file_size", "lefthook_speed"}


def parse_pre_commit_runs(yaml_text: str) -> list[tuple[str, str]]:
    lines = yaml_text.splitlines()
    out: list[tuple[str, str]] = []
    in_pre_commit = False
    in_commands = False
    current_name: str | None = None
    current_run: list[str] = []
    current_run_active = False

    def flush() -> None:
        nonlocal current_run, current_run_active
        if current_name is not None and current_run:
            out.append((current_name, "\n".join(current_run).strip()))
        current_run = []
        current_run_active = False

    for raw in lines:
        line = raw.rstrip()
        if re.match(r"^[A-Za-z][\w-]*:\s*$", line) and not line.startswith(" "):
            flush()
            in_pre_commit = line.startswith("pre-commit:")
            in_commands = False
            current_name = None
            continue

        if not in_pre_commit:
            continue

        if re.match(r"^  commands:\s*$", line):
            in_commands = True
            continue

        if not in_commands:
            continue

        match = re.match(r"^    ([A-Za-z][\w-]*):\s*$", line)
        if match:
            flush()
            current_name = match.group(1)
            continue

        match = re.match(r"^      run:\s*(.*)$", line)
        if match:
            current_run_active = True
            value = match.group(1).strip()
            if value and value not in {">", "|"}:
                current_run.append(value)
            continue

        if current_run_active and line.startswith("        "):
            current_run.append(line.strip())
            continue

        if current_run_active and re.match(r"^      [A-Za-z]", line):
            current_run_active = False

    flush()
    return out


def main(argv: list[str]) -> int:
    path = Path(argv[1]) if len(argv) > 1 else Path("lefthook.yml")
    if not path.exists():
        print(f"ERROR: {path} not found", file=sys.stderr)
        return 1

    commands = parse_pre_commit_runs(path.read_text())
    found_names = {name for name, _ in commands}
    missing = EXPECTED_SENTINELS - found_names
    if missing:
        print(
            f"ERROR: parser found {len(commands)} pre-commit command(s), "
            f"missing sentinel(s): {sorted(missing)}; found: {sorted(found_names)}",
            file=sys.stderr,
        )
        return 1

    violations: list[tuple[str, str, str]] = []
    for name, run in commands:
        normalized = re.sub(r"\s+", " ", run.lower()).strip()
        for needle in FORBIDDEN_SUBSTRINGS:
            if needle in normalized:
                violations.append((name, needle, run))
                break

    if violations:
        print(
            f"ERROR: {path}'s pre-commit stage has workspace-scale commands. "
            "Move heavy gates to pre-push or CI.",
            file=sys.stderr,
        )
        for name, needle, run in violations:
            print(f"  - {name}: matches {needle!r}", file=sys.stderr)
            print(f"      run: {run}", file=sys.stderr)
        return 1

    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))

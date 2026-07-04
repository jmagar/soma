#!/usr/bin/env python3
"""Summarize official MCP conformance checks.json files.

The upstream harness writes timestamped result directories. This helper keeps
local audits quick and CI artifacts readable without changing harness output.
"""

from __future__ import annotations

import argparse
import json
from collections import Counter, defaultdict
from pathlib import Path


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--results", default="results", help="conformance results directory")
    parser.add_argument("--json", action="store_true", help="emit machine-readable JSON")
    args = parser.parse_args()

    root = Path(args.results)
    checks_files = sorted(root.glob("**/checks.json"))
    if not checks_files:
        print(f"No checks.json files found under {root}; this conformance CLI may only emit stdout.")
        return 0

    status_counts: Counter[str] = Counter()
    scenario_counts: dict[str, Counter[str]] = defaultdict(Counter)
    failures: list[dict[str, str]] = []

    for path in checks_files:
        scenario = path.parent.name.rsplit("-", 1)[0]
        checks = json.loads(path.read_text())
        for check in checks:
            status = str(check.get("status", "UNKNOWN"))
            status_counts[status] += 1
            scenario_counts[scenario][status] += 1
            if status not in {"SUCCESS", "SKIPPED"}:
                failures.append(
                    {
                        "scenario": scenario,
                        "id": str(check.get("id", "")),
                        "status": status,
                        "message": str(check.get("errorMessage", "")),
                    }
                )

    total = sum(status_counts.values())
    success = status_counts["SUCCESS"]
    pass_rate = success / total if total else 0.0

    report = {
        "total": total,
        "pass_rate": round(pass_rate, 4),
        "status_counts": dict(sorted(status_counts.items())),
        "scenario_counts": {
            scenario: dict(sorted(counts.items()))
            for scenario, counts in sorted(scenario_counts.items())
        },
        "failures": failures,
    }

    if args.json:
        print(json.dumps(report, indent=2, sort_keys=True))
        return 0

    print(f"MCP conformance checks: {success}/{total} passed ({pass_rate:.1%})")
    for status, count in sorted(status_counts.items()):
        print(f"- {status}: {count}")
    if failures:
        print("\nFailures:")
        for failure in failures:
            message = f" - {failure['message']}" if failure["message"] else ""
            print(f"- {failure['scenario']} / {failure['id']}: {failure['status']}{message}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

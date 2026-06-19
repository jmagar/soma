#!/usr/bin/env python3
"""Compatibility wrapper. Canonical implementation: cargo xtask asciicheck."""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path


def main() -> int:
    repo = Path(__file__).resolve().parents[1]
    return subprocess.run(
        ["cargo", "xtask", "asciicheck", *sys.argv[1:]],
        cwd=repo,
        check=False,
    ).returncode


if __name__ == "__main__":
    raise SystemExit(main())

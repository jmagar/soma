#!/usr/bin/env bash
# Thin wrapper. Canonical implementation: cargo xtask check-version-sync.
set -euo pipefail

PROJECT_DIR="${1:-.}"
cd "$PROJECT_DIR"

cargo xtask check-version-sync

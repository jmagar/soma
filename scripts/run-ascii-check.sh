#!/usr/bin/env bash
# Thin wrapper. Canonical implementation: cargo xtask run-ascii-check.
set -euo pipefail

cargo xtask run-ascii-check "$@"

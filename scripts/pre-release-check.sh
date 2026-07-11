#!/usr/bin/env bash
# Thin wrapper. Canonical implementation: cargo xtask pre-release-check.
set -euo pipefail

cargo xtask pre-release-check "$@"

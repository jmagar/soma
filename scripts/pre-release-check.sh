#!/usr/bin/env bash
# Compatibility wrapper. Canonical implementation: cargo xtask pre-release-check.
set -euo pipefail

cargo xtask pre-release-check "$@"

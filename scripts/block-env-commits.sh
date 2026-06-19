#!/usr/bin/env bash
# Compatibility wrapper. Canonical implementation: cargo xtask block-env-commits.
set -euo pipefail

cargo xtask block-env-commits "$@"

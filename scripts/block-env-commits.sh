#!/usr/bin/env bash
# Thin wrapper. Canonical implementation: cargo xtask block-env-commits.
set -euo pipefail

cargo xtask block-env-commits "$@"

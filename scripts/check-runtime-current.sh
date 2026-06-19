#!/usr/bin/env bash
# Compatibility wrapper. Canonical implementation: cargo xtask check-runtime-current.
set -euo pipefail

cargo xtask check-runtime-current "$@"

#!/usr/bin/env bash
# Thin wrapper. Canonical implementation: cargo xtask check-runtime-current.
set -euo pipefail

cargo xtask check-runtime-current "$@"

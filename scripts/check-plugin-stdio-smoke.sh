#!/usr/bin/env bash
# Thin wrapper. Canonical implementation: cargo xtask check-plugin-stdio-smoke.
set -euo pipefail

cargo xtask check-plugin-stdio-smoke "$@"

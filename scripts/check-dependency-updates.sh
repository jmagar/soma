#!/usr/bin/env bash
# Compatibility wrapper. Canonical implementation: cargo xtask check-dependency-updates.
set -euo pipefail

cargo xtask check-dependency-updates "$@"

#!/usr/bin/env bash
# Thin wrapper. Canonical implementation: cargo xtask check-dependency-updates.
set -euo pipefail

cargo xtask check-dependency-updates "$@"

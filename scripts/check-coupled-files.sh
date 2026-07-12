#!/usr/bin/env bash
# Thin wrapper. Canonical implementation: cargo xtask check-coupled-files.
set -euo pipefail

cargo xtask check-coupled-files "$@"

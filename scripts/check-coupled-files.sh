#!/usr/bin/env bash
# Compatibility wrapper. Canonical implementation: cargo xtask check-coupled-files.
set -euo pipefail

cargo xtask check-coupled-files "$@"

#!/usr/bin/env bash
# Thin wrapper. Canonical implementation: cargo xtask check-file-size.
set -euo pipefail

cargo xtask check-file-size "$@"

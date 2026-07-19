#!/usr/bin/env bash
# Thin wrapper. Canonical implementation: cargo xtask test-trace-headers.
set -euo pipefail

cargo xtask test-trace-headers "$@"

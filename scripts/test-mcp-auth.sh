#!/usr/bin/env bash
# Thin wrapper. Canonical implementation: cargo xtask test-mcp-auth.
set -euo pipefail

cargo xtask test-mcp-auth "$@"

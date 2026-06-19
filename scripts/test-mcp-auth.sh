#!/usr/bin/env bash
# Compatibility wrapper. Canonical implementation: cargo xtask test-mcp-auth.
set -euo pipefail

cargo xtask test-mcp-auth "$@"

#!/usr/bin/env bash
# Compatibility wrapper. Canonical implementation: cargo xtask validate-plugin-layout.
set -euo pipefail

cargo xtask validate-plugin-layout "$@"

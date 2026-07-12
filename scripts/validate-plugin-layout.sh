#!/usr/bin/env bash
# Thin wrapper. Canonical implementation: cargo xtask validate-plugin-layout.
set -euo pipefail

cargo xtask validate-plugin-layout "$@"

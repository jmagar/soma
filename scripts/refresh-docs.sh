#!/usr/bin/env bash
# Compatibility wrapper. Canonical implementation: cargo xtask refresh-docs.
set -euo pipefail

cargo xtask refresh-docs "$@"

#!/usr/bin/env bash
# Thin wrapper. Canonical implementation: cargo xtask refresh-docs.
set -euo pipefail

cargo xtask refresh-docs "$@"

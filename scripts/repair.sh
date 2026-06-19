#!/usr/bin/env bash
# Compatibility wrapper. Canonical implementation: cargo xtask repair.
set -euo pipefail

cargo xtask repair "$@"

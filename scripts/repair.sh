#!/usr/bin/env bash
# Thin wrapper. Canonical implementation: cargo xtask repair.
set -euo pipefail

cargo xtask repair "$@"

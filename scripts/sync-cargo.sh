#!/usr/bin/env bash
# Thin wrapper. Canonical implementation: cargo xtask sync-cargo.
set -euo pipefail

cargo xtask sync-cargo "$@"

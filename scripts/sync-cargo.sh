#!/usr/bin/env bash
# Compatibility wrapper. Canonical implementation: cargo xtask sync-cargo.
set -euo pipefail

cargo xtask sync-cargo "$@"

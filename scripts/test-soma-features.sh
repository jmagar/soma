#!/usr/bin/env bash
# Thin wrapper. Canonical implementation: cargo xtask test-soma-features.
set -euo pipefail

cargo xtask test-soma-features "$@"

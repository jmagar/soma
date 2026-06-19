#!/usr/bin/env bash
# Compatibility wrapper. Canonical implementation: cargo xtask test-template-features.
set -euo pipefail

cargo xtask test-template-features "$@"

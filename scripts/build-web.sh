#!/usr/bin/env bash
# Compatibility wrapper. Canonical implementation: cargo xtask build-web.
set -euo pipefail

cargo xtask build-web "$@"

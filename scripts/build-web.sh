#!/usr/bin/env bash
# Thin wrapper. Canonical implementation: cargo xtask build-web.
set -euo pipefail

cargo xtask build-web "$@"

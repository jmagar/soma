#!/usr/bin/env bash
# Compatibility wrapper. Canonical implementation: cargo xtask web-watch.
set -euo pipefail

cargo xtask web-watch "$@"

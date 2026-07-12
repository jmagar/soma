#!/usr/bin/env bash
# Thin wrapper. Canonical implementation: cargo xtask web-watch.
set -euo pipefail

cargo xtask web-watch "$@"

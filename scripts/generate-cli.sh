#!/usr/bin/env bash
# Thin wrapper. Canonical implementation: cargo xtask generate-cli.
set -euo pipefail

cargo xtask generate-cli "$@"

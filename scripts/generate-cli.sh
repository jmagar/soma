#!/usr/bin/env bash
# Compatibility wrapper. Canonical implementation: cargo xtask generate-cli.
set -euo pipefail

cargo xtask generate-cli "$@"

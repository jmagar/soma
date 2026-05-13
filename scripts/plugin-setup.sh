#!/usr/bin/env bash
# =============================================================================
# scripts/plugin-setup.sh — Standalone repair/setup script for Example MCP server
#
# TEMPLATE: Replace all "example"/"EXAMPLE" occurrences with your service name.
#
# This script is the project-root counterpart to plugins/example/hooks/plugin-setup.sh.
# The hook version runs automatically at Claude Code session start.
# This version can be invoked manually (e.g. `just repair`) without the plugin runtime.
#
# It delegates directly to the plugin hook script, passing through any arguments.
# This single-source-of-truth approach means there's only one script to maintain.
# =============================================================================

set -euo pipefail

# Resolve the project root (this script's location is scripts/, parent is root)
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
PROJECT_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd -P)"

# Delegate to the canonical hook script
HOOK_SCRIPT="${PROJECT_ROOT}/plugins/example/hooks/plugin-setup.sh"

if [[ ! -f "${HOOK_SCRIPT}" ]]; then
  echo "ERROR: Hook script not found at ${HOOK_SCRIPT}" >&2
  echo "       Run from the project root: bash scripts/plugin-setup.sh" >&2
  exit 1
fi

# Set CLAUDE_PLUGIN_ROOT so the hook can find relative paths
export CLAUDE_PLUGIN_ROOT="${PROJECT_ROOT}/plugins/example"

exec bash "${HOOK_SCRIPT}" "$@"

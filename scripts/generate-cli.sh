#!/usr/bin/env bash
# Generate a standalone CLI for this server via mcporter.
# Must be run from the repository root.
# Requires: running server on port 40060 and mcporter in PATH.
# Generated CLI embeds your token — do not commit or share.
set -euo pipefail

if ! command -v mcporter >/dev/null 2>&1; then
    echo "error: mcporter not found. Install it first." >&2
    exit 1
fi

echo "Server must be running on port 40060 (run 'just dev' first)"
echo "Generated CLI embeds your token — do not commit or share"

mkdir -p dist dist/.cache

current_hash=$(timeout 10 curl -sf \
    -H "Authorization: Bearer ${EXAMPLE_MCP_TOKEN:-}" \
    -H "Accept: application/json, text/event-stream" \
    http://localhost:40060/mcp/tools/list 2>/dev/null | sha256sum | cut -d' ' -f1 || echo "nohash")

cache_file="dist/.cache/example-cli.schema_hash"
if [[ -f "$cache_file" ]] && [[ "$(cat "$cache_file")" == "$current_hash" ]] && [[ -f "dist/example-cli" ]]; then
    echo "SKIP: tool schema unchanged — use existing dist/example-cli"
    exit 0
fi

timeout 30 mcporter generate-cli \
    --command http://localhost:40060/mcp \
    --header "Authorization: Bearer ${EXAMPLE_MCP_TOKEN:-}" \
    --name example-cli \
    --output dist/example-cli

printf '%s' "$current_hash" > "$cache_file"
echo "Generated dist/example-cli (requires bun at runtime)"

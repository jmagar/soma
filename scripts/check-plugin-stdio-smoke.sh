#!/usr/bin/env bash
# Smoke-test the installed stdio MCP binary used by plugin manifests.
set -euo pipefail

BIN="${BIN:-rtemplate}"
TIMEOUT_SECS="${TIMEOUT_SECS:-5}"

if ! command -v "${BIN}" >/dev/null 2>&1; then
  echo "plugin stdio smoke: ${BIN} is not on PATH" >&2
  echo "run: just install-local" >&2
  exit 1
fi

response="$(
  printf '%s\n%s\n%s\n' \
    '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"plugin-stdio-smoke","version":"0.0.0"}}}' \
    '{"jsonrpc":"2.0","method":"notifications/initialized","params":{}}' \
    '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"example","arguments":{"action":"status"}}}' \
    | RTEMPLATE_API_URL="" RUST_LOG=warn timeout "${TIMEOUT_SECS}s" "${BIN}" mcp
)"

printf '%s\n' "${response}" \
  | jq -es 'map(select(.id == 2))[0].result.structuredContent.status == "ok"' >/dev/null

echo "plugin stdio smoke passed"

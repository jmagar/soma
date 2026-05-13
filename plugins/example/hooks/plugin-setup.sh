#!/usr/bin/env bash
# =============================================================================
# plugin-setup.sh — SessionStart hook for the Example MCP server plugin
#
# TEMPLATE: Replace all occurrences of:
#   "example"      → your service name in lowercase (e.g. "myservice")
#   "EXAMPLE"      → your service name in uppercase (e.g. "MYSERVICE")
#   "example-mcp"  → your Docker container/service name
#
# This script runs at Claude Code session start and on userConfig changes.
# It either deploys the server (server mode) or validates connectivity (client mode).
#
# Deployment modes:
#   use_docker=true   → start/update via docker compose
#   use_docker=false  → install as a systemd user service (default)
#
# Design decisions inherited from syslog-mcp:
#   - Binary symlink is refreshed every SessionStart (so plugin upgrades take effect)
#   - write_env returns 0 (changed) or 1 (unchanged), never errors on no-change
#   - Port conflict checks skip if the service is already running (it owns the ports)
#   - Docker and systemd modes clean up each other's artifacts when switching
# =============================================================================

set -euo pipefail

# ── Bootstrap ─────────────────────────────────────────────────────────────────
# When invoked directly (e.g. for manual repair), derive CLAUDE_PLUGIN_ROOT from
# the script's own path. The plugin runtime sets this automatically in hooks.

# TEMPLATE: Replace "example-jmagar-lab" with your plugin data directory name.
#           Format is typically: <plugin-name>-<author>-<scope>
: "${CLAUDE_PLUGIN_ROOT:=$(cd "$(dirname "$0")/.." && pwd)}"
: "${CLAUDE_PLUGIN_DATA:=${HOME}/.claude/plugins/data/example-jmagar-lab}"

# ── Helpers ───────────────────────────────────────────────────────────────────

# Read a value from the persisted env file (falls back gracefully when missing)
existing_env_value() {
  local key="$1"
  local file
  local value
  for file in "${CLAUDE_PLUGIN_DATA}/.env" "${CLAUDE_PLUGIN_DATA}/example-mcp.env"; do
    [[ -f "${file}" ]] || continue
    value="$(awk -F= -v key="${key}" '$1 == key {print substr($0, index($0, "=") + 1); exit}' "${file}")"
    if [[ -n "${value}" ]]; then
      printf '%s\n' "${value}"
      return 0
    fi
  done
  return 0
}

validate_port_value() {
  local name="$1" value="$2"
  if ! [[ "${value}" =~ ^[0-9]+$ ]] || (( value < 1 || value > 65535 )); then
    echo "ERROR: ${name} must be a port number (1-65535), got: ${value}" >&2
    exit 1
  fi
}

mcp_host_is_loopback() {
  case "$1" in
    127.*|::1) return 0 ;;
    *) return 1 ;;
  esac
}

strip_trailing_mcp_path() {
  local url="${1%/}"
  if [[ "${url}" == */mcp ]]; then url="${url%/mcp}"; fi
  printf '%s\n' "${url}"
}

derive_public_url() {
  if [[ -n "${PUBLIC_URL}" ]]; then
    strip_trailing_mcp_path "${PUBLIC_URL}"
    return
  fi
  if [[ "${SERVER_URL}" == https://* ]]; then
    strip_trailing_mcp_path "${SERVER_URL}"
  fi
}

# Read Codex OAuth callback URL if present (for multi-client OAuth redirect setup)
codex_oauth_callback_url() {
  local config="${HOME}/.codex/config.toml"
  [[ -f "${config}" ]] || return 0
  awk -F= '
    $1 ~ /^[[:space:]]*mcp_oauth_callback_url[[:space:]]*$/ {
      value = $2
      sub(/^[[:space:]]*"/, "", value)
      sub(/"[[:space:]]*$/, "", value)
      print value
      exit
    }
  ' "${config}"
}

append_csv_unique() {
  local csv="$1" value="$2"
  [[ -n "${value}" ]] || { printf '%s\n' "${csv}"; return; }
  local existing item
  IFS=',' read -r -a existing <<< "${csv}"
  for item in "${existing[@]}"; do
    item="${item#"${item%%[![:space:]]*}"}"
    item="${item%"${item##*[![:space:]]}"}"
    if [[ "${item}" == "${value}" ]]; then
      printf '%s\n' "${csv}"
      return
    fi
  done
  if [[ -n "${csv}" ]]; then
    printf '%s,%s\n' "${csv}" "${value}"
  else
    printf '%s\n' "${value}"
  fi
}

# ── Read userConfig values ────────────────────────────────────────────────────
# TEMPLATE: The CLAUDE_PLUGIN_OPTION_* prefix is set by the plugin runtime from
#           your plugin.json userConfig keys (converted to uppercase with underscores).
#           Add/remove options to match your plugin.json userConfig.

# Seed token from existing env when plugin option isn't set (keeps /repair idempotent)
NO_AUTH="${CLAUDE_PLUGIN_OPTION_NO_AUTH:-$(existing_env_value NO_AUTH)}"
NO_AUTH="${NO_AUTH:-false}"
NO_AUTH="$(printf '%s' "${NO_AUTH}" | tr '[:upper:]' '[:lower:]')"

AUTH_MODE="${CLAUDE_PLUGIN_OPTION_AUTH_MODE:-$(existing_env_value EXAMPLE_MCP_AUTH_MODE)}"
AUTH_MODE="${AUTH_MODE:-bearer}"
AUTH_MODE="$(printf '%s' "${AUTH_MODE}" | tr '[:upper:]' '[:lower:]')"

if [[ "${NO_AUTH}" != "true" && -z "${CLAUDE_PLUGIN_OPTION_API_TOKEN:-}" ]]; then
  _tok="$(existing_env_value EXAMPLE_MCP_TOKEN)"
  [[ -n "${_tok}" ]] && CLAUDE_PLUGIN_OPTION_API_TOKEN="${_tok}"
  unset _tok
fi

USE_DOCKER="${CLAUDE_PLUGIN_OPTION_USE_DOCKER:-false}"
API_TOKEN="${CLAUDE_PLUGIN_OPTION_API_TOKEN:-}"
SERVER_URL="${CLAUDE_PLUGIN_OPTION_SERVER_URL:-http://localhost:3000}"
# TEMPLATE: Port 3000 matches config.toml default. Change if you use a different port.
MCP_HOST="${CLAUDE_PLUGIN_OPTION_MCP_HOST:-0.0.0.0}"
MCP_PORT="${CLAUDE_PLUGIN_OPTION_MCP_PORT:-3000}"
validate_port_value EXAMPLE_MCP_PORT "${MCP_PORT}"

# TEMPLATE: Add your service-specific credential options here.
#           Mirror the userConfig keys from plugin.json (uppercased, underscores).
EXAMPLE_API_URL="${CLAUDE_PLUGIN_OPTION_EXAMPLE_API_URL:-$(existing_env_value EXAMPLE_API_URL)}"
EXAMPLE_API_KEY="${CLAUDE_PLUGIN_OPTION_EXAMPLE_API_KEY:-$(existing_env_value EXAMPLE_API_KEY)}"

PUBLIC_URL="${CLAUDE_PLUGIN_OPTION_PUBLIC_URL:-$(existing_env_value EXAMPLE_MCP_PUBLIC_URL)}"
GOOGLE_CLIENT_ID="${CLAUDE_PLUGIN_OPTION_GOOGLE_CLIENT_ID:-$(existing_env_value EXAMPLE_MCP_GOOGLE_CLIENT_ID)}"
GOOGLE_CLIENT_SECRET="${CLAUDE_PLUGIN_OPTION_GOOGLE_CLIENT_SECRET:-$(existing_env_value EXAMPLE_MCP_GOOGLE_CLIENT_SECRET)}"
AUTH_ADMIN_EMAIL="${CLAUDE_PLUGIN_OPTION_AUTH_ADMIN_EMAIL:-$(existing_env_value EXAMPLE_MCP_AUTH_ADMIN_EMAIL)}"
AUTH_ALLOWED_REDIRECT_URIS="${CLAUDE_PLUGIN_OPTION_AUTH_ALLOWED_REDIRECT_URIS:-$(existing_env_value EXAMPLE_MCP_AUTH_ALLOWED_REDIRECT_URIS)}"

# Token is required unless: no_auth=true OR oauth server mode bound to loopback
if [[ "${NO_AUTH}" != "true" && -z "${API_TOKEN}" ]]; then
  if ! [[ "${AUTH_MODE}" == "oauth" ]] || ! mcp_host_is_loopback "${MCP_HOST}"; then
    echo "ERROR: api_token is required unless no_auth is true or OAuth mode with loopback MCP host" >&2
    exit 1
  fi
fi

# ── Paths ─────────────────────────────────────────────────────────────────────
ENV_FILE="${CLAUDE_PLUGIN_DATA}/.env"
# TEMPLATE: Replace "example-mcp" with your service name
UNIT_FILE="${HOME}/.config/systemd/user/example-mcp.service"
COMPOSE_DIR="${CLAUDE_PLUGIN_DATA}"
COMPOSE_FILE="${COMPOSE_DIR}/docker-compose.yml"

# ── OAuth env block ───────────────────────────────────────────────────────────

oauth_env_block() {
  [[ "${NO_AUTH}" == "true" ]] && return 0
  [[ "${AUTH_MODE}" == "oauth" ]] || return 0

  local public_url
  public_url="$(derive_public_url)"
  if [[ -z "${public_url}" ]]; then
    echo "ERROR: OAuth mode requires public_url or an https server_url" >&2
    return 1
  fi
  if [[ -z "${GOOGLE_CLIENT_ID}" || -z "${GOOGLE_CLIENT_SECRET}" || -z "${AUTH_ADMIN_EMAIL}" ]]; then
    echo "ERROR: OAuth mode requires google_client_id, google_client_secret, and auth_admin_email" >&2
    return 1
  fi

  local redirects="${AUTH_ALLOWED_REDIRECT_URIS}"
  redirects="$(append_csv_unique "${redirects}" "https://claude.ai/api/mcp/auth_callback")"
  redirects="$(append_csv_unique "${redirects}" "https://claudeai.ai/api/mcp/auth_callback")"
  local codex_callback
  codex_callback="$(codex_oauth_callback_url)"
  [[ -n "${codex_callback}" ]] && redirects="$(append_csv_unique "${redirects}" "${codex_callback}")"

  # TEMPLATE: Replace EXAMPLE_MCP_ prefix with your service's env var prefix
  cat << EOF
EXAMPLE_MCP_AUTH_MODE=oauth
EXAMPLE_MCP_PUBLIC_URL=${public_url}
EXAMPLE_MCP_GOOGLE_CLIENT_ID=${GOOGLE_CLIENT_ID}
EXAMPLE_MCP_GOOGLE_CLIENT_SECRET=${GOOGLE_CLIENT_SECRET}
EXAMPLE_MCP_AUTH_ADMIN_EMAIL=${AUTH_ADMIN_EMAIL}
EXAMPLE_MCP_AUTH_ALLOWED_REDIRECT_URIS=${redirects}
EXAMPLE_MCP_AUTH_DISABLE_STATIC_TOKEN_WITH_OAUTH=false
EOF
}

# ── write_env ─────────────────────────────────────────────────────────────────
# Returns 0 if env file was written/changed, 1 if unchanged (not an error)

write_env() {
  mkdir -p "${CLAUDE_PLUGIN_DATA}"

  # TEMPLATE: Add/remove env vars to match your service's configuration surface.
  #           Only include env vars your binary reads from the environment.
  local new_env
  new_env=$(cat << EOF
EXAMPLE_MCP_HOST=${MCP_HOST}
EXAMPLE_MCP_PORT=${MCP_PORT}
NO_AUTH=${NO_AUTH}
EOF
)

  # Upstream service credentials — only write if set
  [[ -n "${EXAMPLE_API_URL}" ]] && new_env="${new_env}
EXAMPLE_API_URL=${EXAMPLE_API_URL}"
  [[ -n "${EXAMPLE_API_KEY}" ]] && new_env="${new_env}
EXAMPLE_API_KEY=${EXAMPLE_API_KEY}"

  # Bearer token — only write if auth is enabled
  if [[ "${NO_AUTH}" != "true" && -n "${API_TOKEN}" ]]; then
    new_env="${new_env}
EXAMPLE_MCP_TOKEN=${API_TOKEN}"
  fi

  # OAuth block (only when auth_mode=oauth)
  local auth_block
  if ! auth_block="$(oauth_env_block)"; then
    return 2
  fi
  [[ -n "${auth_block}" ]] && new_env="${new_env}
${auth_block}"

  # Docker mode: pin UID/GID so container writes files with host user's ownership
  if [[ "${USE_DOCKER}" == "true" ]]; then
    new_env="${new_env}
EXAMPLE_UID=$(id -u)
EXAMPLE_GID=$(id -g)"
  fi

  # Unchanged? Skip the write (avoids unnecessary systemd restarts)
  if [[ -f "${ENV_FILE}" ]] && diff -q <(echo "${new_env}") "${ENV_FILE}" >/dev/null 2>&1; then
    return 1  # unchanged
  fi

  echo "${new_env}" > "${ENV_FILE}"
  chmod 600 "${ENV_FILE}"
  return 0  # changed
}

ensure_env_written() {
  local rc
  write_env; rc=$?
  # rc=0 (changed) and rc=1 (unchanged) are both success
  [[ "${rc}" -le 1 ]] || return "${rc}"
  return 0
}

# ── setup_systemd ─────────────────────────────────────────────────────────────

setup_systemd() {
  mkdir -p "${HOME}/.config/systemd/user"

  # Pre-flight: binary must exist
  # TEMPLATE: Replace "example" with your binary name
  if [[ ! -x "${CLAUDE_PLUGIN_ROOT}/bin/example" ]]; then
    echo "ERROR: example binary not found at ${CLAUDE_PLUGIN_ROOT}/bin/example" >&2
    return 1
  fi

  # Pre-flight: port conflict (skip if service is already running — it owns the ports)
  local service_running=false
  # TEMPLATE: Replace "example-mcp.service" with your systemd unit name
  if systemctl --user is-active --quiet example-mcp.service 2>/dev/null; then
    service_running=true
  fi
  if [[ "${service_running}" == "false" ]]; then
    local port="${MCP_PORT}" proto="tcp"
    if ss -tlnp "sport = :${port}" 2>/dev/null | awk 'NR>1 && NF>0' | grep -q .; then
      echo "ERROR: port ${port}/${proto} is already in use" >&2
      return 1
    fi
  fi

  # Stop Docker container if switching from docker mode
  if [[ -f "${COMPOSE_FILE}" ]] && command -v docker >/dev/null 2>&1; then
    if (cd "${COMPOSE_DIR}" && docker compose ps --quiet example-mcp 2>/dev/null | grep -q .); then
      echo "example-mcp: stopping docker container before systemd cutover"
      (cd "${COMPOSE_DIR}" && docker compose down)
    fi
  fi

  # TEMPLATE: Replace "example" with your binary name and service name throughout
  local new_unit
  new_unit=$(cat << EOF
[Unit]
Description=example-mcp server
After=network.target

[Service]
ExecStart=${CLAUDE_PLUGIN_ROOT}/bin/example serve mcp
EnvironmentFile=${ENV_FILE}
Restart=on-failure
RestartSec=5

[Install]
WantedBy=default.target
EOF
)

  local unit_changed=false
  if ! diff -q <(echo "${new_unit}") "${UNIT_FILE}" >/dev/null 2>&1; then
    echo "${new_unit}" > "${UNIT_FILE}"
    unit_changed=true
  fi

  ensure_env_written

  if [[ "${unit_changed}" == "true" ]]; then
    systemctl --user daemon-reload
    systemctl --user enable --now example-mcp
  else
    systemctl --user restart example-mcp
  fi

  echo "example-mcp: systemd service running on ${MCP_HOST}:${MCP_PORT}"
}

# ── setup_docker ──────────────────────────────────────────────────────────────

setup_docker() {
  mkdir -p "${COMPOSE_DIR}"

  if ! docker info >/dev/null 2>&1; then
    echo "ERROR: docker daemon is not reachable" >&2
    return 1
  fi

  # Port conflict check (skip if container already running)
  local container_running=false
  if [[ -f "${COMPOSE_FILE}" ]] && \
     docker compose -f "${COMPOSE_FILE}" ps --quiet example-mcp 2>/dev/null | grep -q .; then
    container_running=true
  elif docker ps --filter 'name=^/example-mcp$' --quiet 2>/dev/null | grep -q .; then
    container_running=true
  fi
  if [[ "${container_running}" == "false" ]]; then
    if ss -tlnp "sport = :${MCP_PORT}" 2>/dev/null | awk 'NR>1 && NF>0' | grep -q .; then
      echo "ERROR: port ${MCP_PORT}/tcp is already in use" >&2
      return 1
    fi
  fi

  # Remove systemd unit if switching from systemd mode
  if systemctl --user list-unit-files example-mcp.service >/dev/null 2>&1; then
    if systemctl --user is-active --quiet example-mcp.service; then
      echo "example-mcp: stopping systemd unit before docker cutover"
      systemctl --user stop example-mcp.service
    fi
    systemctl --user disable example-mcp.service >/dev/null 2>&1 || true
    [[ -f "${UNIT_FILE}" ]] && { rm -f "${UNIT_FILE}"; systemctl --user daemon-reload; }
  fi

  # Sync compose file from plugin root
  if ! diff -q "${CLAUDE_PLUGIN_ROOT}/docker-compose.yml" "${COMPOSE_FILE}" >/dev/null 2>&1; then
    cp "${CLAUDE_PLUGIN_ROOT}/docker-compose.yml" "${COMPOSE_FILE}"
  fi

  ensure_env_written
  cd "${COMPOSE_DIR}"

  # Ensure the external docker network exists
  # TEMPLATE: Replace "jakenet" with your Docker network name (or make it configurable)
  local network_name="${DOCKER_NETWORK:-jakenet}"
  if ! docker network inspect "${network_name}" >/dev/null 2>&1; then
    echo "example-mcp: creating docker network ${network_name}"
    docker network create "${network_name}"
  fi

  # Source checkout: build locally. Installed plugin: pull from registry.
  # TEMPLATE: Replace "example-mcp" with your Docker image path
  if [[ "${CLAUDE_PLUGIN_OPTION_BUILD_LOCAL:-false}" == "true" && -f "${CLAUDE_PLUGIN_ROOT}/Cargo.toml" ]]; then
    (cd "${CLAUDE_PLUGIN_ROOT}" && docker compose build --no-cache example-mcp)
  else
    docker compose pull --quiet example-mcp 2>&1 || \
      echo "example-mcp: pull failed; will try cached image" >&2
  fi

  if docker compose ps --quiet example-mcp 2>/dev/null | grep -q .; then
    docker compose up -d --force-recreate --no-build
  else
    docker compose up -d --no-build
  fi

  echo "example-mcp: docker container running on ${MCP_HOST}:${MCP_PORT}"
}

# ── validate_client ───────────────────────────────────────────────────────────
# Client mode: just verify the remote server is reachable

validate_client() {
  if curl -sf "${SERVER_URL}/health" >/dev/null 2>&1; then
    echo "example-mcp: connected to ${SERVER_URL}"
  else
    echo "WARNING: example-mcp server at ${SERVER_URL} is not reachable" >&2
  fi
}

# ── link_binary ───────────────────────────────────────────────────────────────
# Re-link every SessionStart so plugin upgrades (which change CLAUDE_PLUGIN_ROOT)
# take effect immediately without user action.

link_binary() {
  mkdir -p "${HOME}/.local/bin"
  # TEMPLATE: Replace "example" with your binary name
  ln -sf "${CLAUDE_PLUGIN_ROOT}/bin/example" "${HOME}/.local/bin/example"
}

# ── Main ──────────────────────────────────────────────────────────────────────

link_binary

# TEMPLATE: This two-mode pattern (server/client) is the canonical shape.
#           Server mode: deploy the binary. Client mode: validate connectivity.
#           The is_server userConfig field (not implemented in this template) can
#           gate this. For a simpler service, you may always run in "server" mode.
if [[ "${USE_DOCKER}" == "true" ]]; then
  setup_docker
else
  setup_systemd
fi

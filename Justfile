# =============================================================================
# Justfile — Development and deployment commands for the Example MCP server
#
# TEMPLATE: Replace "example" with your binary/service name throughout.
#           Replace port 3000 with your service's port if different.
#
# Usage: just <recipe>   (install just: cargo install just)
# =============================================================================

# ── Development ───────────────────────────────────────────────────────────────

# Run the MCP server in development mode (HTTP transport, port 3000, no auth)
dev:
    EXAMPLE_MCP_NO_AUTH=true cargo run -- serve mcp

# Run in stdio MCP transport mode (for Claude Desktop or direct pipe)
mcp:
    cargo run -- mcp

# Run a quick CLI greeting (smoke test without a running server)
greet:
    cargo run -- greet --name "Developer"

# ── Building ──────────────────────────────────────────────────────────────────

# Compile debug build (fast, includes debug symbols)
build:
    cargo build

# Compile optimized release build (slower compile, much faster runtime)
build-release:
    cargo build --release

# ── Code quality ──────────────────────────────────────────────────────────────

# Run cargo check (fast syntax/type check, no binary output)
# Use this before committing to catch errors quickly
check:
    cargo check

# Run the full test suite using cargo-nextest (faster, better output than cargo test)
# Install nextest: cargo install cargo-nextest
test:
    cargo nextest run

# Run tests with the CI profile (fail-fast, 2 retries — mirrors CI)
test-ci:
    cargo nextest run --profile ci

# Run clippy with warnings as errors (matches CI)
lint:
    cargo clippy --all-targets -- -D warnings

# Format all Rust source files
fmt:
    cargo fmt

# Format all TOML files (requires taplo: cargo install taplo-cli)
fmt-toml:
    taplo format

# Check TOML format without modifying files (used in CI + lefthook)
check-toml:
    taplo check

# Run all quality checks in sequence (mirrors CI pipeline)
# Delegates to cargo xtask ci for the full suite (fmt, clippy, nextest, taplo, audit)
ci:
    cargo xtask ci

# ── xtask automation ─────────────────────────────────────────────────────────

# Build release binary and copy to bin/ (Git LFS tracked)
# After running, commit bin/<binary> and push to update the LFS pointer.
dist:
    cargo xtask dist

# Create AGENTS.md and GEMINI.md symlinks next to every CLAUDE.md in the repo.
# Pattern §32: CLAUDE.md is the single source of truth for project instructions.
# Run after adding any new CLAUDE.md file.
# Delegates to cargo xtask (cross-platform, handles nested CLAUDE.md files).
symlink-docs:
    cargo xtask symlink-docs

# Inline version of symlink-docs — no xtask required.
# TEMPLATE: Use this if xtask is unavailable (e.g. before first cargo build).
#           Both versions produce the same result.
symlink-docs-inline:
    find . -name "CLAUDE.md" -not -path "./.git/*" -not -path "./target/*" \
        -exec sh -c 'dir=$(dirname "$1"); ln -sf CLAUDE.md "${dir}/AGENTS.md"; ln -sf CLAUDE.md "${dir}/GEMINI.md"; echo "  link ${dir}/AGENTS.md + ${dir}/GEMINI.md"' _ {} \;

# Validate required environment variables are set before starting the server.
check-env:
    cargo xtask check-env

# ── Utilities ─────────────────────────────────────────────────────────────────

# Generate a cryptographically random bearer token for EXAMPLE_MCP_TOKEN
# Copy the output into your .env file
gen-token:
    openssl rand -hex 32

# Copy .env.example to .env (safe — won't overwrite an existing .env)
setup:
    cp -n .env.example .env || echo ".env already exists — skipping"
    @echo "Edit .env and fill in your credentials"

# ── Docker ────────────────────────────────────────────────────────────────────

# Start the Docker Compose stack in detached mode
# TEMPLATE: The compose file references the "jakenet" external network.
#           Create it first if it doesn't exist: docker network create jakenet
docker-up:
    docker compose up -d

# Stop and remove the Docker Compose stack (data volume persists)
docker-down:
    docker compose down

# Rebuild the Docker image from source and restart the stack
docker-rebuild:
    docker compose build --no-cache
    docker compose up -d --force-recreate

# Follow Docker container logs
docker-logs:
    docker compose logs -f

# ── Health & diagnostics ──────────────────────────────────────────────────────

# Check the MCP server health endpoint (no auth required)
# TEMPLATE: Change port 3000 if you use a different port
health:
    curl -sf http://localhost:3000/health | python3 -m json.tool

# Call the status action via the MCP tool (requires EXAMPLE_MCP_TOKEN in env)
status:
    #!/usr/bin/env bash
    set -euo pipefail
    TOKEN="${EXAMPLE_MCP_TOKEN:-}"
    if [[ -z "${TOKEN}" ]]; then
        echo "Set EXAMPLE_MCP_TOKEN or use 'just dev' (no-auth mode)"
        exit 1
    fi
    curl -sf http://localhost:3000/mcp \
        -H "Authorization: Bearer ${TOKEN}" \
        -H "Content-Type: application/json" \
        -H "Accept: application/json, text/event-stream" \
        -d '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"example","arguments":{"action":"status"}}}' \
        | python3 -m json.tool

# ── Plugin ────────────────────────────────────────────────────────────────────

# Repair the plugin deployment (re-runs plugin-setup.sh manually)
# Useful after a failed install or config change
repair:
    #!/usr/bin/env bash
    set -euo pipefail
    SCRIPT="plugins/example/hooks/plugin-setup.sh"
    if [[ ! -f "${SCRIPT}" ]]; then
        echo "ERROR: ${SCRIPT} not found — run from the project root"
        exit 1
    fi
    bash "${SCRIPT}"

# Install the release binary into bin/ (for plugin distribution)
# Linux only — Windows needs .exe; requires release build first
install: build-release
    #!/bin/sh
    set -eu
    # TEMPLATE: Replace "example" with your binary name
    target_dir="${CARGO_TARGET_DIR:-target}"
    mkdir -p bin
    install -m 755 "${target_dir}/release/example" bin/example
    echo "Installed bin/example"

# Validate all plugin skills have required SKILL.md fields
validate-skills:
    #!/usr/bin/env bash
    set -euo pipefail
    found=0
    for dir in plugins/example/skills/*; do
        [[ -d "$dir" ]] || continue
        found=1
        test -f "$dir/SKILL.md" || { echo "MISSING: $dir/SKILL.md"; exit 1; }
        grep -q '^name:' "$dir/SKILL.md" || { echo "MISSING name: in $dir/SKILL.md"; exit 1; }
        grep -q '^description:' "$dir/SKILL.md" || { echo "MISSING description: in $dir/SKILL.md"; exit 1; }
        echo "OK: $dir/SKILL.md"
    done
    [[ "$found" -eq 1 ]] || { echo "No skills found in plugins/example/skills/"; exit 1; }
    echo "All skills valid"

# ── mcporter integration tests ────────────────────────────────────────────────

# Run mcporter-based integration tests (requires running server + mcporter CLI)
# TEMPLATE: Ensure the server is running first: just dev   or   just docker-up
test-mcporter:
    #!/usr/bin/env bash
    set -euo pipefail
    if ! command -v mcporter &>/dev/null; then
        echo "mcporter not found. Install it first."
        exit 1
    fi
    bash tests/mcporter/test-tools.sh

# ── Publishing ────────────────────────────────────────────────────────────────

# Bump version, tag, and push (triggers CI publish workflow)
# TEMPLATE: Requires main branch + clean working tree
publish bump="patch":
    #!/usr/bin/env bash
    set -euo pipefail
    [ "$(git branch --show-current)" = "main" ] || { echo "Switch to main first"; exit 1; }
    [ -z "$(git status --porcelain)" ] || { echo "Commit or stash changes first"; exit 1; }
    git pull origin main
    CURRENT=$(grep -m1 "^version" Cargo.toml | sed 's/.*"\(.*\)".*/\1/')
    IFS="." read -r major minor patch <<< "${CURRENT}"
    case "{{bump}}" in
      major) major=$((major+1)); minor=0; patch=0 ;;
      minor) minor=$((minor+1)); patch=0 ;;
      patch) patch=$((patch+1)) ;;
      *) echo "Usage: just publish [major|minor|patch]"; exit 1 ;;
    esac
    NEW="${major}.${minor}.${patch}"
    echo "Bumping ${CURRENT} → ${NEW}"
    sed -i "s/^version = \"${CURRENT}\"/version = \"${NEW}\"/" Cargo.toml
    cargo check 2>/dev/null || true
    git add Cargo.toml Cargo.lock
    git commit -m "release: v${NEW}"
    git tag "v${NEW}"
    git push origin main --tags
    echo "Tagged v${NEW} — publish workflow will run automatically"

# Refresh local reference documentation (crawls + repomix)
refresh-docs:
    bash scripts/refresh-docs.sh

# Refresh docs — repomix packs only (no crawl)
refresh-docs-repomix:
    bash scripts/refresh-docs.sh --skip-crawl

# Refresh docs — crawl only (no repomix)
refresh-docs-crawl:
    bash scripts/refresh-docs.sh --skip-repomix

# Dry-run: print what would be refreshed
refresh-docs-dry:
    bash scripts/refresh-docs.sh --dry-run

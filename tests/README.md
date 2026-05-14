# Tests

Three layers of tests covering the CLI parser, service layer, and full HTTP integration.

## Running tests

```bash
# All Rust tests (recommended)
cargo nextest run

# Standard cargo test
cargo test

# CI profile (fail-fast, 2 retries — mirrors CI)
cargo nextest run --profile ci

# End-to-end integration (requires a running server + mcporter)
just dev &
bash tests/mcporter/test-tools.sh

# Template contract checks
just template-check

# Protected MCP auth smoke (requires running bearer-auth server)
EXAMPLE_MCP_TOKEN=<token> just auth-smoke
```

---

## Test files

### `plugin_contract.rs` — Plugin package contract

Tests that the Claude, Codex, and Gemini plugin package surfaces stay aligned:
manifest presence, shared endpoint settings, hook setup delegation, and the
binary-owned hook standard in docs.

### `template-check` — Template-level shell checks

`just template-check` runs:

- `just validate-plugin`
- `just schema-docs-check`
- `just template-features`

`scripts/test-template-features.sh` covers `.env` commit blocking, `CLAUDE.md`
sibling symlink creation, plugin validation, schema-doc validation, and ASCII
hygiene.

### `tool_dispatch.rs` — Service layer

Tests the MCP tool actions at the service level. No real credentials or running server needed — `loopback_state()` from `src/lib.rs::testing` builds a stub `AppState` with no auth and a placeholder HTTP client.

```
tests/tool_dispatch.rs
```

**Tests (7):**

| Test | What it asserts |
|---|---|
| `test_greet_no_name_returns_greeting` | Response contains `"greeting"` key with "Hello" in the value |
| `test_greet_with_name_includes_name` | Passing `name="Alice"` reflects "Alice" in the greeting |
| `test_greet_target_defaults_to_world` | Default target is "World" — catches silent default breakage |
| `test_echo_returns_message` | Echo round-trips the exact message string |
| `test_status_returns_ok` | `status` field equals `"ok"` |
| `test_all_actions_return_valid_json_object` | All actions return JSON objects, not arrays or primitives |
| `test_schemas_actions_list_is_non_empty` | `mcp::router()` compiles and exposes at least one action |

> **TEMPLATE**: Add one test per action you add. Each test should assert on response values, not just JSON validity.

The `call_service_action()` helper in the test file routes action names to service methods. Add a new arm there when you add a new action.

---

### `cli_parse.rs` — CLI argument parsing

Unit tests for `flag_value()`, the helper that extracts flag values from `argv`. No async, no credentials, no running process.

```
tests/cli_parse.rs
```

**Tests (4):**

| Test | What it asserts |
|---|---|
| `test_greet_no_name_parsed` | Missing `--name` flag returns `None` |
| `test_greet_with_name_parsed` | `--name Alice` extracts `"Alice"` |
| `test_echo_message_parsed` | `--message "Hello, World!"` extracts exact string |
| `test_echo_no_message_defaults` | Missing `--message` returns the documented default fallback |

> **TEMPLATE**: Add a test for each new CLI flag. Focus on the parsing contract — the right value comes out of the right flag — not on what the service does with it.

---

### `mcporter/test-tools.sh` — End-to-end integration

Bash script that hits a live server over HTTP using the `mcporter` CLI. Validates semantic correctness: the right values come back, not just valid JSON.

```
tests/mcporter/test-tools.sh
```

**Prerequisites:**

```bash
# Required
mcporter    # MCP client CLI
curl
jq
python3

# Optional (skip auth tests if absent)
EXAMPLE_MCP_TOKEN=<token>
```

**Usage:**

```bash
# Run all suites sequentially
bash tests/mcporter/test-tools.sh

# Run suites in parallel (faster, output interleaved)
bash tests/mcporter/test-tools.sh --parallel

# Server URL defaults to http://localhost:3000/mcp
# Override with env vars:
EXAMPLE_MCP_HOST=192.168.1.10 EXAMPLE_MCP_PORT=3100 bash tests/mcporter/test-tools.sh
```

**Three suites:**

**`suite_auth`** — Skipped if `EXAMPLE_MCP_TOKEN` is unset.
- Unauthenticated POST to `/mcp` returns HTTP 401
- Bad bearer token returns HTTP 401

**`suite_core`** — Primary semantic test suite.

| Action | Test | Assertion type |
|---|---|---|
| `greet` | Returns greeting object | key present |
| `greet` | `name="Alice"` reflected in response | substring |
| `greet` | Default target is `"World"` | exact match |
| `echo` | Returns echo object | key present |
| `echo` | Message round-trips exactly | exact match |
| `status` | Returns status field | key present |
| `status` | Status value equals `"ok"` | exact match |
| `help` | Returns help content | key present |
| `help` | Help mentions `"greet"` action | substring |

**`suite_schema_resource`** — Fetches the `example://schema/mcp-tool` MCP resource.
- Resource URI resolves
- Tool name equals `"example"`
- `inputSchema` is present and typed as `"object"`
- `inputSchema.properties` includes `"action"`

> **TEMPLATE**: Add a `suite_core` block for each action you add. Use `run_test_semantic()` for value assertions, `run_test()` for structural checks.

Test output and all curl/mcporter calls are logged to a timestamped file in `/tmp/`.

---

## Test helpers (Rust)

`src/lib.rs` exports a `testing` module (available under `cfg(test)` and the `test-support` feature):

| Helper | Returns | Use for |
|---|---|---|
| `testing::loopback_state()` | `AppState` | No-auth, stub client — most tests |
| `testing::bearer_state(token)` | `AppState` | Tests that exercise auth middleware |

These build a real `AppState` without real credentials. The stub `ExampleClient` points to `http://localhost:1/stub` — a port that always refuses connections, so tests that hit the service layer get deterministic stub responses rather than real network calls.

---

## Design principles

- **Semantic assertions**: every test checks that responses contain the *right data*, not just valid JSON. "Did `greet(name='Alice')` return a greeting containing 'Alice'?" beats "Did it return a 200?".
- **Explicit defaults**: default values are asserted in tests so silent regressions are caught.
- **Echo uses exact match**: the echo action test asserts the full round-tripped string, not a substring, to catch truncation.
- **Layered coverage**: CLI parsing, service logic, and HTTP integration are each covered by the appropriate tool. Don't test HTTP semantics in `tool_dispatch.rs` or service logic in `cli_parse.rs`.
- **Auth is optional at test time**: tests skip or adjust gracefully when credentials aren't set.

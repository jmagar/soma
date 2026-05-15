# Testing

The test strategy is layered: parse at the CLI layer, test business/service behavior without a server, then smoke-test live MCP HTTP with mcporter.

## Rust tests

```bash
cargo nextest run
cargo test
just test-ci
```

Key files:

| File | Purpose |
|---|---|
| `tests/cli_parse.rs` | CLI parser behavior. |
| `tests/tool_dispatch.rs` | Service/action semantics without live credentials. |
| `tests/api_routes.rs` | REST and mounted auth route behavior. |
| `tests/plugin_contract.rs` | Plugin package and hook contracts. |
| `tests/template_invariants.rs` | Automation/template invariants. |

## Live MCP tests

```bash
just dev
bash tests/mcporter/test-mcp.sh
```

The mcporter harness validates tools and resources against a running server. It logs calls to `/tmp/test-mcp.<timestamp>.log`.

## Template checks

```bash
just template-check
cargo xtask patterns
scripts/pre-release-check.sh
```

## Test helpers

`rmcp_template::testing::loopback_state()` builds no-auth `AppState` for integration tests. `bearer_state(token)` builds a bearer-auth state.

## Principles

- Assert semantic values, not just valid JSON.
- Assert defaults explicitly.
- Keep business logic tests below HTTP when possible.
- Use live mcporter tests for transport/resource/auth integration.

# Best Practices And CI Review

Scope: Rust API design, runtime hygiene, and CI coverage for PR #138.

## Findings

- P2: REST feature tests and clippy were not represented as explicit CI legs.
- P2: Windows helper tests hardcoded `python` instead of honoring the workflow-selected launcher.
- P2: compatibility checks blocked runtime workers.
- P3: approval policies could not be async.
- P3: docs for lifting the REST API into another crate were incomplete.

## Fixes

- Added Linux CI REST clippy and locked REST tests.
- Added Windows REST tests.
- Updated `soma-gateway` and `soma-mcp` Python test helpers to honor `SOMA_PYTHON_COMMAND`, then fall back to platform defaults.
- Moved compatibility refresh into `spawn_blocking`.
- Added `ApprovalFuture` and `AsyncFnApprovalHandler`.
- Expanded downstream docs with direct `tokio`/`axum` dependencies and route-specific mounting options.

## Targeted Verification

- `cargo test -p soma-gateway gateway::dispatch::tests::gateway_test_connects_and_discovers_stdio_upstream`
- `cargo test -p soma-gateway upstream::pool::live::tests::stdio_live_discovery_and_call_routes_echo`
- `cargo test -p soma-mcp gateway_proxy::tests::mcp_server_exposes_live_gateway_tools_resources_and_prompts`

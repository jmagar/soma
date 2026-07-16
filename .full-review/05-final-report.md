# Final Report

Workflow: `comprehensive-review:full-review`

Target: PR #138, `Add REST bridge for codex app-server client`

Result: completed. No P0 findings were found. All P1, P2, and P3 findings from the review pass were addressed in this follow-up branch.

## Fix Summary

- Hardened REST defaults so `rest::router()` is non-executing by default.
- Added explicit one-shot and trusted bridge router options.
- Separated trusted bridge access from unsafe request-controlled Codex options.
- Bounded session startup, stateful calls, text-turn duration, and text-turn output.
- Preserved terminal turn completion events under backpressure.
- Added reply deadlines and `410 Gone` behavior for expired server-originated requests.
- Improved custom backend liftability with default bridge methods.
- Made approval handling async-capable through `ApprovalFuture` and `AsyncFnApprovalHandler`.
- Expanded REST, approval, smoke, and CI coverage.
- Documented downstream dependencies, first-run/cost warnings, trusted bridge risks, and stateful callable flow.

## Verification

- `cargo fmt`
- `cargo test -p codex-app-server-client --lib turn_completed_notification_is_delivered_when_the_event_channel_is_full`
- `cargo test -p codex-app-server-client --test smoke -- --nocapture`
- `cargo test -p codex-app-server-client --features rest --test rest`
- `cargo test -p codex-app-server-client --all-features`
- `cargo clippy -p codex-app-server-client --all-targets --features rest -- -D warnings`
- `cargo test -p codex-app-server-client --features rest --locked`
- `cargo test -p soma-gateway gateway::dispatch::tests::gateway_test_connects_and_discovers_stdio_upstream`
- `cargo test -p soma-gateway upstream::pool::live::tests::stdio_live_discovery_and_call_routes_echo`
- `cargo test -p soma-mcp gateway_proxy::tests::mcp_server_exposes_live_gateway_tools_resources_and_prompts`

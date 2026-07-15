# RMCP Traces Implementation Record

Date: 2026-07-15

Issue: GitHub #76

Target: `rmcp 2.2.0`

## Outcome

Implemented a standalone `rmcp-traces` workspace crate and a narrow Soma MCP
proof path. The crate validates bounded request-side trace metadata and exposes
log-safe summaries. Soma reads `RequestContext.meta` after auth and logs only
short trace/span identifiers, sampled state, trust label, optional metadata
presence/counts, and safe invalid reasons.

## Locked Decisions

- Target `rmcp 2.2.0` explicitly.
- Keep `rmcp-traces` below product/runtime crates.
- Do not expose result `_meta` helpers in v1.
- Do not implement HTTP propagation in v1.
- Treat inbound trace metadata as untrusted unless a host policy marks it trusted.
- Never log raw `baggage`, raw `tracestate`, or arbitrary `_meta`.
- Keep Soma tool business logic and shim dispatch unchanged.

## Implemented

- Added `crates/rmcp-traces`.
- Normalized workspace `rmcp` pins to `2.2.0`.
- Added bounded `traceparent`, `tracestate`, and `baggage` parsing.
- Added fail-soft `TraceSummary` extraction for logs.
- Removed public propagation helpers so v1 remains request-summary only.
- Integrated trace summary logging in `crates/soma-mcp/src/rmcp_server.rs`.
- Added crate, MCP adapter, and integration tests.
- Documented scope and deferrals in `crates/rmcp-traces/README.md` and
  `docs/adr/0012-rmcp-traces-rmcp-2-2.md`.

## Review Follow-Ups Addressed

- Removed the public `TraceContext::apply_to_meta` propagation escape hatch.
- Preserved valid trace IDs when optional metadata is invalid.
- Accepted bounded higher-version traceparents with additive fields.
- Replaced global tracing-subscriber log capture with deterministic adapter tests.
- Deduplicated baggage key parsing.
- Reduced this durable plan artifact to an implementation record.

## Verification

- `cargo fmt --all`
- `cargo test -p rmcp-traces`
- `cargo test -p soma-mcp --all-features`
- `cargo test -p soma --test tool_dispatch --all-features`
- `cargo test -p soma --test dispatch_logging --all-features`
- `cargo test --workspace --all-features`
- `cargo clippy --workspace --all-features -- -D warnings`
- `cargo xtask check-version-sync`
- `cargo xtask check-release-versions --base origin/main --head HEAD --mode pr`
- `cargo package -p rmcp-traces --allow-dirty --no-verify`
- `cargo tree -p rmcp-traces`
- `cargo tree -i rmcp --workspace`

## Deferred

- Result `_meta` helpers with one serialized response budget.
- HTTP trace propagation behind explicit trust policy.
- Cross-server Lab, Cortex, Axon, and rmcp-template migrations.

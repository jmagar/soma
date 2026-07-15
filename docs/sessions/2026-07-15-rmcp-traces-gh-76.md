---
date: 2026-07-15
repo: git@github.com:jmagar/soma.git
branch: codex/rmcp-traces-issue-76
worktree: /home/jmagar/workspace/soma/.worktrees/rmcp-traces-issue-76
pr: https://github.com/jmagar/soma/pull/134
issue: https://github.com/jmagar/soma/issues/76
beads: rmcp-template-xh6c and children .1-.16
---

# RMCP Traces GH #76

Implemented GH #76 for the corrected `rmcp 2.2.0` target.

## Outcome

- Added the standalone `rmcp-traces` workspace crate.
- Normalized the workspace RMCP target to `2.2.0`.
- Implemented bounded request-side `traceparent`, `tracestate`, and `baggage` parsing.
- Kept v1 request-summary only: no result `_meta` helpers and no HTTP propagation.
- Integrated safe trace summaries into Soma MCP `call_tool` logging.
- Logged only `trace_id_prefix`, `span_id_prefix`, sampled state, trust, tracestate presence, baggage counts, sensitive baggage counts, and safe invalid reasons.
- Preserved valid trace ID prefixes when optional metadata is invalid.
- Added early-branch logging for response paging errors, auth denial, unknown tools, cached pages, validation errors, success, and execution failures.

## Review Fixes

- Removed the public `TraceContext::apply_to_meta` propagation escape hatch.
- Made `TraceSummary` fields private and exposed read-only accessors.
- Renamed logged trace/span fields to prefix semantics.
- Collected multiple safe invalid reasons instead of only the first one.
- Reported optional `tracestate`/`baggage` summary information even when `traceparent` is absent.
- Rejected non-ASCII fixed-position `traceparent` input without panicking before string slicing.
- Added v00 exact-length and higher-version 512/513-byte boundary tests.
- Added real RMCP duplex client/server coverage for request `_meta` flowing through `call_tool`.
- Added assertions that Soma does not attach result `_meta` on success, paging, continuation, or structured error paths.

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

## Notes

`marketplace-no-mcp` was not touched. The final Beads trail is closed under
`rmcp-template-xh6c` with review follow-ups `.14`, `.15`, and `.16` covering the
last PR-toolkit remediation pass.

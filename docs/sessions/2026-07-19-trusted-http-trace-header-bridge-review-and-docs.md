---
date: 2026-07-19 00:47:44 EST
repo: git@github.com:jmagar/soma.git
branch: claude/traces-crate-followups-01d8e6
head: 24ae6a3dca6667fe3c805d0f18236c895190ca55
plan: docs/superpowers/plans/2026-07-18-trusted-http-trace-header-bridge.md
working directory: /home/jmagar/workspace/soma/.claude/worktrees/traces-crate-followups-01d8e6
worktree: /home/jmagar/workspace/soma/.claude/worktrees/traces-crate-followups-01d8e6
pr: '#168 Trusted HTTP trace-header bridge (rmcp-template-mdei.2-.5) - https://github.com/jmagar/soma/pull/168'
beads: rmcp-template-mdei, rmcp-template-mdei.1-.12, rmcp-template-3rj0, rmcp-template-v0d4
---

# Trusted HTTP trace-header bridge implementation, review, and documentation

## User Request

Execute `docs/superpowers/plans/2026-07-18-trusted-http-trace-header-bridge.md`
with `vibin:work-it` in the supplied PR worktree, explain exactly what the PR
lands and what remains, correct the crate and canonical trace documentation,
and save the complete session to Markdown.

## Session Overview

PR #168 implements trusted inbound W3C trace-header bridging for Soma from
typed configuration through authenticated `tools/call` execution, safe
logging, CORS, non-propagation guards, tests, and a live smoke. Review
follow-ups hardened atomic parsing, environment isolation, subprocess bounds,
Windows behavior, and log matching. The session also expanded the
`rmcp-traces` README and `docs/TRACE_CONTEXT.md` after finding that the former
still called HTTP extraction a non-goal and the latter omitted important
operator and API contracts.

At closeout, the feature branch was pushed at `24ae6a3`, all trace-related
beads were closed, focused verification passed, and the live smoke reported 22
passed and 0 failed. PR #168 remained open while CI for the latest commit was
still running.

## Sequence of Events

1. The trusted trace-header plan was executed across typed configuration,
   runtime trust validation, MCP resolution, CORS, non-propagation, docs, and
   live verification.
2. Review findings added fail-closed parsing, atomic summary/raw extraction,
   safe gateway success/failure logging, bounded subprocess behavior, and
   test-environment isolation.
3. The PR was audited against `origin/main`, its plan, Beads state, and remote
   checks to distinguish delivered behavior from intentional future work.
4. A documentation audit found the crate README stale and the canonical guide
   too narrow. Both were expanded and committed as `cf64747`.
5. The save-session maintenance pass found concurrent uncommitted ANSI log
   normalization in the trace smoke. Bead `rmcp-template-v0d4` was filed; the
   concurrent change was then committed and pushed as `24ae6a3`, verified, and
   the bead was closed.
6. Repository plans, Beads, worktrees, branches, stale docs, and build artifacts
   were audited before this session artifact was generated.

## Key Findings

- The reusable crate now documents both RMCP `_meta` and optional trusted HTTP
  extraction, including default limits and fail-closed behavior at
  `crates/shared/traces/README.md:1-140`.
- The canonical Soma contract defines scope, auth-before-extraction ordering,
  and `_meta` precedence at `docs/TRACE_CONTEXT.md:8-54`.
- An explicit trusted-gateway declaration can coexist with mounted bearer or
  OAuth authentication; the resulting deployment matrix is documented at
  `docs/TRACE_CONTEXT.md:84-115`.
- Baggage is validated and safely summarized but is not inserted into Soma's
  domain execution context or forwarded downstream
  (`docs/TRACE_CONTEXT.md:16-23`, `docs/TRACE_CONTEXT.md:136-158`).
- Real `soma serve` logs contain ANSI formatting even when redirected to a
  file. Commit `24ae6a3` strips CSI sequences before smoke assertions, fixing
  false negative substring matches without changing plain logs.

## Technical Decisions

- Keep `rmcp-traces` a leaf crate. Product config and trust decisions remain in
  Soma; the crate only validates, bounds, labels, and summarizes inputs.
- Treat bearer/OAuth identity and CORS permission as distinct from trace-header
  trust. Only loopback or an explicit header-sanitizing gateway permits HTTP
  extraction.
- Give any RMCP `_meta` trace key precedence over HTTP. Even invalid `_meta`
  prevents fallback, so losing HTTP values are never parsed or logged.
- Restrict HTTP extraction to authenticated `tools/call`; pre-auth and paging
  failures do not ingest caller-controlled trace fields.
- Keep this slice inbound-only. Upstream propagation and an OTLP/exporter
  remain separate policy and implementation decisions.

## Files Changed

The implementation diff at closeout was `66 files changed, 5261 insertions,
168 deletions` relative to `origin/main...HEAD`. This table includes every
feature-diff path plus this generated session artifact.

| status | path | previous path | purpose | evidence |
|---|---|---|---|---|
| modified | `.env.example` | - | Document trace-header environment configuration. | `git diff --name-status origin/main...HEAD` |
| modified | `CHANGELOG.md` | - | Record the trusted HTTP bridge under Unreleased. | feature diff |
| modified | `CLAUDE.md` | - | Keep project environment guidance aligned. | feature diff |
| modified | `Cargo.lock` | - | Lock the optional HTTP feature dependency graph. | feature diff |
| modified | `README.md` | - | Add the operator-facing trace-header setting and canonical-doc link. | feature diff |
| modified | `apps/soma/src/http.rs` | - | Gate browser CORS headers by trace mode. | feature diff |
| modified | `apps/soma/src/http_tests.rs` | - | Verify mode-specific CORS behavior. | feature diff |
| modified | `apps/soma/src/lib.rs` | - | Add trace-aware integration test state helpers. | feature diff |
| modified | `apps/soma/tests/api_routes.rs` | - | Prove proxy routes do not forward trace headers. | feature diff |
| modified | `apps/soma/tests/doctor_cli.rs` | - | Isolate doctor tests from inherited trace config. | feature diff |
| created | `apps/soma/tests/mcp_trace_headers.rs` | - | Exercise real Streamable HTTP trace ingestion. | feature diff |
| modified | `apps/soma/tests/plugin_contract.rs` | - | Verify plugin trace-header settings. | feature diff |
| modified | `apps/soma/tests/stdio_mcp.rs` | - | Bound and isolate stdio integration operations. | feature diff |
| modified | `config.toml` | - | Add the typed trace-header mode example. | feature diff |
| modified | `crates/shared/codemode/Cargo.toml` | - | Support serialized environment-sensitive tests. | feature diff |
| modified | `crates/shared/codemode/src/artifacts/store_tests.rs` | - | Isolate environment-sensitive tests. | feature diff |
| modified | `crates/shared/codemode/src/artifacts_tests.rs` | - | Isolate shared test state. | feature diff |
| modified | `crates/shared/codemode/src/config_tests.rs` | - | Serialize environment mutation. | feature diff |
| modified | `crates/shared/codemode/src/execute/budget_tests.rs` | - | Serialize execution budget environment access. | feature diff |
| modified | `crates/shared/codemode/src/execute/runner_tests.rs` | - | Isolate runner environment state. | feature diff |
| modified | `crates/shared/codemode/src/execute_tests.rs` | - | Prevent concurrent `SOMA_HOME` interference. | feature diff |
| modified | `crates/shared/codemode/src/home_tests.rs` | - | Serialize `SOMA_HOME` tests. | feature diff |
| modified | `crates/shared/codemode/src/pool/checkout_tests.rs` | - | Isolate pooled runner test state. | feature diff |
| modified | `crates/shared/codemode/src/pool/runner_handle_tests.rs` | - | Isolate runner handle test state. | feature diff |
| modified | `crates/shared/codemode/src/runner_exe_tests.rs` | - | Serialize executable path environment access. | feature diff |
| modified | `crates/shared/codemode/src/state_tests.rs` | - | Isolate codemode state tests. | feature diff |
| modified | `crates/shared/mcp/server/src/trace.rs` | - | Return summary and raw fields atomically. | feature diff |
| modified | `crates/shared/mcp/server/src/trace_tests.rs` | - | Prove atomic, log-safe extraction. | feature diff |
| modified | `crates/shared/traces/README.md` | - | Document crate APIs, HTTP feature, limits, and safety contract. | `cf64747` |
| modified | `crates/soma/cli/src/doctor/checks.rs` | - | Diagnose invalid trace trust configuration. | feature diff |
| modified | `crates/soma/cli/src/doctor/checks_tests.rs` | - | Verify doctor trust diagnostics. | feature diff |
| modified | `crates/soma/cli/src/setup.rs` | - | Persist and validate trace-header setup. | feature diff |
| modified | `crates/soma/cli/src/setup_tests.rs` | - | Verify setup failure classification and repair. | feature diff |
| modified | `crates/soma/client/src/client_tests.rs` | - | Prove upstream API non-propagation. | feature diff |
| modified | `crates/soma/config/src/config.rs` | - | Add `TraceHeaderMode` and environment parsing. | feature diff |
| modified | `crates/soma/config/src/config_tests.rs` | - | Test defaults, TOML, and all environment values. | feature diff |
| modified | `crates/soma/config/src/env_registry.rs` | - | Register `SOMA_MCP_TRACE_HEADERS`. | feature diff |
| modified | `crates/soma/config/src/env_registry_tests.rs` | - | Verify registry mapping. | feature diff |
| modified | `crates/soma/config/src/lib.rs` | - | Export the typed trace mode. | feature diff |
| modified | `crates/soma/mcp/Cargo.toml` | - | Enable `rmcp-traces/http` under Soma HTTP. | feature diff |
| modified | `crates/soma/mcp/src/gateway_proxy_tests.rs` | - | Protect gateway outbound header allow-list. | feature diff |
| modified | `crates/soma/mcp/src/lib.rs` | - | Register trace resolution modules. | feature diff |
| modified | `crates/soma/mcp/src/rmcp_server.rs` | - | Resolve context after auth and emit safe summaries. | feature diff |
| modified | `crates/soma/mcp/src/rmcp_server_tests.rs` | - | Verify trace behavior across result paths. | feature diff |
| created | `crates/soma/mcp/src/trace_resolution.rs` | - | Implement source precedence and mode mapping. | feature diff |
| created | `crates/soma/mcp/src/trace_resolution_tests.rs` | - | Test precedence, conflicts, and invalid inputs. | feature diff |
| modified | `crates/soma/runtime/src/server.rs` | - | Enforce startup trust-boundary validation. | feature diff |
| modified | `crates/soma/runtime/src/server_tests.rs` | - | Cover loopback, gateway, bearer, and OAuth combinations. | feature diff |
| modified | `docs/CLAUDE.md` | - | Keep documentation guidance aligned. | feature diff |
| modified | `docs/CONFIG.md` | - | Document typed config and trust requirements. | feature diff |
| modified | `docs/ENV.md` | - | Document generated environment mapping. | feature diff |
| modified | `docs/PLUGINS.md` | - | Document plugin configuration mapping. | feature diff |
| created | `docs/TRACE_CONTEXT.md` | - | Provide the canonical Soma trace contract. | `8809b7b`, `cf64747` |
| modified | `docs/generated/plugin-settings.md` | - | Regenerate plugin option documentation. | feature diff |
| modified | `docs/generated/scripts-index.md` | - | Register the smoke wrapper. | feature diff |
| created | `docs/superpowers/plans/2026-07-18-trusted-http-trace-header-bridge.md` | - | Record the implementation plan. | `2dc0f3d` |
| modified | `packages/soma-rmcp/README.md` | - | Document package trace setting. | feature diff |
| modified | `plugins/soma/.claude-plugin/plugin.json` | - | Expose Claude trace-header user configuration. | feature diff |
| modified | `plugins/soma/gemini-extension.json` | - | Expose Gemini trace-header configuration. | feature diff |
| modified | `scripts/README.md` | - | Document the trace smoke wrapper. | feature diff |
| modified | `scripts/generate-docs.py` | - | Generate trace environment and plugin docs. | feature diff |
| created | `scripts/test-trace-headers.sh` | - | Add a thin live-smoke wrapper. | feature diff |
| modified | `xtask/src/main.rs` | - | Register the trace-header smoke command. | feature diff |
| modified | `xtask/src/scripts_lane_a.rs` | - | Keep script inventory contracts aligned. | feature diff |
| created | `xtask/src/trace_headers_smoke.rs` | - | Implement bounded real-server verification and ANSI-safe log matching. | `d1be063`, `24ae6a3` |
| modified | `xtask/src/workspace_commands.rs` | - | Route the new xtask command. | feature diff |
| created | `docs/sessions/2026-07-19-trusted-http-trace-header-bridge-review-and-docs.md` | - | Preserve this complete session record. | save-to-md artifact |

## Beads Activity

| bead | title | actions | final status | why it mattered |
|---|---|---|---|---|
| `rmcp-template-mdei` | rmcp-traces: trusted HTTP trace header bridge | Reviewed and closed after all slices. | closed | Parent delivery record for PR #168. |
| `rmcp-template-mdei.1` | rmcp-traces: add optional HTTP header extraction feature | Observed as completed foundation. | closed | Supplies bounded reusable HTTP extraction. |
| `rmcp-template-mdei.2` | soma: add explicit HTTP trace-header trust config | Claimed, implemented, verified, closed. | closed | Adds typed config and startup policy. |
| `rmcp-template-mdei.3` | soma-mcp: consume trusted HTTP trace headers safely | Claimed, implemented, verified, closed. | closed | Adds post-auth `tools/call` consumption. |
| `rmcp-template-mdei.4` | soma: gate browser CORS trace headers on trust config | Claimed, implemented, verified, closed. | closed | Keeps browser transport permission mode-aware. |
| `rmcp-template-mdei.5` | soma: live smoke and docs for trusted trace headers | Claimed, implemented, verified, closed. | closed | Adds docs, live proof, and non-propagation tests. |
| `rmcp-template-mdei.6` | Avoid duplicate TraceSummary parsing on meta-only call_tool paths | Created during review and closed after remediation. | closed | Removes duplicate hot-path parsing. |
| `rmcp-template-mdei.7` | Bound CORS preflight curl subprocesses in trace-header smoke | Created during review and closed after remediation. | closed | Prevents smoke hangs and unchecked curl failures. |
| `rmcp-template-mdei.8` | Regenerate docs and plugin settings for trace-header configuration | Created during review and closed after regeneration. | closed | Restores generated/manual surface parity. |
| `rmcp-template-mdei.9` | Render canonical trace-header values in startup rejection errors | Created during review and closed after remediation. | closed | Makes operator errors use real config values. |
| `rmcp-template-mdei.10` | Keep validated trace summary and raw fields bound to the same Meta | Created during review and closed after atomic extraction. | closed | Prevents summary/raw input drift. |
| `rmcp-template-mdei.11` | Bound high-level stdio MCP integration operations | Created during verification and closed after hardening. | closed | Prevents indefinite test waits under load. |
| `rmcp-template-mdei.12` | Isolate doctor CLI tests from inherited trace-header config | Created during verification and closed after isolation. | closed | Prevents environment-dependent CI failures. |
| `rmcp-template-3rj0` | Expand canonical trace documentation and crate README | Created, claimed, implemented, verified, closed. | closed | Corrected stale crate claims and completed the canonical guide. |
| `rmcp-template-v0d4` | Normalize ANSI output before trace-header smoke log assertions | Created during maintenance, then verified and closed after concurrent commit `24ae6a3`. | closed | Fixes false smoke failures from colorized logs. |

## Repository Maintenance

### Plans

`docs/plans/` does not exist, so there were no plan files eligible for the
skill's `docs/plans/complete/` archive. The applicable completed implementation
plan remains under `docs/superpowers/plans/` as a historical execution record;
it was not moved or rewritten. `docs/TRACE_CONTEXT.md` is the canonical current
contract where late review decisions differ from the original plan.

### Beads

`bd dep tree rmcp-template-mdei --direction=both` showed the epic and all 12
direct trace children closed. The maintenance pass created
`rmcp-template-v0d4` for an observed uncommitted ANSI smoke fix, then closed it
only after commit `24ae6a3` was pushed and focused plus live verification
passed. `bd dolt push` succeeded.

### Worktrees and branches

`git worktree list --porcelain`, local/remote branch listings, PR state, and
merge ancestry were inspected. The current branch is the active, unmerged PR
#168 branch and was preserved. The protected `marketplace-no-mcp` worktree was
left untouched. Other feature, refactor, Codex, and session-log worktrees were
not removed because they were active, unmerged/ahead, dirty-status unknown, or
owned by concurrent sessions. No branch cleanup was safe within this session's
scope.

### Stale docs and generated artifacts

The stale `rmcp-traces` README and light canonical trace guide were corrected
in `cf64747`; generated-doc checks and README audit passed. Untracked `.cargo/`,
`target/`, Palette, and web build/cache directories were observed and left in
place because they were pre-existing warm artifacts with uncertain concurrent
ownership. No tracked implementation changes were hidden or discarded.

## Tools and Skills Used

- **Skills and plugins.** `vibin:work-it` drove plan execution;
  `vibin:save-to-md` drove this maintenance and publishing workflow. The
  existing delivery record also documented independent review passes. No
  browser automation was needed.
- **Shell and file tools.** `rg`, `sed`, `nl`, `find`, `jq`, Git, and
  `apply_patch` inspected and updated code/docs. One oversized command output
  was truncated, so later queries were narrowed to relevant fields.
- **Rust tooling.** Cargo, xtask, formatting, focused tests, integration tests,
  and the real trace-header smoke provided verification. The mise `soldr`
  wrapper emitted a very large Cargo metadata stream and a non-blocking PyO3
  eligibility warning; final test results were still captured.
- **GitHub CLI.** `gh pr view` and `gh pr checks` verified PR #168 and remote CI.
  One earlier GraphQL request hit a TLS handshake timeout; subsequent calls
  succeeded.
- **Beads.** `bd show`, `bd list`, `bd dep tree`, `bd create`, `bd close`, and
  `bd dolt push` audited and reconciled tracker state. No MCP, Labby, Axon, or
  subagent tool call was required for the save workflow; the session-start
  Labby health check was unreachable but non-blocking.

## Commands Executed

| command | result |
|---|---|
| `git diff --shortstat origin/main...HEAD` | Reported 66 files, 5261 insertions, 168 deletions at closeout. |
| `gh pr view 168 ...` | Confirmed open PR #168 targeting `main`. |
| `gh pr checks 168` | Earlier revision was green; latest `24ae6a3` checks were newly pending at artifact time. |
| `bd dep tree rmcp-template-mdei --direction=both` | Confirmed the epic and direct trace children closed. |
| `cargo test -p rmcp-traces` | Passed 17 tests with default features. |
| `cargo test -p rmcp-traces --features http` | Passed 30 tests including 13 HTTP cases. |
| `cargo test -p soma --test mcp_trace_headers --features test-support` | Passed 5 real HTTP integration tests. |
| `cargo xtask check-docs` | Generated docs were current. |
| `python scripts/check-readme-guide.py` | Passed README audit. |
| `cargo xtask run-ascii-check` | Passed ASCII hygiene. |
| `cargo xtask check-coupled-files` | Passed coupled-file validation. |
| `cargo test -p xtask strip_ansi_tests` | Passed 2 ANSI normalization tests. |
| `cargo xtask test-trace-headers` | Passed 22 live assertions, 0 failed. |
| `bd dolt push` | Pushed tracker updates successfully. |

## Errors Encountered

- The crate README still said HTTP propagation was a v1 non-goal even though
  the PR implemented inbound HTTP extraction. It was rewritten to document the
  real feature and API.
- `docs/TRACE_CONTEXT.md` incorrectly said every mounted bearer/OAuth
  deployment rejects non-off modes. Runtime tests proved explicit gateway trust
  can coexist with mounted auth; the guide now documents that matrix.
- Colorized subprocess logs split trace field names from values, causing live
  smoke substring checks to fail falsely. Commit `24ae6a3` normalizes ANSI/CSI
  sequences before matching; 2 focused tests and 22 live assertions passed.
- No Claude transcript matched the worktree-derived glob, so no transcript
  path or session ID is claimed in this artifact. Conversation context, Git,
  PR, plan, existing session records, and Beads were used as evidence.
- The session-start Labby endpoint at `http://localhost:8765` was unreachable.
  This work required no Labby tool, so the failure did not block delivery.

## Behavior Changes (Before/After)

| area | before | after |
|---|---|---|
| HTTP trace ingestion | Soma did not consume inbound W3C HTTP trace headers. | Trusted modes bridge validated headers into `tools/call` execution after auth. |
| Trust configuration | No explicit trace-header trust mode existed. | `off`, `trusted`, and `trusted-with-baggage` are typed and startup-validated. |
| Source precedence | No HTTP source existed. | Any RMCP `_meta` trace key wins; losing HTTP values are not parsed or logged. |
| Browser transport | Trace headers were not mode-gated in CORS. | Static CORS allow-headers match the configured mode. |
| Outbound safety | No dedicated regression proof existed. | Client and gateway tests prove inbound trace headers are not propagated. |
| Documentation | The crate README contradicted the feature and the canonical guide was incomplete. | Both API and operator contracts are comprehensive and cross-linked. |
| Live smoke logs | ANSI formatting could cause false failed assertions. | Captured logs are normalized before matching; plain logs remain unchanged. |

## Verification Evidence

| command | expected | actual | status |
|---|---|---|---|
| `cargo fmt --all --check` | No Rust formatting drift. | Completed successfully. | pass |
| `cargo test -p rmcp-traces` | Core trace parsing remains valid. | 17 passed, 0 failed. | pass |
| `cargo test -p rmcp-traces --features http` | HTTP extraction and core tests pass. | 30 passed, 0 failed. | pass |
| `cargo test -p soma --test mcp_trace_headers --features test-support` | Real HTTP trace integration passes. | 5 passed, 0 failed. | pass |
| `cargo xtask check-docs` | Generated docs remain current. | `generated docs are current`. | pass |
| `python scripts/check-readme-guide.py` | README structure is acceptable. | `PASS: README.md`. | pass |
| `cargo xtask run-ascii-check` | Tracked source/docs are ASCII-clean. | Completed successfully. | pass |
| `cargo xtask check-coupled-files` | Companion files remain aligned. | Coupled-file check passed. | pass |
| `git diff --check` | No whitespace errors. | Completed successfully. | pass |
| `cargo test -p xtask strip_ansi_tests` | ANSI normalization is focused and non-destructive. | 2 passed, 0 failed. | pass |
| `cargo xtask test-trace-headers` | Real server behavior passes all scenarios without leaking baggage. | 22 passed, 0 failed. | pass |
| `gh pr checks 168` | Latest remote revision eventually completes required CI. | Fresh checks for `24ae6a3` were pending at capture time. | warn |

## Risks and Rollback

- Enabling HTTP trace extraction behind a proxy that does not strip or replace
  untrusted trace headers can let clients choose correlation identities.
  Roll back operationally with `SOMA_MCP_TRACE_HEADERS=off`, the default.
- `trusted-with-baggage` processes potentially sensitive baggage. Prefer
  `trusted` unless baggage is explicitly required; raw values are not logged or
  forwarded by this implementation.
- Code rollback is a normal revert of the feature commits or PR before merge.
  Reverting `24ae6a3` affects only smoke log matching, not runtime ingestion.

## Decisions Not Taken

- Outbound propagation was not added because upstream trust, baggage, and
  sampling policy require a separate design.
- No OpenTelemetry SDK/exporter or span creation was added; this slice carries
  and safely reports inbound context only.
- HTTP extraction was not broadened beyond `tools/call`; resources, prompts,
  and listings remain unchanged.
- The manual 22-assertion smoke was not added to CI because it performs a full
  build and is intentionally a bounded local diagnostic.
- No unrelated worktree, branch, warm cache, or protected
  `marketplace-no-mcp` state was deleted during session maintenance.

## References

- PR #168: https://github.com/jmagar/soma/pull/168
- `docs/superpowers/plans/2026-07-18-trusted-http-trace-header-bridge.md`
- `docs/TRACE_CONTEXT.md`
- `crates/shared/traces/README.md`
- `docs/sessions/2026-07-18-trusted-http-trace-header-bridge.md`
- Beads epic `rmcp-template-mdei`

## Open Questions

- Will all required checks for the latest PR head `24ae6a3` complete green?
  They had restarted and were pending when this artifact was written.

## Next Steps

The implementation and documentation work from this session is complete. The
immediate delivery steps are:

1. Watch the fresh remote checks with `gh pr checks 168 --watch`.
2. Merge PR #168 after required checks are green.
3. Deploy/restart Soma and leave `SOMA_MCP_TRACE_HEADERS=off` unless the target
   is loopback or its gateway is verified to strip or overwrite client trace
   headers.
4. If enabled behind a gateway, start with `trusted`; use
   `trusted-with-baggage` only after an explicit data-policy decision.

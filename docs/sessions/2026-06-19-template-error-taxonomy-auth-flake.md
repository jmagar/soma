---
date: 2026-06-19 02:46:33 EST
repo: git@github.com:jmagar/soma-mcp.git
branch: main
head: e4a290b
session id: 8dd2c014-bb7a-46f4-941d-3d4510a9f94d
transcript: /home/jmagar/.claude/projects/-home-jmagar-workspace-soma/8dd2c014-bb7a-46f4-941d-3d4510a9f94d.jsonl
working directory: /home/jmagar/workspace/soma
worktree: /home/jmagar/workspace/soma
beads: soma-821, soma-sk6, soma-chl, soma-hi8, soma-lo4
---

# Template error taxonomy and auth test stabilization

## User Request

The session centered on continuing the `soma` extraction and hardening work: port shared action/error patterns into Soma, keep REST/CLI/MCP parity registry-driven, and then address the order-sensitive `soma-auth` Google token test exposed by full parallel `cargo test`.

## Session Overview

Implemented a shared `ToolError`/`ServiceError` taxonomy that now drives REST responses, MCP structured tool errors, and CLI error output. Added CLI metadata to `ActionSpec`, registry-driven REST and CLI parity tests, and a shared REST action wrapper. After verification exposed a parallel auth-test flake, fixed the WireMock lifetime bug in the Google OAuth tests and verified the full default-threaded test suite.

## Sequence of Events

1. Created and claimed the error-taxonomy bead, then inspected the current action, API, CLI, MCP, and test layers.
2. Added shared error types in `crates/soma-contracts/src/errors.rs` and wired service-level classification through `soma-service`.
3. Replaced duplicated REST/MCP/CLI error rendering with the shared taxonomy and added registry-driven REST/CLI parity checks.
4. Ran focused tests, fixed the optional `greet` REST body regression, then ran full tests and clippy.
5. Observed full parallel `cargo test` exposing a Google OAuth test flake, created `soma-sk6`, found the dropped `MockServer` lifetime bug, and fixed it.
6. Fast-forwarded local `main` to `origin/main` after the cargo-generate xtask PR merged, then performed this save-session maintenance pass.

## Key Findings

- The shared error vocabulary now lives in `crates/soma-contracts/src/errors.rs:6` and maps service error kind to HTTP status, retryability, REST payloads, and MCP payloads.
- REST action execution now flows through `run_rest_action_request` and `rest_error_response` in `crates/soma-api/src/api.rs:229`, so parse, scope, execution, and error mapping share one wrapper.
- CLI action metadata is part of `ActionSpec` at `crates/soma-contracts/src/actions.rs:153`, allowing parser parity tests to check registry coverage instead of hand-maintained expectations.
- REST route parity is asserted against `ACTION_SPECS` in `crates/soma/tests/api_routes.rs:61`.
- The auth flake was caused by a test helper returning a provider pointed at a `wiremock::MockServer` that had already been dropped; the fixed wrapper keeps the server alive in `crates/soma-auth/src/google.rs:776`.

## Technical Decisions

- Kept the canonical error taxonomy in `soma-contracts`, with service-specific classification in `soma-service`, so REST/MCP/CLI can depend on one `ToolError` shape without creating dependency cycles.
- Preserved legacy REST compatibility by keeping the `error` string field in REST error payloads while adding structured fields such as `kind`, `code`, `retryable`, and `remediation`.
- Used registry parity tests rather than source-text checks for REST and CLI where the code already exposes typed route/action metadata.
- Fixed the auth test by extending the test fixture lifetime instead of weakening assertions or forcing serial test execution.
- Fast-forwarded `main` before writing this session artifact because `origin/main` had advanced and local `main` was an ancestor.

## Files Changed

| status | path | previous path | purpose | evidence |
|---|---|---|---|---|
| modified | `.gitignore` | - | Ignore local `mcp-server-inventory.md` artifact in the error-taxonomy merge; note later PR #43 tracked an inventory file intentionally. | `185b0fd` |
| modified | `crates/soma-contracts/src/actions.rs` | - | Add CLI metadata to `ActionSpec` and preserve REST route metadata. | `crates/soma-contracts/src/actions.rs:153` |
| created | `crates/soma-contracts/src/errors.rs` | - | Define shared `ServiceErrorKind`, `ToolError`, REST payload, MCP payload, and execution classification. | `crates/soma-contracts/src/errors.rs:6` |
| created | `crates/soma-contracts/src/errors_tests.rs` | - | Add generated-docs/cargo-generate PR tests for error contracts after fast-forward. | `e4a290b` |
| modified | `crates/soma-contracts/src/lib.rs` | - | Export the new `errors` module. | `185b0fd` |
| modified | `crates/soma-service/src/lib.rs` | - | Add `classify_service_error` and route scaffold/action validation into the shared taxonomy. | `185b0fd` |
| modified | `crates/soma-api/src/api.rs` | - | Add shared REST action wrapper and structured error response mapping. | `crates/soma-api/src/api.rs:229` |
| modified | `crates/soma-mcp/src/rmcp_server.rs` | - | Replace MCP-local execution/validation error payload builders with shared `ToolError` payloads. | `185b0fd` |
| modified | `crates/soma-mcp/src/rmcp_server_tests.rs` | - | Update MCP structured-error tests to use shared classification. | `185b0fd` |
| modified | `crates/soma-cli/src/lib.rs` | - | Print CLI errors from shared `ToolError` fields. | `crates/soma-cli/src/lib.rs:196` |
| modified | `crates/soma-cli/src/cli_tests.rs` | - | Add registry-driven CLI parser coverage and CLI error-format checks. | `185b0fd` |
| modified | `crates/soma/tests/api_routes.rs` | - | Add REST route/action registry parity test. | `crates/soma/tests/api_routes.rs:61` |
| modified | `crates/soma-auth/src/google.rs` | - | Keep WireMock server alive for mocked Google providers. | `crates/soma-auth/src/google.rs:776` |
| modified | `scripts/check-stale-claims.py` | - | Generated-docs merge included stale-claim guard updates. | `185b0fd`, `e4a290b` |
| created | `mcp-server-inventory.md` | - | Cargo-generate xtask PR tracked the inventory file after fast-forward. | `e4a290b` |
| modified | `README.md`, `docs/CARGO_GENERATE.md`, `xtask/README.md`, `cargo-generate.toml`, `scaffold/cargo-generate/post.rhai`, `xtask/src/*`, `config/Dockerfile` | - | Cargo-generate xtask PR moved rewrite logic into Rust and updated docs/runtime contracts. | `e4a290b` |

## Beads Activity

| bead | title | actions | final status | why it mattered |
|---|---|---|---|---|
| `soma-821` | Add shared error taxonomy and action parity metadata | Created, claimed, closed | closed | Tracked shared `ToolError`/`ServiceError`, REST wrapper, CLI metadata, parity tests, and inventory ignore work. |
| `soma-sk6` | Fix parallel soma-auth Google token test flake | Created, claimed, closed | closed | Tracked the order-sensitive Google token test fix and required default-threaded `cargo test` proof. |
| `soma-chl` | Generate volatile docs and metadata from canonical specs | Read during maintenance | closed | Recent session context for generated docs, OpenAPI, plugin settings, and script docs automation. |
| `soma-hi8` | Generate remaining plugin and action documentation surfaces | Read during maintenance | closed | Recent session context for plugin manifest generation, REST route metadata in `ACTION_SPECS`, and stale-claim checking. |
| `soma-lo4` | Merge generated-docs automation into main | Read during maintenance | closed | Explained the `185b0fd` merge that included generated-docs automation and the shared-error work. |

## Repository Maintenance

### Plans

`find docs/plans -maxdepth 2 -type f` returned no plan files, so no completed plans were moved and `docs/plans/complete/` was not created.

### Beads

`bd show` confirmed `soma-821` and `soma-sk6` are closed with reasons matching the observed implementation and verification. `bd dolt push` was run after both implementation commits before this save-session pass.

### Worktrees and branches

`git worktree list --porcelain` showed registered worktrees for `codex/cargo-generate-xtask-post`, `codex/generated-docs-automation`, `marketplace-no-mcp`, and `codex/xtask-scripts-migration`. No worktrees or branches were removed: `marketplace-no-mcp` is documented as long-lived, and the other worktrees have branch ownership or remote state that was not safe to delete inside a session-log request.

### Stale docs

Recent commits updated generated docs and cargo-generate docs. No additional stale docs were edited during the save-session pass; stale-doc review beyond files touched by the session was treated as out of scope and recorded here instead of making broad changes.

### Git state

`git fetch --prune` showed local `main` behind `origin/main` by four commits, with local `main` an ancestor of `origin/main`. `git pull --ff-only` fast-forwarded from `b1392e8` to `e4a290b` before this artifact was written.

## Tools and Skills Used

- **Skill:** `vibin:save-to-md` was used to perform the maintenance pass and generate this markdown artifact.
- **Shell commands:** Used `git`, `bd`, `cargo`, `rg`, `sed`, `nl`, `find`, `wc`, and `gh` for repo evidence, issue tracking, verification, and context collection.
- **File tools:** Used patch-based file edits for Rust changes and this generated session artifact.
- **Beads CLI:** Created, claimed, closed, and inspected beads for the implementation and test-flake work.
- **Rust toolchain:** Used `cargo fmt`, `cargo test`, and `cargo clippy -- -D warnings`; default parallel tests were required evidence for the auth flake fix.
- **External CLIs:** `gh pr view` returned `none` for an active PR during the maintenance pass.
- **MCP/tools/subagents:** Earlier visible session context mentions agent/subagent review work, but no current callable subagent output was used during this save-session pass.

## Commands Executed

| command | result |
|---|---|
| `bd create ... soma-821` / `bd update soma-821 --claim` | Created and claimed the shared taxonomy task. |
| `cargo test -p soma-contracts -p soma-service -p soma-api -p soma-cli -p soma-mcp` | Passed focused package tests after implementation. |
| `cargo test -p soma --test api_routes --test template_invariants` | Initially caught the optional `greet` body regression; passed after `optional_name_params`. |
| `cargo clippy -- -D warnings` | Passed after the shared taxonomy work and after the auth flake fix. |
| `cargo test` | Initially exposed a parallel `soma-auth` Google token flake; passed after the mock lifetime fix. |
| `cargo test -p soma-auth --lib` | Passed under default parallelism after the mock lifetime fix. |
| `for i in 1 2 3 4 5; do cargo test -p soma-auth google::tests::google_exchange_rejects_ -- --nocapture || exit 1; done` | Passed five repeated runs of the formerly flaky Google negative tests. |
| `git fetch --prune` / `git pull --ff-only` | Confirmed local `main` was an ancestor of `origin/main` and fast-forwarded to `e4a290b`. |
| `bd close soma-821` and `bd close soma-sk6` | Closed both beads with implementation and verification reasons. |

## Errors Encountered

- **REST optional parameter regression:** Converting `POST /v1/greet {}` into `{ "name": null }` made the shared parser reject the request. Fixed by omitting `name` when absent via `optional_name_params`.
- **Parallel auth test flake:** Full default `cargo test` failed with Google token errors such as local mock `404` or wrong validation reason. Root cause was a dropped `MockServer` in `mocked_google_provider_with_id_token`; fixed by returning a wrapper that owns the server.
- **Moving remote branch:** After pushing `b1392e8`, `origin/main` advanced through PR #43. Resolved safely with `git fetch --prune` and `git pull --ff-only` because local `main` was an ancestor.

## Behavior Changes (Before/After)

| area | before | after |
|---|---|---|
| REST errors | REST had local validation/internal error mapping. | REST maps service errors through shared `ToolError` with structured fields and status codes. |
| MCP tool errors | MCP had local validation/execution payload builders. | MCP uses shared `ToolError::to_mcp_payload` while preserving `kind=mcp_tool_error`. |
| CLI errors | CLI returned raw `anyhow` service errors. | CLI prints stable `error`, `code`, `kind`, `retryable`, and `remediation` lines. |
| Action metadata | CLI and REST metadata could drift from `ACTION_SPECS`. | CLI metadata and REST route metadata are registry-driven and tested. |
| Google OAuth tests | Mock Google provider helpers could drop their `MockServer` before use. | `MockedGoogleProvider` keeps WireMock alive for each provider. |

## Verification Evidence

| command | expected | actual | status |
|---|---|---|---|
| `cargo test -p soma-contracts -p soma-service -p soma-api -p soma-cli -p soma-mcp` | Focused shared-surface tests pass. | Passed. | pass |
| `cargo test -p soma --test api_routes --test template_invariants` | REST registry parity and invariants pass. | Passed after optional `greet` fix. | pass |
| `cargo test -p soma-auth --lib` | Auth tests pass with default test threading. | Passed, 133 tests. | pass |
| `for i in 1 2 3 4 5; do cargo test -p soma-auth google::tests::google_exchange_rejects_ -- --nocapture || exit 1; done` | Formerly flaky Google negative tests pass repeatedly. | Passed 5 runs. | pass |
| `cargo test` | Full workspace tests pass with default test threading. | Passed. | pass |
| `cargo clippy -- -D warnings` | No clippy warnings. | Passed. | pass |
| `git status --short --branch` | Clean tree on `main` before writing session artifact. | Clean after fast-forward to `origin/main`. | pass |

## Risks and Rollback

The error taxonomy touches all public action surfaces, so derived servers should validate REST, MCP, and CLI expectations after rebasing. Rollback path for the taxonomy is reverting merge commit `185b0fd` or a narrower revert of the shared error files and surface wiring. Rollback path for the auth test fix is reverting `b1392e8`, but that would restore the parallel test flake.

## Decisions Not Taken

- Did not delete registered worktrees or local branches during the maintenance pass; ownership and long-lived branch semantics were not clear enough for safe cleanup.
- Did not force serial test execution to hide the auth flake; fixed the fixture lifetime instead.
- Did not broadly rewrite stale docs during the save-session pass; generated docs had already been updated by the automation commits and broader doc drift needs separate scope.

## References

- Beads: `soma-821`, `soma-sk6`, `soma-chl`, `soma-hi8`, `soma-lo4`.
- Commits: `185b0fd Merge generated docs automation`, `b1392e8 Fix Google auth mock lifetime flake`, `e4a290b Merge pull request #43 from jmagar/codex/cargo-generate-xtask-post`.
- Transcript path observed by the skill: `/home/jmagar/.claude/projects/-home-jmagar-workspace-soma/8dd2c014-bb7a-46f4-941d-3d4510a9f94d.jsonl` with 169 lines. It appears to be a Claude-session transcript from 2026-05-28, not a full Codex transcript for the current work.

## Open Questions

- Whether the registered `codex/generated-docs-automation` and `codex/cargo-generate-xtask-post` worktrees should now be removed after their work merged. They were left intact because cleanup ownership was unclear.
- Whether `mcp-server-inventory.md` should remain tracked after PR #43 or be converted back to a local-only artifact in a follow-up.

## Next Steps

- Continue from `origin/main` at `e4a290b` for future work.
- If cleanup is desired, explicitly audit and remove merged worktrees/branches with owner approval.
- Keep running default-threaded `cargo test` for auth changes so parallel mock lifetime issues are caught before commit.

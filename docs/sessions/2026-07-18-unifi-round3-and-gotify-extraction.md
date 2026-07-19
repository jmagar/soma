---
date: 2026-07-18 21:21:07 EST
repo: git@github.com:jmagar/soma.git
branch: claude/gotify-integration-crate
head: 2c07950
working directory: /home/jmagar/workspace/soma/.claude/worktrees/soma-crate-structure-f70dc9
worktree: /home/jmagar/workspace/soma/.claude/worktrees/soma-crate-structure-f70dc9
pr: #166 "unifi: round-3 template hardening — tracing, docs, semver, changelog" (https://github.com/jmagar/soma/pull/166, merged as e6d37b3) and #169 "Extract crates/integrations/gotify from gotify-rmcp" (https://github.com/jmagar/soma/pull/169, merged as 9284b0b)
beads: rmcp-template-6kqz, rmcp-template-udc9
---

## User Request

Continue hardening `crates/integrations/unifi` (the `crates/integrations/*`
reference template) per a prior suggestions round, then extract the next
service — Gotify — using `unifi` as the template. Both to be pushed through
CI (`/vibin:gh-fix-ci ... and merge into main when green`) and merged
autonomously. This session picks up from an earlier checkpoint already
recorded in `docs/sessions/2026-07-17-unifi-integration-crate-template.md`
(crate-location design, initial `unifi` extraction, and a first hardening
round already landed as PR #163 before this session's visible work begins).

## Session Overview

Closed out all 7 items from a prior "make unifi a shining example" review
(tracing instrumentation consolidated into the one shared HTTP call site,
`README.md` promoted to the crate's entire rustdoc source via
`#![doc = include_str!(...)]`, `#[non_exhaustive]` extended across the
public error/enum surface with a proper `ActionRequest::new` constructor,
a per-crate `CHANGELOG.md`, `.cookie_store(true)` verified dead against a
real UniFi controller and removed, `#![forbid(unsafe_code)]`, a curated
clippy lint tier) as PR #166. Extracted `crates/integrations/gotify` from
`gotify-rmcp` applying every one of those patterns from the start, as PR
#169. Both PRs collided with a large, unrelated architecture-refactor batch
(PRs #151-164) that landed on `main` mid-session, requiring two rounds of
merge-conflict reconciliation. Getting both PRs' Windows CI green surfaced
and fixed three pre-existing, unrelated cross-platform bugs in code
neither PR originally touched. Both PRs are now merged into `main`.

## Sequence of Events

1. Ran a live-controller-verified investigation (via the `unifi` MCP tool
   backed by the still-deployed, unmodified `unifi-rmcp`) confirming a
   Connector-API path-double-prefix fix from an earlier round was correct
   by the crate's own `validate_connector_path` contract, and
   cross-validated the `/v1/sites/{siteId}/clients` URL-construction shape
   against real controller traffic.
2. Proposed 5 further hardening items for `unifi` (tracing coverage gap,
   README/`lib.rs` doc duplication, incomplete `#[non_exhaustive]`
   coverage, missing `CHANGELOG.md`, unverified `.cookie_store(true)`) plus
   2 lower-priority ones (`#![forbid(unsafe_code)]`, a curated clippy lint
   tier). User asked to address all 7 "thoroughly and COMPLETELY."
3. Investigated `.cookie_store(true)` empirically: built a client
   identical to the crate's own except without a cookie jar, and hit the
   real UniFi controller's internal and official APIs directly — every
   request succeeded identically and the controller never sent
   `Set-Cookie`. Removed the cookie store and reqwest's now-unneeded
   `cookies` feature, dropping 6 transitive dependencies workspace-wide.
4. Implemented the remaining 6 items (commit `85ad976`): tracing moved
   into `http::request_json` (the one function every dispatch path
   shares); `lib.rs` became `#![doc = include_str!("../README.md")]`;
   `ApiSourceFamily`/`AuthScope`/`Capability` marked `#[non_exhaustive]`;
   `ActionRequest` got a `new()` constructor before the same treatment
   (5 call sites migrated off the struct literal); `CHANGELOG.md` added;
   `#![forbid(unsafe_code)]` and
   `#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used,
   clippy::panic))]` added, with 5 narrowly-justified `#[allow]`s at the
   crate's existing build-time-only panic sites.
5. Opened PR #166. While verifying, discovered `crates/integrations/unifi/src`
   had never been registered with `xtask/src/test_siblings.rs`'s
   classification lists (dormant gap since the crate's original creation);
   fixed (`73ee9d1`).
6. Merged a large, unrelated architecture-refactor batch (PRs #151-164,
   "soma-architecture-refactor-plan-v3" — `apps/soma` split into
   `bootstrap.rs`, new `crates/soma/client`, `crates/soma/config`,
   `crates/soma/integrations`, `crates/soma/palette`,
   `crates/shared/cli-core`, `crates/shared/http-api`,
   `crates/shared/http-server`, `crates/shared/provider-adapters`,
   `crates/shared/tauri-shell`, a new `apps/palette` Tauri desktop app)
   into PR #166's branch to resolve a real git conflict (`644108d`): a
   duplicate `crates/integrations/unifi/src` entry (the refactor had
   independently found and fixed the same classification gap) and a
   mechanical `Cargo.lock` conflict, regenerated via
   `cargo check --workspace --all-targets` rather than hand-edited.
   Followed by a small fixup removing the now-duplicate entry (`6286c09`).
7. Windows CI then surfaced 3 unrelated, pre-existing bugs in the
   just-merged refactor code, fixed one at a time across several
   CI round-trips: `check_binary_in_path` never handling the Windows
   `.exe` suffix (`9f6ddb5`); the MSRV job's `timeout-minutes: 20` too
   short for the workspace's new size, raised to 40 (`5af599a`); a test
   fixture (`doctor_cli.rs`) hardcoding the Unix binary name in its
   pinned expected JSON, independent of the production bug (`651ec2e`);
   and a test hardcoding `HOME` (not set on Windows), swapped for
   Cargo-guaranteed `CARGO_PKG_NAME` (`3003a7e`).
8. PR #166 went fully green (24 checks) and was merged into `main`
   (`e6d37b3`).
9. On the user's direction ("do gotify"), extracted
   `crates/integrations/gotify` from `gotify-rmcp` (`87dc91c`) — deliberately
   simpler than `unifi` (no dynamic action dispatch; Gotify's ~13-operation
   API doesn't need it), applying every pattern from step 4 from the start.
   Opened PR #169.
10. PR #169 independently hit the exact same 3 pre-existing Windows bugs
    from step 7 (a sibling branch, not a descendant of round-3, so it
    never inherited those fixes). Reapplied the `doctor_cli.rs` fixture
    fix (`60224cc`) and the `gateway_tests.rs` `HOME` fix (`f187c3e`)
    directly; then discovered the *actual* `check_binary_in_path`
    production fix itself was missing here too (round-3 and gotify are
    siblings, not stacked) and applied it (`413e07e`).
11. Merged `main` (now including round-3) into PR #169's branch to resolve
    a second conflict in `xtask/src/test_siblings.rs` (`2c07950`) — same
    shape as step 6, a stale/duplicate `unifi` entry this branch still had
    queued for removal alongside its own new `gotify` entry.
12. One `Lefthook Speed` job failure traced to a transient GitHub Actions
    infrastructure glitch (`Missing file at path: .../set_output_...`
    immediately after checkout, no command ever ran) — reran, not a code
    issue.
13. PR #169 went fully green (24 checks) and was merged into `main`
    (`9284b0b`).
14. Closed both beads; flagged one related-but-dormant bug
    (`crates/shared/codemode/src/home.rs` has no `USERPROFILE` fallback
    for Windows, same bug class as step 7/10 but not currently causing a
    failure) as a separate background task rather than fixing it inline.

## Key Findings

- `crates/soma/cli/src/doctor/checks.rs:114` (`check_binary_in_path`):
  `dir.join(binary)` with no platform suffix handling meant this check
  always failed on Windows even when the binary was correctly built and on
  `PATH`, since Windows names it `soma.exe`. The crate's own pre-existing
  Windows unit test masked this for years by hardcoding `"cmd.exe"`
  instead of testing the bare-name path production code actually uses.
- `apps/soma/tests/doctor_cli.rs:127` (pre-fix): computed its pinned
  expected-JSON fixture as `bin_dir().join("soma")` — a second, independent
  instance of the same "no `.exe` on Windows" bug, this time in a test
  fixture rather than production code. Fixed by reusing `binary()`
  (`CARGO_BIN_EXE_soma`, the same path the check itself resolves against)
  instead of hand-reconstructing the expected filename.
- `crates/shared/provider-adapters/src/gateway_tests.rs:96` (pre-fix):
  `expand_env_templates_substitutes_multiple_variables` read `HOME` and
  asserted it must be set — true on Unix/macOS CI runners, `NotPresent` on
  this repo's Windows runner (confirmed empirically). Fixed with
  `CARGO_PKG_NAME`, verified empirically to be a genuine process
  environment variable Cargo sets for every `cargo test` run on every
  platform (not just a compile-time `env!()` macro).
- `.github/workflows/msrv.yml:62`: `timeout-minutes: 20` on the `msrv` job,
  too short once the workspace grew to include a Tauri desktop app
  (GTK/webkit2gtk/wasmtime) compiling from scratch under a cold,
  MSRV-pinned toolchain cache. Confirmed by two identical ~20-minute
  cancellations before the fix; passed in 11-15 minutes after raising it
  to 40.
- `crates/shared/codemode/src/home.rs`'s `soma_home()`/`home_dir()` read
  only `HOME`, no `USERPROFILE` fallback — same bug class as above but not
  currently exercised by a failing test. Flagged as a separate background
  task rather than fixed in-session (out of scope for "get these PRs
  green"; several other crates share the identical `HOME`-only pattern —
  see the task description for the full list).
- The concurrently-merged architecture refactor had independently found
  and partially fixed the same `xtask/src/test_siblings.rs` classification
  gap this session found for `unifi` — both sides added an entry for
  `crates/integrations/unifi/src` to `UNCHECKED_SRC_ROOTS`, producing a
  duplicate key that required manual merge resolution twice (once per PR).

## Technical Decisions

- Kept `unifi` and `gotify` on separate, sibling feature branches (not
  stacked) — each PR is independently reviewable and mergeable, at the
  cost of needing to reapply any fix discovered on one branch to the
  other by hand, which happened three times this session (the Windows CI
  bug class in Key Findings).
- Resolved both `main`-merge conflicts by taking `origin/main`'s
  structure as the base and re-adding only this session's own content
  (rather than trying to preserve branch-local formatting), since the
  incoming refactor was the larger, more authoritative change in both
  cases.
- Regenerated `Cargo.lock` via `cargo check --workspace --all-targets`
  after resolving `Cargo.toml`-level state, rather than hand-merging the
  lockfile's conflict markers — a large generated file, not something to
  reason about line-by-line.
- `check_binary_in_path` checks both the bare name and
  `format!("{binary}{}", std::env::consts::EXE_SUFFIX)` in every `PATH`
  directory (a harmless double-check on Unix, where `EXE_SUFFIX` is
  empty) rather than branching on `cfg(windows)`, keeping the function
  identical on every platform.
- `gotify` deliberately has no dynamic action-dispatch layer (unlike
  `unifi`'s `ActionDispatcher`) — Gotify's official API is ~13 fixed
  operations with no benefit from capability-catalog indirection. Recorded
  explicitly in `gotify`'s own README as the default shape, with `unifi`'s
  dispatcher reserved for services whose action count actually needs it.
- Did not fix `crates/shared/codemode/src/home.rs`'s `USERPROFILE` gap
  inline — flagged it instead, since it wasn't causing an observed CI
  failure and touching a shared, unrelated crate's core path-resolution
  logic was a larger, separate piece of work than "get these two PRs
  green."

## Files Changed

Session-authored changes only; the much larger set of files that arrived
via merging the concurrent architecture refactor (~350 paths per merge,
listed in full in that work's own session logs) are not this session's
authorship and are omitted here.

| Status | Path | Purpose | Evidence |
|---|---|---|---|
| modified | `crates/integrations/unifi/src/{lib,client,http,actions,api,capabilities}.rs` and submodules | Tracing consolidation, `#![doc = include_str!]`, `#[non_exhaustive]`, `#![forbid(unsafe_code)]`, curated clippy lints + 5 justified `#[allow]`s | `85ad976` |
| modified | `crates/integrations/unifi/src/http.rs` | Removed `.cookie_store(true)`, dropped `cookies` reqwest feature | `85ad976` |
| created | `crates/integrations/unifi/CHANGELOG.md` | Per-crate changelog, Keep a Changelog format | `85ad976` |
| modified | `crates/integrations/unifi/README.md`, `crates/integrations/README.md` | Documented every new pattern for the next extraction | `85ad976`, `73ee9d1` |
| modified | `crates/integrations/unifi/tests/action_dispatch.rs` | Migrated off `ActionRequest` struct literal to `ActionRequest::new` | `85ad976` |
| modified | `xtask/src/test_siblings.rs` | Registered `crates/integrations/unifi/src` in `UNCHECKED_SRC_ROOTS`; later deduped against the refactor's independent fix | `73ee9d1`, `6286c09` |
| modified | `.github/workflows/msrv.yml` | `timeout-minutes: 20` → `40` on the `msrv` job | `5af599a` |
| modified | `crates/soma/cli/src/doctor/checks.rs`, `checks_tests.rs` | `check_binary_in_path` now checks the platform `EXE_SUFFIX`; Windows unit test rewritten to exercise the bare-name path | `9f6ddb5` (round-3), `413e07e` (gotify, reapplied) |
| modified | `apps/soma/tests/doctor_cli.rs` | Pinned-fixture expected value now reuses `binary()` instead of hardcoding the Unix filename | `651ec2e` (round-3), `60224cc` (gotify, reapplied) |
| modified | `crates/shared/provider-adapters/src/gateway_tests.rs` | `HOME` → `CARGO_PKG_NAME` in the multi-variable substitution test | `3003a7e` (round-3), `f187c3e` (gotify, reapplied) |
| created | `crates/integrations/gotify/{Cargo.toml,LICENSE,README.md,CHANGELOG.md,src/**,tests/**}` | New crate: `GotifyClient`/`GotifyService`, typed non-exhaustive errors, consolidated tracing, configurable timeout, forbid(unsafe_code), curated clippy lints — every `unifi` pattern applied from the start | `87dc91c` |
| modified | `crates/integrations/README.md`, `xtask/src/test_siblings.rs`, `Cargo.toml`, `Cargo.lock` | Registered the new crate as a workspace member and classified its `src/` root | `87dc91c` |
| modified | `xtask/src/test_siblings.rs` | Resolved second merge conflict: kept `gotify` entry, dropped a stale duplicate `unifi` entry already superseded on `main` | `2c07950` |

## Beads Activity

- `rmcp-template-6kqz` — **created, claimed, closed**. "Round 3 unifi
  template hardening: tracing coverage, doc dedup, non_exhaustive surface,
  CHANGELOG, cookie_store audit, lint hardening." Closed with a summary of
  all 7 items plus the 3 incidental Windows CI fixes, on merge of PR #166.
- `rmcp-template-udc9` — **created, closed** (retroactively, after the
  work was already done — CLAUDE.md's bead-before-code convention wasn't
  followed at the start of the gotify extraction; documented after the
  fact rather than left unrecorded). "Extract crates/integrations/gotify
  from gotify-rmcp." Closed on merge of PR #169.
- No other beads were created, claimed, or modified. `bd list
  --status=in_progress` at session end shows 2 pre-existing, unrelated
  items (`rmcp-template-56f` "Repair main CI — main red since 2026-05-31",
  `rmcp-template-xse` "Land pending local WIP: cargo-rustc-wrapper
  binary-sync") — neither touched this session, left as-is.

## Repository Maintenance

- **Plans**: `docs/plans/` does not exist in this repository (confirmed
  via `ls docs/plans/` — no such file or directory). Nothing to move.
- **Beads**: see Beads Activity above; `bd dolt push` run at session end,
  confirmed "Push complete."
- **Worktrees/branches**: this session's two branches
  (`claude/unifi-template-round3`, `claude/gotify-integration-crate`) are
  now both fully merged into `main` (confirmed via successful `gh pr merge`
  on both and `origin/main` containing both merge commits). Left both the
  local branches and their worktrees in place — this session's own
  worktree (`soma-crate-structure-f70dc9`) is the one this log is being
  written from, and no explicit cleanup request was made. Safe to remove
  once convenient. Five other worktrees exist for unrelated, apparently
  active work (`incus-api-crate-d65a18`, `oauth-provider-support-f427c9`,
  `traces-crate-followups-01d8e6`, `cortex-auto-update-crate-review`, plus
  the protected `marketplace-no-mcp`) — none inspected or touched, out of
  scope.
- **Stale docs**: `crates/integrations/README.md` and both crates'
  `README.md`/`CHANGELOG.md` were kept current as part of the session's own
  commits (each fix immediately propagated into the checklist/docs, see
  Technical Decisions). No other documentation was identified as
  contradicted by this session's changes.

## Tools and Skills Used

- **Shell (Bash)**: `git` (status/diff/log/merge/push extensively, including
  two real conflict resolutions), `cargo` (build/test/clippy/fmt/doc/xtask
  checks, run repeatedly across both branches and post-merge states), `gh`
  (`pr view`/`pr checks --watch`/`pr merge`/`run rerun`/`api` for raw job
  logs), `bd` (beads).
- **File tools**: Read/Edit/Write across both crates' source, tests,
  manifests, and docs, plus the shared `xtask` and workflow files touched
  by CI fixes.
- **MCP**: `mcp__plugin_unifi_unifi__unifi` (the live, deployed `unifi-rmcp`
  server against the user's real UniFi controller) — used early in the
  session to verify the Connector-path fix and empirically test whether
  `.cookie_store(true)` was load-bearing (it wasn't).
- **ToolSearch**: used to load the `unifi` MCP tool's schema before first
  use.
- **Skills**: `vibin:save-to-md` (this document; also produced the earlier
  checkpoint at `docs/sessions/2026-07-17-unifi-integration-crate-template.md`).
- **Background task tooling**: `gh pr checks --watch` run repeatedly in the
  background across both PRs (7+ watch cycles total) rather than
  synchronously blocking, since individual CI runs took 15-25 minutes;
  several watch invocations surfaced transient `gh`/network errors
  distinct from real check failures, disambiguated each time by a direct
  `gh pr checks <n>` re-query rather than trusting the watch command's own
  exit code.
- No browser automation, Artifact, or Workflow tool was used this session.

## Commands Executed

| Command | Result |
|---|---|
| Live-controller probe via `mcp__plugin_unifi_unifi__unifi` and a throwaway `reqwest` client without `cookie_store` | Confirmed `.cookie_store(true)` dead: identical success on every request, controller never sent `Set-Cookie` |
| `cargo test -p unifi` (post round-3) | 81 tests green (64 unit + 5 dispatch + 10 client + 2 doctests) |
| `cargo xtask check-test-siblings` | Failed once (`crates/integrations/unifi/src` unclassified), clean after `73ee9d1` |
| `git merge origin/main` (round-3 branch) | `CONFLICT` in `Cargo.lock` and `xtask/src/test_siblings.rs`; resolved via regeneration + manual dedup |
| `cargo check --workspace --all-targets` (post-merge, regenerating `Cargo.lock`) | Succeeded after ~2 minutes (workspace grew to 31 packages) |
| `gh pr checks 166 --watch` (repeated) | Cycled through Windows/MSRV failures across ~5 pushes before going fully green |
| `gh run rerun <id> --failed` (×2, MSRV then Lefthook Speed) | Both transient; reran clean |
| `gh pr merge 166 --merge` | Merged as `e6d37b3` |
| `crates/integrations/gotify` extraction + `cargo test -p gotify` | 21 tests green (7 unit + 13 integration + 1 doctest) |
| `git merge origin/main` (gotify branch) | `CONFLICT` in `xtask/src/test_siblings.rs` only (`Cargo.lock`/`README.md` auto-merged); resolved |
| `gh pr merge 169 --merge` | Merged as `9284b0b` |
| `bd close rmcp-template-6kqz`, `bd close rmcp-template-udc9`, `bd dolt push` | Both closed with merge-linked reasons; push complete |

## Errors Encountered

- `gh pr checks <n> --watch` failed with exit code 1 on 6 separate
  occasions across the session. Each time, a direct `gh pr checks <n>`
  re-query showed either a real check failure (handled — see below) or
  (once) a transient `error connecting to api.github.com` unrelated to any
  actual CI state.
- MSRV job canceled twice at ~20 minutes ("The operation was canceled"),
  root-caused to `timeout-minutes: 20` in `.github/workflows/msrv.yml`
  being too short for the post-refactor workspace size, not a compile
  error. Fixed by raising it to 40; both subsequent runs completed in
  11-15 minutes.
- Windows `Build Windows` job failed 4 separate times across both PRs,
  each for a different root cause (see Key Findings): `check_binary_in_path`
  missing `.exe`, the `doctor_cli.rs` fixture hardcoding the Unix filename,
  `gateway_tests.rs` hardcoding `HOME`, and (on the gotify branch
  specifically) the `check_binary_in_path` fix itself being absent since
  gotify was a sibling, not descendant, branch of round-3.
- `Lefthook Speed` failed once with `##[error]Missing file at path:
  .../_runner_file_commands/set_output_...` immediately after checkout,
  before any real command ran — a GitHub Actions/self-hosted-runner
  infrastructure glitch, not a code issue. Reran clean.
- `crates/shared/codemode` had one unrelated, pre-existing flaky test
  (`budget_rejects_operations_over_configured_limit`) discovered while
  running the full workspace test suite once — passed cleanly in
  isolation, only flaked under full parallel load. Not touched (out of
  scope, unrelated crate).

## Behavior Changes (Before/After)

| Area | Before | After |
|---|---|---|
| `unifi` tracing | Only the 8 named `UnifiClient` methods were instrumented (via a private `get()` wrapper) | All ~236 dynamically-dispatched actions and the 8 named methods are instrumented consistently, via `http::request_json` |
| `unifi`/`lib.rs` docs | Separate `//!` summary duplicating `README.md`'s quickstart/module-layout content | `#![doc = include_str!("../README.md")]` — one source of truth, both README code examples are real doctests |
| `unifi` public error/enum surface | Only `UnifiError` was `#[non_exhaustive]` | `ApiSourceFamily`, `AuthScope`, `Capability` also `#[non_exhaustive]`; `ActionRequest` has a `new()` constructor and is `#[non_exhaustive]` too |
| `unifi` HTTP client | `.cookie_store(true)` set, unverified | Removed; verified dead against a real controller; `cookies` reqwest feature dropped |
| `crates/soma/cli`'s `doctor` command | `check_binary_in_path` always failed on Windows regardless of whether the binary was correctly installed | Correctly resolves `soma.exe` via `EXE_SUFFIX` on Windows, `soma` unchanged on Unix |
| MSRV CI job | 20-minute timeout, canceled twice on the larger post-refactor workspace | 40-minute timeout, completes in 11-15 minutes |
| `crates/integrations/*` | Only `unifi` existed | `gotify` added, second crate proving out the template's patterns |

## Verification Evidence

| Command | Expected | Actual | Status |
|---|---|---|---|
| `cargo test -p unifi` | all pass | 81/81 | pass |
| `cargo test -p soma-provider-adapters` | all pass | 6/6 | pass |
| `cargo test -p soma-cli doctor` / `cargo test -p soma --test doctor_cli` | all pass | 21/21, 2/2 | pass |
| `cargo test -p gotify` | all pass | 21/21 (7 unit + 13 integration + 1 doctest) | pass |
| `cargo xtask check-test-siblings` | no unclassified members | 16 trees classified, 0 unclassified (post round-3); 16 trees (post gotify merge) | pass |
| `cargo xtask check-architecture` | passes | 31 workspace packages, 83 internal edges | pass |
| `cargo fmt --check` / `cargo clippy --workspace --all-targets -D warnings` | clean | clean (both branches, post-merge) | pass |
| `gh pr checks 166` (final) | all required checks green | 24/24 green, `CLEAN`/`MERGEABLE` | pass |
| `gh pr checks 169` (final) | all required checks green | 24/24 green, `CLEAN`/`MERGEABLE` | pass |

## Risks and Rollback

- Both merges are standard (non-squash) merge commits on `main`
  (`e6d37b3`, `9284b0b`); rollback would be `git revert -m 1 <sha>` for
  either if needed.
- The `check_binary_in_path` fix changes real production behavior (a
  previously-always-failing Windows check now passes) — low risk, since
  the prior behavior was unconditionally wrong on Windows, not a
  deliberate design choice.
- The `unifi`/`gotify` `#[non_exhaustive]` additions are semver-relevant
  but both crates are `publish = false` and have no external consumers
  yet, so there is no live compatibility impact.
- No production deployment was touched; both crates remain unpublished
  and neither is yet a dependency of the shipped `soma` binary.

## Decisions Not Taken

- Did not fix `crates/shared/codemode/src/home.rs`'s missing `USERPROFILE`
  fallback inline — spawned as a separate background task instead, since
  it wasn't causing an observed failure and is a distinct, unrelated
  crate's concern.
- Did not investigate or fix the pre-existing `crates/shared/codemode`
  flaky test (`budget_rejects_operations_over_configured_limit`) —
  unrelated to either PR, passes in isolation, only flakes under full
  parallel test load.
- Did not delete either merged feature branch or its worktree — both are
  safely merged, but no cleanup was explicitly requested and this
  session's own worktree is one of them.

## References

- PR #166: https://github.com/jmagar/soma/pull/166 (merged as `e6d37b3`)
- PR #169: https://github.com/jmagar/soma/pull/169 (merged as `9284b0b`)
- Prior checkpoint: `docs/sessions/2026-07-17-unifi-integration-crate-template.md`
- `crates/integrations/README.md`, `crates/integrations/unifi/README.md`,
  `crates/integrations/gotify/README.md`

## Open Questions

- Whether `crates/shared/codemode/src/home.rs` and the other
  `HOME`-only-no-`USERPROFILE` sites named in the spawned follow-up task
  (`crates/shared/auth/src/config.rs:501`,
  `crates/shared/mcp/gateway/src/config/defaults.rs:42`,
  `crates/soma/config/src/config.rs:311`) should all be fixed in one pass
  or independently.
- Whether the `Lefthook Speed` infrastructure glitch
  (`Missing file at path: .../set_output_...`) is a known, recurring
  self-hosted-runner issue worth a standing retry/alert, or a one-off.

## Next Steps

- **Immediate**: none blocking — both PRs are merged, working tree clean,
  beads closed and pushed.
- **Follow-on (flagged, not started)**: add a `USERPROFILE` fallback to
  `crates/shared/codemode/src/home.rs` (and decide whether to fix the
  sibling `HOME`-only sites in the same pass) — tracked as a spawned
  background task, not a bead.
- **Follow-on (discussed, not started)**: extract the next
  `crates/integrations/*` crate. Tailscale or Apprise (both simple REST
  APIs) were suggested as good next candidates over unraid (GraphQL) or
  yarr (11 services), which are better attempted once the template's
  proven on one or two more straightforward services.
- **Recommended immediate next command**: none required.

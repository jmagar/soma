---
date: 2026-07-12 16:42:49 EST
repo: git@github.com:jmagar/soma.git
branch: main
head: 04b2c2a719a3e7c7a8eb1fbb9b0e9ecfb932836f
working directory: /home/jmagar/workspace/soma
worktree: /home/jmagar/workspace/soma
pr: "#114 Fix provider and release CI gates (https://github.com/jmagar/soma/pull/114)"
beads: rmcp-template-0899, rmcp-template-f3uk, rmcp-template-veus, rmcp-template-rwx2, rmcp-template-4pcv, rmcp-template-rk1w, rmcp-template-hdtx
---

# PR #114 Lavra review and merge

## User Request

Run `lavra:lavra-review` on the PR, address all issues surfaced by review, fix CI, merge it, and save the session to markdown.

## Session Overview

PR #114, "Fix provider and release CI gates", was reviewed, remediated, verified, merged, and synced locally. The final PR merge commit is `c8a153e7929d59b2f143935cecaac7ae6ab7e685`, merged at `2026-07-12T14:26:36Z`.

The branch landed fixes across provider runtime refresh, Python provider execution, release workflow safety, registry metadata checks, and Windows CI stability. After merge, a main-branch Trivy action pin fix was present at local `HEAD` as `04b2c2a`.

## Sequence of Events

1. Reviewed PR #114 with Lavra review workflow and tracked findings in Beads.
2. Implemented provider runtime and release workflow fixes on `codex/pr101-lavra-review-fixes`.
3. Repeated local verification and CI remediation until all required PR checks passed.
4. Confirmed PR #114 was merged on GitHub and that local `main` contained merge commit `c8a153e`.
5. Synced local `main`; first pull hit an SSH ControlPath issue, then succeeded with SSH multiplexing disabled.
6. Ran the save-to-md repository maintenance pass and created this session artifact.

## Key Findings

- Provider refresh fingerprints needed to track Python dependency modules and sidecar-backed WASM manifests, not only direct provider files.
- Manual release workflows needed validated `refs/tags/v*` refs, tag ancestry checks, version sync gates, and correct publish ordering.
- Provider validate/inspect report construction belonged in `soma-service`, not the CLI shim.
- Provider execution envelope shape needed direct ABI coverage for AI SDK, Python, and WASM providers.
- Windows provider CI required platform-aware Python launcher and sidecar command resolution behavior.
- The local checkout had unrelated dirty files at save time: `README.md`, `docs/RMCP_README_GUIDE.md`, and `scripts/check-readme-guide.py`.

## Technical Decisions

- Provider report logic was moved to service-owned APIs so CLI, MCP, REST, and other surfaces share one source of truth.
- Python bridge code was split into a sibling module and test file to satisfy repository structure and size checks.
- Release metadata validation was aligned to the current npm-only MCP registry manifest instead of stale OCI metadata paths.
- Provider smoke tests were adjusted so CI does not depend on TypeScript sidecar execution availability where the runner lacks that runtime path.
- Worktree cleanup was limited to `git worktree prune` for a proven non-existent worktree path.

## Files Changed

| status | path | previous path | purpose | evidence |
|---|---|---|---|---|
| modified | `.github/workflows/ci.yml` | - | Configure Windows Python launcher and CI behavior | PR #114 file list |
| modified | `.github/workflows/docker-publish.yml` | - | Validate release refs and registry metadata publishing | PR #114 file list |
| modified | `.github/workflows/release.yml` | - | Harden release tag validation and artifact publish ordering | PR #114 file list |
| modified | `crates/soma-cli/src/lib.rs` | - | Remove CLI-owned provider report construction | PR #114 file list |
| modified | `crates/soma-service/src/provider_registry.rs` | - | Export provider registry report support | PR #114 file list |
| modified | `crates/soma-service/src/provider_registry/refresh.rs` | - | Refresh behavior updates | PR #114 file list |
| created | `crates/soma-service/src/provider_registry/refresh_tests.rs` | - | Refresh regression coverage | PR #114 file list |
| created | `crates/soma-service/src/provider_registry/reports.rs` | - | Service-owned provider report models | PR #114 file list |
| created | `crates/soma-service/src/provider_registry/reports_tests.rs` | - | Report behavior tests | PR #114 file list |
| modified | `crates/soma-service/src/providers.rs` | - | Provider module wiring | PR #114 file list |
| modified | `crates/soma-service/src/providers/ai_sdk.rs` | - | AI SDK provider execution and envelope handling | PR #114 file list |
| modified | `crates/soma-service/src/providers/filesystem.rs` | - | Provider fingerprint dependency tracking | PR #114 file list |
| modified | `crates/soma-service/src/providers/filesystem_tests.rs` | - | Filesystem fingerprint regression tests | PR #114 file list |
| modified | `crates/soma-service/src/providers/mcp.rs` | - | MCP provider behavior alignment | PR #114 file list |
| modified | `crates/soma-service/src/providers/python.rs` | - | Move bridge implementation out and keep provider wrapper focused | PR #114 file list |
| created | `crates/soma-service/src/providers/python_bridge.rs` | - | Python sidecar bridge implementation | PR #114 file list |
| created | `crates/soma-service/src/providers/python_bridge_tests.rs` | - | Python bridge sibling tests | PR #114 file list |
| modified | `crates/soma-service/src/providers/python_tests.rs` | - | Python provider tests | PR #114 file list |
| modified | `crates/soma-service/src/providers/sidecar.rs` | - | Sidecar command resolution and environment handling | PR #114 file list |
| modified | `crates/soma-service/src/providers/sidecar_tests.rs` | - | Sidecar launcher tests | PR #114 file list |
| modified | `crates/soma/tests/ai_sdk_provider.rs` | - | AI SDK envelope and provider behavior tests | PR #114 file list |
| modified | `crates/soma/tests/architecture_boundaries.rs` | - | CLI/service boundary assertions | PR #114 file list |
| modified | `crates/soma/tests/drop_provider_probe.rs` | - | Provider drop-in probe stabilization | PR #114 file list |
| modified | `crates/soma/tests/mcp_provider.rs` | - | MCP provider test alignment | PR #114 file list |
| modified | `crates/soma/tests/plugin_contract.rs` | - | Registry package contract alignment | PR #114 file list |
| modified | `crates/soma/tests/provider_registry.rs` | - | Provider registry report assertions | PR #114 file list |
| modified | `crates/soma/tests/python_provider.rs` | - | Python provider dependency and schema tests | PR #114 file list |
| modified | `crates/soma/tests/wasm_provider.rs` | - | WASM envelope tests | PR #114 file list |
| modified | `crates/soma/tests/workflow_shapes.rs` | - | Workflow guard and newline normalization tests | PR #114 file list |
| modified | `release-please-config.json` | - | Remove stale package path tracking | PR #114 file list |
| modified | `release/components.toml` | - | Track npm registry metadata version shape | PR #114 file list |
| modified | `xtask/src/release_versions.rs` | - | Release version check changes | PR #114 file list |
| created | `xtask/src/release_versions_identifiers.rs` | - | Split release identifier version helpers | PR #114 file list |
| modified | `xtask/src/release_versions_manifest.rs` | - | Manifest version check alignment | PR #114 file list |
| modified | `xtask/src/release_versions_tests.rs` | - | Release version tests | PR #114 file list |
| modified | `xtask/src/rmcp_release_monitor.rs` | - | Root-relative monitor impact scan | PR #114 file list |
| created | `docs/sessions/2026-07-12-pr114-lavra-review-merge.md` | - | This generated session log | save-to-md |

## Beads Activity

| bead | title | action(s) | final status | why it mattered |
|---|---|---|---|---|
| `rmcp-template-0899` | PR-101: provider refresh fingerprint misses sidecar dependencies | Closed with comment | closed | Tracks provider refresh fingerprint bug fixed in PR #114 |
| `rmcp-template-f3uk` | PR-101: release reruns trust unsafe refs and publish out of order | Closed with comment | closed | Tracks release workflow safety and ordering fixes |
| `rmcp-template-veus` | PR-101: installer asset names drift from release artifacts | Closed with comment | closed | Tracks installer/release artifact name mismatch |
| `rmcp-template-rwx2` | PR-101: provider report logic belongs in soma-service | Closed with comment | closed | Tracks service boundary remediation |
| `rmcp-template-4pcv` | PR-101: provider execution envelope lacks direct ABI guard | Closed with comment | closed | Tracks provider ABI regression coverage |
| `rmcp-template-rk1w` | PR-101: CI regressions surfaced after merge | Closed with comment | closed | Tracks CI failures fixed in PR #114 |
| `rmcp-template-hdtx` | Fix main CI blocking release-please | Closed after merge and main fix | closed | Tracks post-merge main CI and release-please blockers |

Command evidence: targeted `bd show` reads reported all listed beads as `closed`; `bd list --status open --label pr-101 --json` returned `[]`.

## Repository Maintenance

### Plans

`find docs/plans -maxdepth 2 -type f` returned no files, so no completed plan files were moved.

### Beads

No bead state changes were needed during the save pass. Relevant PR #114 beads were already closed and `bd list --status open --label pr-101 --json` returned an empty list.

### Worktrees and branches

`git worktree list --porcelain` initially reported a prunable metadata record:

```text
worktree /home/jmagar/workspace/template-rmcp/.worktrees/provider-drop-in-ux
branch refs/heads/codex/provider-drop-in-ux
prunable gitdir file points to non-existent location
```

`git worktree prune --verbose` removed only that stale metadata record. Other worktrees were left in place because they are real paths or detached worktrees with unclear ownership. Local and remote branches were inspected with `git branch -vv` and `git branch -r -vv`; no branch deletion was performed.

### Stale docs

`README.md`, `docs/RMCP_README_GUIDE.md`, and `scripts/check-readme-guide.py` were dirty before the session artifact was committed. Their diffs update related-server links and add README guide validation for linked related-server entries. They were left untouched and excluded from the session commit because ownership was unclear.

### Transparency

The session artifact commit stages and commits only this file. Existing dirty work in `README.md`, `docs/RMCP_README_GUIDE.md`, and `scripts/check-readme-guide.py` remains outside the commit.

## Tools and Skills Used

- **Skills.** `lavra:lavra-review` for PR review remediation; `vibin:gh-fix-ci` for CI follow-through; `vibin:save-to-md` for this session artifact.
- **Shell and Git.** Used for status checks, branch/worktree inspection, commit containment checks, pull/sync, and worktree pruning.
- **GitHub CLI.** Used for PR metadata, PR checks, merge status, CI job state, and PR file/commit inventory.
- **Beads CLI.** Used to inspect and confirm relevant tracker state.
- **Review agents.** The session used PR review toolkit agents as requested earlier in the conversation; the final remediation was verified through local tests and GitHub checks.

## Commands Executed

| command | result |
|---|---|
| `gh pr checks 114 --watch=false` | All required checks passed; Dependabot auto-merge job skipped as expected |
| `gh pr view 114 --json state,mergedAt,mergeCommit,title,url` | PR #114 reported `MERGED` with merge commit `c8a153e` |
| `git branch --contains c8a153e7929d59b2f143935cecaac7ae6ab7e685` | `main` contains the merge commit |
| `git pull --ff-only` | Failed once due local SSH ControlPath issue |
| `GIT_SSH_COMMAND='ssh -o ControlMaster=no -o ControlPath=none' git pull --ff-only` | Succeeded; repository already up to date |
| `git status --short --branch` | On `main...origin/main`; only pre-existing dirty `docs/RMCP_README_GUIDE.md` remained |
| `git worktree prune --verbose` | Removed stale metadata for non-existent provider-drop-in-ux worktree |
| `bd list --status open --label pr-101 --json` | Returned `[]` |

## Errors Encountered

- `git pull --ff-only` failed once with `unix_listener: cannot bind to path /tmp/ssh_mux_...: No such file or directory`. Retried with `GIT_SSH_COMMAND='ssh -o ControlMaster=no -o ControlPath=none'`, which succeeded.
- Earlier CI iterations surfaced Windows sidecar launcher failures, release metadata drift, TypeScript sidecar runtime assumptions, and workflow shape drift. Those were fixed before PR #114 merged.
- `bd list --all --sort updated --reverse --limit 100 --json` produced a large truncated output in the terminal view; targeted `bd show` reads were used for the relevant beads.

## Behavior Changes (Before/After)

| area | before | after |
|---|---|---|
| Provider refresh | Dependency changes could be missed for Python imports or sidecar-backed WASM manifests | Fingerprints include relevant dependency files and sidecar manifests |
| Provider reports | CLI owned validate/inspect report logic | `soma-service` owns report models and tests |
| Provider execution ABI | Envelope shape was not directly asserted across all runtimes | AI SDK, Python, and WASM paths have envelope regression coverage |
| Release workflows | Manual reruns and publishing had unsafe or stale assumptions | Workflows validate tags, ancestry, versions, and publish ordering |
| Windows CI | Sidecar launcher and Python command behavior failed in CI | Windows build and test checks pass |

## Verification Evidence

| command | expected | actual | status |
|---|---|---|---|
| `cargo fmt --all -- --check` | Formatting clean | Passed earlier in PR remediation | pass |
| `cargo clippy -p soma-service -p soma --all-features -- -D warnings` | No warnings | Passed earlier in PR remediation | pass |
| `cargo test -p soma-service providers::python -- --nocapture` | Python provider service tests pass | Passed earlier in PR remediation | pass |
| `cargo test -p soma --test python_provider -- --nocapture` | Python integration tests pass | Passed earlier in PR remediation | pass |
| `cargo test -p soma --test workflow_shapes -- --nocapture` | Workflow shape guards pass | Passed earlier in PR remediation | pass |
| `cargo test -p xtask release_versions -- --nocapture` | Release version checks pass | Passed earlier in PR remediation | pass |
| `cargo xtask check-version-sync` | Version metadata in sync | Passed earlier in PR remediation | pass |
| `gh pr checks 114 --watch=false` | Required checks green | All required checks passed, including `Build Windows` and `CI Gate` | pass |
| `git branch --contains c8a153e7929d59b2f143935cecaac7ae6ab7e685` | `main` contains merge commit | `main` contains merge commit | pass |

## Risks and Rollback

- Release workflow changes affect publication behavior. Rollback path is to revert merge commit `c8a153e` or a narrower follow-up commit for release workflow files.
- Provider runtime changes touch multiple execution paths. Rollback path is to revert the PR and rerun provider tests before shipping.
- `README.md`, `docs/RMCP_README_GUIDE.md`, and `scripts/check-readme-guide.py` remain dirty and uncommitted; do not assume this session artifact reflects or owns those diffs.

## Decisions Not Taken

- Did not delete local or remote branches after merge because several branches/worktrees had unclear ownership.
- Did not edit or commit `README.md`, `docs/RMCP_README_GUIDE.md`, or `scripts/check-readme-guide.py` because those dirty diffs pre-existed this save request or had unclear ownership.
- Did not create new beads during the save pass because all relevant PR #114 and CI-follow-up beads were already closed.

## References

- PR #114: https://github.com/jmagar/soma/pull/114
- Merge commit: `c8a153e7929d59b2f143935cecaac7ae6ab7e685`
- Current local head during save: `04b2c2a719a3e7c7a8eb1fbb9b0e9ecfb932836f`
- CI run for final PR checks: https://github.com/jmagar/soma/actions/runs/29195720060

## Open Questions

- Ownership and intended commit path for the existing `README.md`, `docs/RMCP_README_GUIDE.md`, and `scripts/check-readme-guide.py` diffs is not established by this session.
- Detached worktrees `/home/jmagar/workspace/soma-ci-fix`, `/home/jmagar/workspace/soma-pr100-fixes`, and `/home/jmagar/workspace/template-rmcp-pr100-review` were not classified as safe to delete.

## Next Steps

- Decide whether to commit, revise, or discard the existing related-server README guide changes in a separate task.
- If branch cleanup is desired, inspect PR status and merge ancestry for old local and remote branches before deletion.

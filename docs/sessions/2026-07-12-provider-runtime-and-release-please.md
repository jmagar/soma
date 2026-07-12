---
date: 2026-07-12 07:33:37 EST
repo: git@github.com:jmagar/soma.git
branch: main
head: f6b767c
working directory: /home/jmagar/workspace/soma
worktree: /home/jmagar/workspace/soma f6b767c [main]
beads: rmcp-template-ihhp, rmcp-template-pt4o, rmcp-template-7rc0, rmcp-template-u5ej
---

# Provider runtime and release-please session

## User Request

The session started with questions about how the `.wasm` provider works, how a `.wasm` module is created, and whether the current provider system had broader improvements worth making. It then expanded into implementing those provider-system improvements, updating release-please according to `/home/jmagar/docs/ai/release-please.md`, making sure `server.json` participated in releases, merging to `main`, and saving the session.

## Session Overview

Implemented and landed provider runtime polish plus a release-please migration for Soma. The provider work added runtime hardening, clearer provider provenance, sidecar manifest handling for Wasm, provider CLI operations, refresh caching, and generated surface checks. The release work replaced the auto-tag release path with release-please, added the manifest/config pair, wired release PR fixups, moved artifact workflows to `release: published`, and ensured `server.json` and OCI image metadata stay synced.

At save time, `main` was clean at `f6b767c` and matched `origin/main`. A stale documentation snippet in `docs/PATTERNS.md` was found and tracked as follow-up bead `rmcp-template-u5ej` instead of being edited inside the session-log-only commit.

## Sequence of Events

1. Reviewed the `.wasm` provider model: Soma loads `.wasm` files as trusted providers, reads provider metadata from a `soma.provider` custom section or `.wasm.json` sidecar, executes calls through Wasmtime, serializes JSON input, and validates JSON output.
2. Reviewed the provider architecture and implemented the agreed provider-system improvements, including ABI/runtime envelope behavior, consistent error provenance, CLI provider operations, sidecar support, refresh caching, and generated provider docs.
3. Verified the provider work with targeted provider tests, generated-surface checks, version sync checks, and full workspace tests, then closed `rmcp-template-ihhp`.
4. Read `/home/jmagar/docs/ai/release-please.md` and migrated Soma to the documented release-please pattern, including config/manifest files, a CI-gated release-please workflow, fixup jobs, Dependabot `deps(...)` commit messages, and `release: published` artifact workflows.
5. Updated release inventory and tests so `server.json`, npm package metadata, OpenAPI metadata, Cargo lock/package versions, and OCI identifiers are checked as one release component.
6. Confirmed the release-please commit was already on `main` and `origin/main`, then observed later stabilization commit `f6b767c` on `main`.
7. Ran the save-session maintenance pass: checked plans, beads, worktrees, branches, stale docs, active PR state, recent commits, dirty state, and session transcript availability.

## Key Findings

- `.github/workflows/release-please.yml:15` runs release-please after the `CI` workflow completes on `main`; lines 43-53 require `RELEASE_PLEASE_TOKEN` and use the pinned release-please action.
- `.github/workflows/release-please.yml:71` runs `cargo xtask sync-release-please-version`, regenerates provider surfaces, and checks version/provider-surface sync before committing release PR fixups.
- `release-please-config.json:3` intentionally uses `release-type: simple`; `release-please-config.json:69` updates repo-specific version files through `extra-files`.
- `release/components.toml:30` through `release/components.toml:35` now model `server.json` root/package versions and both OCI image identifiers, including the nested MCP publisher metadata.
- `README.md:83` documents `.json`, `.ts`, `.py`, and `.wasm` dynamic provider loading; `README.md:280` lists supported provider kinds.
- `crates/soma-service/src/providers/filesystem.rs:277` loads a `.wasm.json` sidecar manifest when present; `crates/soma-service/src/providers/filesystem_tests.rs:9` covers this behavior.
- `docs/PATTERNS.md:1680` still says `release.yml` mutates `server.json` on tag. That is stale under the release-please flow and is tracked by `rmcp-template-u5ej`.

## Technical Decisions

- Kept release-please as the release authority, with `cargo xtask sync-release-please-version` handling repo-specific derived files that release-please does not natively update.
- Used `release-type: simple` because Soma's shipped version spans a binary crate, `Cargo.lock`, npm package metadata, generated OpenAPI, `server.json`, and OCI identifiers rather than one native release-please package file.
- Left old `check-release-versions` and `release-plan` xtask commands available as diagnostics/backcompat, but removed them from active release and pre-release paths.
- Did not delete stale worktrees or branches during the save-session maintenance pass because some entries were missing/prunable metadata, detached PR review worktrees, or branches with unclear ownership.
- Created a bead for the stale docs snippet instead of editing `docs/PATTERNS.md` because the save-to-md contract commits only the generated session artifact.

## Files Changed

| status | path | previous path | purpose | evidence |
|---|---|---|---|---|
| modified | `.github/workflows/docker-publish.yml` | - | Move Docker/MCP registry publishing to release-published/tag-aware flow and update provider/release behavior. | `git diff-tree -r 94c4349`, `git diff-tree -r f471390` |
| deleted | `.github/workflows/npm-publish.yml` | - | Fold npm publish behavior into the release flow. | `git diff-tree -r 94c4349` |
| modified | `.github/workflows/release.yml` | - | Run artifacts from release-please-published releases and manual tag reruns. | `git diff-tree -r 94c4349`, `git diff-tree -r f471390` |
| modified | `README.md` | - | Document Soma provider/runtime/distribution behavior. | `git diff-tree -r 94c4349`; lines 83, 280, 653 |
| modified | `crates/soma-cli/src/cli_tests.rs` | - | Add provider CLI regression coverage. | `git diff-tree -r 94c4349` |
| modified | `crates/soma-cli/src/lib.rs` | - | Add provider CLI operations and parsing behavior. | `git diff-tree -r 94c4349` |
| modified | `crates/soma-service/src/provider_errors.rs` | - | Add provider error provenance/redaction behavior. | `git diff-tree -r 94c4349` |
| modified | `crates/soma-service/src/provider_registry.rs` | - | Add provider registry runtime polish; later split refresh code. | `git diff-tree -r 94c4349`, `git diff-tree -r f6b767c` |
| created | `crates/soma-service/src/provider_registry/refresh.rs` | - | Split and stabilize provider refresh runtime. | `git diff-tree -r f6b767c` |
| modified | `crates/soma-service/src/providers/ai_sdk.rs` | - | Harden AI SDK sidecar runtime handling. | `git diff-tree -r 94c4349` |
| modified | `crates/soma-service/src/providers/filesystem.rs` | - | Support `.wasm.json` sidecars and provider file filtering. | `git diff-tree -r 94c4349`; lines 277-307 |
| modified | `crates/soma-service/src/providers/filesystem_tests.rs` | - | Cover Wasm sidecar loading and fingerprinting. | `git diff-tree -r 94c4349`; lines 9-46 |
| modified | `crates/soma-service/src/providers/python.rs` | - | Harden Python sidecar/runtime behavior. | `git diff-tree -r 94c4349` |
| modified | `crates/soma-service/src/providers/sidecar.rs` | - | Stabilize provider sidecar runtime behavior. | `git diff-tree -r f6b767c` |
| modified | `crates/soma-service/src/providers/wasm.rs` | - | Improve Wasm execution limits, timeout, provenance, and output validation. | `git diff-tree -r 94c4349`; lines 36-101 |
| modified | `crates/soma/tests/ai_sdk_provider.rs` | - | Add AI SDK timeout/env/runtime tests. | `git diff-tree -r f6b767c` |
| modified | `crates/soma/tests/mcporter/test-mcp.sh` | - | Adjust MCP smoke behavior for sidecar/provider runtime. | `git diff-tree -r f6b767c` |
| modified | `crates/soma/tests/plugin_contract.rs` | - | Align plugin/registry expectations with 0.4.7 metadata. | `git diff-tree -r 94c4349` |
| modified | `docs/CI.md` | - | Document provider/release workflow changes. | `git diff-tree -r 94c4349`, `git diff-tree -r f471390` |
| modified | `docs/PATTERNS.md` | - | Update most provider/release patterns; one stale `server.json` snippet remains tracked by `rmcp-template-u5ej`. | `git diff-tree -r 94c4349`, `git diff-tree -r f471390`; stale lines 1680-1688 |
| modified | `docs/RMCP_README_GUIDE.md` | - | Align README guide with provider/runtime surface. | `git diff-tree -r 94c4349` |
| modified | `docs/SCRIPTS.md` | - | Refresh script/provider automation docs. | `git diff-tree -r 94c4349` |
| modified | `docs/generated/provider-surfaces.json` | - | Regenerate provider surface inventory. | `git diff-tree -r 94c4349` |
| modified | `docs/generated/provider-surfaces.md` | - | Regenerate provider surface docs. | `git diff-tree -r 94c4349` |
| modified | `docs/generated/scripts-index.md` | - | Refresh generated scripts index. | `git diff-tree -r 94c4349` |
| created and deleted | `docs/superpowers/plans/2026-07-09-provider-drop-in-ux.md` | - | Provider implementation plan was committed in provider polish and later removed during release-please cleanup. | `git diff-tree -r 94c4349`, `git diff-tree -r f471390` |
| modified | `release/components.toml` | - | Add release-please files and nested `server.json` OCI image pointer to version inventory. | `git diff-tree -r 94c4349`, `git diff-tree -r f471390`; lines 21-35 |
| modified | `scripts/README.md` | - | Document script/provider automation updates. | `git diff-tree -r 94c4349` |
| created | `scripts/check-readme-guide.py` | - | Add README guide validation helper. | `git diff-tree -r 94c4349` |
| modified | `server.json` | - | Align package versions and OCI identifiers with release version 0.4.7. | `git diff-tree -r 94c4349` |
| modified | `xtask/src/generated_surfaces.rs` | - | Extend generated provider/distribution surface checks. | `git diff-tree -r 94c4349` |
| modified | `xtask/src/release_versions.rs` | - | Add release-please manifest sync and version inventory handling. | `git diff-tree -r 94c4349` |
| modified | `xtask/src/release_versions_tests.rs` | - | Add release-please/server.json version-sync coverage. | `git diff-tree -r 94c4349` |
| modified | `.github/dependabot.yml` | - | Emit `deps(...)` commits so dependency updates appear in release notes. | `git diff-tree -r f471390` |
| deleted | `.github/workflows/auto-tag.yml` | - | Remove old auto-tag release path. | `git diff-tree -r f471390` |
| modified | `.github/workflows/ci.yml` | - | Use version sync checks instead of old release bump gate. | `git diff-tree -r f471390` |
| created | `.github/workflows/release-please.yml` | - | Add CI-gated release-please workflow and release PR fixups. | `git diff-tree -r f471390`; lines 15-93 |
| created | `.release-please-manifest.json` | - | Track release-please root package version. | `git diff-tree -r f471390` |
| modified | `Justfile` | - | Reframe local publish/version helpers around release-please. | `git diff-tree -r f471390` |
| modified | `crates/soma/tests/workflow_shapes.rs` | - | Assert release-please and release-published workflow shape. | `git diff-tree -r f471390` |
| modified | `docs/PLUGINS.md` | - | Document release-please version sync for plugins. | `git diff-tree -r f471390` |
| modified | `docs/WINDOWS-RUNNER.md` | - | Align Windows/release runner wording. | `git diff-tree -r f471390` |
| modified | `docs/adr/0008-versioning-and-distribution.md` | - | Update ADR to release-please versioning model. | `git diff-tree -r f471390` |
| created | `release-please-config.json` | - | Configure release-please, changelog sections, and extra files. | `git diff-tree -r f471390`; lines 1-103 |
| modified | `xtask/src/main.rs` | - | Add `sync-release-please-version` command. | `git diff-tree -r f471390` |
| modified | `xtask/src/scripts_lane_b.rs` | - | Make pre-release checks use version sync instead of old release-version gate. | `git diff-tree -r f471390` |
| modified | `Cargo.lock` | - | Stabilize provider sidecar runtime dependency resolution in current HEAD. | `git diff-tree -r f6b767c` |
| created | `docs/sessions/2026-07-12-provider-runtime-and-release-please.md` | - | Save this session log. | save-to-md artifact |

## Beads Activity

| bead | title | action(s) | final status | why it mattered |
|---|---|---|---|---|
| `rmcp-template-ihhp` | Implement provider runtime hardening | Created/claimed during provider work; closed after implementation and verification. | closed | Tracked ABI envelope, provider CLI ops, Wasm sidecars, refresh cache, provenance, and generated-surface contract docs. |
| `rmcp-template-pt4o` | Implement release-please release flow | Created for release-please migration; closed after config/workflow/docs/tests passed. | closed | Tracked replacing/customizing Soma release automation per `/home/jmagar/docs/ai/release-please.md`, including `server.json` and OCI handling. |
| `rmcp-template-7rc0` | Land current Soma branch on main | Created/claimed for landing; closed after commits `94c4349` and `f471390` were on `main` and verified. | closed | Captured the merge/landing state and verification across provider and release work. |
| `rmcp-template-u5ej` | Update PATTERNS server.json release snippet for release-please | Created during save-session stale-doc pass. | open | Tracks the remaining stale `docs/PATTERNS.md:1680` tag-time `server.json` mutation snippet. |

## Repository Maintenance

### Plans

`find docs/plans -maxdepth 2 -type f` returned no files. Historical plan files under `docs/superpowers/plans/` were observed, but the skill only calls for completed-plan moves under `docs/plans/`; no plan files were moved.

### Beads

Relevant beads were read with `bd show rmcp-template-ihhp`, `bd show rmcp-template-pt4o`, and `bd show rmcp-template-7rc0`. The stale-doc follow-up bead `rmcp-template-u5ej` was created with `bd create` and left open.

### Worktrees and branches

`git worktree list --porcelain`, `git branch -vv`, and remote branch listings were inspected. `codex/langchain-llamaindex-providers` and `codex/provider-dropin-ux` were ancestors of `origin/main`; `codex/provider-drop-in-ux` and `marketplace-no-mcp` were not. Detached or missing worktree entries were observed for older PR review worktrees, but no worktree or branch was deleted because ownership and active-use status were not clear enough for safe cleanup inside a session-log commit.

### Stale docs

The stale-doc scan found `docs/PATTERNS.md:1680` through `docs/PATTERNS.md:1688`, which still describes tag-time `server.json` mutation in `release.yml`. No doc edit was made in this save operation because the save-to-md contract commits only the generated session artifact; follow-up bead `rmcp-template-u5ej` records the exact fix.

### Transparency

No transcript file was found at the Claude-style injected path. The command produced a zsh no-match message for `/home/jmagar/.claude/projects/-home-jmagar-workspace-soma/*.jsonl`; the note is based on visible conversation context and observed repository commands.

## Tools and Skills Used

- **Skill.** `vibin:save-to-md` was used to drive this session-log workflow, including maintenance checks and the session-file-only commit contract.
- **Shell commands.** Used `git`, `bd`, `rg`, `jq`, `sed`, `nl`, `find`, `ls`, `gh`, `curl`, `cargo`, and `actionlint` for repository inspection, release verification, and documentation evidence.
- **File edits.** Used `apply_patch` for code/docs edits and for creating this markdown artifact.
- **External CLIs.** Used `actionlint` for GitHub Actions linting, `cargo`/`xtask` for Rust and repo-specific verification, `bd` for beads, and `gh` for active PR checks.
- **Network access.** Used `curl` and `git ls-remote` against `github.com/googleapis/release-please-action` to verify release-please action refs and outputs.
- **MCP servers/subagents/browser tools.** No MCP server tools, subagents, or browser automation were used in the observed save-session pass.

## Commands Executed

| command | result |
|---|---|
| `sed -n '1,620p' /home/jmagar/.codex/plugins/cache/dendrite-no-mcp/vibin/local/skills/save-to-md/SKILL.md` | Read the skill instructions. |
| `git log --oneline -10` | Confirmed recent commits including `f6b767c`, `f471390`, and `94c4349`. |
| `git diff-tree --no-commit-id --name-status -r 94c4349` | Listed provider runtime polish files. |
| `git diff-tree --no-commit-id --name-status -r f471390` | Listed release-please migration files. |
| `git diff-tree --no-commit-id --name-status -r f6b767c` | Listed provider sidecar stabilization files. |
| `bd show rmcp-template-ihhp --json` | Confirmed provider hardening bead was closed. |
| `bd show rmcp-template-pt4o --json` | Confirmed release-please bead was closed. |
| `bd show rmcp-template-7rc0 --json` | Confirmed landing bead was closed. |
| `bd create --title "Update PATTERNS server.json release snippet for release-please" ...` | Created follow-up bead `rmcp-template-u5ej`. |
| `rg -n "auto-tag|check-release-versions|release-plan|push: tags|release-please|server.json|wasm|sidecar" ...` | Found one stale `docs/PATTERNS.md` release snippet and current release-please references. |
| `cargo fmt --all` | Passed during release-please verification. |
| `actionlint .github/workflows/release-please.yml .github/workflows/release.yml .github/workflows/docker-publish.yml .github/workflows/ci.yml` | Passed after workflow fixes. |
| `cargo test -p xtask release_versions --all-features` | Passed after release-version test fix. |
| `cargo test -p soma --test workflow_shapes --all-features` | Passed. |
| `cargo xtask sync-release-please-version && cargo xtask check-version-sync && cargo xtask generate-provider-surfaces --check && cargo xtask check-mcp-registry` | Passed. |
| `cargo test --workspace --all-features` | Passed before the later `f6b767c` sidecar stabilization commit was observed. |

## Errors Encountered

- `curl` against `https://raw.githubusercontent.com/googleapis/release-please-action/8b8fd2cc23b2e18957157a9d923d75aa0c6f6ad5/action.yml` returned 404 because the documented SHA is the annotated `v4` tag object rather than a raw-file commit path. `git ls-remote` and the GitHub tarball endpoint confirmed the ref was still usable by GitHub Actions.
- `actionlint` initially rejected an `env.RELEASE_TAG` use in a job-level `if`; the workflow was patched to use an allowed expression context.
- `cargo test -p xtask release_versions --all-features` initially failed on a `String` versus `&Path` comparison in release-version tests; the assertions were fixed and the test passed.
- The Claude-style transcript lookup produced a zsh no-match error; no transcript file was read.

## Behavior Changes (Before/After)

| area | before | after |
|---|---|---|
| Provider loading | Provider manifests and executable providers worked, but lacked the full hardening pass requested in review. | Provider runtime has clearer ABI/provenance behavior, sidecar support, refresh caching, CLI operations, and generated-surface validation. |
| Wasm providers | Wasm manifests primarily relied on embedded metadata. | `.wasm.json` sidecar manifests are supported and included in fingerprinting. |
| Provider diagnostics | Provider failures were less consistently tied to provider kind/source/phase. | Provider errors carry more consistent provenance and redaction behavior. |
| Release automation | Old flow used auto-tag/tag-triggered release assumptions. | Release-please opens release PRs after CI, owns changelog/tag/release creation, and fixups sync derived files. |
| Artifact workflows | Release artifacts were tied to tag-oriented workflows. | Release and Docker workflows run from `release: published` with manual tag reruns. |
| `server.json` versioning | `server.json` version/image references could drift from release metadata. | `server.json` root/package versions and OCI identifiers are modeled in release inventory and release-please fixups. |

## Verification Evidence

| command | expected | actual | status |
|---|---|---|---|
| `cargo fmt --all` | Formatting succeeds. | Succeeded. | pass |
| `actionlint .github/workflows/release-please.yml .github/workflows/release.yml .github/workflows/docker-publish.yml .github/workflows/ci.yml` | Workflows lint cleanly. | Succeeded after job-level `if` expression fix. | pass |
| `cargo test -p xtask release_versions --all-features` | Release-version tests pass. | 12 passed. | pass |
| `cargo test -p soma --test workflow_shapes --all-features` | Workflow shape tests pass. | 3 passed. | pass |
| `cargo xtask sync-release-please-version` | Manifest-driven version sync works. | Succeeded. | pass |
| `cargo xtask check-version-sync` | Version-bearing files agree. | Reported `OK: soma version-bearing files are in sync at 0.4.7`. | pass |
| `cargo xtask generate-provider-surfaces --check` | Generated provider surfaces are current. | Reported current. | pass |
| `cargo xtask check-mcp-registry` | `server.json` validates against the MCP registry schema. | Reported valid. | pass |
| `cargo test --workspace --all-features` | Full workspace tests pass. | Passed before `f6b767c` was observed. | pass |
| `git merge-base --is-ancestor f471390 origin/main` | Release-please commit is landed on `origin/main`. | Exit code 0. | pass |
| `git merge-base --is-ancestor 94c4349 origin/main` | Provider runtime polish commit is landed on `origin/main`. | Exit code 0. | pass |

## Risks and Rollback

- Release-please requires a valid `RELEASE_PLEASE_TOKEN` PAT or GitHub App token. If missing or expired, release PRs or downstream workflows may silently stop.
- The documented release-please action ref is an annotated tag object. It is usable by Actions, but raw-file checks should use the peeled commit when inspecting files.
- `docs/PATTERNS.md:1680` still contains stale tag-time `server.json` text. Follow-up bead `rmcp-template-u5ej` tracks the correction.
- Rollback path: revert `f471390` to return to the prior release automation, and revert `94c4349`/`f6b767c` to remove provider runtime polish. That rollback would also require revisiting any released artifacts or tags created after the migration.

## Decisions Not Taken

- Did not remove old xtask `check-release-versions` and `release-plan` commands because they may still be useful diagnostics, and active release/pre-release paths no longer call them.
- Did not delete local or remote branches during the maintenance pass because some were unmerged, detached, or had unclear ownership.
- Did not run `git worktree prune` even though one missing/prunable worktree entry was observed; this save operation was scoped to documentation and branch ownership was not clear enough for cleanup.
- Did not edit `docs/PATTERNS.md` inside this save operation because the skill requires committing only the generated session artifact.

## References

- `/home/jmagar/docs/ai/release-please.md`
- `README.md:83`
- `README.md:280`
- `README.md:653`
- `.github/workflows/release-please.yml:15`
- `release-please-config.json:1`
- `release/components.toml:30`
- `docs/PATTERNS.md:1680`
- `https://github.com/googleapis/release-please-action`

## Open Questions

- Should stale local/remote branches at the same commit as `main` be pruned now that `main` and `origin/main` contain the provider/release work?
- Should the missing/prunable worktree metadata be cleaned with `git worktree prune` after confirming no active task depends on those paths?
- Should `docs/PATTERNS.md:1680` be fixed immediately or bundled with a broader documentation cleanup for release-please?
- The save-session note was written without a transcript file because none was available through the Claude-style transcript path.

## Next Steps

- Fix `rmcp-template-u5ej` by updating `docs/PATTERNS.md:1680` through `docs/PATTERNS.md:1688` to describe release-please manifest sync and Docker publish tag preparation instead of tag-time `release.yml` mutation.
- Confirm the repository secret `RELEASE_PLEASE_TOKEN` exists and is backed by a PAT or GitHub App token with contents, pull request, and issue write permissions.
- Let the next Conventional Commit on `main` exercise `.github/workflows/release-please.yml`, then inspect the release PR fixup commit for `Cargo.lock`, generated surfaces, and `server.json`.
- Separately audit stale worktrees and branches, with deletion only after branch ancestry and ownership are explicit.

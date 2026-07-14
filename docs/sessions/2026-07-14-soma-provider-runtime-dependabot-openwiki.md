---
date: 2026-07-14 13:52:01 EDT
repo: git@github.com:jmagar/soma.git
branch: fix/openwiki-no-cache
head: 269e33e9ae2d315ca8ea9f7bb8c1ee21baef63d0
session id: b74df89b-c5f9-4d30-8a18-3b69a5ddc0ac
transcript: /home/jmagar/.claude/projects/-home-jmagar-workspace-soma/b74df89b-c5f9-4d30-8a18-3b69a5ddc0ac.jsonl
working directory: /home/jmagar/workspace/soma
worktree: /home/jmagar/workspace/soma 269e33e9ae2d315ca8ea9f7bb8c1ee21baef63d0 [fix/openwiki-no-cache]
pr: "#125 fix(ci): install openwiki fresh each run instead of from mise cache - https://github.com/jmagar/soma/pull/125"
beads: rmcp-template-0lnb, rmcp-template-e6yx, rmcp-template-77ls, rmcp-template-7724, rmcp-template-k0xk, rmcp-template-t1xd, rmcp-template-a452, rmcp-template-vkze, rmcp-template-hfky, rmcp-template-aax1, rmcp-template-c9nq, rmcp-template-qfck, rmcp-template-l0sn
---

# Soma provider runtime, Labby verification, and OpenWiki CI closeout

## User Request

The session began with a request to list the current Soma crates, then expanded into making Soma the monorepo home for exported Labby-style runtime crates, validating dropped provider tools through Labby, landing the provider-runtime work, merging Dependabot work, and fixing the OpenWiki workflow.

## Session Overview

- Confirmed Soma's workspace crate layout and captured the next large architecture target: port `labby-gateway` and `labby-codemode` into Soma through bead `rmcp-template-0lnb`.
- Completed the provider runtime and Labby operational work, including AI SDK, Python, remote MCP, REST, and web runner behavior, then merged PR #117 as `fa54917a40e5a9c964a8d4a6fd1098e0b3a138d1`.
- Continued the CI cleanup stream after #117 by merging/fixing Dependabot-related PRs and opening PR #125 for the remaining OpenWiki `better-sqlite3` native binding failure.
- Saved this session artifact on current branch `fix/openwiki-no-cache`, which is still open as PR #125 while its latest MSRV/CI checks run.

## Sequence of Events

1. Listed the active Soma workspace members: `soma`, `soma-api`, `soma-auth`, `soma-cli`, `soma-contracts`, `soma-mcp`, `soma-observability`, `soma-plugin-support`, `soma-runtime`, `soma-service`, `soma-test-support`, `soma-web`, and `xtask`.
2. Compared Soma with `/home/jmagar/workspace/lab`, then created and designed bead `rmcp-template-0lnb` for porting `labby-gateway` and `labby-codemode`.
3. Worked through dropped provider behavior with Labby: AI SDK demo provider, Python provider operationalization, HTTP MCP connection to `https://soma.dinglebear.ai/mcp`, provider inventory, and REST execution for provider tools.
4. Landed the provider-runtime branch through PR #117 after fixing review and CI failures, then merged it to `main`.
5. Cleaned repository state and ran status checks, preserving protected and active branches while identifying remaining open PRs.
6. Moved into Dependabot/CI repair, fixing a yanked `spin` dependency stream and narrowing the remaining OpenWiki workflow failure to stale or blocked `better-sqlite3` native bindings.
7. Opened PR #125 with a fresh OpenWiki install and explicit native binding build step; the latest branch delta against `origin/main` is one workflow file.
8. Ran the save-to-md maintenance pass: fetched/pruned remotes, fast-forwarded local `main` to `origin/main`, inspected beads, plans, branches, worktrees, PRs, and check state, then wrote this artifact.

## Key Findings

- `soma-runtime` and the Labby runtime slice do different jobs. The port plan keeps Soma's `AppState`/`AuthPolicy` runtime separate from the Labby support slice needed by gateway/codemode.
- `soma-auth` is a vendored subset of `labby-auth`; gateway porting should upgrade it to the Labby codebase while preserving Soma's RFC 9207 and RFC 7591 behavior.
- AI SDK provider tools do not need an AI provider. In Soma, the AI SDK provider shape is a dropped tool provider that is executed by an LLM host, not a model-provider integration.
- MCP-only tools, prompts, and resources should appear as inventory in the web runner instead of disappearing; REST execution only applies to REST-capable dropped provider tools.
- The current OpenWiki fix is localized to `.github/workflows/openwiki-update.yml:22`, where the mise install cache is disabled, and `.github/workflows/openwiki-update.yml:31`, where the `better-sqlite3` binding is built and loaded explicitly.

## Technical Decisions

- Provider runtime behavior stays behind the shared Soma provider registry instead of one-off tool shims, keeping MCP, REST, CLI, stdio, and web surfaces aligned.
- Remote provider mode is explicit, so local dropped providers and remote Soma API mode do not accidentally cross streams via `SOMA_API_URL`.
- Labby connection configuration belongs in Labby's config and env files, not in hardcoded command strings or Labby source changes.
- The OpenWiki workflow now favors a fresh daily install over cached npm tool state because cached installs kept reviving bindings compiled against an older Node ABI.
- The Labby gateway/codemode port is tracked as a feature bead rather than buried in session prose, because it is a multi-wave architecture task.

## Files Changed

| status | path | previous path | purpose | evidence |
|---|---|---|---|---|
| created | `docs/sessions/2026-07-14-soma-provider-runtime-dependabot-openwiki.md` |  | Save this conversation and closeout state. | This save-to-md run. |
| modified | `.github/workflows/openwiki-update.yml` |  | Current PR #125 disables OpenWiki install caching and explicitly rebuilds `better-sqlite3`. | `git diff --name-status origin/main...HEAD` showed this as the only branch delta. |

PR #117 was merged during the session as `fa54917a40e5a9c964a8d4a6fd1098e0b3a138d1`. Its observed file set was:

```text
M	.env.example
M	.github/workflows/ci.yml
M	.github/workflows/release.yml
M	CLAUDE.md
M	Justfile
M	README.md
M	apps/web/CLAUDE.md
M	apps/web/README.md
M	apps/web/app/page.tsx
M	apps/web/app/tools/page.tsx
M	apps/web/lib/api.test.ts
M	apps/web/lib/api.ts
M	apps/web/lib/soma.test.ts
M	apps/web/lib/soma.ts
M	cargo-generate.toml
M	config.soma.toml
M	config/Dockerfile
M	crates/soma-api/src/api.rs
M	crates/soma-auth/src/auth_context.rs
M	crates/soma-auth/src/config.rs
M	crates/soma-auth/src/jwt.rs
M	crates/soma-auth/src/metadata.rs
M	crates/soma-auth/src/middleware.rs
M	crates/soma-auth/src/routes.rs
M	crates/soma-cli/src/doctor.rs
M	crates/soma-cli/src/doctor/checks.rs
M	crates/soma-cli/src/doctor/checks_tests.rs
M	crates/soma-cli/src/lib.rs
M	crates/soma-cli/src/setup_tests.rs
M	crates/soma-contracts/src/config.rs
M	crates/soma-contracts/src/config_tests.rs
M	crates/soma-mcp/src/lib.rs
M	crates/soma-mcp/src/response_paging.rs
M	crates/soma-mcp/src/response_paging_tests.rs
M	crates/soma-mcp/src/schemas.rs
M	crates/soma-mcp/src/schemas_tests.rs
M	crates/soma-mcp/src/tools.rs
M	crates/soma-mcp/src/tools_tests.rs
M	crates/soma-observability/src/logging/aurora.rs
M	crates/soma-runtime/src/server.rs
M	crates/soma-service/src/app.rs
M	crates/soma-service/src/app_tests.rs
M	crates/soma-service/src/lib.rs
M	crates/soma-service/src/provider_registry.rs
M	crates/soma-service/src/provider_registry/refresh.rs
M	crates/soma-service/src/provider_registry/reports.rs
M	crates/soma-service/src/providers.rs
A	crates/soma-service/src/providers/remote.rs
A	crates/soma-service/src/providers/remote_tests.rs
M	crates/soma-service/src/providers/static_rust.rs
M	crates/soma-service/src/soma.rs
M	crates/soma-service/src/soma_tests.rs
M	crates/soma-web/assets/source/CLAUDE.md
M	crates/soma-web/assets/source/README.md
M	crates/soma-web/assets/source/app/page.tsx
M	crates/soma-web/assets/source/app/tools/page.tsx
M	crates/soma-web/assets/source/lib/api.test.ts
M	crates/soma-web/assets/source/lib/api.ts
M	crates/soma-web/assets/source/lib/soma.test.ts
M	crates/soma-web/assets/source/lib/soma.ts
M	crates/soma/Cargo.toml
M	crates/soma/src/bin/soma.rs
M	crates/soma/src/bin/soma_tests.rs
M	crates/soma/src/lib.rs
D	crates/soma/src/main.rs
M	crates/soma/src/routes.rs
M	crates/soma/src/runtime.rs
M	crates/soma/tests/ai_sdk_provider.rs
M	crates/soma/tests/api_routes.rs
A	crates/soma/tests/cli_remote_api.rs
M	crates/soma/tests/drop_provider_probe.rs
M	crates/soma/tests/generated_surfaces.rs
M	crates/soma/tests/mcp_provider.rs
M	crates/soma/tests/mcporter/test-mcp.sh
M	crates/soma/tests/plugin_contract.rs
M	crates/soma/tests/provider_registry.rs
M	crates/soma/tests/provider_surfaces.rs
M	crates/soma/tests/python_provider.rs
A	crates/soma/tests/soma_serve.rs
M	crates/soma/tests/stdio_mcp.rs
A	crates/soma/tests/stdio_remote_api.rs
M	crates/soma/tests/wasm_provider.rs
M	docs/ARCHITECTURE.md
M	docs/AUTH.md
M	docs/CARGO_GENERATE.md
M	docs/CLAUDE.md
M	docs/DEPLOYMENT.md
M	docs/DOCKER.md
M	docs/JUSTFILE.md
M	docs/OBSERVABILITY.md
M	docs/PATTERNS.md
M	docs/PLUGINS.md
M	docs/PRE-COMMIT.md
M	docs/QUICKSTART.md
M	docs/RMCP_README_GUIDE.md
M	docs/SCAFFOLD.md
M	docs/SYSTEMD.md
M	docs/WEB.md
M	docs/WINDOWS-RUNNER.md
M	docs/adr/0001-stdio-first-plugin-adapter.md
A	docs/adr/0011-product-first-template-second.md
M	docs/adr/README.md
M	docs/contracts/README.md
M	docs/contracts/examples/scaffold-intent-upstream-client.json
M	docs/contracts/plugin-stdio-adapter.md
M	docs/contracts/scaffold-intent.schema.json
M	docs/generated/openapi.json
M	docs/generated/palette-manifest.json
M	docs/generated/plugin.json
M	docs/generated/provider-surfaces.json
M	docs/generated/provider-surfaces.md
M	docs/generated/scripts-index.md
A	docs/generated/skills/local-ai-sdk-tools/SKILL.md
A	docs/generated/skills/local-python-tools/SKILL.md
M	docs/generated/skills/static-rust/SKILL.md
A	docs/sessions/2026-07-13-runtime-modes-provider-packaging.md
M	docs/specs/scaffold-intent-handoff.md
M	docs/superpowers/plans/2026-07-11-hard-break-soma-rename.md
M	entrypoint.sh
M	install.sh
M	lefthook.yml
A	packages/soma-rmcp/LICENSE
M	packages/soma-rmcp/README.md
M	packages/soma-rmcp/package.json
A	packages/soma-rmcp/scripts/check-package.js
A	packages/soma-rmcp/scripts/sync-readme.js
M	plugins/soma/.codex-plugin/README.md
M	plugins/soma/CLAUDE.md
M	plugins/soma/README.md
M	plugins/soma/skills/scaffold-project/SKILL.md
M	plugins/soma/skills/soma/SKILL.md
A	providers/python-runtime-check.py
M	scaffold/cargo-generate/post.rhai
M	scripts/README.md
M	scripts/check-readme-guide.py
M	scripts/generate-docs.py
A	scripts/readme_related_servers.py
M	server.json
M	xtask/README.md
M	xtask/src/cargo_generate.rs
M	xtask/src/cargo_generate_post.rs
M	xtask/src/generated_surfaces.rs
M	xtask/src/patterns/checks.rs
M	xtask/src/scaffold.rs
M	xtask/src/scripts.rs
M	xtask/src/scripts_lane_a.rs
M	xtask/src/scripts_lane_c.rs
M	xtask/src/scripts_lane_d.rs
```

## Beads Activity

| bead | title | action | final status | why it mattered |
|---|---|---|---|---|
| `rmcp-template-0lnb` | Port labby-gateway and labby-codemode crates into soma monorepo | Created and populated with a five-wave port design. | open | Captures the next architecture phase from the crate comparison work. |
| `rmcp-template-e6yx` | Add dropped AI SDK demo provider | Closed. | closed | Verified a dropped AI SDK tool through Labby Code Mode. |
| `rmcp-template-77ls` | Make Python providers Labby-operational | Closed. | closed | Tracked the Python provider runtime, Labby launcher, and adapter dependency fix. |
| `rmcp-template-7724` | Show dropped provider tools in Soma web runner | Closed. | closed | Tracked `/v1/providers`, web tool inventory, and runner visibility. |
| `rmcp-template-k0xk` | Execute dropped provider tools through REST | Closed. | closed | Tracked the generic REST execution path for dropped provider tools. |
| `rmcp-template-t1xd` | Implement explicit Soma runtime modes | Closed after PR #117 was opened. | closed | Covered the local/remote/disabled provider runtime work. |
| `rmcp-template-a452` | Review PR 117: remote stdio MCP discovery omits remote provider catalog | Closed. | closed | Review fix for remote catalog discovery. |
| `rmcp-template-vkze` | Review PR 117: remote adapter provider REST route mismatch | Closed. | closed | Review fix for remote REST route resolution. |
| `rmcp-template-hfky` | Review PR 117: document REST default-on provider tool contract | Closed. | closed | Review fix for intentional REST contract docs. |
| `rmcp-template-aax1` | Review PR 117: explicit local runtime still uses SOMA_API_URL target | Closed. | closed | Review fix for local runtime target selection. |
| `rmcp-template-c9nq` | Review PR 117: module map still references deleted soma main.rs | Closed. | closed | Review fix for stale module map docs. |
| `rmcp-template-qfck` | Review PR 117: soma-rmcp README overstates npx binary availability | Closed. | closed | Review fix for package README launcher wording. |
| `rmcp-template-l0sn` | Review PR 117: web runner should execute provider tools through reliable generic REST | Closed. | closed | Review fix for web provider REST execution and GET params. |

No new bead was created during this save-to-md pass because the known remaining work is already tracked by PR #125, open PRs #99/#104/#115/#122, and open feature bead `rmcp-template-0lnb`.

## Repository Maintenance

- Plans: `find docs/plans -maxdepth 2 -type f` returned no plan files, so nothing was moved to `docs/plans/complete/`.
- Beads: read `rmcp-template-0lnb`, recent bead interactions, and relevant recent closed beads. No bead state was changed during this closeout.
- Worktrees and branches: `git fetch --all --prune` pruned deleted remote refs `origin/fix/openwiki-binding-force` and `origin/fix/spin-yanked`. Local `main` was fast-forwarded to `origin/main` with `git branch -f main origin/main`.
- Worktrees left alone: current branch `fix/openwiki-no-cache` is active PR #125; `.claude/worktrees/codex-app-server-api-4798cc` is an active worktree; `_no_mcp_worktrees/rmcp-template` is the protected `marketplace-no-mcp` branch and remains off limits.
- Branches left alone: `codex/pr101-review-fixes`, `codex/provider-drop-in-ux`, `release-please--branches--main--components--soma`, and the open Dependabot branches were not deleted because they are unmerged, open, or unclear ownership.
- Stale docs: PR #117 already updated runtime, provider, plugin, generated surface, and package docs. This pass only added the session artifact.

## Tools and Skills Used

- `vibin:save-to-md`: used for this final session artifact, maintenance pass, path-limited commit, and push contract.
- `vibin:repo-status`: used earlier to inspect the checkout, worktrees, branches, PR mergeability, and cleanup candidates.
- `vibin:gh-fix-ci`: used earlier to investigate failing GitHub checks, Dependabot PRs, and OpenWiki workflow failures.
- `lavra-review`: used for PR #117 review; the surfaced issues were filed as beads and fixed before merge.
- Labby gateway and Code Mode: used to verify dropped provider behavior through the live gateway.
- Shell, Git, GitHub CLI, and Beads CLI: used for repo state, PR state, CI state, branch maintenance, bead reads, and commits.
- Subagents/background agents: used in the transcript to explore crate overlap and watch/merge CI streams. The background watcher output file for #125 was checked during this closeout and had no current output.

## Commands Executed

| command | result |
|---|---|
| `git fetch --all --prune` | Fetched `origin/main`, updated `origin/fix/openwiki-no-cache`, and pruned two deleted remote refs. |
| `git pull --rebase` | Fast-forwarded current branch from `552f777` to `269e33e`, updating `Cargo.lock` from the remote branch merge commit. |
| `git diff --name-status origin/main...HEAD` | Reported only `M .github/workflows/openwiki-update.yml` for PR #125. |
| `gh pr view 117 --json ...` | Confirmed PR #117 is merged as `fa54917a40e5a9c964a8d4a6fd1098e0b3a138d1` at `2026-07-14T01:14:03Z`. |
| `gh pr view --json ...` | Confirmed current PR #125 is open, mergeable, and at head `269e33e9ae2d315ca8ea9f7bb8c1ee21baef63d0`. |
| `gh run list --branch fix/openwiki-no-cache --limit 10 --json ...` | Showed PR #125 CI pending and MSRV in progress on `269e33e`; earlier MSRV succeeded on `552f777`. |
| `bd show rmcp-template-0lnb --json` | Confirmed the Labby gateway/codemode port bead is open and contains the five-wave design. |
| `git branch -f main origin/main` | Fast-forwarded inactive local `main` to `e4e8abc`. |

## Errors Encountered

- Initial Labby/Soma gateway setup drifted toward command-line env injection and Labby code edits. The correction was to keep env in Labby's config/env files and connect Soma via HTTP at `https://soma.dinglebear.ai/mcp`.
- Python providers were not fully operational until the plain Python smoke action, launcher PATH/Python command, and adapter dependencies were fixed and verified through Labby.
- The web runner initially lost MCP-only tools/prompts/resources from visible inventory; the fix surfaced them as inventory and kept execution to REST-capable provider tools.
- PR #117 review and CI surfaced concrete issues: clippy on Rust 1.97, stale required-file checks for deleted `crates/soma/src/main.rs`, missing `remote_tests.rs`, stale palette manifest, and mismatched schema-doc coupled checks. Each was fixed before merge.
- The merge-status script path attempted in the transcript failed with a permission issue, so direct `gh`, `git`, and repo-status checks were used instead.
- OpenWiki continued failing because cached or policy-blocked `better-sqlite3` bindings survived rebuild attempts. PR #125 disables the cache and runs the install script explicitly.

## Behavior Changes (Before/After)

| area | before | after |
|---|---|---|
| Provider runtime | Local and remote provider behavior could blur through environment defaults. | Runtime modes are explicit and tested across local, remote, and disabled paths. |
| Labby provider proof | Dropped AI SDK/Python tools were not durably proven through the gateway. | AI SDK and Python provider actions were verified through Labby Code Mode. |
| Web runner | Dropped providers and MCP-only inventory were incomplete or invisible. | `/v1/providers` and the tool runner show dropped provider tools plus MCP-only inventory. |
| REST provider execution | The runner did not have a reliable generic provider execution path. | REST-capable dropped provider tools execute through generic REST dispatch. |
| OpenWiki workflow | Cached OpenWiki installs could revive stale native bindings. | PR #125 disables cache and explicitly builds/loads `better-sqlite3`. |

## Verification Evidence

| command | expected | actual | status |
|---|---|---|---|
| `gh pr view 117 --json state,mergeCommit,mergedAt` | PR #117 merged. | State `MERGED`, merge commit `fa54917a40e5a9c964a8d4a6fd1098e0b3a138d1`. | pass |
| `git status -sb` | Current checkout clean before writing the session artifact. | `## fix/openwiki-no-cache...origin/fix/openwiki-no-cache` with no dirty files. | pass |
| `git diff --name-status origin/main...HEAD` | PR #125 delta isolated to OpenWiki workflow. | `M .github/workflows/openwiki-update.yml`. | pass |
| `gh pr view --json mergeable,headRefOid,statusCheckRollup` | Current PR #125 visible and mergeable. | Mergeable `MERGEABLE`, head `269e33e`, MSRV in progress, CI pending. | warn |
| `bd show rmcp-template-0lnb --json` | Follow-up porting work tracked. | Bead exists, status `open`, priority `1`, with five-wave design. | pass |
| `find docs/plans -maxdepth 2 -type f` | Plans checked for completion cleanup. | No plan files found. | pass |

## Risks and Rollback

- PR #125 is still waiting on CI/MSRV at the time of this artifact. If it fails, rollback is to revert or amend the one-file workflow change in `.github/workflows/openwiki-update.yml`.
- The Labby gateway/codemode port is intentionally not started yet. It is large enough to require the bead's staged plan, especially around auth, Code Mode, Windows process handling, and rmcp client pooling.
- Open PRs #99, #104, #115, and #122 may still need rebase or conflict work before they can land cleanly.

## Decisions Not Taken

- Did not port `labby-web` in the first gateway/codemode plan; the bead keeps the first scope to support/runtime/openapi/codemode/gateway.
- Did not delete `marketplace-no-mcp`, even though broad cleanup was requested earlier, because project instructions mark it as protected long-lived state.
- Did not delete active or open PR branches during the save-to-md maintenance pass.
- Did not force OpenWiki through another cached rebuild path; current evidence favored a fresh install plus explicit binding build.

## References

- PR #117: https://github.com/jmagar/soma/pull/117
- PR #125: https://github.com/jmagar/soma/pull/125
- Current OpenWiki workflow: `.github/workflows/openwiki-update.yml`
- Bead `rmcp-template-0lnb`: port Labby gateway and codemode crates into Soma.
- Transcript: `/home/jmagar/.claude/projects/-home-jmagar-workspace-soma/b74df89b-c5f9-4d30-8a18-3b69a5ddc0ac.jsonl`

## Open Questions

- Will PR #125's pending CI/MSRV checks finish green after the merge-from-main commit `269e33e`?
- Which remaining open PR should be landed next after #125: #122, #115, #104, or #99?
- Should `codex/pr101-review-fixes` still be kept as an unmerged docs branch, or retired in a later cleanup pass?

## Next Steps

1. Watch PR #125 checks to completion. If green, merge it and dispatch the OpenWiki update workflow once.
2. Re-run `gh pr list --state open --json number,title,headRefName,mergeable,url` after #125 lands, then handle #122 and release-please PR #115 based on their refreshed mergeability.
3. Start `rmcp-template-0lnb` with the dependency-first support slice, not with gateway files directly.
4. Keep `marketplace-no-mcp` untouched unless Jacob explicitly names that branch and asks to retire it.

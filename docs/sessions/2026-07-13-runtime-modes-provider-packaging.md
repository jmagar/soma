---
date: 2026-07-13 19:57:26 EST
repo: git@github.com:jmagar/soma.git
branch: codex/python-provider-operational
head: cb57b58
session id: b74df89b-c5f9-4d30-8a18-3b69a5ddc0ac
transcript: /home/jmagar/.claude/projects/-home-jmagar-workspace-soma/b74df89b-c5f9-4d30-8a18-3b69a5ddc0ac.jsonl
working directory: /home/jmagar/workspace/soma
worktree: /home/jmagar/workspace/soma
pr: #117 Finalize Soma provider runtime packaging (https://github.com/jmagar/soma/pull/117)
beads: rmcp-template-t1xd, rmcp-template-f7hs
---

# Runtime modes and provider packaging session

## User Request

The session began with runtime-shape brainstorming: make `soma` the CLI and single
runtime binary, make `soma serve` own the real HTTP runtime, and make `soma mcp`
and CLI commands connect to the running API in remote mode instead of running
providers in process. The user later asked for a thorough stale-doc sweep using
several agents, then asked to create a PR and save the session.

## Session Overview

The branch `codex/python-provider-operational` now contains the explicit
single-binary Soma runtime shape and provider packaging updates in commit
`cb57b58 feat: finalize soma provider runtime packaging`. The PR is open as
https://github.com/jmagar/soma/pull/117, the local branch is clean, and
`git rev-list --left-right --count @{upstream}...HEAD` reported `0 0`.

This save pass also closed the completed runtime-mode bead
`rmcp-template-t1xd`, left follow-up bead `rmcp-template-f7hs` open for deeper
generated provider/API docs work, and created this session artifact.

## Sequence of Events

1. Clarified the runtime model: `soma` is the single binary; `soma serve` owns
   HTTP MCP, REST, provider registry, auth, web, health; `soma mcp` is stdio; CLI
   commands use local or remote/API mode.
2. Implemented the runtime and provider packaging branch, including remote CLI
   and stdio MCP forwarding, generic REST provider execution, generated surfaces,
   tests, docs, and package metadata.
3. Reframed the repository as product-first and template-second by adding ADR
   0011 and separating product runtime verification from scaffold/export lane
   verification.
4. Dispatched five read-only docs sweep agents covering public docs,
   operator/deploy docs, plugin/MCP docs, scaffold/template docs, and
   generated/release/provider docs.
5. Applied the stale-doc fixes from the agents, verified docs, created follow-up
   bead `rmcp-template-f7hs`, and pushed the Beads/Dolt update.
6. Confirmed PR #117 already existed, confirmed the branch was pushed and clean,
   and closed completed bead `rmcp-template-t1xd`.

## Key Findings

- `soma serve mcp` was stale in active docs and runtime commands. The active
  product shape is `soma serve` for HTTP runtime and `soma mcp` for stdio MCP.
- `SOMA_API_URL` examples ending in `/v1` were misleading because the REST client
  appends `/v1/<action>`; the corrected placeholder is the API base URL.
- Plugin docs referenced a shared `plugins/soma/.mcp.json` that was not present
  in the live tree; plugin documentation now treats stdio MCP registration as
  client or gateway config.
- ADR 0011 records that Soma is product-first and template-second; scaffold
  verification is not the default acceptance gate for product runtime changes.
- Generated provider/API documentation still needs deeper generator work:
  `provider-surfaces.md` has limited REST detail, generated OpenAPI provider
  schemas are generic, and script index summaries need cleanup. This is tracked
  by `rmcp-template-f7hs`.

## Technical Decisions

- Use one canonical `soma` binary and explicit modes rather than preserving a
  separate `soma-server` identity.
- Keep dynamic provider actions available through REST so remote CLI and stdio
  MCP adapters can target a running server from another device.
- Keep scaffold/cargo-generate as an export lane, not as the product identity or
  mandatory product-runtime verification target.
- Do not rewrite broad historical session docs or stale generic pattern examples
  unless they actively mislead current product docs.
- Create a follow-up bead for generated provider/OpenAPI documentation richness
  instead of hiding that remaining work in prose.

## Files Changed

The following file inventory comes from `git diff-tree --no-commit-id
--name-status -r HEAD` for commit `cb57b58`, plus this session artifact.

| status | path | previous path | purpose | evidence |
|---|---|---|---|---|
| created | `docs/sessions/2026-07-13-runtime-modes-provider-packaging.md` | - | Save this session log. | This artifact. |
| modified | `.env.example` | - | Update generated environment examples for the runtime/API shape. | HEAD file list. |
| modified | `.github/workflows/ci.yml` | - | Align CI smoke commands with `soma serve`. | HEAD file list. |
| modified | `.github/workflows/release.yml` | - | Release the canonical `soma` binary instead of split server/local names. | HEAD file list. |
| modified | `CLAUDE.md` | - | Update project instructions for the single-binary runtime. | HEAD file list. |
| modified | `Justfile` | - | Align developer recipes with `soma serve`, provider/runtime checks, and docs. | HEAD file list. |
| modified | `README.md` | - | Product-first runtime docs, provider REST, scaffold/export lane wording. | HEAD file list. |
| modified | `apps/web/CLAUDE.md` | - | Sync web-source instructions. | HEAD file list. |
| modified | `apps/web/README.md` | - | Sync web documentation. | HEAD file list. |
| modified | `apps/web/app/page.tsx` | - | Web UI text/runtime alignment. | HEAD file list. |
| modified | `cargo-generate.toml` | - | Remove split-binary scaffold assumptions. | HEAD file list. |
| modified | `config.soma.toml` | - | Regenerated config sample with base API URL. | HEAD file list. |
| modified | `config/Dockerfile` | - | Default container command to `serve`. | HEAD file list. |
| modified | `crates/soma-auth/src/auth_context.rs` | - | Runtime/auth naming alignment. | HEAD file list. |
| modified | `crates/soma-auth/src/config.rs` | - | Runtime/auth naming alignment. | HEAD file list. |
| modified | `crates/soma-auth/src/jwt.rs` | - | Runtime/auth naming alignment. | HEAD file list. |
| modified | `crates/soma-auth/src/metadata.rs` | - | Runtime/auth naming alignment. | HEAD file list. |
| modified | `crates/soma-auth/src/middleware.rs` | - | Runtime/auth naming alignment. | HEAD file list. |
| modified | `crates/soma-auth/src/routes.rs` | - | Runtime/auth naming alignment. | HEAD file list. |
| modified | `crates/soma-cli/src/doctor.rs` | - | Align doctor checks with canonical binary/runtime. | HEAD file list. |
| modified | `crates/soma-cli/src/doctor/checks.rs` | - | Align doctor checks with canonical binary/runtime. | HEAD file list. |
| modified | `crates/soma-cli/src/doctor/checks_tests.rs` | - | Update doctor test coverage. | HEAD file list. |
| modified | `crates/soma-cli/src/lib.rs` | - | Add/align runtime mode CLI behavior. | HEAD file list. |
| modified | `crates/soma-cli/src/setup_tests.rs` | - | Update setup test coverage. | HEAD file list. |
| modified | `crates/soma-contracts/src/config.rs` | - | Add runtime mode/API base config behavior. | HEAD file list. |
| modified | `crates/soma-contracts/src/config_tests.rs` | - | Cover config/runtime mode behavior. | HEAD file list. |
| modified | `crates/soma-mcp/src/lib.rs` | - | Export MCP pieces needed by runtime tests. | HEAD file list. |
| modified | `crates/soma-mcp/src/response_paging.rs` | - | Structured response behavior alignment. | HEAD file list. |
| modified | `crates/soma-mcp/src/response_paging_tests.rs` | - | Response paging tests. | HEAD file list. |
| modified | `crates/soma-mcp/src/schemas.rs` | - | Provider/action schema projection updates. | HEAD file list. |
| modified | `crates/soma-mcp/src/schemas_tests.rs` | - | Schema coverage for provider/runtime surfaces. | HEAD file list. |
| modified | `crates/soma-mcp/src/tools.rs` | - | MCP dispatch and remote/API mode behavior. | HEAD file list. |
| modified | `crates/soma-mcp/src/tools_tests.rs` | - | MCP tool dispatch tests. | HEAD file list. |
| modified | `crates/soma-observability/src/logging/aurora.rs` | - | Runtime naming/logging polish. | HEAD file list. |
| modified | `crates/soma-runtime/src/server.rs` | - | Runtime server state behavior. | HEAD file list. |
| modified | `crates/soma-service/src/app.rs` | - | Service/runtime dispatch updates. | HEAD file list. |
| modified | `crates/soma-service/src/app_tests.rs` | - | Service runtime tests. | HEAD file list. |
| modified | `crates/soma-service/src/provider_registry.rs` | - | Provider registry/runtime behavior. | HEAD file list. |
| modified | `crates/soma-service/src/soma.rs` | - | REST client forwarding behavior. | HEAD file list. |
| modified | `crates/soma-service/src/soma_tests.rs` | - | REST client tests. | HEAD file list. |
| modified | `crates/soma-web/assets/source/CLAUDE.md` | - | Sync embedded web source docs. | HEAD file list. |
| modified | `crates/soma-web/assets/source/README.md` | - | Sync embedded web source docs. | HEAD file list. |
| modified | `crates/soma-web/assets/source/app/page.tsx` | - | Sync embedded web text. | HEAD file list. |
| modified | `crates/soma/Cargo.toml` | - | Canonical binary/package description updates. | HEAD file list. |
| modified | `crates/soma/src/bin/soma.rs` | - | Canonical `soma` mode dispatch. | HEAD file list. |
| modified | `crates/soma/src/bin/soma_tests.rs` | - | Binary mode tests. | HEAD file list. |
| modified | `crates/soma/src/lib.rs` | - | Public facade/test helper updates. | HEAD file list. |
| deleted | `crates/soma/src/main.rs` | - | Remove separate server binary entrypoint. | HEAD file list. |
| modified | `crates/soma/src/runtime.rs` | - | Runtime builder/mode wiring. | HEAD file list. |
| modified | `crates/soma/tests/ai_sdk_provider.rs` | - | Provider test alignment. | HEAD file list. |
| created | `crates/soma/tests/cli_remote_api.rs` | - | Test remote CLI/API behavior. | HEAD file list. |
| modified | `crates/soma/tests/drop_provider_probe.rs` | - | Provider probe test alignment. | HEAD file list. |
| modified | `crates/soma/tests/generated_surfaces.rs` | - | Generated surface expectations. | HEAD file list. |
| modified | `crates/soma/tests/mcp_provider.rs` | - | Provider MCP test alignment. | HEAD file list. |
| modified | `crates/soma/tests/mcporter/test-mcp.sh` | - | Smoke script command alignment. | HEAD file list. |
| modified | `crates/soma/tests/plugin_contract.rs` | - | Plugin contract alignment. | HEAD file list. |
| modified | `crates/soma/tests/python_provider.rs` | - | Python provider test alignment. | HEAD file list. |
| created | `crates/soma/tests/soma_serve.rs` | - | Test `soma serve` runtime command. | HEAD file list. |
| modified | `crates/soma/tests/stdio_mcp.rs` | - | Stdio MCP runtime coverage. | HEAD file list. |
| created | `crates/soma/tests/stdio_remote_api.rs` | - | Test remote stdio MCP API forwarding. | HEAD file list. |
| modified | `crates/soma/tests/wasm_provider.rs` | - | WASM provider test alignment. | HEAD file list. |
| modified | `docs/ARCHITECTURE.md` | - | Single-binary architecture docs. | HEAD file list. |
| modified | `docs/AUTH.md` | - | Auth docs for provider-backed REST and base URL. | HEAD file list. |
| modified | `docs/CARGO_GENERATE.md` | - | Scaffold/export lane wording. | HEAD file list. |
| modified | `docs/CLAUDE.md` | - | Agent instructions alignment. | HEAD file list. |
| modified | `docs/DEPLOYMENT.md` | - | Deployment docs for `soma serve` and dynamic REST. | HEAD file list. |
| modified | `docs/DOCKER.md` | - | Docker docs for `CMD ["serve"]`. | HEAD file list. |
| modified | `docs/JUSTFILE.md` | - | Justfile docs alignment. | HEAD file list. |
| modified | `docs/OBSERVABILITY.md` | - | REST observability wording for dynamic routes. | HEAD file list. |
| modified | `docs/PATTERNS.md` | - | Pattern docs for canonical binary/modes. | HEAD file list. |
| modified | `docs/PLUGINS.md` | - | Plugin docs without missing shared `.mcp.json`. | HEAD file list. |
| modified | `docs/PRE-COMMIT.md` | - | Pre-commit docs alignment. | HEAD file list. |
| modified | `docs/QUICKSTART.md` | - | Quickstart command alignment. | HEAD file list. |
| modified | `docs/RMCP_README_GUIDE.md` | - | Product-first README guide. | HEAD file list. |
| modified | `docs/SCAFFOLD.md` | - | Scaffold/export lane wording. | HEAD file list. |
| modified | `docs/SYSTEMD.md` | - | Systemd command alignment. | HEAD file list. |
| modified | `docs/WEB.md` | - | Web API/provider REST docs. | HEAD file list. |
| modified | `docs/WINDOWS-RUNNER.md` | - | Windows runner docs alignment. | HEAD file list. |
| modified | `docs/adr/0001-stdio-first-plugin-adapter.md` | - | Update ADR for single canonical binary. | HEAD file list. |
| created | `docs/adr/0011-product-first-template-second.md` | - | Record product-first/template-second decision. | HEAD file list. |
| modified | `docs/adr/README.md` | - | Add ADR 0011 index entry. | HEAD file list. |
| modified | `docs/contracts/README.md` | - | Contract docs alignment. | HEAD file list. |
| modified | `docs/contracts/examples/scaffold-intent-upstream-client.json` | - | Scaffold intent example alignment. | HEAD file list. |
| modified | `docs/contracts/plugin-stdio-adapter.md` | - | Plugin stdio contract alignment. | HEAD file list. |
| modified | `docs/contracts/scaffold-intent.schema.json` | - | Scaffold intent schema alignment. | HEAD file list. |
| modified | `docs/generated/openapi.json` | - | Regenerated OpenAPI. | HEAD file list. |
| modified | `docs/generated/plugin.json` | - | Regenerated plugin metadata. | HEAD file list. |
| modified | `docs/generated/provider-surfaces.json` | - | Regenerated provider surfaces JSON. | HEAD file list. |
| modified | `docs/generated/provider-surfaces.md` | - | Regenerated provider surfaces markdown. | HEAD file list. |
| modified | `docs/generated/scripts-index.md` | - | Regenerated script index. | HEAD file list. |
| created | `docs/generated/skills/local-ai-sdk-tools/SKILL.md` | - | Generated provider skill docs. | HEAD file list. |
| created | `docs/generated/skills/local-python-tools/SKILL.md` | - | Generated provider skill docs. | HEAD file list. |
| modified | `docs/generated/skills/static-rust/SKILL.md` | - | Generated static skill docs. | HEAD file list. |
| modified | `docs/specs/scaffold-intent-handoff.md` | - | Scaffold handoff docs alignment. | HEAD file list. |
| modified | `docs/superpowers/plans/2026-07-11-hard-break-soma-rename.md` | - | Historical plan note alignment. | HEAD file list. |
| modified | `entrypoint.sh` | - | Container default command docs/runtime alignment. | HEAD file list. |
| modified | `install.sh` | - | Installer binary/runtime naming. | HEAD file list. |
| created | `packages/soma-rmcp/LICENSE` | - | Package license artifact. | HEAD file list. |
| modified | `packages/soma-rmcp/README.md` | - | Mirrored package README. | HEAD file list. |
| modified | `packages/soma-rmcp/package.json` | - | Package metadata and scripts. | HEAD file list. |
| created | `packages/soma-rmcp/scripts/check-package.js` | - | Package validation script. | HEAD file list. |
| created | `packages/soma-rmcp/scripts/sync-readme.js` | - | Package README sync script. | HEAD file list. |
| modified | `plugins/soma/.codex-plugin/README.md` | - | Codex plugin docs alignment. | HEAD file list. |
| modified | `plugins/soma/CLAUDE.md` | - | Plugin agent docs alignment. | HEAD file list. |
| modified | `plugins/soma/README.md` | - | Plugin package docs alignment. | HEAD file list. |
| modified | `plugins/soma/skills/scaffold-project/SKILL.md` | - | Scaffold/export skill wording. | HEAD file list. |
| modified | `plugins/soma/skills/soma/SKILL.md` | - | Soma skill command/runtime docs. | HEAD file list. |
| modified | `scaffold/cargo-generate/post.rhai` | - | Cargo-generate post-processing alignment. | HEAD file list. |
| modified | `scripts/README.md` | - | Script docs alignment. | HEAD file list. |
| modified | `scripts/check-readme-guide.py` | - | README guide validation updates. | HEAD file list. |
| modified | `scripts/generate-docs.py` | - | Generated doc placeholder and metadata updates. | HEAD file list. |
| created | `scripts/readme_related_servers.py` | - | README related-server helper. | HEAD file list. |
| modified | `server.json` | - | Registry metadata and API base placeholder. | HEAD file list. |
| modified | `xtask/README.md` | - | xtask docs alignment. | HEAD file list. |
| modified | `xtask/src/cargo_generate.rs` | - | Cargo-generate checks alignment. | HEAD file list. |
| modified | `xtask/src/cargo_generate_post.rs` | - | Post-generation rewrite alignment. | HEAD file list. |
| modified | `xtask/src/generated_surfaces.rs` | - | Generated surfaces alignment. | HEAD file list. |
| modified | `xtask/src/scaffold.rs` | - | Scaffold/export behavior alignment. | HEAD file list. |
| modified | `xtask/src/scripts_lane_a.rs` | - | Script lane validation updates. | HEAD file list. |
| modified | `xtask/src/scripts_lane_c.rs` | - | Script lane validation updates. | HEAD file list. |
| modified | `xtask/src/scripts_lane_d.rs` | - | Script lane validation updates. | HEAD file list. |

## Beads Activity

| bead | title | action | final status | why it mattered |
|---|---|---|---|---|
| `rmcp-template-t1xd` | Implement explicit Soma runtime modes | Created earlier in the session, claimed, updated with implementation notes, and closed during this save pass. | closed | Tracked the core single-binary runtime-mode work; closed only after observing PR #117, pushed HEAD `cb57b58`, and local clean/ahead-behind `0 0`. |
| `rmcp-template-f7hs` | Tighten generated provider/API docs after runtime-mode docs sweep | Created during docs sweep and pushed to Dolt. | open | Captures remaining generator-level docs work that was outside the stale-wording pass. |

## Repository Maintenance

### Plans

`find docs/plans -maxdepth 2 -type f` returned no files. No plan files were moved
to `docs/plans/complete/`.

### Beads

`bd show rmcp-template-t1xd --json` showed the runtime-mode bead was still
`in_progress` before this save pass. It was closed with evidence from PR #117,
HEAD `cb57b58`, ahead/behind `0 0`, and passing local docs/format checks.
`bd show rmcp-template-f7hs --json` shows the generated provider/API docs
follow-up remains open. `bd dolt push` completed successfully.

### Worktrees and branches

`git worktree list --porcelain` showed the active worktree at
`/home/jmagar/workspace/soma`, one active mise worktree, the protected
`marketplace-no-mcp` worktree, and three prunable detached worktree records.
No worktrees or branches were removed because cleanup was not required for the
session log and several entries are protected, active, prunable-but-detached, or
not clearly owned by this session.

### Stale docs

The docs sweep was completed before this session artifact. Active product docs
were updated for `soma serve`, product-first wording, remote API mode, plugin
stdio registration, and base API URLs. Remaining generated docs work was filed
as `rmcp-template-f7hs`.

### Transparency

The local git checkout was clean before writing this artifact. The only file to
be committed by this save workflow should be this session markdown file.

## Tools and Skills Used

- **Skill.** Used `vibin:save-to-md`; the skill file was read from
  `/home/jmagar/.codex/plugins/cache/dendrite-no-mcp/vibin/local/skills/save-to-md/SKILL.md`.
- **Shell commands.** Used git, gh, bd, rg, sed, tail, wc, and cargo commands for
  evidence, verification, beads, and PR state.
- **File tools.** Used `apply_patch` to create this markdown file. Earlier
  implementation and docs changes were already committed in HEAD `cb57b58`.
- **Subagents.** Spawned five read-only docs sweep agents for independent stale
  docs lanes: public docs, operator/deploy docs, plugin/MCP docs,
  scaffold/template docs, and generated/release/provider docs.
- **External CLIs.** Used `gh` for PR state and `bd`/Dolt for issue tracking.
- **MCP/browser tools.** No browser or web-search tools were used in this save
  pass. The available Claude transcript was read, but it represented a separate
  short crate-listing session rather than the current Codex implementation
  conversation.

## Commands Executed

| command | result |
|---|---|
| `git status --short --branch` | Clean checkout on `codex/python-provider-operational...origin/codex/python-provider-operational`. |
| `git rev-list --left-right --count @{upstream}...HEAD` | `0 0`; local and upstream branch match. |
| `gh pr view --json number,title,url,state,isDraft,headRefName,baseRefName` | PR #117 open from `codex/python-provider-operational` to `main`; later observed `isDraft: false`. |
| `cargo xtask check-docs` | Passed after regenerating docs: `generated docs are current`. |
| `cargo xtask check-version-sync` | Passed: Soma version-bearing files are in sync at `0.4.7`. |
| `cargo fmt --all --check` | Passed. |
| `rg -n "soma serve mcp|serve mcp|SERVER_BINARY_NAME|target/release/soma-server|--bin soma-server|soma-server|production runtime template|scaffold/runtime template|https://api\\.example\\.com/v1|plugins/soma/\\.mcp\\.json|src/main\\.rs" ...` | Only acceptable remaining hits: `docs/CI.md` references xtask's own `xtask/src/main.rs`; ADR 0011 says Soma must not carry `soma-server` identity. |
| `bd close rmcp-template-t1xd --reason ...` | Closed completed runtime-mode bead. |
| `bd dolt push` | Push complete. |

## Errors Encountered

- `cargo xtask check-docs` initially failed because `.env.example` and
  `config.soma.toml` were stale after changing generator placeholders. Resolved
  by running `cargo xtask generate-docs`, then `cargo xtask check-docs` passed.
- One early `bd status --porcelain` command failed because `bd status` does not
  support `--porcelain`. Subsequent Beads checks used supported commands.
- The available Claude transcript file was huge per line and produced truncated
  shell output when read directly. It still showed the session id and that the
  transcript content was a separate crate-listing session.

## Behavior Changes (Before/After)

| area | before | after |
|---|---|---|
| Runtime command shape | Split identity around `soma` and `soma-server` existed in code/docs. | `soma` is canonical; `soma serve`, `soma mcp`, and `soma <command>` select modes. |
| Remote CLI/MCP behavior | CLI and stdio MCP could imply local provider/in-process behavior. | Remote/API mode forwards actions to the running REST API. |
| Provider REST surface | Provider actions were not fully documented/exposed for generic REST execution. | Generic provider execution and catalog routes are part of the branch; richer generated docs remain follow-up. |
| Product/template framing | Soma could read as a template-first repo. | ADR 0011 and docs frame Soma as product-first with scaffold/export as a separate lane. |
| Package docs | npm/package docs lagged root README and runtime wording. | Package README and package metadata align with the product-first runtime shape. |

## Verification Evidence

| command | expected | actual | status |
|---|---|---|---|
| `cargo xtask check-docs` | Generated docs current. | `generated docs are current`. | pass |
| `cargo xtask check-version-sync` | Version-bearing files in sync. | `OK: soma version-bearing files are in sync at 0.4.7.` | pass |
| `cargo fmt --all --check` | Formatting clean. | Exit code 0. | pass |
| `git rev-list --left-right --count @{upstream}...HEAD` | No local/unpushed commits before session save. | `0 0`. | pass |
| `gh pr view 117 --json ...` | PR exists for current branch. | PR #117 open, not draft, merge state `UNSTABLE` while CI was running. | warn |

## Risks and Rollback

- The branch touches a broad runtime and packaging surface. Roll back the feature
  branch by reverting the PR commits or closing PR #117 before merge.
- Generated OpenAPI/provider docs are improved but still not as detailed as the
  runtime now supports. Track and resolve `rmcp-template-f7hs` before relying on
  generated provider REST docs as complete API documentation.
- PR #117 was observed as `UNSTABLE` because CI was still running, not because a
  specific failed check was observed in the final PR query.

## Decisions Not Taken

- Did not continue treating `cargo xtask check-cargo-generate` as a product
  runtime acceptance gate. ADR 0011 records scaffold/template checks as a
  separate lane.
- Did not remove stale/prunable detached worktree records during the session log
  save because the save workflow is scoped to documentation and no cleanup was
  proven necessary or requested.
- Did not rewrite historical session logs or broad reference docs unless they
  were active docs that could mislead current implementation work.

## References

- PR #117: https://github.com/jmagar/soma/pull/117
- ADR 0011: `docs/adr/0011-product-first-template-second.md`
- Runtime-mode bead: `rmcp-template-t1xd`
- Generated docs follow-up bead: `rmcp-template-f7hs`
- Transcript path observed for this repo:
  `/home/jmagar/.claude/projects/-home-jmagar-workspace-soma/b74df89b-c5f9-4d30-8a18-3b69a5ddc0ac.jsonl`

## Open Questions

- Whether to prune the three prunable detached worktree records observed in
  `git worktree list --porcelain`; they were left untouched because they were
  outside the session scope.
- Whether to wait for every PR #117 CI job before merging; the final PR query
  showed CI `Changes` in progress and merge state `UNSTABLE`.

## Next Steps

1. Watch PR #117 until CI finishes, then address any failed checks.
2. Work `rmcp-template-f7hs` to improve generated provider REST/OpenAPI docs and
   script-index quality.
3. If desired, run a separate worktree hygiene pass to inspect and prune only
   proven-safe prunable detached worktree records.
4. Merge PR #117 once required checks are green and review is complete.

# Hard-Break Soma Rename Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Complete the pivot from the former template identity into Soma as a product. The repo should expose Soma as the binary, crate family, plugin, env-var prefix, package docs, and user-facing identity, with no legacy aliases, shims, compatibility env vars, or old-name documentation left in tracked files.

**Architecture:** Treat this as a hard product rename, not a compatibility migration. Rename filesystem paths first, then package identities and Rust imports, then user-facing strings, env registry, plugins, workflows, and generated contract docs. The resulting product ships a batteries-included `soma` binary and a `soma-rmcp` npm package.

**Tech Stack:** Rust workspace, Cargo packages, RMCP transport crates, Node package wrapper, Claude/Codex plugin metadata, GitHub Actions, xtask contract checks, Beads.

## Global Constraints

- No aliases, legacy support, shim binaries, compatibility env vars, or old package names.
- Leave unrelated pre-existing work alone, especially `docs/superpowers/plans/2026-07-09-provider-drop-in-ux.md`.
- Keep the canonical binary name `soma`.
- Keep the npm package name `soma-rmcp`.
- Rename old crate/package/import prefixes to `soma`.
- Rename the old env prefix to `SOMA_`.
- Rename template/example service residue to Soma where it identifies the product, MCP tool, config, scopes, routes, or tests.
- Do not preserve old runner labels or old repository/package names in workflows; a hard break may require infra labels to be updated outside this repo.
- End with exact-string audits proving tracked files contain no retired identifiers.

---

## Task 1: Rename Filesystem And Package Skeleton

**Files:**
- Move the former product crate to `crates/soma`
- Move the former support crates to `crates/soma-*`
- Move the Node wrapper package to `packages/soma-rmcp`
- Move the plugin bundle to `plugins/soma`
- Update: workspace manifests, lockfile, package manifests, plugin manifests

- [x] Rename tracked directories with `git mv`.
- [x] Rename package names, crate dependencies, binary/test package references, and Cargo workspace members.
- [x] Rename Node wrapper files and npm bin mapping to Soma surfaces.
- [x] Rename plugin directory, skill directory, and plugin metadata to Soma.

## Task 2: Replace Product Identifiers And Env Contract

**Files:**
- Modify: Rust crates, tests, generated docs, scripts, workflows, plugin files, README docs

- [x] Replace old crate import identifiers with `soma_api` and sibling `soma_*` imports.
- [x] Replace old env vars with `SOMA_HOME`, `SOMA_PROVIDER_DIR`, and `SOMA_NOAUTH`.
- [x] Remove compatibility reads for legacy env vars.
- [x] Rename user-facing server, config, service, and command copy to Soma.
- [x] Rename MCP tool/config/scope residue from example/template identity to Soma where it represents the shipped product.

## Task 3: Repair Generated Surfaces And Contracts

**Files:**
- Modify: `xtask`, generated docs, plugin contract fixtures, package tests, workflow references

- [x] Update xtask file paths and hard-coded contract expectations.
- [x] Update generated documentation paths and regenerate where supported.
- [x] Update plugin hook, manifest, and marketplace references to call Soma.
- [x] Update CI/workflow labels and package references to Soma.

## Task 4: Verification And Closeout

**Checks:**
- `cargo fmt`
- `cargo check --all-targets --all-features`
- targeted Rust tests affected by rename
- `cargo xtask check-docs`
- `cargo xtask validate-plugin-layout`
- `cargo xtask patterns`
- npm package dry run for `packages/soma-rmcp`
- exact-string audit over tracked files for old identifiers

- [x] Run the checks that are practical in this branch.
- [x] Fix failures caused by the rename.
- [x] Commit and push the hard-break rename.
- [x] Close the rename bead after verification.

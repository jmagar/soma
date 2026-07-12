# Contract-Backed Family Testing Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans or work-it to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a safe, reusable testing pattern for Rust MCP servers that are REST API clients, starting in `soma`.

**Decision:** Use three evidence tiers:

1. **Static-spec evidence:** `xtask` audits schema docs, OpenAPI docs, action surfaces, plugin contracts, and Soma invariants without starting a server.
2. **Contract-real evidence:** derived servers test REST-client behavior against local mock upstreams and schema fixtures. These tests prove request construction, parsing, error mapping, and safety gates against a contract; they do not prove live upstream health.
3. **Production-real evidence:** optional live smoke through `mcporter`, restricted to explicit read-only allowlists. This is never the default path.

**Research Applied:** FastMCP in-memory tests map to Rust service/tool-dispatch integration tests; `wiremock` fits local upstream simulation; `jsonschema` fits fixture validation; OpenAPI 3.1 documents can be incomplete and need curated overlays; destructive live calls are out of scope by default.

**Engineering Review Applied:** Keep `xtask` dependency-minimal. Do not embed service-specific REST assertions in `xtask`. Ship the current-repo `contract-audit` first, document the per-server mock pattern, and defer cross-repo manifests, generated mock servers, and disposable live upstream provisioning.

---

### File Structure

- Modify `xtask/src/main.rs`: add `contract-audit` command that orchestrates existing static/spec checks.
- Modify `Justfile`: add `contract-audit` recipe and include it in Soma checks where appropriate.
- Modify `README.md`: document the command and family testing policy.
- Modify `docs/TESTING.md`: document static-spec, contract-real, and production-real evidence tiers.
- Modify `docs/PATTERNS.md`: add the reusable REST-client contract testing pattern.
- Modify `tests/template_invariants.rs`: cover the new recipe/help/docs invariants.

### Task 1: Add Static Contract Audit Command

**Files:**
- Modify: `xtask/src/main.rs`
- Modify: `Justfile`

- [ ] **Step 1: Add `cargo xtask contract-audit`**

The command should run only local, non-destructive checks:

```text
patterns
check-test-siblings
scripts/check-schema-docs.py --check
scripts/check-openapi.py --check
scripts/check-scaffold-intent-contract.py
scripts/test-soma-features.sh
```

It must stream subprocess output and fail with the command name that failed. It must not contact live upstream services.

- [ ] **Step 2: Add Justfile recipe**

Add `just contract-audit` as the human-friendly wrapper. Include it in `soma-check` only if that keeps existing checks equivalent or clearer.

- [ ] **Step 3: Update help text**

`cargo xtask --help` should list `contract-audit` and describe it as a local static/spec audit.

### Task 2: Document Contract-Backed Testing Pattern

**Files:**
- Modify: `docs/TESTING.md`
- Modify: `docs/PATTERNS.md`
- Modify: `README.md`

- [ ] **Step 1: Define evidence tiers**

Document the boundary between:

```text
static-spec      local repo contracts, no server or upstream
contract-real    local mock upstreams plus schema fixtures
production-real  explicit read-only live smoke
```

- [ ] **Step 2: Define derived-server mock pattern**

Document that REST-client servers should use `wiremock` or equivalent to assert method/path/query/header/body, return curated fixtures, and validate response fixtures with `jsonschema` or OpenAPI-derived schemas when practical.

- [ ] **Step 3: Define destructive safety rule**

Destructive actions must either fail before network without confirmation or run only against mocks/disposable targets. Live smoke must never include destructive actions unless a disposable target is explicitly configured.

### Task 3: Template Invariants And Verification

**Files:**
- Modify: `tests/template_invariants.rs`

- [ ] **Step 1: Add invariants for command visibility**

Cover that `xtask/src/main.rs`, `Justfile`, and docs mention `contract-audit`.

- [ ] **Step 2: Run focused verification**

Run:

```bash
cargo fmt
cargo test -p xtask
cargo test --test template_invariants
cargo xtask contract-audit
```

Expected: all pass. If full `contract-audit` exposes pre-existing drift, fix the drift or narrow the command only if the check is inappropriate for static-spec evidence.

### Deferred Work

- Cross-repo family manifest runner for `rustarr`, `rustcane`, `synapse2`, and future servers.
- OpenAPI-generated mock upstream server.
- Disposable live upstream provisioning for destructive end-to-end tests.
- Live `mcporter` read-only smoke manifests.

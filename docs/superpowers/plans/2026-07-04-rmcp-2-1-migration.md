# RMCP 2.1 Migration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Upgrade soma from `rmcp 1.7.0` to `rmcp 2.1.0` while preserving action-dispatch MCP behavior, auth, response paging, resources/prompts, elicitation fallbacks, stdio, streamable HTTP, and Soma docs.

**Architecture:** Treat this as a compiler-driven protocol-adapter migration. `soma-mcp` owns RMCP model and transport adaptation; `soma` owns feature wiring and integration tests; service/contracts/runtime stay protocol-agnostic except for existing runtime state/auth boundaries. Response paging is part of the migration because RMCP 2.1 result envelopes make existing budget and cursor assumptions visible.

**Tech Stack:** Rust 2021, `rmcp = "2.1.0"`, Tokio, Axum 0.8, serde/serde_json, schemars 1.2.1, existing Cargo/xtask gates, Beads for durable task tracking.

## Global Constraints

- Current local Soma pins `rmcp = "1.7.0"` in `crates/soma-mcp/Cargo.toml`, `crates/soma/Cargo.toml`, and `crates/soma` dev-dependencies.
- Target published crate is `rmcp = "2.1.0"`; `cargo search rmcp --limit 5` reports `rmcp = "2.1.0"`.
- RMCP 2.1 uses `ContentBlock` for tool result content and `Resource` for resource declarations.
- RMCP 2.1 `Tool` supports richer optional fields, but current `tool_definitions()` emits only `name`, `description`, `inputSchema`, and template-specific `x-soma-*` keys.
- RMCP 2.1 `Meta` reserves `traceparent`, `tracestate`, and `baggage`; this migration must not manually serialize duplicate `_meta`.
- Preserve the thin-shim rule: no protocol migration logic in service-layer business methods, and no business logic in `crates/soma-mcp/src/tools.rs`.
- Preserve structured tool errors as `CallToolResult::structured_error(...)`; keep protocol `ErrorData` for auth/scope denial, unknown MCP tool names, resource/prompt lookup, malformed protocol requests, and server serialization defects.
- Do not implement OpenTelemetry, `rmcp-traces`, automatic result `_meta` propagation, or HTTP trace-header/CORS support in this migration.
- Any deprecated RMCP 2.1 surface retained intentionally must use narrow scoped allowances with comments; no broad crate-wide deprecation suppression.

---

## File Structure

- `crates/soma-mcp/Cargo.toml`: bump RMCP and preserve feature forwarding.
- `crates/soma/Cargo.toml`: bump optional and dev RMCP dependencies.
- `Cargo.lock`: regenerated dependency state.
- `crates/soma-mcp/src/rmcp_server.rs`: RMCP handler/model adaptation, tool/resource conversion, structured error result construction.
- `crates/soma-mcp/src/response_paging.rs`: result content migration plus response budget/cursor hardening.
- `crates/soma-runtime/src/server.rs`: response page cache cursor generation, binding, max entries/bytes, and clone reduction.
- `crates/soma-mcp/src/transport.rs`: streamable HTTP signature adaptation if compiler requires it.
- `crates/soma-mcp/src/tools.rs`: elicitation API-name adaptation only; keep dispatch thin.
- `crates/soma-mcp/src/rmcp_server_tests.rs`: protocol result/resource/tool/paging/security regression tests.
- `crates/soma/tests/tool_dispatch.rs`: real MCP call path, auth/scope, and metadata log-safety tests.
- `crates/soma/tests/stdio_mcp.rs`: stdio child-process smoke.
- `crates/soma/tests/api_routes.rs`: mounted HTTP `/mcp` bearer-auth smoke if the existing router test harness can exercise it cleanly.
- `docs/RMCP-2.1-MIGRATION.md`, `CHANGELOG.md`, `CLAUDE.md`: migration notes and source-of-truth agent docs.

---

### Task 1: Dependency Bump, Preflight, And Feature Graph Guard

**Files:**
- Modify: `crates/soma-mcp/Cargo.toml`
- Modify: `crates/soma/Cargo.toml`
- Modify: `Cargo.lock`

**Interfaces:**
- Consumes: existing feature names `stdio`, `http`, `mcp-stdio`, `mcp-http`, `server`, `full`, `local-adapter`, `test-support`.
- Produces: all direct RMCP dependencies in Soma server/test graph resolve to `rmcp 2.1.0`.

- [ ] **Step 1: Verify upstream API definitions before editing**

Run:

```bash
sed -n '1,120p' /home/jmagar/workspace/upstream/rmcp/crates/rmcp/Cargo.toml
sed -n '1,120p' /home/jmagar/workspace/upstream/rmcp/crates/rmcp/src/model/tool.rs
sed -n '1,100p' /home/jmagar/workspace/upstream/rmcp/crates/rmcp/src/model/resource.rs
```

Expected: upstream source confirms `rmcp 2.1.0`, `Tool`, `ContentBlock`, and `Resource` APIs used by later tasks.

- [ ] **Step 2: Bump direct RMCP versions**

Edit `crates/soma-mcp/Cargo.toml`:

```toml
rmcp = { version = "2.1.0", default-features = false, features = [
  "server",
  "macros",
  "elicitation",
  "schemars",
] }
```

Edit `crates/soma/Cargo.toml`:

```toml
rmcp = { version = "2.1.0", default-features = false, optional = true }
```

Edit the dev-dependency in `crates/soma/Cargo.toml`:

```toml
rmcp = { version = "2.1.0", default-features = false, features = [
  "client",
  "transport-child-process",
] }
```

- [ ] **Step 3: Regenerate lockfile**

Run:

```bash
cargo update -p rmcp --precise 2.1.0
```

Expected: `Cargo.lock` updates `rmcp` and `rmcp-macros` to `2.1.0`.

- [ ] **Step 4: Check graph and MSRV immediately**

Run:

```bash
cargo tree -i rmcp
cargo tree -e features -p soma --features full -i rmcp
cargo tree -e features -p soma --features local-adapter -i rmcp
cargo +1.96 check -p soma-mcp --all-features
```

Expected: feature-resolved graphs use `rmcp 2.1.0` in the server/test surface. The MSRV check may fail on API changes, but it must not fail because RMCP 2.1 requires a Rust version newer than `1.96`.

- [ ] **Step 5: Commit**

```bash
git add crates/soma-mcp/Cargo.toml crates/soma/Cargo.toml Cargo.lock
git commit -m "chore: bump rmcp to 2.1.0"
```

---

### Task 2: Compiler-Driven MCP Adapter Migration

**Files:**
- Modify: `crates/soma-mcp/src/rmcp_server.rs`
- Modify: `crates/soma-mcp/src/response_paging.rs`
- Modify: `crates/soma-mcp/src/transport.rs`
- Modify: `crates/soma-mcp/src/tools.rs`
- Test: `crates/soma-mcp/src/rmcp_server_tests.rs`
- Test: `crates/soma/tests/tool_dispatch.rs`

**Interfaces:**
- Consumes: `rmcp_server(state) -> SomaRmcpServer`, `tool_result_from_json(...) -> Result<CallToolResult, ErrorData>`, `schema_resource() -> Resource`, `rmcp_tool_from_json(Value) -> Result<Tool, ErrorData>`, `execute_tool(...) -> anyhow::Result<Value>`.
- Produces: RMCP 2.1-compatible handler, content, resource, tool, transport, and elicitation adapter code.

- [ ] **Step 1: Run the compiler map**

Run:

```bash
cargo check -p soma-mcp --all-features
```

Expected: FAIL only on RMCP API/model deltas such as `Content` to `ContentBlock`, `RawResource` to `Resource`, handler signatures, transport generics, or elicitation names.

- [ ] **Step 2: Add result/content regression tests**

Add to `crates/soma-mcp/src/rmcp_server_tests.rs`:

```rust
#[test]
fn structured_tool_error_serializes_rmcp_2_1_content_block_text() {
    let result = tool_error_result(serde_json::json!({
        "kind": "mcp_tool_error",
        "schema_version": 1,
        "code": "example_error",
        "message": "safe message",
        "retryable": false,
        "remediation": "Change the request.",
    }))
    .expect("tool error result should serialize");

    let serialized = serde_json::to_value(&result).expect("result should serialize");
    assert_eq!(serialized["isError"], true);
    assert_eq!(serialized["content"][0]["type"], "text");
    let text = serialized["content"][0]["text"].as_str().expect("text content");
    let payload: serde_json::Value = serde_json::from_str(text).expect("text should be JSON");
    assert_eq!(payload["code"], "example_error");
    assert_eq!(result.structured_content.as_ref(), Some(&payload));
}
```

- [ ] **Step 3: Replace model types and keep structured result policy**

Use RMCP 2.1 imports in `rmcp_server.rs`:

```rust
use rmcp::model::{
    CallToolRequestParams, CallToolResult, ContentBlock, GetPromptRequestParams, GetPromptResult,
    Implementation, ListPromptsResult, ListResourcesResult, ListToolsResult,
    PaginatedRequestParams, ReadResourceRequestParams, ReadResourceResult, Resource,
    ResourceContents, ServerCapabilities, ServerInfo, Tool,
};
```

Use RMCP 2.1 imports in `response_paging.rs`:

```rust
use rmcp::{
    model::{CallToolResult, ContentBlock},
    ErrorData,
};
```

Set result content with:

```rust
result.content = vec![ContentBlock::text(text)];
```

- [ ] **Step 4: Migrate resource construction**

Use:

```rust
fn schema_resource() -> Resource {
    Resource::new(SCHEMA_RESOURCE_URI, "soma tool schema")
        .with_description("JSON schema for the Soma MCP tool and its action-based parameters")
        .with_mime_type("application/json")
}
```

- [ ] **Step 5: Make tool conversion policy explicit**

Keep conversion limited to fields currently emitted by `tool_definitions()`: `name`, `description`, and `inputSchema`. Add a test proving custom `x-soma-*` guidance remains inside `inputSchema` only if the generator actually places it there; otherwise document that top-level `x-soma-*` keys are intentionally not mapped to RMCP 2.1 `_meta` during this migration.

Use:

```rust
fn rmcp_tool_from_json(value: Value) -> Result<Tool, ErrorData> {
    let name = value
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| ErrorData::internal_error("tool definition missing name", None))?;
    let description = value
        .get("description")
        .and_then(Value::as_str)
        .map(|d| Cow::Owned(d.to_string()));
    let input_schema = value
        .get("inputSchema")
        .and_then(Value::as_object)
        .cloned()
        .ok_or_else(|| ErrorData::internal_error("tool definition missing inputSchema", None))?;

    Ok(Tool::new_with_raw(
        Cow::Owned(name.to_string()),
        description,
        Arc::new(input_schema),
    ))
}
```

- [ ] **Step 6: Fix elicitation variants with real RMCP 2.1 names**

Keep graceful fallbacks and use the names already present in the current code:

```rust
Err(ElicitationError::UserDeclined) => {
    Ok(service.elicited_name_greeting(ElicitedNameOutcome::Declined))
}
Err(ElicitationError::UserCancelled) => {
    Ok(service.elicited_name_greeting(ElicitedNameOutcome::Cancelled))
}
Err(ElicitationError::CapabilityNotSupported) => {
    Ok(service.elicited_name_greeting(ElicitedNameOutcome::Unsupported))
}
```

- [ ] **Step 7: Run adapter tests**

Run:

```bash
cargo test -p soma-mcp structured_tool_error_serializes_rmcp_2_1_content_block_text --all-features
cargo test -p soma-mcp oversized_tool_errors_return_valid_overflow_envelope --all-features
cargo test -p soma-mcp server_info_advertises_tools_resources_prompts --all-features
cargo test -p soma --features test-support test_real_call_tool_path_returns_status_json
```

Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add crates/soma-mcp/src/rmcp_server.rs crates/soma-mcp/src/response_paging.rs crates/soma-mcp/src/transport.rs crates/soma-mcp/src/tools.rs crates/soma-mcp/src/rmcp_server_tests.rs crates/soma/tests/tool_dispatch.rs
git commit -m "refactor: migrate MCP adapter to rmcp 2.1"
```

---

### Task 3: Response Paging Security And Budget Hardening

**Files:**
- Modify: `crates/soma-runtime/src/server.rs`
- Modify: `crates/soma-mcp/src/response_paging.rs`
- Modify: `crates/soma-mcp/src/rmcp_server.rs`
- Test: `crates/soma-mcp/src/rmcp_server_tests.rs`

**Interfaces:**
- Consumes: `ResponsePageStore`, `_response_cursor`, `_response_offset`, `_response_page_bytes`, `MAX_RESPONSE_BYTES`.
- Produces: high-entropy caller-bound cursors, bounded cache memory, bounded continuation envelopes, and explicit full-result budget tests.

- [ ] **Step 1: Write cursor isolation and cache-bound tests**

Add tests covering:

```rust
#[test]
fn response_page_cursors_are_not_sequential() {
    let store = ResponsePageStore::default();
    let first = store.insert_for_test("first");
    let second = store.insert_for_test("second");
    assert_ne!(first, second);
    assert!(!first.starts_with("rsp_"));
    assert!(!second.starts_with("rsp_"));
}

#[test]
fn response_page_cursor_rejects_wrong_subject_or_action() {
    let store = ResponsePageStore::default();
    let cursor = store.insert_bound_for_test("alice", "soma", Some("status"), "payload");
    assert!(store.get_bound_for_test(&cursor, "alice", "soma", Some("status")).is_some());
    assert!(store.get_bound_for_test(&cursor, "bob", "soma", Some("status")).is_none());
    assert!(store.get_bound_for_test(&cursor, "alice", "soma", Some("echo")).is_none());
}
```

If helper names differ during implementation, keep the behavior: cursors are high entropy, same-subject continuations work, cross-subject/action continuations fail.

- [ ] **Step 2: Replace sequential cursors**

Use high-entropy cursor IDs such as:

```rust
fn new_response_cursor() -> String {
    format!("rsp_{}", uuid::Uuid::new_v4().simple())
}
```

Add `uuid` as a dependency only in the crate that owns `ResponsePageStore`.

- [ ] **Step 3: Bind cached pages to caller/tool/action**

Store metadata beside serialized content:

```rust
struct ResponsePageEntry {
    owner: String,
    tool: String,
    action: Option<String>,
    serialized: std::sync::Arc<str>,
    inserted_at: std::time::Instant,
}
```

Use the authenticated subject, actor key, or stable loopback-dev owner as `owner`. If auth is disabled on loopback, use a constant owner such as `"loopback-dev"`.

- [ ] **Step 4: Add cache limits**

Enforce both:

```rust
const MAX_RESPONSE_PAGE_CACHE_ENTRIES: usize = 128;
const MAX_RESPONSE_PAGE_CACHE_BYTES: usize = 16 * 1024 * 1024;
```

Evict oldest entries until both limits are satisfied after insert.

- [ ] **Step 5: Avoid whole-response clone on cached reads**

Store serialized responses as `Arc<str>` and return `Arc<str>` from cache lookups so continuation reads do not clone megabytes of response data.

- [ ] **Step 6: Add full-result envelope and large-args tests**

Add tests that serialize the full `CallToolResult`:

```rust
let full = serde_json::to_vec(&result).expect("result serializes");
assert!(
    full.len() <= MAX_RESPONSE_BYTES || result_is_documented_as_content_budget_only(&result),
    "full CallToolResult envelope exceeded the declared budget"
);
```

Add a large original-arguments test where continuation output remains bounded and does not echo a large argument body into every page response.

- [ ] **Step 7: Run paging tests**

Run:

```bash
cargo test -p soma-mcp --all-features response_page
cargo test -p soma-mcp --all-features cursor
```

Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add crates/soma-runtime/src/server.rs crates/soma-mcp/src/response_paging.rs crates/soma-mcp/src/rmcp_server.rs crates/soma-mcp/src/rmcp_server_tests.rs crates/soma-runtime/Cargo.toml Cargo.lock
git commit -m "fix: harden MCP response paging cursors"
```

---

### Task 4: Metadata And Log-Safety Regression Coverage

**Files:**
- Modify: `crates/soma-mcp/src/rmcp_server.rs`
- Modify: `crates/soma/tests/tool_dispatch.rs`
- Test: `crates/soma/tests/dispatch_logging.rs`

**Interfaces:**
- Consumes: RMCP 2.1 request `context.meta` and result `CallToolResult::meta`.
- Produces: explicit non-goal policy for this migration: do not consume request `_meta`, do not attach result `_meta`, and never log raw `baggage`/`tracestate`.

- [ ] **Step 1: Add non-goal assertion for result `_meta`**

Add:

```rust
#[test]
fn template_does_not_attach_protocol_result_meta_during_rmcp_2_1_migration() {
    let result = tool_error_result(serde_json::json!({
        "kind": "mcp_tool_error",
        "schema_version": 1,
        "code": "example_error",
        "message": "safe message",
        "retryable": false,
        "remediation": "Change the request.",
    }))
    .expect("tool error result should serialize");

    let serialized = serde_json::to_value(&result).expect("result should serialize");
    assert!(serialized.get("_meta").is_none());
    assert!(result.meta.is_none());
}
```

- [ ] **Step 2: Add log-capture baggage test if metadata is read**

If implementation reads `context.meta`, add a tracing-capture test proving sample values do not appear in success or structured-error logs:

```rust
assert!(!logs.contains("secret-user-id=do-not-log"));
assert!(!logs.contains("baggage"));
assert!(!logs.contains("tracestate"));
```

If implementation does not read `context.meta`, add a comment in `rmcp_server.rs` near `call_tool`:

```rust
// RMCP 2.1 exposes request `_meta` through RequestContext. This migration
// intentionally does not read or propagate trace/baggage metadata; future
// metadata support belongs in rmcp-traces with bounded redaction.
```

- [ ] **Step 3: Run metadata/log tests**

Run:

```bash
cargo test -p soma-mcp template_does_not_attach_protocol_result_meta_during_rmcp_2_1_migration --all-features
cargo test -p soma --features test-support dispatch_logging
```

Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/soma-mcp/src/rmcp_server.rs crates/soma/tests/tool_dispatch.rs crates/soma/tests/dispatch_logging.rs
git commit -m "test: document rmcp 2.1 metadata non-goals"
```

---

### Task 5: Mounted HTTP Auth And Stdio Smoke Coverage

**Files:**
- Modify: `crates/soma-mcp/src/transport.rs`
- Modify: `crates/soma/tests/api_routes.rs`
- Modify: `crates/soma/tests/stdio_mcp.rs`
- Test: `crates/soma/tests/api_routes.rs`
- Test: `crates/soma/tests/stdio_mcp.rs`

**Interfaces:**
- Consumes: server router, `bearer_state(token)`, `/mcp`, stdio child process.
- Produces: proof that RMCP 2.1 streamable HTTP preserves auth context and stdio remains JSON-RPC clean.

- [ ] **Step 1: Add mounted `/mcp` bearer-auth smoke**

In the existing route test harness, POST a JSON-RPC `tools/call` request to `/mcp` with:

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "soma",
    "arguments": { "action": "status" }
  }
}
```

Assert:

```rust
// no Authorization header
assert_eq!(response.status(), axum::http::StatusCode::UNAUTHORIZED);

// bad bearer token
assert_eq!(response.status(), axum::http::StatusCode::UNAUTHORIZED);

// good bearer token
assert!(response.status().is_success());
assert!(body.contains("\"status\":\"ok\""));
```

- [ ] **Step 2: Preserve stdio extraction**

Keep the serialized result extraction:

```rust
fn text_content_json(result: &rmcp::model::CallToolResult) -> serde_json::Value {
    let value = serde_json::to_value(result).expect("tool result should serialize");
    let text = value["content"][0]["text"]
        .as_str()
        .expect("tool result should contain text content");
    serde_json::from_str(text).expect("tool text content should be JSON")
}
```

- [ ] **Step 3: Run transport tests**

Run:

```bash
cargo test -p soma --features mcp-http mcp_http_bearer_auth_reaches_call_tool
cargo test -p soma --features local-adapter stdio_child_process_lists_tools_and_calls_actions
```

Expected: PASS. If the streamable HTTP protocol requires initialize before `tools/call`, include the initialize request in the helper and keep the auth assertions unchanged.

- [ ] **Step 4: Commit**

```bash
git add crates/soma-mcp/src/transport.rs crates/soma/tests/api_routes.rs crates/soma/tests/stdio_mcp.rs
git commit -m "test: cover rmcp 2.1 transports"
```

---

### Task 6: Documentation, Changelog, And Deferred Hardening Notes

**Files:**
- Create: `docs/RMCP-2.1-MIGRATION.md`
- Modify: `CHANGELOG.md`
- Modify: `CLAUDE.md`

**Interfaces:**
- Consumes: final implemented API changes from Tasks 1-5.
- Produces: downstream migration notes before final verification and release checks.

- [ ] **Step 1: Write migration docs**

Create `docs/RMCP-2.1-MIGRATION.md`:

```markdown
# RMCP 2.1 Migration Notes

Soma targets `rmcp = "2.1.0"` for its MCP server, stdio client tests, and streamable HTTP transport.

## Local Changes

- `Content` result construction became `ContentBlock::text(...)`.
- Schema resources use RMCP 2.1 `Resource` construction.
- Tool definitions still come from `soma-contracts::actions::ACTION_SPECS`; this migration maps only fields currently emitted by Soma.
- Request `_meta` is accepted by RMCP 2.1, but Soma does not consume request metadata or attach protocol-level result `_meta` during this migration.
- Response paging cursors are high-entropy and bound to the caller/action context.
- `traceparent`, `tracestate`, and `baggage` are reserved RMCP `_meta` keys. Do not manually serialize a second `_meta` object.

## Deferred Work

- `rmcp-traces` integration and bounded trace metadata redaction.
- HTTP `traceparent`/`tracestate`/`baggage` header and CORS support.
- Rich RMCP 2.1 `ToolAnnotations`, `ToolExecution`, icons, and output schema mapping beyond fields currently emitted by Soma.
- Elicitation input length/format bounds before using scaffold elicitation as a mutating workflow.

## Verification

Run:

```bash
cargo fmt --all --check
cargo test -p soma-mcp --all-features
cargo test -p soma --features test-support
cargo test -p soma --features local-adapter stdio_child_process_lists_tools_and_calls_actions
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo tree -e features -p soma --features full -i rmcp
```
```

- [ ] **Step 2: Update changelog**

Add under `[Unreleased]`:

```markdown
### Changed

- Upgrade the MCP adapter and integration tests from `rmcp 1.7.0` to `rmcp 2.1.0`, preserving action-dispatch tools, resources, prompts, elicitation, stdio, streamable HTTP, and mounted auth behavior.
```

- [ ] **Step 3: Update agent memory source**

In `CLAUDE.md`, replace stale template version language with:

```markdown
Soma targets `rmcp 2.1.0`; older derived servers may still be on 1.6/1.7 until migrated.
```

Verify symlinks:

```bash
test -L AGENTS.md && test "$(readlink AGENTS.md)" = "CLAUDE.md"
test -L GEMINI.md && test "$(readlink GEMINI.md)" = "CLAUDE.md"
```

- [ ] **Step 4: Commit**

```bash
git add docs/RMCP-2.1-MIGRATION.md CHANGELOG.md CLAUDE.md
git commit -m "docs: document rmcp 2.1 migration"
```

---

### Task 7: Final Verification And Release Parity

**Files:**
- Modify only deterministic generated files if commands produce them.

**Interfaces:**
- Consumes: all previous tasks.
- Produces: green local verification aligned with CI expectations and release/version checks.

- [ ] **Step 1: Run formatting and diff hygiene**

Run:

```bash
cargo fmt --all --check
git diff --check
```

Expected: PASS.

- [ ] **Step 2: Run package checks**

Run:

```bash
cargo check -p soma-mcp --all-features
cargo check -p soma --all-features
cargo +1.96 check -p soma --all-features
```

Expected: PASS.

- [ ] **Step 3: Run tests**

Run:

```bash
cargo test -p soma-mcp --all-features
cargo test -p soma --features test-support
cargo test -p soma --features local-adapter stdio_child_process_lists_tools_and_calls_actions
cargo test -p soma --features mcp-http mcp_http_bearer_auth_reaches_call_tool
```

Expected: PASS.

- [ ] **Step 4: Run lint aligned with CI-plus-all-features**

Run:

```bash
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Expected: PASS.

- [ ] **Step 5: Run release checks**

Run:

```bash
cargo xtask check-version-sync
cargo xtask check-release-versions --base origin/main --head HEAD --mode pr
```

Expected: PASS or a concrete version/changelog instruction that is fixed before completion.

- [ ] **Step 6: Re-run feature graph checks**

Run:

```bash
cargo tree -i rmcp
cargo tree -e features -p soma --features full -i rmcp
cargo tree -e features -p soma --features local-adapter -i rmcp
```

Expected: template server/test graph resolves to `rmcp v2.1.0`; any older transitive RMCP must be outside the server model/trait boundary and documented in `docs/RMCP-2.1-MIGRATION.md`.

- [ ] **Step 7: Commit generated verification changes only if present**

Run:

```bash
git status --short
```

If deterministic generated files changed, commit them:

```bash
git add Cargo.lock docs/generated/openapi.json server.json CHANGELOG.md
git commit -m "chore: refresh generated files for rmcp 2.1"
```

If no files changed, do not create an empty commit.

---

### Deferred Follow-Up: Non-Core Hardening

**Files:**
- Future work only; no core migration file ownership.

**Interfaces:**
- Consumes: successful RMCP 2.1 migration.
- Produces: backlog items when the team wants richer public crate/server hardening.

- [ ] **Deferred Item 1: Elicitation input bounds**

Add length/format validation for `ScaffoldIntentInput` before reusing scaffold elicitation for mutating project creation flows.

- [ ] **Deferred Item 2: HTTP trace-header support**

Add CORS/header support for `traceparent`, `tracestate`, and `baggage` only when `rmcp-traces` or an equivalent bounded redaction crate is ready.

- [ ] **Deferred Item 3: Rich RMCP 2.1 tool metadata**

Map action metadata into `ToolAnnotations`, `ToolExecution`, output schema, icons, or `_meta` only after deciding the public contract for those hints.

---

## Self-Review

**Spec coverage:** The updated plan covers dependency unification, RMCP 2.1 model migration, response paging security/performance hardening, metadata non-goals and log-safety, mounted HTTP auth, stdio smoke, docs, and final CI-aligned verification.

**Placeholder scan:** The plan contains no unresolved placeholder tasks. Compiler-guided alternatives are bounded to API signature selection and keep exact behavior assertions.

**Type consistency:** The plan consistently uses `CallToolResult`, `ContentBlock`, `Resource`, `Tool`, `Meta`, `Peer`, `RequestContext<RoleServer>`, `ResponsePageStore`, and existing template helper names.

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-07-04-rmcp-2-1-migration.md`. Two execution options:

1. **Subagent-Driven (recommended)** - Dispatch a fresh subagent per task, review between tasks, fast iteration.
2. **Inline Execution** - Execute tasks in this session using executing-plans, batch execution with checkpoints.


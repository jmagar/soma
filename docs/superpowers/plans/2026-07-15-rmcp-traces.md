# RMCP Traces Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement GitHub issue #76 by adding a standalone `rmcp-traces` crate targeting `rmcp 2.2.0`, plus one narrow Soma MCP request-side proof path.

**Architecture:** `rmcp-traces` is a leaf platform crate that depends on RMCP model types and owns only validation, bounds, trust labels, and redacted trace summaries. Soma consumes it only in `crates/soma-mcp/src/rmcp_server.rs` after auth is established, logging a safe request trace summary without touching tool business logic, response paging, or result `_meta`.

**Tech Stack:** Rust 2021, `rmcp = "2.2.0"`, serde/serde_json, optional package dry-run checks, existing Soma MCP integration tests, existing `soma-test-support` tracing capture helpers, Beads for tracking.

## Global Constraints

- Target `rmcp 2.2.0` explicitly in manifests, docs, tests, and compatibility notes.
- Do not introduce a path dependency on upstream RMCP.
- Do not compile multiple incompatible `rmcp::model::Meta` or `CallToolResult` types.
- `rmcp-traces` must not depend on `soma-*`, auth, gateway, codemode, Axum, Tower, reqwest, OpenTelemetry SDK/exporters, or tracing subscriber setup.
- V1 is request-side only: do not attach result `_meta`.
- V1 defers HTTP propagation; document the deferral instead of adding CORS/header behavior.
- Treat inbound metadata as untrusted unless a host explicitly marks it trusted.
- Never log raw `baggage`, raw arbitrary `_meta`, cookies, tokens, API keys, email samples, or full baggage members.
- Keep trace hot-path metadata reads O(1): inspect only `traceparent`, `tracestate`, and `baggage`.
- Keep trace plumbing out of `crates/soma-mcp/src/tools.rs` and service actions.
- Preserve Soma's structured-error vs protocol-error policy and response paging behavior.
- Keep plugin manifests versionless.

---

## Engineering Review Applied

The Lavra engineering review found no blocker for the core request-side crate, but it narrowed the implementation:

- Architecture: make `rmcp 2.2.0` explicit, normalize manifests, keep `rmcp-traces` as a leaf crate, integrate only in `soma-mcp`.
- Simplicity: skip result `_meta`, generic sanitizer sprawl, HTTP propagation, and new test infrastructure in v1.
- Security: do not parse/log trace metadata before auth; document that upstream `rmcp=debug` can log raw requests before Soma redaction; test Soma logs with RMCP debug filtered out.
- Performance: bound inputs before owned conversion; keep baggage summaries as counts only; avoid repeated raw `_meta` clones.

## File Structure

- Modify `Cargo.toml`: add `crates/rmcp-traces` to the workspace.
- Modify `crates/soma/Cargo.toml`, `crates/soma-mcp/Cargo.toml`, `crates/soma-service/Cargo.toml`, `crates/soma-auth/Cargo.toml`: pin direct RMCP dependencies to `2.2.0`.
- Create `docs/adr/0012-rmcp-traces-rmcp-2-2.md`: compatibility target, source evidence, non-goals, deferrals.
- Create `crates/rmcp-traces/Cargo.toml`: public-oriented crate metadata, no product deps.
- Create `crates/rmcp-traces/src/lib.rs`: crate docs and exports.
- Create `crates/rmcp-traces/src/trace_context.rs`: `TraceParent`, `TraceContext`, `TraceSummary`, bounds, redaction helpers.
- Create `crates/rmcp-traces/README.md`: usage, limits, deferrals, migration notes.
- Create `crates/rmcp-traces/tests/core_trace_context.rs`: core validation and redaction tests.
- Modify `crates/soma-mcp/src/rmcp_server.rs`: read `RequestContext.meta` after auth and log safe trace summary fields.
- Modify `crates/soma-mcp/Cargo.toml`: depend on `rmcp-traces`.
- Modify `crates/soma/tests/tool_dispatch.rs`: real MCP request metadata/logging regression test.
- Modify `CHANGELOG.md`: note the new crate and request-side MCP trace logging.
- Update Beads: close implemented children; close HTTP child as explicitly deferred by v1 scope.

### Task 1: Pin RMCP 2.2.0 And Record The Contract

**Files:**
- Modify: `crates/soma/Cargo.toml`
- Modify: `crates/soma-mcp/Cargo.toml`
- Modify: `crates/soma-service/Cargo.toml`
- Modify: `crates/soma-auth/Cargo.toml`
- Create: `docs/adr/0012-rmcp-traces-rmcp-2-2.md`
- Modify: `CHANGELOG.md`

**Interfaces:**
- Consumes: current Cargo graph resolving `rmcp v2.2.0`.
- Produces: all direct RMCP dependency declarations explicitly request `2.2.0`; ADR gives later tasks the compatibility and non-goal contract.

- [ ] **Step 1: Write the failing compatibility check**

Run:

```bash
rg -n 'rmcp.*2\.1\.0|version = "2\.1\.0"' crates/soma/Cargo.toml crates/soma-mcp/Cargo.toml crates/soma-service/Cargo.toml crates/soma-auth/Cargo.toml
```

Expected: FAIL for the task, meaning the command prints current `2.1.0` declarations that must be changed.

- [ ] **Step 2: Pin all direct RMCP declarations to 2.2.0**

Change only RMCP dependency declarations. The resulting relevant lines must be:

```toml
# crates/soma/Cargo.toml
rmcp = { version = "2.2.0", default-features = false, optional = true }

[dev-dependencies]
rmcp = { version = "2.2.0", default-features = false, features = [
  "client",
  "transport-child-process",
] }
```

```toml
# crates/soma-mcp/Cargo.toml
rmcp = { version = "2.2.0", default-features = false, features = [
  "server",
  "macros",
  "elicitation",
  "schemars",
] }
```

```toml
# crates/soma-service/Cargo.toml
rmcp = { version = "2.2.0", default-features = false, features = ["client", "transport-child-process", "transport-streamable-http-client-reqwest"] }
```

```toml
# crates/soma-auth/Cargo.toml
[dependencies.rmcp-client]
package = "rmcp"
version = "2.2.0"
default-features = false
features = ["client", "transport-streamable-http-client-reqwest"]
optional = true
```

- [ ] **Step 3: Add the compatibility ADR**

Create `docs/adr/0012-rmcp-traces-rmcp-2-2.md` with this content:

```markdown
# ADR 0012: rmcp-traces targets RMCP 2.2.0

## Status

Accepted

## Context

GitHub issue #76 adds a standalone `rmcp-traces` crate for MCP trace metadata. The compatibility target is `rmcp 2.2.0`, not older planning text.

Live evidence used for this decision:

- `Cargo.lock` resolves `rmcp` and `rmcp-macros` to `2.2.0`.
- Local upstream RMCP source at `/home/jmagar/workspace/upstream/rmcp` was fast-forwarded to `origin/main` commit `92581bee53cd883e5b479616b369c1fcb4eb2fc2`.
- RMCP 2.2.0 exposes `RequestParamsMeta` helpers for request `_meta`, `traceparent`, `tracestate`, and `baggage`.
- RMCP 2.2.0 exposes `Meta` as a transparent JSON object with reserved W3C trace keys.
- RMCP 2.2.0 serializes `CallToolResult::_meta`, but Soma v1 intentionally does not attach result metadata.

## Decision

`rmcp-traces` v1 targets `rmcp 2.2.0` and uses the published RMCP model types directly. The workspace must not compile two incompatible `rmcp::model::Meta` or `rmcp::model::CallToolResult` types.

The v1 crate is request-side only:

- Parse and validate `traceparent`.
- Preserve bounded `tracestate` and `baggage` values in private fields.
- Expose redacted summaries that never print raw baggage.
- Integrate into Soma MCP after auth by reading `RequestContext.meta`.

## Non-goals

- No result `_meta` helpers in v1.
- No HTTP propagation in v1.
- No OpenTelemetry SDK or exporter.
- No tracing subscriber setup.
- No auth, gateway, codemode, Axum, Tower, or reqwest dependency.
- No product-specific session IDs or UI widget semantics.
- No automatic Lab, Cortex, Axon, or fleet migration.

## Security Notes

Inbound trace metadata is untrusted unless a host application explicitly marks it trusted. Public caller sampled flags are advisory only. Baggage is a privacy hotspot and must not be logged raw.

Upstream RMCP debug logging can log raw requests before Soma receives `RequestContext.meta`. Production deployments should not enable broad `rmcp=debug` logging for untrusted traffic until that upstream behavior is filtered or changed.

## Consequences

The initial proof is small and reversible. HTTP propagation and result metadata can be added later behind explicit budgets and trust policies.
```

- [ ] **Step 4: Add changelog entry**

Under `## [Unreleased]` in `CHANGELOG.md`, add:

```markdown
- Add an `rmcp-traces` platform crate targeting `rmcp 2.2.0` with bounded request trace metadata parsing and redacted Soma MCP trace summaries.
```

- [ ] **Step 5: Verify the pin**

Run:

```bash
cargo update -p rmcp --precise 2.2.0
cargo update -p rmcp-macros --precise 2.2.0
rg -n 'rmcp.*2\.1\.0|version = "2\.1\.0"' crates/soma/Cargo.toml crates/soma-mcp/Cargo.toml crates/soma-service/Cargo.toml crates/soma-auth/Cargo.toml
cargo tree -i rmcp --workspace
```

Expected:

- `cargo update` is a no-op or keeps `Cargo.lock` at `rmcp 2.2.0`.
- `rg` exits with code 1 because no direct RMCP manifest declaration still says `2.1.0`.
- `cargo tree` prints one `rmcp v2.2.0` root and no second RMCP version.

- [ ] **Step 6: Commit**

```bash
git add crates/soma/Cargo.toml crates/soma-mcp/Cargo.toml crates/soma-service/Cargo.toml crates/soma-auth/Cargo.toml Cargo.lock docs/adr/0012-rmcp-traces-rmcp-2-2.md CHANGELOG.md docs/superpowers/plans/2026-07-15-rmcp-traces.md
git commit -m "chore: target rmcp 2.2.0 for traces"
```

### Task 2: Add The rmcp-traces Crate Skeleton

**Files:**
- Modify: `Cargo.toml`
- Create: `crates/rmcp-traces/Cargo.toml`
- Create: `crates/rmcp-traces/src/lib.rs`
- Create: `crates/rmcp-traces/src/trace_context.rs`
- Create: `crates/rmcp-traces/README.md`

**Interfaces:**
- Consumes: Task 1's `rmcp 2.2.0` compatibility target.
- Produces: package `rmcp-traces`, library module `rmcp_traces`, public exports `TraceContext`, `TraceLimits`, `TraceParent`, `TraceParseError`, `TraceSummary`, and `TraceTrust`.

- [ ] **Step 1: Write the failing package check**

Run:

```bash
cargo check -p rmcp-traces
```

Expected: FAIL with "package ID specification `rmcp-traces` did not match any packages".

- [ ] **Step 2: Add the workspace member**

In root `Cargo.toml`, add the new member alphabetically near other crates:

```toml
  "crates/rmcp-traces",
```

- [ ] **Step 3: Create `crates/rmcp-traces/Cargo.toml`**

```toml
[package]
name = "rmcp-traces"
version = "0.1.0"
edition = "2021"
rust-version = "1.96"
authors.workspace = true
description = "Bounded, log-safe W3C trace metadata helpers for RMCP."
homepage.workspace = true
license = "MIT"
repository.workspace = true
readme = "README.md"
keywords = ["mcp", "rmcp", "trace", "trace-context", "baggage"]
categories = ["development-tools", "network-programming"]

[features]
default = []

[dependencies]
rmcp = { version = "2.2.0", default-features = false }
serde_json = "1"
```

- [ ] **Step 4: Create `src/lib.rs`**

```rust
//! Bounded, log-safe helpers for RMCP trace metadata.
//!
//! `rmcp-traces` complements RMCP's own `_meta` serialization. It does not own
//! MCP wire encoding and it does not attach result `_meta` in v1.

#![forbid(unsafe_code)]

mod trace_context;

pub use trace_context::{
    BAGGAGE_KEY, TRACEPARENT_KEY, TRACESTATE_KEY, TraceContext, TraceLimits, TraceParent,
    TraceParseError, TraceSummary, TraceTrust,
};
```

- [ ] **Step 5: Create minimal `src/trace_context.rs`**

```rust
use std::{error::Error, fmt};

use rmcp::model::Meta;
use serde_json::Value;

pub const TRACEPARENT_KEY: &str = "traceparent";
pub const TRACESTATE_KEY: &str = "tracestate";
pub const BAGGAGE_KEY: &str = "baggage";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TraceTrust {
    Untrusted,
    Trusted,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TraceLimits {
    pub max_traceparent_len: usize,
    pub max_tracestate_len: usize,
    pub max_baggage_len: usize,
    pub max_baggage_members: usize,
}

impl Default for TraceLimits {
    fn default() -> Self {
        Self {
            max_traceparent_len: 55,
            max_tracestate_len: 512,
            max_baggage_len: 8 * 1024,
            max_baggage_members: 64,
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct TraceParent {
    raw: String,
    trace_id: String,
    span_id: String,
    sampled: bool,
}

impl TraceParent {
    pub fn parse(value: &str) -> Result<Self, TraceParseError> {
        let limits = TraceLimits::default();
        if value.len() > limits.max_traceparent_len {
            return Err(TraceParseError::ValueTooLong {
                field: TRACEPARENT_KEY,
                actual: value.len(),
                max: limits.max_traceparent_len,
            });
        }
        parse_traceparent(value)
    }

    pub fn as_str(&self) -> &str {
        &self.raw
    }

    pub fn trace_id(&self) -> &str {
        &self.trace_id
    }

    pub fn span_id(&self) -> &str {
        &self.span_id
    }

    pub fn sampled(&self) -> bool {
        self.sampled
    }

    fn trace_id_short(&self) -> &str {
        &self.trace_id[..8]
    }

    fn span_id_short(&self) -> &str {
        &self.span_id[..8]
    }
}

impl fmt::Debug for TraceParent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TraceParent")
            .field("trace_id", &self.trace_id_short())
            .field("span_id", &self.span_id_short())
            .field("sampled", &self.sampled)
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct TraceContext {
    traceparent: TraceParent,
    tracestate: Option<String>,
    baggage: Option<String>,
    trust: TraceTrust,
}

impl TraceContext {
    pub fn from_meta(meta: &Meta, trust: TraceTrust) -> Result<Option<Self>, TraceParseError> {
        Self::from_meta_with_limits(meta, trust, TraceLimits::default())
    }

    pub fn from_meta_with_limits(
        meta: &Meta,
        trust: TraceTrust,
        limits: TraceLimits,
    ) -> Result<Option<Self>, TraceParseError> {
        let Some(traceparent) = optional_meta_str(meta, TRACEPARENT_KEY)? else {
            return Ok(None);
        };
        if traceparent.len() > limits.max_traceparent_len {
            return Err(TraceParseError::ValueTooLong {
                field: TRACEPARENT_KEY,
                actual: traceparent.len(),
                max: limits.max_traceparent_len,
            });
        }
        let traceparent = parse_traceparent(traceparent)?;
        let tracestate = bounded_optional_meta_string(meta, TRACESTATE_KEY, limits.max_tracestate_len)?;
        let baggage = bounded_optional_meta_string(meta, BAGGAGE_KEY, limits.max_baggage_len)?;
        Ok(Some(Self {
            traceparent,
            tracestate,
            baggage,
            trust,
        }))
    }

    pub fn apply_to_meta(&self, meta: &mut Meta) {
        meta.set_traceparent(self.traceparent.as_str());
        if let Some(tracestate) = &self.tracestate {
            meta.set_tracestate(tracestate);
        }
        if let Some(baggage) = &self.baggage {
            meta.set_baggage(baggage);
        }
    }

    pub fn traceparent(&self) -> &TraceParent {
        &self.traceparent
    }

    pub fn summary(&self) -> TraceSummary {
        TraceSummary::from_context(self)
    }
}

impl fmt::Debug for TraceContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.summary().fmt(f)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TraceSummary {
    pub trace_id: Option<String>,
    pub span_id: Option<String>,
    pub sampled: Option<bool>,
    pub trust: TraceTrust,
    pub has_tracestate: bool,
    pub baggage_member_count: usize,
    pub sensitive_baggage_member_count: usize,
    pub invalid: Option<String>,
}

impl TraceSummary {
    pub fn absent() -> Self {
        Self {
            trace_id: None,
            span_id: None,
            sampled: None,
            trust: TraceTrust::Untrusted,
            has_tracestate: false,
            baggage_member_count: 0,
            sensitive_baggage_member_count: 0,
            invalid: None,
        }
    }

    pub fn invalid(error: &TraceParseError) -> Self {
        Self {
            invalid: Some(error.safe_reason()),
            ..Self::absent()
        }
    }

    pub fn from_context(context: &TraceContext) -> Self {
        let (baggage_member_count, sensitive_baggage_member_count) =
            summarize_baggage(context.baggage.as_deref());
        Self {
            trace_id: Some(context.traceparent.trace_id_short().to_owned()),
            span_id: Some(context.traceparent.span_id_short().to_owned()),
            sampled: Some(context.traceparent.sampled()),
            trust: context.trust,
            has_tracestate: context.tracestate.is_some(),
            baggage_member_count,
            sensitive_baggage_member_count,
            invalid: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TraceParseError {
    NonStringMeta { field: &'static str },
    ValueTooLong { field: &'static str, actual: usize, max: usize },
    InvalidTraceParentLength { actual: usize },
    InvalidTraceParentFormat,
    UnsupportedVersion,
    InvalidTraceId,
    InvalidSpanId,
    InvalidFlags,
}

impl TraceParseError {
    pub fn safe_reason(&self) -> String {
        match self {
            Self::NonStringMeta { field } => format!("{field} was not a string"),
            Self::ValueTooLong { field, actual, max } => {
                format!("{field} exceeded {max} bytes (actual {actual})")
            }
            Self::InvalidTraceParentLength { actual } => {
                format!("traceparent length was {actual}, expected 55")
            }
            Self::InvalidTraceParentFormat => "traceparent format was invalid".to_owned(),
            Self::UnsupportedVersion => "traceparent version was unsupported".to_owned(),
            Self::InvalidTraceId => "traceparent trace id was invalid".to_owned(),
            Self::InvalidSpanId => "traceparent span id was invalid".to_owned(),
            Self::InvalidFlags => "traceparent flags were invalid".to_owned(),
        }
    }
}

impl fmt::Display for TraceParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.safe_reason())
    }
}

impl Error for TraceParseError {}

fn optional_meta_str<'a>(meta: &'a Meta, field: &'static str) -> Result<Option<&'a str>, TraceParseError> {
    match meta.get(field) {
        None => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.as_str())),
        Some(_) => Err(TraceParseError::NonStringMeta { field }),
    }
}

fn bounded_optional_meta_string(
    meta: &Meta,
    field: &'static str,
    max: usize,
) -> Result<Option<String>, TraceParseError> {
    let Some(value) = optional_meta_str(meta, field)? else {
        return Ok(None);
    };
    if value.len() > max {
        return Err(TraceParseError::ValueTooLong {
            field,
            actual: value.len(),
            max,
        });
    }
    Ok(Some(value.to_owned()))
}

fn parse_traceparent(value: &str) -> Result<TraceParent, TraceParseError> {
    if value.len() != 55 {
        return Err(TraceParseError::InvalidTraceParentLength {
            actual: value.len(),
        });
    }
    let bytes = value.as_bytes();
    if bytes[2] != b'-' || bytes[35] != b'-' || bytes[52] != b'-' {
        return Err(TraceParseError::InvalidTraceParentFormat);
    }
    let version = &value[0..2];
    let trace_id = &value[3..35];
    let span_id = &value[36..52];
    let flags = &value[53..55];
    if version != "00" {
        return Err(TraceParseError::UnsupportedVersion);
    }
    if !is_lower_hex(version) || !is_lower_hex(trace_id) || !is_lower_hex(span_id) || !is_lower_hex(flags) {
        return Err(TraceParseError::InvalidTraceParentFormat);
    }
    if trace_id.bytes().all(|b| b == b'0') {
        return Err(TraceParseError::InvalidTraceId);
    }
    if span_id.bytes().all(|b| b == b'0') {
        return Err(TraceParseError::InvalidSpanId);
    }
    let flag_byte = u8::from_str_radix(flags, 16).map_err(|_| TraceParseError::InvalidFlags)?;
    Ok(TraceParent {
        raw: value.to_owned(),
        trace_id: trace_id.to_owned(),
        span_id: span_id.to_owned(),
        sampled: flag_byte & 0x01 == 0x01,
    })
}

fn is_lower_hex(value: &str) -> bool {
    value.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}

fn summarize_baggage(baggage: Option<&str>) -> (usize, usize) {
    let Some(baggage) = baggage else {
        return (0, 0);
    };
    let mut total = 0;
    let mut sensitive = 0;
    for member in baggage.split(',') {
        let key = member.split_once('=').map(|(key, _)| key).unwrap_or(member).trim();
        if key.is_empty() {
            continue;
        }
        total += 1;
        if is_sensitive_key(key) {
            sensitive += 1;
        }
    }
    (total, sensitive)
}

fn is_sensitive_key(key: &str) -> bool {
    let normalized: String = key
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect();
    matches!(
        normalized.as_str(),
        "authorization"
            | "cookie"
            | "setcookie"
            | "password"
            | "secret"
            | "token"
            | "accesstoken"
            | "refreshtoken"
            | "apikey"
            | "xapikey"
            | "privatekey"
            | "session"
            | "sessionid"
    )
}
```

- [ ] **Step 6: Create `README.md`**

```markdown
# rmcp-traces

Bounded, log-safe helpers for W3C trace metadata carried through RMCP `_meta`.

This crate targets `rmcp 2.2.0`. RMCP owns wire serialization for `_meta`; this crate adds validation, bounds, trust labels, and redacted summaries.

## V1 Scope

- Parse and validate request-side `traceparent`.
- Preserve bounded `tracestate` and `baggage` privately.
- Produce `TraceSummary` values safe for logs.
- Treat inbound metadata as untrusted by default.

## Non-goals

- No result `_meta` helpers in v1.
- No HTTP propagation in v1.
- No OpenTelemetry SDK/exporter.
- No tracing subscriber setup.
- No auth, gateway, Axum, Tower, reqwest, codemode, or product runtime dependencies.

## Safety

Never log raw baggage. Baggage may contain PII or credentials. `TraceContext` debug formatting delegates to `TraceSummary` and does not print raw baggage values.

Upstream RMCP debug logs can include raw request values before an application receives `RequestContext.meta`. Avoid broad `rmcp=debug` logging for untrusted production traffic.
```

- [ ] **Step 7: Verify the skeleton**

Run:

```bash
cargo check -p rmcp-traces
cargo check -p rmcp-traces --no-default-features
cargo metadata --no-deps --format-version 1 | jq -e '.packages[] | select(.name=="rmcp-traces")'
```

Expected: all commands pass.

- [ ] **Step 8: Commit**

```bash
git add Cargo.toml crates/rmcp-traces
git commit -m "feat: add rmcp-traces crate skeleton"
```

### Task 3: Add Core Trace Context Tests And Finish Validation

**Files:**
- Modify: `crates/rmcp-traces/src/trace_context.rs`
- Create: `crates/rmcp-traces/tests/core_trace_context.rs`

**Interfaces:**
- Consumes: public API from Task 2.
- Produces: validated trace context behavior for `Meta`, including fail-soft parse errors and redacted summaries.

- [ ] **Step 1: Add core tests**

Create `crates/rmcp-traces/tests/core_trace_context.rs`:

```rust
use rmcp::model::Meta;
use rmcp_traces::{TraceContext, TraceLimits, TraceParent, TraceTrust};
use serde_json::json;

const VALID_TRACEPARENT: &str =
    "00-0af7651916cd43dd8448eb211c80319c-00f067aa0ba902b7-01";

#[test]
fn traceparent_round_trips_through_meta() {
    let mut meta = Meta::new();
    meta.insert("unrelated".to_owned(), json!("kept"));
    meta.set_traceparent(VALID_TRACEPARENT);
    meta.set_tracestate("vendor=value");
    meta.set_baggage("region=us-east-1,accessToken=super-secret-token");

    let context = TraceContext::from_meta(&meta, TraceTrust::Untrusted)
        .expect("valid trace metadata")
        .expect("trace context exists");

    let mut output = Meta::new();
    output.insert("unrelated".to_owned(), json!("kept"));
    context.apply_to_meta(&mut output);

    assert_eq!(output.get_traceparent(), Some(VALID_TRACEPARENT));
    assert_eq!(output.get_tracestate(), Some("vendor=value"));
    assert_eq!(
        output.get_baggage(),
        Some("region=us-east-1,accessToken=super-secret-token")
    );
    assert_eq!(output.get("unrelated"), Some(&json!("kept")));
}

#[test]
fn malformed_traceparents_are_rejected() {
    for value in [
        "",
        "00-0af7651916cd43dd8448eb211c80319c-00f067aa0ba902b7",
        "01-0af7651916cd43dd8448eb211c80319c-00f067aa0ba902b7-01",
        "00-00000000000000000000000000000000-00f067aa0ba902b7-01",
        "00-0af7651916cd43dd8448eb211c80319c-0000000000000000-01",
        "00-0AF7651916CD43DD8448EB211C80319C-00f067aa0ba902b7-01",
        "00-0af7651916cd43dd8448eb211c80319c-00f067aa0ba902b7-zz",
    ] {
        assert!(TraceParent::parse(value).is_err(), "{value} should be rejected");
    }
}

#[test]
fn oversized_values_are_rejected_before_parsing() {
    let mut meta = Meta::new();
    meta.set_traceparent(&"x".repeat(4096));
    assert!(TraceContext::from_meta(&meta, TraceTrust::Untrusted).is_err());

    let mut meta = Meta::new();
    meta.set_traceparent(VALID_TRACEPARENT);
    meta.set_baggage(&"a".repeat(20));
    let limits = TraceLimits {
        max_baggage_len: 8,
        ..TraceLimits::default()
    };
    assert!(TraceContext::from_meta_with_limits(&meta, TraceTrust::Untrusted, limits).is_err());
}

#[test]
fn absent_or_non_string_trace_metadata_is_fail_soft() {
    let meta = Meta::new();
    assert!(TraceContext::from_meta(&meta, TraceTrust::Untrusted).unwrap().is_none());

    let mut meta = Meta::new();
    meta.insert("traceparent".to_owned(), json!(123));
    assert!(TraceContext::from_meta(&meta, TraceTrust::Untrusted).is_err());
}

#[test]
fn summary_never_contains_raw_baggage_values() {
    let mut meta = Meta::new();
    meta.set_traceparent(VALID_TRACEPARENT);
    meta.set_baggage(
        "email=alice@example.com,accessToken=super-secret-token,x-api-key=abc123,sessionId=s123",
    );

    let context = TraceContext::from_meta(&meta, TraceTrust::Untrusted)
        .unwrap()
        .unwrap();
    let summary = context.summary();
    let debug = format!("{context:?} {summary:?}");

    assert_eq!(summary.baggage_member_count, 4);
    assert_eq!(summary.sensitive_baggage_member_count, 3);
    assert!(debug.contains("0af76519"));
    assert!(!debug.contains("alice@example.com"));
    assert!(!debug.contains("super-secret-token"));
    assert!(!debug.contains("abc123"));
    assert!(!debug.contains("s123"));
}
```

- [ ] **Step 2: Run the tests**

```bash
cargo test -p rmcp-traces --test core_trace_context
```

Expected: PASS. If a test fails, fix `trace_context.rs` without expanding scope beyond the public API in Task 2.

- [ ] **Step 3: Run crate feature checks**

```bash
cargo test -p rmcp-traces
cargo check -p rmcp-traces --no-default-features
cargo package -p rmcp-traces --allow-dirty --no-verify
cargo tree -p rmcp-traces
```

Expected:

- Tests and checks pass.
- Package dry-run completes.
- `cargo tree -p rmcp-traces` includes `rmcp`, `serde_json`, and their transitive dependencies only; it does not include `soma-auth`, `jsonwebtoken`, `rsa`, `reqwest`, Axum, Tower, or product crates.

- [ ] **Step 4: Commit**

```bash
git add crates/rmcp-traces Cargo.lock
git commit -m "feat: validate rmcp trace metadata"
```

### Task 4: Integrate Safe Request Trace Summaries Into Soma MCP

**Files:**
- Modify: `crates/soma-mcp/Cargo.toml`
- Modify: `crates/soma-mcp/src/rmcp_server.rs`
- Modify: `crates/soma/tests/tool_dispatch.rs`

**Interfaces:**
- Consumes: `rmcp_traces::{TraceContext, TraceSummary, TraceTrust}`.
- Produces: Soma MCP logs safe trace summary fields after auth and before tool execution.

- [ ] **Step 1: Write the failing integration test**

Modify the import in `crates/soma/tests/tool_dispatch.rs` to include request metadata helpers and tracing capture:

```rust
use rmcp::{
    model::{CallToolRequestParams, ClientRequest, Meta, Request},
    service::{PeerRequestOptions, ServiceError},
    ServiceExt,
};
use soma_test_support::{tracing_test_lock, SharedBuf};
```

Add this test after `test_real_call_tool_path_returns_status_json`:

```rust
#[allow(clippy::await_holding_lock)]
#[tokio::test(flavor = "current_thread")]
async fn test_real_call_tool_path_logs_safe_trace_summary() -> anyhow::Result<()> {
    let _lock = tracing_test_lock();
    let buf = SharedBuf::new();
    let subscriber = tracing_subscriber::fmt()
        .with_writer(buf.writer())
        .with_ansi(false)
        .without_time()
        .with_max_level(tracing::Level::DEBUG)
        .finish();
    let guard = tracing::subscriber::set_default(subscriber);

    let (server_transport, client_transport) = tokio::io::duplex(16 * 1024);
    let server_handle = tokio::spawn(async move {
        rmcp_server(loopback_state())
            .serve(server_transport)
            .await?
            .waiting()
            .await?;
        anyhow::Ok(())
    });

    let mut args = serde_json::Map::new();
    args.insert("action".to_owned(), json!("status"));

    let mut meta = Meta::new();
    meta.set_traceparent("00-0af7651916cd43dd8448eb211c80319c-00f067aa0ba902b7-01");
    meta.set_tracestate("vendor=value");
    meta.set_baggage(
        "email=alice@example.com,accessToken=super-secret-token,x-api-key=abc123,sessionId=s123",
    );
    let mut options = PeerRequestOptions::no_options();
    options.meta = Some(meta);

    let client = ().serve(client_transport).await?;
    let result = client
        .send_cancellable_request(
            ClientRequest::CallToolRequest(Request::new(
                CallToolRequestParams::new("soma").with_arguments(args),
            )),
            options,
        )
        .await?
        .await_response()
        .await?;

    assert!(result.structured_content.is_some());

    client.cancel().await?;
    server_handle.await??;
    drop(guard);

    let logs = buf.contents();
    assert!(logs.contains("trace_id"), "missing trace id summary: {logs}");
    assert!(logs.contains("0af76519"), "missing short trace id: {logs}");
    assert!(logs.contains("baggage_member_count"), "missing baggage count: {logs}");
    assert!(!logs.contains("alice@example.com"), "raw email leaked: {logs}");
    assert!(!logs.contains("super-secret-token"), "raw token leaked: {logs}");
    assert!(!logs.contains("abc123"), "raw api key leaked: {logs}");
    assert!(!logs.contains("s123"), "raw session leaked: {logs}");
    Ok(())
}
```

Run:

```bash
cargo test -p soma --test tool_dispatch test_real_call_tool_path_logs_safe_trace_summary --all-features -- --nocapture
```

Expected: FAIL because `rmcp-traces` is not integrated and the logs do not contain safe trace summary fields. If upstream RMCP debug logs leak raw request metadata before Soma gets the request, change the subscriber to use `tracing_subscriber::filter::Targets` so `soma_mcp=debug` and `rmcp=info`, then keep the ADR warning from Task 1.

- [ ] **Step 2: Add the dependency to `crates/soma-mcp/Cargo.toml`**

```toml
rmcp-traces = { path = "../rmcp-traces" }
```

- [ ] **Step 3: Add safe summary logging after auth**

In `crates/soma-mcp/src/rmcp_server.rs`, add:

```rust
use rmcp_traces::{TraceContext, TraceSummary, TraceTrust};
```

Add this helper near the other private helpers:

```rust
fn trace_summary_from_context(context: &RequestContext<RoleServer>) -> TraceSummary {
    match TraceContext::from_meta(&context.meta, TraceTrust::Untrusted) {
        Ok(Some(trace_context)) => trace_context.summary(),
        Ok(None) => TraceSummary::absent(),
        Err(error) => TraceSummary::invalid(&error),
    }
}
```

In `call_tool`, after `let auth = require_auth_context(&self.state, &context)?;`, add:

```rust
        let trace_summary = trace_summary_from_context(&context);
```

Change the start log from:

```rust
        tracing::info!(tool = %tool_name, action = %action, "MCP tool execution started");
```

to:

```rust
        tracing::info!(
            tool = %tool_name,
            action = %action,
            trace_id = ?trace_summary.trace_id,
            span_id = ?trace_summary.span_id,
            trace_sampled = ?trace_summary.sampled,
            trace_trust = ?trace_summary.trust,
            has_tracestate = trace_summary.has_tracestate,
            baggage_member_count = trace_summary.baggage_member_count,
            sensitive_baggage_member_count = trace_summary.sensitive_baggage_member_count,
            trace_invalid = ?trace_summary.invalid,
            "MCP tool execution started"
        );
```

Do not add trace fields to auth-denial protocol errors, unknown-tool protocol errors, response page payloads, cached pages, or result `_meta`.

- [ ] **Step 4: Verify focused integration**

Run:

```bash
cargo test -p soma --test tool_dispatch test_real_call_tool_path_logs_safe_trace_summary --all-features -- --nocapture
cargo test -p soma --test tool_dispatch test_real_call_tool_path_returns_status_json --all-features
cargo test -p soma-mcp response_paging --all-features
```

Expected: all pass. The log test must not contain raw baggage values.

- [ ] **Step 5: Commit**

```bash
git add crates/soma-mcp/Cargo.toml crates/soma-mcp/src/rmcp_server.rs crates/soma/tests/tool_dispatch.rs Cargo.lock
git commit -m "feat: log safe MCP trace summaries"
```

### Task 5: Document Deferrals And Close Beads

**Files:**
- Modify: `crates/rmcp-traces/README.md`
- Modify: `crates/rmcp-traces/src/lib.rs`
- Modify: Beads state through `bd`

**Interfaces:**
- Consumes: implemented request-side crate and integration.
- Produces: public documentation that HTTP/result metadata are intentionally deferred, plus Beads status matching the v1 scope.

- [ ] **Step 1: Expand crate docs**

Update `crates/rmcp-traces/src/lib.rs` crate docs to include:

```rust
//! ## RMCP Version
//!
//! This crate targets `rmcp 2.2.0`.
//!
//! ## Deferred V1 Surfaces
//!
//! Result `_meta` helpers are deferred because protocol-level metadata must be
//! budgeted together with Soma's normal, paged, cached-page, structured-error,
//! auth-denial, and protocol-denial paths.
//!
//! HTTP propagation is deferred because baggage and sampled flags need an
//! application trust-boundary policy before public header forwarding is safe.
```

Update `crates/rmcp-traces/README.md` to add:

```markdown
## Soma Integration

Soma reads `RequestContext.meta` in `crates/soma-mcp/src/rmcp_server.rs` after auth context is available. It logs only `TraceSummary` fields: short trace/span identifiers, sampled flag, trust label, tracestate presence, baggage member count, and sensitive baggage member count.

Soma does not attach result `_meta` in v1. This prevents trace metadata from bypassing response paging or `MAX_RESPONSE_BYTES`.

## Future Work

- HTTP propagation behind an app-level trust policy.
- Result `_meta` helpers with one serialized budget across every result path.
- Detailed Lab, Cortex, and Axon migrations after the Soma proof is stable.
```

- [ ] **Step 2: Run doc tests and package check**

```bash
cargo test -p rmcp-traces --doc
cargo package -p rmcp-traces --allow-dirty --no-verify
```

Expected: both pass.

- [ ] **Step 3: Update Beads status**

After code and docs pass, record completion in Beads with issue-specific
comments and close reasons:

- Compatibility target and crate API contract captured in `docs/adr/0012-rmcp-traces-rmcp-2-2.md`.
- `rmcp-traces` workspace crate skeleton and package metadata added.
- Core RMCP Meta trace helpers implemented and tested.
- HTTP propagation explicitly deferred for v1 and documented as future work.
- Log-safe trace summaries and baggage redaction counts implemented and tested.
- Soma MCP request path logs safe trace summaries without raw baggage.
- Crate docs document usage, non-goals, deferrals, and migration path.
- Epic close reason: GH #76 v1 implemented for `rmcp 2.2.0`; HTTP/result metadata are explicitly deferred.

Expected: child beads and epic close cleanly. If `bd close rmcp-template-xh6c` refuses because a child is still open, inspect `bd show rmcp-template-xh6c` and close the remaining completed child with a specific reason.

- [ ] **Step 4: Commit**

```bash
git add crates/rmcp-traces docs/adr/0012-rmcp-traces-rmcp-2-2.md .beads
git commit -m "docs: document rmcp traces scope"
```

If `.beads` is ignored or the `bd` backend stores data outside Git, do not force-add unrelated tracker internals. Use `bd dolt status` and `bd dolt push` during final publication.

### Task 6: Full Verification And PR Readiness

**Files:**
- Verify all touched files.

**Interfaces:**
- Consumes: Tasks 1-5.
- Produces: green worktree ready for review waves and PR creation.

- [ ] **Step 1: Run formatting**

```bash
cargo fmt --all -- --check
```

Expected: PASS. If it fails, run `cargo fmt --all`, inspect the diff, and rerun the check.

- [ ] **Step 2: Run focused package tests**

```bash
cargo test -p rmcp-traces
cargo test -p soma-mcp --all-features
cargo test -p soma --test tool_dispatch --all-features
```

Expected: PASS.

- [ ] **Step 3: Run dependency and package gates**

```bash
cargo check -p rmcp-traces --no-default-features
cargo package -p rmcp-traces --allow-dirty --no-verify
cargo tree -p rmcp-traces
cargo tree -i rmcp --workspace
```

Expected:

- `rmcp-traces` builds without default features.
- Package dry-run passes.
- `rmcp-traces` does not pull product/runtime/auth/web dependencies.
- Workspace has only `rmcp v2.2.0`.

- [ ] **Step 4: Run repo quality gates**

```bash
cargo test --workspace --all-features
cargo clippy --workspace --all-features -- -D warnings
cargo xtask check-version-sync
cargo xtask check-release-versions --base origin/main --head HEAD --mode pr
```

Expected: PASS. If a gate is pre-existing and unrelated, capture evidence, then fix it in this worktree if it blocks the PR.

- [ ] **Step 5: Final status**

```bash
git status --short
bd dolt status || true
```

Expected: only intentional files are dirty before final review/commit/push. No background process remains running.

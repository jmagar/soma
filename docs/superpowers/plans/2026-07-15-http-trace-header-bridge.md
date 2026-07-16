# HTTP Trace Header Bridge Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the first GH #76 follow-up slice: an optional `rmcp-traces/http` feature that safely extracts trusted inbound W3C HTTP trace headers into bounded RMCP metadata and safe summaries.

**Architecture:** Keep this slice inside the leaf `rmcp-traces` crate. The HTTP module borrows `http::HeaderMap`, parses raw header strings through crate-private helpers, and constructs returned `Meta` only after a valid single `traceparent`. Soma runtime config, MCP consumption, CORS gating, and outbound propagation stay in later beads.

**Tech Stack:** Rust 2021, `rmcp = 2.2.0`, optional `http = 1`, `cargo test`, existing `rmcp::model::Meta` trace metadata methods.

## Global Constraints

- Target RMCP version is exactly `2.2.0`.
- `rmcp-traces` must remain a leaf crate; no Soma runtime/config dependency belongs in this slice.
- HTTP support must be feature-gated behind `rmcp-traces/http`.
- Default HTTP policy strips baggage.
- Missing or invalid `traceparent` must return empty `Meta` and zero optional metadata counts.
- Split header joining must be bounded before allocation.
- No derived `Debug` may expose raw `traceparent`, `tracestate`, or `baggage`.
- Trace flags such as `03` are valid; sampled is `flags & 0x01 != 0`.

---

## File Structure

- Modify `crates/rmcp-traces/Cargo.toml`: add optional `http` dependency and feature.
- Modify `crates/rmcp-traces/src/lib.rs`: export the `http` module behind the feature.
- Modify `crates/rmcp-traces/src/trace_context.rs`: expose crate-private parser/validator/summary helpers and add trust-preserving absent summaries.
- Create `crates/rmcp-traces/src/http.rs`: implement `HttpTracePolicy`, `HttpTraceExtraction`, bounded header collection, and `extract_http_trace`.
- Create `crates/rmcp-traces/tests/http_propagation.rs`: cover HTTP extraction behavior and privacy.
- Modify `crates/rmcp-traces/tests/core_trace_context.rs`: add focused regression tests only for shared helper behavior.

### Task 1: Feature Gate And Test Skeleton

**Files:**
- Modify: `crates/rmcp-traces/Cargo.toml`
- Modify: `crates/rmcp-traces/src/lib.rs`
- Create: `crates/rmcp-traces/tests/http_propagation.rs`

**Interfaces:**
- Produces: `rmcp_traces::http::{HttpTracePolicy, HttpTraceExtraction, extract_http_trace}` behind `feature = "http"`.

- [ ] **Step 1: Add the optional dependency and feature**

Change `crates/rmcp-traces/Cargo.toml` to include:

```toml
[features]
default = []
http = ["dep:http"]

[dependencies]
http = { version = "1", optional = true }
rmcp = { version = "=2.2.0", default-features = false }
serde_json = "1"
```

- [ ] **Step 2: Export the module behind the feature**

Add this to `crates/rmcp-traces/src/lib.rs` after `mod trace_context;`:

```rust
#[cfg(feature = "http")]
pub mod http;
```

- [ ] **Step 3: Create the failing HTTP extraction test**

Create `crates/rmcp-traces/tests/http_propagation.rs` with:

```rust
#![cfg(feature = "http")]

use http::HeaderMap;
use rmcp_traces::http::{extract_http_trace, HttpTracePolicy};
use rmcp_traces::{TraceTrust, BAGGAGE_KEY, TRACEPARENT_KEY, TRACESTATE_KEY};

const VALID_TRACEPARENT: &str =
    "00-0af7651916cd43dd8448eb211c80319c-00f067aa0ba902b7-01";

#[test]
fn extracts_valid_trace_headers_and_strips_baggage_by_default() {
    let mut headers = HeaderMap::new();
    headers.insert(TRACEPARENT_KEY, VALID_TRACEPARENT.parse().unwrap());
    headers.insert(TRACESTATE_KEY, "vendor=value".parse().unwrap());
    headers.insert(BAGGAGE_KEY, "sessionId=s123,region=us-east-1".parse().unwrap());

    let extraction = extract_http_trace(&headers, HttpTracePolicy::default());

    assert_eq!(extraction.meta.get(TRACEPARENT_KEY).and_then(|v| v.as_str()), Some(VALID_TRACEPARENT));
    assert_eq!(extraction.meta.get(TRACESTATE_KEY).and_then(|v| v.as_str()), Some("vendor=value"));
    assert!(extraction.meta.get(BAGGAGE_KEY).is_none());
    assert_eq!(extraction.summary.trace_id_prefix(), Some("0af76519"));
    assert_eq!(extraction.summary.span_id_prefix(), Some("00f067aa"));
    assert_eq!(extraction.summary.sampled(), Some(true));
    assert_eq!(extraction.summary.trust(), TraceTrust::Untrusted);
    assert!(extraction.summary.has_tracestate());
    assert_eq!(extraction.summary.baggage_member_count(), 0);
    assert_eq!(extraction.summary.invalid_count(), 0);
}
```

- [ ] **Step 4: Run the failing feature test**

Run: `cargo test -p rmcp-traces --features http extracts_valid_trace_headers_and_strips_baggage_by_default -- --exact`

Expected: compile failure because `rmcp_traces::http` does not exist yet.

### Task 2: Crate-Private Trace Helpers

**Files:**
- Modify: `crates/rmcp-traces/src/trace_context.rs`
- Modify: `crates/rmcp-traces/tests/core_trace_context.rs`

**Interfaces:**
- Produces: crate-private `TraceSummary::absent_with_trust(trust: TraceTrust) -> TraceSummary`.
- Produces: crate-private `parse_traceparent_value(value: &str, limits: TraceLimits) -> Result<TraceParent, TraceParseError>`.
- Produces: crate-private `validate_tracestate_value(value: Option<&str>) -> Result<(), TraceParseError>`.
- Produces: crate-private `validate_baggage_value(value: Option<&str>, max_members: usize) -> Result<(), TraceParseError>`.
- Produces: crate-private `TraceSummary::from_valid_traceparent(traceparent: &TraceParent, trust: TraceTrust) -> TraceSummary`.
- Produces: crate-private `TraceSummary::record_invalid_reason(&mut self, error: &TraceParseError)`.

- [ ] **Step 1: Write a regression test for trace flags**

Append to `crates/rmcp-traces/tests/core_trace_context.rs`:

```rust
#[test]
fn trace_flags_accept_reserved_bits_and_keep_sampled_bit() {
    let mut meta = Meta::new();
    meta.set_traceparent("00-0af7651916cd43dd8448eb211c80319c-00f067aa0ba902b7-03");

    let summary = TraceSummary::from_meta(&meta, TraceTrust::Untrusted);

    assert_eq!(summary.trace_id_prefix(), Some("0af76519"));
    assert_eq!(summary.sampled(), Some(true));
    assert_eq!(summary.invalid_count(), 0);
}
```

- [ ] **Step 2: Run the new test before adding the HTTP helpers**

Run: `cargo test -p rmcp-traces trace_flags_accept_reserved_bits_and_keep_sampled_bit`

Expected: pass if existing flags behavior is already correct.

- [ ] **Step 3: Add the helper methods and crate-private parser wrappers**

In `crates/rmcp-traces/src/trace_context.rs`, add:

```rust
impl TraceSummary {
    pub(crate) fn absent_with_trust(trust: TraceTrust) -> Self {
        Self {
            trace_id_prefix: None,
            span_id_prefix: None,
            sampled: None,
            trust,
            has_tracestate: false,
            baggage_member_count: 0,
            sensitive_baggage_member_count: 0,
            invalid_reasons: Vec::new(),
        }
    }

    pub(crate) fn from_valid_traceparent(traceparent: &TraceParent, trust: TraceTrust) -> Self {
        Self::from_traceparent(traceparent, trust)
    }

    pub(crate) fn record_invalid_reason(&mut self, error: &TraceParseError) {
        self.record_invalid(error);
    }
}

pub(crate) fn parse_traceparent_value(
    value: &str,
    limits: TraceLimits,
) -> Result<TraceParent, TraceParseError> {
    if value.len() > limits.max_traceparent_len {
        return Err(TraceParseError::ValueTooLong {
            field: TRACEPARENT_KEY,
            actual: value.len(),
            max: limits.max_traceparent_len,
        });
    }
    parse_traceparent(value)
}

pub(crate) fn validate_tracestate_value(value: Option<&str>) -> Result<(), TraceParseError> {
    validate_tracestate(value)
}

pub(crate) fn validate_baggage_value(
    value: Option<&str>,
    max_members: usize,
) -> Result<(), TraceParseError> {
    validate_baggage(value, max_members)
}
```

Keep `TraceParent` itself `pub(crate)` so `http.rs` can pass references to summary helpers without making it public.

- [ ] **Step 4: Strengthen baggage validation**

Update `validate_baggage` so each member validates key, value, and properties:

```rust
let Some((key, rest)) = member.split_once('=') else {
    return Err(TraceParseError::InvalidBaggageMember);
};
if !is_valid_baggage_key(key.trim()) {
    return Err(TraceParseError::InvalidBaggageMember);
}
let mut value_and_props = rest.split(';');
let value = value_and_props.next().unwrap_or("").trim();
if !is_valid_baggage_value(value) {
    return Err(TraceParseError::InvalidBaggageMember);
}
for property in value_and_props {
    let property = property.trim();
    if property.is_empty() {
        return Err(TraceParseError::InvalidBaggageMember);
    }
    if let Some((property_key, property_value)) = property.split_once('=') {
        if !is_valid_baggage_key(property_key.trim())
            || !is_valid_baggage_value(property_value.trim())
        {
            return Err(TraceParseError::InvalidBaggageMember);
        }
    } else if !is_valid_baggage_key(property) {
        return Err(TraceParseError::InvalidBaggageMember);
    }
}
```

Add:

```rust
fn is_valid_baggage_value(value: &str) -> bool {
    value
        .bytes()
        .all(|byte| matches!(byte, 0x21 | 0x23..=0x2b | 0x2d..=0x3a | 0x3c..=0x5b | 0x5d..=0x7e))
}
```

- [ ] **Step 5: Run helper tests**

Run: `cargo test -p rmcp-traces trace_flags_accept_reserved_bits_and_keep_sampled_bit`

Expected: the trace flags test passes. The trust-preserving absent summary is proven through the HTTP missing-`traceparent` test in Task 4 without expanding public API.

### Task 3: HTTP Extraction Module

**Files:**
- Create: `crates/rmcp-traces/src/http.rs`
- Modify: `crates/rmcp-traces/tests/http_propagation.rs`

**Interfaces:**
- Consumes: helpers from Task 2.
- Produces: `extract_http_trace(headers: &HeaderMap, policy: HttpTracePolicy) -> HttpTraceExtraction`.

- [ ] **Step 1: Implement policy and redacted extraction Debug**

Create `crates/rmcp-traces/src/http.rs` with:

```rust
use std::fmt;

use ::http::{HeaderMap, HeaderName, HeaderValue};
use rmcp::model::Meta;

use crate::{
    parse_traceparent_value, validate_baggage_value, validate_tracestate_value, TraceLimits,
    TraceParseError, TraceSummary, TraceTrust, BAGGAGE_KEY, TRACEPARENT_KEY, TRACESTATE_KEY,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HttpTracePolicy {
    pub trust: TraceTrust,
    pub limits: TraceLimits,
    pub include_baggage: bool,
}

impl Default for HttpTracePolicy {
    fn default() -> Self {
        Self {
            trust: TraceTrust::Untrusted,
            limits: TraceLimits::default(),
            include_baggage: false,
        }
    }
}

pub struct HttpTraceExtraction {
    pub meta: Meta,
    pub summary: TraceSummary,
}

impl fmt::Debug for HttpTraceExtraction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HttpTraceExtraction")
            .field("meta_keys", &self.meta.keys().collect::<Vec<_>>())
            .field("summary", &self.summary)
            .finish()
    }
}
```

- [ ] **Step 2: Implement bounded header reading**

Add:

```rust
const TRACEPARENT_HEADER: HeaderName = HeaderName::from_static(TRACEPARENT_KEY);
const TRACESTATE_HEADER: HeaderName = HeaderName::from_static(TRACESTATE_KEY);
const BAGGAGE_HEADER: HeaderName = HeaderName::from_static(BAGGAGE_KEY);

fn single_header_value<'a>(
    headers: &'a HeaderMap,
    name: &'static HeaderName,
    field: &'static str,
) -> Result<Option<&'a str>, TraceParseError> {
    let mut values = headers.get_all(name).iter();
    let Some(value) = values.next() else {
        return Ok(None);
    };
    if values.next().is_some() {
        return Err(TraceParseError::InvalidTraceParentFormat);
    }
    header_to_str(value, field).map(Some)
}

fn bounded_join_headers(
    headers: &HeaderMap,
    name: &'static HeaderName,
    field: &'static str,
    max: usize,
) -> Result<Option<String>, TraceParseError> {
    let mut out = String::new();
    let mut saw_value = false;
    for value in headers.get_all(name).iter() {
        let value = header_to_str(value, field)?;
        let separator = usize::from(saw_value);
        let needed = out.len() + separator + value.len();
        if needed > max {
            return Err(TraceParseError::ValueTooLong {
                field,
                actual: max + 1,
                max,
            });
        }
        if saw_value {
            out.push(',');
        }
        out.push_str(value);
        saw_value = true;
    }
    Ok(saw_value.then_some(out))
}

fn header_to_str<'a>(
    value: &'a HeaderValue,
    field: &'static str,
) -> Result<&'a str, TraceParseError> {
    value
        .to_str()
        .map_err(|_| TraceParseError::NonStringMeta { field })
}
```

- [ ] **Step 3: Implement extraction**

Add:

```rust
pub fn extract_http_trace(headers: &HeaderMap, policy: HttpTracePolicy) -> HttpTraceExtraction {
    let mut meta = Meta::new();
    let traceparent = match single_header_value(headers, &TRACEPARENT_HEADER, TRACEPARENT_KEY) {
        Ok(Some(value)) => value,
        Ok(None) => {
            return HttpTraceExtraction {
                meta,
                summary: TraceSummary::absent_with_trust(policy.trust),
            };
        }
        Err(error) => {
            let mut summary = TraceSummary::absent_with_trust(policy.trust);
            summary.record_invalid_reason(&error);
            return HttpTraceExtraction { meta, summary };
        }
    };

    let traceparent = match parse_traceparent_value(traceparent, policy.limits) {
        Ok(traceparent) => traceparent,
        Err(error) => {
            let mut summary = TraceSummary::absent_with_trust(policy.trust);
            summary.record_invalid_reason(&error);
            return HttpTraceExtraction { meta, summary };
        }
    };

    meta.set_traceparent(headers.get(TRACEPARENT_KEY).unwrap().to_str().unwrap());
    let mut summary = TraceSummary::from_valid_traceparent(&traceparent, policy.trust);

    match bounded_join_headers(headers, &TRACESTATE_HEADER, TRACESTATE_KEY, policy.limits.max_tracestate_len) {
        Ok(Some(tracestate)) => match validate_tracestate_value(Some(&tracestate)) {
            Ok(()) => {
                meta.set_tracestate(tracestate);
                summary.set_has_tracestate_for_http();
            }
            Err(error) => summary.record_invalid_reason(&error),
        },
        Ok(None) => {}
        Err(error) => summary.record_invalid_reason(&error),
    }

    if policy.include_baggage {
        match bounded_join_headers(headers, &BAGGAGE_HEADER, BAGGAGE_KEY, policy.limits.max_baggage_len) {
            Ok(Some(baggage)) => match validate_baggage_value(Some(&baggage), policy.limits.max_baggage_members) {
                Ok(()) => {
                    meta.set_baggage(baggage.clone());
                    summary.set_baggage_counts_for_http(Some(&baggage));
                }
                Err(error) => summary.record_invalid_reason(&error),
            },
            Ok(None) => {}
            Err(error) => summary.record_invalid_reason(&error),
        }
    }

    HttpTraceExtraction { meta, summary }
}
```

Add crate-private summary setters in `trace_context.rs` instead of making fields public:

```rust
pub(crate) fn set_has_tracestate_for_http(&mut self) {
    self.has_tracestate = true;
}

pub(crate) fn set_baggage_counts_for_http(&mut self, baggage: Option<&str>) {
    let (total, sensitive) = summarize_baggage(baggage);
    self.baggage_member_count = total;
    self.sensitive_baggage_member_count = sensitive;
}
```

- [ ] **Step 4: Run the first HTTP test**

Run: `cargo test -p rmcp-traces --features http extracts_valid_trace_headers_and_strips_baggage_by_default -- --exact`

Expected: pass.

### Task 4: HTTP Edge Cases And Privacy Tests

**Files:**
- Modify: `crates/rmcp-traces/tests/http_propagation.rs`

**Interfaces:**
- Consumes: `extract_http_trace`.

- [ ] **Step 1: Add coverage for missing, invalid, duplicate, split, baggage, and Debug privacy**

Update the imports in `crates/rmcp-traces/tests/http_propagation.rs`:

```rust
use http::{HeaderMap, HeaderValue};
use rmcp_traces::http::{extract_http_trace, HttpTracePolicy};
use rmcp_traces::{TraceLimits, TraceTrust, BAGGAGE_KEY, TRACEPARENT_KEY, TRACESTATE_KEY};
```

Append:

```rust
#[test]
fn missing_traceparent_returns_empty_meta_with_configured_trust() {
    let mut headers = HeaderMap::new();
    headers.insert(TRACESTATE_KEY, "vendor=value".parse().unwrap());
    headers.insert(BAGGAGE_KEY, "sessionId=s123".parse().unwrap());
    let policy = HttpTracePolicy {
        trust: TraceTrust::Trusted,
        ..HttpTracePolicy::default()
    };

    let extraction = extract_http_trace(&headers, policy);

    assert!(extraction.meta.is_empty());
    assert_eq!(extraction.summary.trust(), TraceTrust::Trusted);
    assert_eq!(extraction.summary.trace_id_prefix(), None);
    assert!(!extraction.summary.has_tracestate());
    assert_eq!(extraction.summary.baggage_member_count(), 0);
    assert_eq!(extraction.summary.invalid_count(), 0);
}

#[test]
fn invalid_traceparent_suppresses_optional_metadata_counts() {
    let mut headers = HeaderMap::new();
    headers.insert(TRACEPARENT_KEY, "00-invalid".parse().unwrap());
    headers.insert(TRACESTATE_KEY, "vendor=value".parse().unwrap());
    headers.insert(BAGGAGE_KEY, "sessionId=s123".parse().unwrap());

    let extraction = extract_http_trace(&headers, HttpTracePolicy::default());

    assert!(extraction.meta.is_empty());
    assert_eq!(extraction.summary.trace_id_prefix(), None);
    assert!(!extraction.summary.has_tracestate());
    assert_eq!(extraction.summary.baggage_member_count(), 0);
    assert_eq!(extraction.summary.invalid_count(), 1);
}

#[test]
fn duplicate_traceparent_is_rejected_without_optional_metadata() {
    let mut headers = HeaderMap::new();
    headers.append(TRACEPARENT_KEY, VALID_TRACEPARENT.parse().unwrap());
    headers.append(
        TRACEPARENT_KEY,
        "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"
            .parse()
            .unwrap(),
    );
    headers.insert(BAGGAGE_KEY, "sessionId=s123".parse().unwrap());

    let extraction = extract_http_trace(&headers, HttpTracePolicy::default());

    assert!(extraction.meta.is_empty());
    assert_eq!(extraction.summary.trace_id_prefix(), None);
    assert_eq!(extraction.summary.baggage_member_count(), 0);
    assert_eq!(extraction.summary.invalid_count(), 1);
}

#[test]
fn split_optional_headers_join_within_limits() {
    let mut headers = HeaderMap::new();
    headers.insert(TRACEPARENT_KEY, VALID_TRACEPARENT.parse().unwrap());
    headers.append(TRACESTATE_KEY, "vendor=value".parse().unwrap());
    headers.append(TRACESTATE_KEY, "other=two".parse().unwrap());
    headers.append(BAGGAGE_KEY, "sessionId=s123".parse().unwrap());
    headers.append(BAGGAGE_KEY, "region=us-east-1".parse().unwrap());
    let policy = HttpTracePolicy {
        include_baggage: true,
        ..HttpTracePolicy::default()
    };

    let extraction = extract_http_trace(&headers, policy);

    assert_eq!(extraction.meta.get(TRACESTATE_KEY).and_then(|v| v.as_str()), Some("vendor=value,other=two"));
    assert_eq!(extraction.meta.get(BAGGAGE_KEY).and_then(|v| v.as_str()), Some("sessionId=s123,region=us-east-1"));
    assert!(extraction.summary.has_tracestate());
    assert_eq!(extraction.summary.baggage_member_count(), 2);
    assert_eq!(extraction.summary.sensitive_baggage_member_count(), 1);
    assert_eq!(extraction.summary.invalid_count(), 0);
}

#[test]
fn split_optional_headers_fail_before_over_allocation() {
    let mut headers = HeaderMap::new();
    headers.insert(TRACEPARENT_KEY, VALID_TRACEPARENT.parse().unwrap());
    headers.append(TRACESTATE_KEY, "vendor=value".parse().unwrap());
    headers.append(TRACESTATE_KEY, "other=two".parse().unwrap());
    let policy = HttpTracePolicy {
        limits: TraceLimits {
            max_tracestate_len: 12,
            ..TraceLimits::default()
        },
        ..HttpTracePolicy::default()
    };

    let extraction = extract_http_trace(&headers, policy);

    assert_eq!(extraction.summary.trace_id_prefix(), Some("0af76519"));
    assert!(extraction.meta.get(TRACESTATE_KEY).is_none());
    assert_eq!(extraction.summary.invalid_reasons(), &["tracestate exceeded 12 bytes (actual 13)".to_owned()]);
}

#[test]
fn include_baggage_validates_and_counts_sensitive_members() {
    let mut headers = HeaderMap::new();
    headers.insert(TRACEPARENT_KEY, VALID_TRACEPARENT.parse().unwrap());
    headers.insert(
        BAGGAGE_KEY,
        "email=alice@example.com,accessToken=super-secret-token,region=us-east-1"
            .parse()
            .unwrap(),
    );
    let policy = HttpTracePolicy {
        include_baggage: true,
        ..HttpTracePolicy::default()
    };

    let extraction = extract_http_trace(&headers, policy);

    assert!(extraction.meta.get(BAGGAGE_KEY).is_some());
    assert_eq!(extraction.summary.baggage_member_count(), 3);
    assert_eq!(extraction.summary.sensitive_baggage_member_count(), 1);
    assert_eq!(extraction.summary.invalid_count(), 0);
}

#[test]
fn invalid_baggage_is_not_inserted_into_returned_meta() {
    let mut headers = HeaderMap::new();
    headers.insert(TRACEPARENT_KEY, VALID_TRACEPARENT.parse().unwrap());
    headers.insert(BAGGAGE_KEY, "region=us-east-1;".parse().unwrap());
    let policy = HttpTracePolicy {
        include_baggage: true,
        ..HttpTracePolicy::default()
    };

    let extraction = extract_http_trace(&headers, policy);

    assert!(extraction.meta.get(BAGGAGE_KEY).is_none());
    assert_eq!(extraction.summary.baggage_member_count(), 0);
    assert_eq!(
        extraction.summary.invalid_reasons(),
        &["baggage member format was invalid".to_owned()]
    );
}

#[test]
fn extraction_debug_is_redacted() {
    let mut headers = HeaderMap::new();
    headers.insert(TRACEPARENT_KEY, VALID_TRACEPARENT.parse().unwrap());
    headers.insert(TRACESTATE_KEY, "vendor=secret-state".parse().unwrap());
    headers.insert(
        BAGGAGE_KEY,
        "email=alice@example.com,accessToken=super-secret-token,x-api-key=abc123,sessionId=s123"
            .parse()
            .unwrap(),
    );
    let policy = HttpTracePolicy {
        include_baggage: true,
        ..HttpTracePolicy::default()
    };

    let extraction = extract_http_trace(&headers, policy);
    let debug = format!("{extraction:?}");

    assert!(debug.contains("trace_id_prefix"));
    assert!(!debug.contains(VALID_TRACEPARENT));
    assert!(!debug.contains("secret-state"));
    assert!(!debug.contains("alice@example.com"));
    assert!(!debug.contains("super-secret-token"));
    assert!(!debug.contains("abc123"));
    assert!(!debug.contains("s123"));
}

#[test]
fn trace_flags_with_reserved_bits_are_valid() {
    let mut headers = HeaderMap::new();
    headers.insert(
        TRACEPARENT_KEY,
        "00-0af7651916cd43dd8448eb211c80319c-00f067aa0ba902b7-03"
            .parse()
            .unwrap(),
    );

    let extraction = extract_http_trace(&headers, HttpTracePolicy::default());

    assert_eq!(extraction.summary.trace_id_prefix(), Some("0af76519"));
    assert_eq!(extraction.summary.sampled(), Some(true));
    assert_eq!(extraction.summary.invalid_count(), 0);
}

#[test]
fn non_visible_ascii_header_value_is_rejected_safely() {
    let mut headers = HeaderMap::new();
    headers.insert(TRACEPARENT_KEY, HeaderValue::from_bytes(b"00-\xff").unwrap());

    let extraction = extract_http_trace(&headers, HttpTracePolicy::default());

    assert!(extraction.meta.is_empty());
    assert_eq!(extraction.summary.invalid_count(), 1);
    assert!(!format!("{extraction:?}").contains("\\xff"));
}
```

Use the concrete constants from Task 1. For malformed privacy probes, include `email=alice@example.com`, `accessToken=super-secret-token`, `x-api-key=abc123`, and `sessionId=s123`.

- [ ] **Step 2: Run the HTTP test file**

Run: `cargo test -p rmcp-traces --features http --test http_propagation`

Expected: all HTTP propagation tests pass.

### Task 5: Docs And Full Verification

**Files:**
- Modify: `crates/rmcp-traces/src/lib.rs` or `README.md` if the feature needs public docs.

**Interfaces:**
- Produces: verified first slice, ready for Soma config/integration beads.

- [ ] **Step 1: Add a short feature note**

Update crate docs to include:

```rust
//! Optional `http` feature support extracts inbound W3C `traceparent`,
//! `tracestate`, and, when explicitly enabled, `baggage` headers into RMCP
//! request metadata. Baggage is default-off and HTTP extraction never adds
//! outbound propagation or result `_meta`.
```

- [ ] **Step 2: Format**

Run: `cargo fmt --all`

Expected: no output other than normal formatter completion.

- [ ] **Step 3: Run focused tests**

Run:

```bash
cargo test -p rmcp-traces --features http
cargo test -p rmcp-traces --all-features
```

Expected: both pass.

- [ ] **Step 4: Inspect dependencies**

Run: `cargo tree -p rmcp-traces --features http`

Expected: the optional `http` crate appears only with the `http` feature and no HTTP client/runtime dependencies are introduced.

- [ ] **Step 5: Update bead status**

Run:

```bash
bd update rmcp-template-mdei.1 --claim
bd close rmcp-template-mdei.1 "Implemented optional rmcp-traces/http extraction slice with tests"
bd swarm validate rmcp-template-mdei
```

Expected: `.1` is closed and the remaining swarm remains valid with `.2` ready.

## Later Beads Handoff

- `rmcp-template-mdei.2` owns typed config and startup trust validation.
- `rmcp-template-mdei.3` owns Soma MCP consumption after auth and RMCP `_meta` conflict handling.
- `rmcp-template-mdei.4` owns CORS allow-header gating.
- `rmcp-template-mdei.5` owns docs, live smoke, and outbound non-propagation proof.

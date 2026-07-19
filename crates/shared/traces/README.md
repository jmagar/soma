# rmcp-traces

Bounded, log-safe helpers for W3C trace metadata carried through RMCP `_meta`
or trusted inbound HTTP headers.

This crate targets `rmcp 2.2.0`. RMCP owns MCP wire serialization; this crate
adds validation, size limits, trust labels, source-neutral redacted summaries,
and an optional HTTP extraction adapter. Product runtimes remain responsible
for deciding whether an HTTP peer is trusted.

## Features

- Parse and validate request-side `traceparent`.
- Validate bounded `tracestate` and `baggage` values.
- Produce `TraceSummary` values that are safe to emit in logs.
- Treat RMCP `_meta` as untrusted by default.
- Preserve v00 `traceparent` exactly while allowing bounded higher-version
  additive fields and exposing only stable trace/span ID prefixes.
- With the opt-in `http` Cargo feature, extract trusted inbound W3C headers
  into RMCP `Meta` without adding outbound propagation.

The crate's default feature set has no dependency on the `http` crate.

## RMCP `_meta`

Use `TraceSummary::from_meta` when trace metadata arrived through MCP:

```rust
use rmcp::model::Meta;
use rmcp_traces::{TraceSummary, TraceTrust};

let meta = Meta::new();
let summary = TraceSummary::from_meta(&meta, TraceTrust::Untrusted);

assert_eq!(summary.trace_id_prefix(), None);
assert_eq!(summary.invalid_count(), 0);
```

`TraceSummary::from_meta_with_limits` accepts caller-supplied `TraceLimits`.
The defaults are:

| Field | Limit |
|---|---:|
| `traceparent` | 512 bytes |
| `tracestate` | 512 bytes and 32 members |
| `baggage` | 8 KiB and 64 members |

`tracestate` without a valid `traceparent` is reported as invalid. Baggage is
validated and counted independently, but its values are never exposed by
`TraceSummary`.

## Trusted HTTP extraction

Enable the optional adapter:

```toml
[dependencies]
rmcp-traces = { version = "0.4.7", features = ["http"] }
```

Then supply an application-selected policy:

```rust
use http::HeaderMap;
use rmcp_traces::http::{extract_http_trace, HttpTracePolicy};
use rmcp_traces::TraceTrust;

let headers = HeaderMap::new();
let extraction = extract_http_trace(
    &headers,
    HttpTracePolicy {
        trust: TraceTrust::Trusted,
        include_baggage: false,
        ..HttpTracePolicy::default()
    },
);

// Validated fields are returned as RMCP Meta; logs should use only summary.
let meta = extraction.meta;
let summary = extraction.summary;
```

HTTP extraction has these fail-closed semantics:

- `traceparent` is required before optional fields are accepted.
- Multiple `traceparent` header values are rejected.
- Split `tracestate` and `baggage` fields are joined only within configured
  bounds and are then validated.
- Invalid `traceparent` returns empty metadata and suppresses optional fields.
- Invalid optional fields are omitted while a valid `traceparent` remains.
- Baggage extraction is disabled unless `include_baggage` is explicitly true.
- Header values that are not visible ASCII produce a safe reason without
  copying the value into logs.

`HttpTracePolicy::default()` is intentionally conservative: it labels input
untrusted, uses default limits, and excludes baggage. The crate does not infer
trust from authentication, CORS, socket addresses, or proxy headers.

## Safety contract

Never log the returned `Meta` or raw baggage. Baggage may contain PII,
credentials, or session identifiers. `TraceSummary` and
`HttpTraceExtraction` debug formatting expose only:

- eight-character trace and span ID prefixes;
- the sampled bit and caller-supplied trust label;
- tracestate presence;
- total and sensitive-looking baggage member counts; and
- bounded, value-free invalid reasons.

Upstream RMCP debug logs can include raw request values before an application
receives `RequestContext.meta`. Avoid broad `rmcp=debug` logging for untrusted
production traffic.

## Integration responsibilities

An application using the HTTP feature must:

1. authenticate/authorize the request before extraction;
2. establish a real transport trust boundary before labeling headers trusted;
3. define precedence if both RMCP `_meta` and HTTP headers are present;
4. prevent accidental forwarding of inbound trace headers; and
5. log `TraceSummary`, never raw metadata.

Soma's implementation, including its precedence and deployment policy, is
documented in [`docs/TRACE_CONTEXT.md`](../../../docs/TRACE_CONTEXT.md).

## Non-goals

- No result `_meta` helpers in v1.
- No outbound HTTP propagation.
- No OpenTelemetry SDK/exporter or tracing subscriber setup.
- No auth, gateway, Axum, Tower, reqwest, codemode, or product-runtime policy.

## Testing

```bash
cargo test -p rmcp-traces
cargo test -p rmcp-traces --features http
```

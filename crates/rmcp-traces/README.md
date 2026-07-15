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

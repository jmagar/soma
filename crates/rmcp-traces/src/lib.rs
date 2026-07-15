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

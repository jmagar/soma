use rmcp::model::Meta;
use rmcp_traces::{
    TraceContext, TraceLimits, TraceParent, TraceParseError, TraceSummary, TraceTrust,
};
use serde_json::json;

const VALID_TRACEPARENT: &str = "00-0af7651916cd43dd8448eb211c80319c-00f067aa0ba902b7-01";

#[test]
fn trace_context_parses_meta_and_summarizes_safely() {
    let mut meta = Meta::new();
    meta.set_traceparent(VALID_TRACEPARENT);
    meta.set_tracestate("vendor=value");
    meta.set_baggage("region=us-east-1,accessToken=super-secret-token");

    let context = TraceContext::from_meta(&meta, TraceTrust::Untrusted)
        .expect("valid trace metadata")
        .expect("trace context exists");

    let summary = context.summary();

    assert_eq!(context.traceparent().as_str(), VALID_TRACEPARENT);
    assert_eq!(summary.trace_id.as_deref(), Some("0af76519"));
    assert_eq!(summary.span_id.as_deref(), Some("00f067aa"));
    assert_eq!(summary.sampled, Some(true));
    assert!(summary.has_tracestate);
    assert_eq!(summary.baggage_member_count, 2);
    assert_eq!(summary.sensitive_baggage_member_count, 1);
}

#[test]
fn malformed_traceparents_are_rejected() {
    for value in [
        "",
        "00-0af7651916cd43dd8448eb211c80319c-00f067aa0ba902b7",
        "ff-0af7651916cd43dd8448eb211c80319c-00f067aa0ba902b7-01",
        "00-00000000000000000000000000000000-00f067aa0ba902b7-01",
        "00-0af7651916cd43dd8448eb211c80319c-0000000000000000-01",
        "00-0AF7651916CD43DD8448EB211C80319C-00f067aa0ba902b7-01",
        "00-0af7651916cd43dd8448eb211c80319c-00f067aa0ba902b7-zz",
    ] {
        assert!(
            TraceParent::parse(value).is_err(),
            "{value} should be rejected"
        );
    }
}

#[test]
fn higher_version_traceparents_preserve_stable_fields() {
    let traceparent =
        TraceParent::parse("01-0af7651916cd43dd8448eb211c80319c-00f067aa0ba902b7-01-extra")
            .expect("higher versions can carry additive fields");

    assert_eq!(traceparent.trace_id(), "0af7651916cd43dd8448eb211c80319c");
    assert_eq!(traceparent.span_id(), "00f067aa0ba902b7");
    assert!(traceparent.sampled());
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
fn summary_preserves_valid_traceparent_when_optional_metadata_is_invalid() {
    let mut meta = Meta::new();
    meta.set_traceparent(VALID_TRACEPARENT);
    meta.set_baggage("a=1,b=2,c=3");
    let limits = TraceLimits {
        max_baggage_members: 2,
        ..TraceLimits::default()
    };

    let summary = TraceSummary::from_meta_with_limits(&meta, TraceTrust::Untrusted, limits);

    assert_eq!(summary.trace_id.as_deref(), Some("0af76519"));
    assert_eq!(summary.span_id.as_deref(), Some("00f067aa"));
    assert_eq!(summary.sampled, Some(true));
    assert_eq!(
        summary.invalid.as_deref(),
        Some("baggage exceeded 2 members (actual at least 3)")
    );
    assert_eq!(summary.baggage_member_count, 0);
    assert_eq!(summary.sensitive_baggage_member_count, 0);
}

#[test]
fn excessive_baggage_member_count_is_rejected() {
    let mut meta = Meta::new();
    meta.set_traceparent(VALID_TRACEPARENT);
    meta.set_baggage("a=1,b=2,c=3");
    let limits = TraceLimits {
        max_baggage_members: 2,
        ..TraceLimits::default()
    };
    let error = TraceContext::from_meta_with_limits(&meta, TraceTrust::Untrusted, limits)
        .expect_err("baggage member cap should be enforced");

    assert!(matches!(
        error,
        TraceParseError::TooManyBaggageMembers { actual: 3, max: 2 }
    ));
    assert!(!error.safe_reason().contains("a=1"));
}

#[test]
fn absent_or_non_string_trace_metadata_is_fail_soft() {
    let meta = Meta::new();
    assert!(TraceContext::from_meta(&meta, TraceTrust::Untrusted)
        .unwrap()
        .is_none());

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

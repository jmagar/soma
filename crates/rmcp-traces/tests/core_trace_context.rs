use rmcp::model::Meta;
use rmcp_traces::{TraceContext, TraceLimits, TraceParent, TraceTrust};
use serde_json::json;

const VALID_TRACEPARENT: &str = "00-0af7651916cd43dd8448eb211c80319c-00f067aa0ba902b7-01";

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
        assert!(
            TraceParent::parse(value).is_err(),
            "{value} should be rejected"
        );
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

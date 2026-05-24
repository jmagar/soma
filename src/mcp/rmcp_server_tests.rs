use serde_json::json;

use crate::{
    actions::{required_scope_for_action, ValidationError, READ_SCOPE, WRITE_SCOPE},
    token_limit::MAX_RESPONSE_BYTES,
};

use super::{
    check_scope, execution_error_payload, response_page_request, scope_satisfied,
    tool_error_result, tool_result_from_json, unknown_action_payload, unknown_tool_error,
    validation_error_payload, ResponsePageRequest,
};

fn scopes(s: &[&str]) -> Vec<String> {
    s.iter().map(|x| x.to_string()).collect()
}

#[test]
fn read_scope_satisfies_read_requirement() {
    assert!(scope_satisfied(&scopes(&[READ_SCOPE]), READ_SCOPE));
}

#[test]
fn write_scope_satisfies_read_requirement() {
    assert!(
        scope_satisfied(&scopes(&[WRITE_SCOPE]), READ_SCOPE),
        "write scope should satisfy read requirement (write ⊇ read)"
    );
}

#[test]
fn empty_scopes_denied() {
    assert!(!scope_satisfied(&[], READ_SCOPE));
}

#[test]
fn unrelated_scope_denied() {
    assert!(!scope_satisfied(&scopes(&["other:scope"]), READ_SCOPE));
}

#[test]
fn read_scope_does_not_satisfy_write() {
    assert!(
        !scope_satisfied(&scopes(&[READ_SCOPE]), WRITE_SCOPE),
        "read scope must not satisfy write requirement"
    );
}

#[test]
fn greet_requires_read_scope() {
    assert_eq!(required_scope_for_action("greet"), Some(READ_SCOPE));
}

#[test]
fn help_requires_no_scope() {
    assert_eq!(required_scope_for_action("help"), None);
}

#[test]
fn unknown_action_gets_deny_scope() {
    use crate::actions::DENY_SCOPE;
    assert_eq!(
        required_scope_for_action("nonexistent_action"),
        Some(DENY_SCOPE)
    );
}

#[test]
fn tool_result_from_json_returns_scrollable_page_envelope() {
    let store = crate::server::ResponsePageStore::default();
    let result = tool_result_from_json(
        json!({
            "payload": "x".repeat(MAX_RESPONSE_BYTES + 1)
        }),
        &store,
        ResponsePageRequest::default(),
        "example",
        Some("status"),
        None,
    )
    .expect("tool result should serialize");
    let text = result.content[0]
        .raw
        .as_text()
        .expect("tool result should contain text")
        .text
        .as_str();
    let parsed: serde_json::Value =
        serde_json::from_str(text).expect("paged text should remain valid JSON");

    assert_eq!(parsed["kind"], "mcp_response_page");
    assert_eq!(parsed["schema_version"], 1);
    assert_eq!(parsed["code"], "response_page");
    assert_eq!(parsed["truncated"], false);
    assert_eq!(parsed["page"]["offset"], 0);
    assert_eq!(parsed["page"]["has_more"], true);
    assert_eq!(parsed["continuation"]["arguments"]["action"], "status");
    assert!(parsed["continuation"]["arguments"]["_response_cursor"]
        .as_str()
        .unwrap()
        .starts_with("rsp_"));
    assert!(
        parsed["continuation"]["arguments"]["_response_offset"]
            .as_u64()
            .unwrap()
            > 0
    );
    assert!(parsed["serialized_bytes"].as_u64().unwrap() > MAX_RESPONSE_BYTES as u64);
    assert!(!text.contains("[TRUNCATED"));
    assert_eq!(result.structured_content.as_ref(), Some(&parsed));
}

#[test]
fn tool_result_from_json_returns_requested_continuation_page() {
    let store = crate::server::ResponsePageStore::default();
    let first = tool_result_from_json(
        json!({
            "payload": "x".repeat(MAX_RESPONSE_BYTES + 1)
        }),
        &store,
        ResponsePageRequest::default(),
        "example",
        Some("status"),
        None,
    )
    .expect("first page should serialize");
    let first_payload: serde_json::Value =
        serde_json::from_str(first.content[0].raw.as_text().unwrap().text.as_str()).unwrap();
    let next_offset = first_payload["continuation"]["arguments"]["_response_offset"]
        .as_u64()
        .expect("first page should expose next offset") as usize;
    let cursor = first_payload["continuation"]["arguments"]["_response_cursor"]
        .as_str()
        .expect("first page should expose response cursor")
        .to_owned();

    let second = tool_result_from_json(
        json!({ "this": "would be a different re-executed response" }),
        &store,
        ResponsePageRequest {
            cursor: Some(cursor),
            offset: next_offset,
            page_bytes: 1024,
        },
        "example",
        Some("status"),
        None,
    )
    .expect("second page should serialize");
    let second_payload: serde_json::Value =
        serde_json::from_str(second.content[0].raw.as_text().unwrap().text.as_str()).unwrap();

    assert_eq!(second_payload["kind"], "mcp_response_page");
    assert_eq!(second_payload["page"]["offset"], next_offset);
    assert_eq!(second_payload["page"]["page_bytes"], 1024);
    assert_ne!(second_payload["content"], first_payload["content"]);
    assert!(!second_payload["content"]
        .as_str()
        .expect("page content should be text")
        .contains("would be a different re-executed response"));
}

#[test]
fn response_page_cursor_rejects_missing_or_expired_cursor() {
    let store = crate::server::ResponsePageStore::default();
    let error = tool_result_from_json(
        json!({ "payload": "this value should not be executed" }),
        &store,
        ResponsePageRequest {
            cursor: Some("rsp_missing".to_owned()),
            offset: 1,
            page_bytes: 1024,
        },
        "example",
        Some("status"),
        None,
    )
    .expect_err("missing cursor should be rejected");

    let data = error.data.expect("error should include structured data");
    assert_eq!(data["kind"], "mcp_protocol_error");
    assert_eq!(data["code"], "response_cursor_not_found");
    assert_eq!(data["field"], "_response_cursor");
}

#[test]
fn response_page_cursor_handles_out_of_range_offsets() {
    let store = crate::server::ResponsePageStore::default();
    let first = tool_result_from_json(
        json!({
            "payload": "x".repeat(MAX_RESPONSE_BYTES + 1)
        }),
        &store,
        ResponsePageRequest::default(),
        "example",
        Some("status"),
        None,
    )
    .expect("first page should serialize");
    let first_payload: serde_json::Value =
        serde_json::from_str(first.content[0].raw.as_text().unwrap().text.as_str()).unwrap();
    let cursor = first_payload["continuation"]["arguments"]["_response_cursor"]
        .as_str()
        .expect("first page should expose response cursor")
        .to_owned();
    let serialized_bytes = first_payload["serialized_bytes"]
        .as_u64()
        .expect("first page should expose size") as usize;

    let result = tool_result_from_json(
        json!({ "payload": "this value should not be executed" }),
        &store,
        ResponsePageRequest {
            cursor: Some(cursor),
            offset: serialized_bytes + 100,
            page_bytes: 1024,
        },
        "example",
        Some("status"),
        None,
    )
    .expect("out of range continuation should serialize");
    let payload: serde_json::Value =
        serde_json::from_str(result.content[0].raw.as_text().unwrap().text.as_str()).unwrap();

    assert_eq!(payload["kind"], "mcp_response_page");
    assert_eq!(payload["page"]["offset"], serialized_bytes);
    assert_eq!(payload["page"]["has_more"], false);
    assert!(payload["content"].as_str().unwrap().is_empty());
    assert!(payload["continuation"].is_null());
}

#[test]
fn response_page_continuation_preserves_original_arguments() {
    let store = crate::server::ResponsePageStore::default();
    let mut args = serde_json::Map::new();
    args.insert("action".to_owned(), json!("echo"));
    args.insert("message".to_owned(), json!("hello from original args"));

    let result = tool_result_from_json(
        json!({
            "payload": "x".repeat(MAX_RESPONSE_BYTES + 1)
        }),
        &store,
        ResponsePageRequest::default(),
        "example",
        Some("echo"),
        Some(&args),
    )
    .expect("tool result should serialize");
    let parsed: serde_json::Value =
        serde_json::from_str(result.content[0].raw.as_text().unwrap().text.as_str()).unwrap();

    assert_eq!(parsed["continuation"]["arguments"]["action"], "echo");
    assert_eq!(
        parsed["continuation"]["arguments"]["message"],
        "hello from original args"
    );
    assert!(parsed["continuation"]["arguments"]["_response_cursor"].is_string());
    assert!(
        parsed["continuation"]["arguments"]["_response_offset"]
            .as_u64()
            .unwrap()
            > 0
    );
}

#[test]
fn response_page_request_rejects_offset_without_cursor() {
    let args = serde_json::Map::from_iter([
        ("action".to_owned(), json!("status")),
        ("_response_offset".to_owned(), json!(1)),
    ]);

    let error = response_page_request(Some(&args)).expect_err("offset needs cursor");
    let data = error.data.expect("error should include structured data");
    assert_eq!(data["kind"], "mcp_protocol_error");
    assert_eq!(data["code"], "missing_response_cursor");
    assert_eq!(data["field"], "_response_cursor");
}

#[test]
fn response_page_request_rejects_bad_types_and_zero_page_size() {
    let bad_cursor_args = serde_json::Map::from_iter([
        ("action".to_owned(), json!("status")),
        ("_response_cursor".to_owned(), json!(42)),
    ]);
    let bad_cursor = response_page_request(Some(&bad_cursor_args)).unwrap_err();
    assert_eq!(bad_cursor.data.unwrap()["code"], "invalid_response_cursor");

    let bad_offset_args = serde_json::Map::from_iter([
        ("action".to_owned(), json!("status")),
        ("_response_cursor".to_owned(), json!("rsp_test")),
        ("_response_offset".to_owned(), json!("one")),
    ]);
    let bad_offset = response_page_request(Some(&bad_offset_args)).unwrap_err();
    assert_eq!(
        bad_offset.data.unwrap()["code"],
        "invalid_response_page_arg"
    );

    let zero_page_args = serde_json::Map::from_iter([
        ("action".to_owned(), json!("status")),
        ("_response_page_bytes".to_owned(), json!(0)),
    ]);
    let zero_page = response_page_request(Some(&zero_page_args)).unwrap_err();
    assert_eq!(
        zero_page.data.unwrap()["code"],
        "invalid_response_page_bytes"
    );
}

#[test]
fn validation_errors_become_structured_tool_errors() {
    let error = anyhow::Error::from(ValidationError::MissingField {
        field: "message".to_owned(),
    });
    let payload = validation_error_payload("example", Some("echo"), &error);
    let result = tool_error_result(payload).expect("tool error should serialize");

    assert_eq!(result.is_error, Some(true));
    let structured = result
        .structured_content
        .as_ref()
        .expect("structured content should be present");
    assert_eq!(structured["kind"], "mcp_tool_error");
    assert_eq!(structured["schema_version"], 1);
    assert_eq!(structured["code"], "missing_field");
    assert_eq!(structured["tool"], "example");
    assert_eq!(structured["action"], "echo");
    assert_eq!(structured["field"], "message");
    assert!(structured["remediation"]
        .as_str()
        .unwrap_or_default()
        .contains("action=help"));
}

#[test]
fn unknown_actions_become_retryable_tool_errors() {
    let result = tool_error_result(unknown_action_payload("example", "missing"))
        .expect("unknown action payload should serialize");

    assert_eq!(result.is_error, Some(true));
    let structured = result.structured_content.as_ref().unwrap();
    assert_eq!(structured["code"], "unknown_action");
    assert_eq!(structured["bad_value"], "missing");
    assert!(structured["available_actions"]
        .as_array()
        .unwrap()
        .contains(&json!("help")));
}

#[test]
fn oversized_tool_errors_return_valid_overflow_envelope() {
    let result = tool_error_result(json!({
        "kind": "mcp_tool_error",
        "schema_version": 1,
        "code": "huge_error",
        "message": "x".repeat(MAX_RESPONSE_BYTES + 1),
    }))
    .expect("tool error should serialize");
    let text = result.content[0]
        .raw
        .as_text()
        .expect("tool error should contain text")
        .text
        .as_str();
    let parsed: serde_json::Value =
        serde_json::from_str(text).expect("overflow error text should remain valid JSON");

    assert_eq!(result.is_error, Some(true));
    assert_eq!(parsed["kind"], "mcp_tool_error");
    assert_eq!(parsed["code"], "error_payload_too_large");
    assert_eq!(parsed["original_code"], "huge_error");
    assert!(parsed["serialized_bytes"].as_u64().unwrap() > MAX_RESPONSE_BYTES as u64);
    assert_eq!(result.structured_content.as_ref(), Some(&parsed));
}

#[test]
fn unknown_tool_stays_protocol_error_with_structured_data() {
    let error = unknown_tool_error("bad_tool");

    assert!(error.message.contains("unknown tool"));
    let data = error
        .data
        .expect("unknown tool should include structured data");
    assert_eq!(data["kind"], "mcp_protocol_error");
    assert_eq!(data["code"], "unknown_tool");
    assert_eq!(data["tool"], "bad_tool");
    assert_eq!(data["available_tools"], json!(["example"]));
}

#[test]
fn execution_errors_do_not_expose_raw_error_text() {
    let raw_error = anyhow::anyhow!("upstream timeout talking to secret-api-key");
    let payload = execution_error_payload("example", Some("status"), &raw_error);

    assert_eq!(
        payload,
        json!({
            "kind": "mcp_tool_error",
            "schema_version": 1,
            "code": "execution_error",
            "reason_kind": "timeout",
            "tool": "example",
            "action": "status",
            "message": "Tool execution failed. Check server logs for details.",
            "retryable": true,
            "remediation": "Check service configuration and upstream availability, then retry. Use action=status or action=help for diagnostics.",
        })
    );
    assert!(!payload.to_string().contains("secret-api-key"));
}

#[test]
fn insufficient_scope_uses_structured_protocol_error_data() {
    let auth = lab_auth::AuthContext {
        sub: "agent-subject".to_owned(),
        actor_key: None,
        scopes: scopes(&[READ_SCOPE]),
        issuer: "test".to_owned(),
        via_session: false,
        csrf_token: None,
        email: None,
    };

    let error = check_scope(&auth, WRITE_SCOPE, "echo").expect_err("write scope should be denied");
    let data = error
        .data
        .expect("insufficient scope should include structured data");
    assert_eq!(data["kind"], "mcp_auth_error");
    assert_eq!(data["code"], "insufficient_scope");
    assert_eq!(data["action"], "echo");
    assert_eq!(data["required_scope"], WRITE_SCOPE);
    assert_eq!(data["granted_scopes"], json!([READ_SCOPE]));
    assert_eq!(data["retryable"], false);
}

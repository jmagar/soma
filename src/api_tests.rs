use serde_json::json;

use super::ActionRequest;

#[test]
fn action_request_defaults_to_empty_action_and_null_params() {
    let req: ActionRequest = serde_json::from_str("{}").unwrap();
    assert_eq!(req.action, "");
    assert!(req.params.is_null());
}

#[test]
fn action_request_parses_action_and_params() {
    let req: ActionRequest = serde_json::from_value(json!({
        "action": "greet",
        "params": {"name": "Alice"}
    }))
    .unwrap();
    assert_eq!(req.action, "greet");
    assert_eq!(req.params["name"], "Alice");
}

#[test]
fn action_request_params_defaults_to_null_when_omitted() {
    let req: ActionRequest = serde_json::from_value(json!({ "action": "status" })).unwrap();
    assert_eq!(req.action, "status");
    assert!(req.params.is_null());
}

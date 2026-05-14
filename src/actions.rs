use anyhow::{anyhow, Result};
use serde_json::{json, Value};

use crate::app::ExampleService;

pub const READ_SCOPE: &str = "example:read";
pub const WRITE_SCOPE: &str = "example:write";
pub const DENY_SCOPE: &str = "example:__deny__";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionTransport {
    Any,
    McpOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActionSpec {
    pub name: &'static str,
    pub required_scope: Option<&'static str>,
    pub transport: ActionTransport,
}

pub const ACTION_SPECS: &[ActionSpec] = &[
    ActionSpec {
        name: "greet",
        required_scope: Some(READ_SCOPE),
        transport: ActionTransport::Any,
    },
    ActionSpec {
        name: "echo",
        required_scope: Some(READ_SCOPE),
        transport: ActionTransport::Any,
    },
    ActionSpec {
        name: "status",
        required_scope: Some(READ_SCOPE),
        transport: ActionTransport::Any,
    },
    ActionSpec {
        name: "elicit_name",
        required_scope: Some(READ_SCOPE),
        transport: ActionTransport::McpOnly,
    },
    ActionSpec {
        name: "help",
        required_scope: None,
        transport: ActionTransport::Any,
    },
];

pub fn action_names() -> Vec<&'static str> {
    ACTION_SPECS.iter().map(|spec| spec.name).collect()
}

pub fn rest_action_names() -> Vec<&'static str> {
    ACTION_SPECS
        .iter()
        .filter(|spec| spec.transport == ActionTransport::Any)
        .map(|spec| spec.name)
        .collect()
}

pub fn mcp_only_action_names() -> Vec<&'static str> {
    ACTION_SPECS
        .iter()
        .filter(|spec| spec.transport == ActionTransport::McpOnly)
        .map(|spec| spec.name)
        .collect()
}

pub fn required_scope_for_action(action: &str) -> Option<&'static str> {
    ACTION_SPECS
        .iter()
        .find(|spec| spec.name == action)
        .map(|spec| spec.required_scope)
        .unwrap_or(Some(DENY_SCOPE))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExampleAction {
    Greet { name: Option<String> },
    Echo { message: String },
    Status,
    Help,
    ElicitName,
}

impl ExampleAction {
    pub fn from_mcp_args(args: &Value) -> Result<Self> {
        let action = args
            .get("action")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("action is required"))?;
        Self::from_params(action, args)
    }

    pub fn from_rest(action: &str, params: &Value) -> Result<Self> {
        Self::from_params(action, params)
    }

    fn from_params(action: &str, params: &Value) -> Result<Self> {
        match action {
            "greet" => Ok(Self::Greet {
                name: string_param(params, "name")?,
            }),
            "echo" => {
                let message = optional_string_param(params, "message")?
                    .filter(|message| !message.is_empty())
                    .ok_or_else(|| anyhow!("`message` is required for action=echo"))?;
                Ok(Self::Echo { message })
            }
            "status" => Ok(Self::Status),
            "help" => Ok(Self::Help),
            "elicit_name" => Ok(Self::ElicitName),
            other => Err(anyhow!(
                "unknown example action: {other}; use action=help for documentation"
            )),
        }
    }
}

pub async fn execute_service_action(
    service: &ExampleService,
    action: &ExampleAction,
) -> Result<Value> {
    match action {
        ExampleAction::Greet { name } => service.greet(name.as_deref()).await,
        ExampleAction::Echo { message } => service.echo(message).await,
        ExampleAction::Status => service.status().await,
        ExampleAction::Help => Ok(rest_help()),
        ExampleAction::ElicitName => Err(anyhow!(
            "action=elicit_name is only available over MCP because it requires a peer"
        )),
    }
}

pub fn rest_help() -> Value {
    json!({
        "actions": rest_action_names(),
        "mcp_only_actions": mcp_only_action_names(),
        "usage": "POST /v1/example with {\"action\": \"<action>\", \"params\": {...}}",
        "examples": {
            "greet":  {"action": "greet",  "params": {"name": "Alice"}},
            "echo":   {"action": "echo",   "params": {"message": "Hello!"}},
            "status": {"action": "status", "params": {}},
        }
    })
}

fn string_param(params: &Value, name: &str) -> Result<Option<String>> {
    optional_string_param(params, name)
}

fn optional_string_param(params: &Value, name: &str) -> Result<Option<String>> {
    match params.get(name) {
        None => Ok(None),
        Some(value) => value
            .as_str()
            .map(|value| Some(value.to_owned()))
            .ok_or_else(|| anyhow!("`{name}` must be a string")),
    }
}

pub fn is_validation_error(error: &anyhow::Error) -> bool {
    let message = error.to_string();
    message.contains(" is required")
        || message.contains(" must be a string")
        || message.contains("unknown example action")
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn action_metadata_is_the_action_source_of_truth() {
        assert_eq!(
            action_names(),
            vec!["greet", "echo", "status", "elicit_name", "help"]
        );
        assert_eq!(rest_action_names(), vec!["greet", "echo", "status", "help"]);
        assert_eq!(mcp_only_action_names(), vec!["elicit_name"]);
        assert_eq!(required_scope_for_action("help"), None);
        assert_eq!(required_scope_for_action("greet"), Some(READ_SCOPE));
        assert_eq!(required_scope_for_action("unknown"), Some(DENY_SCOPE));
    }

    #[test]
    fn mcp_args_parse_flat_shape() {
        let action = ExampleAction::from_mcp_args(&json!({
            "action": "echo",
            "message": "hello"
        }))
        .expect("flat MCP args should parse");
        assert_eq!(
            action,
            ExampleAction::Echo {
                message: "hello".into()
            }
        );
    }

    #[test]
    fn rest_args_parse_nested_params_shape() {
        let action = ExampleAction::from_rest("greet", &json!({ "name": "Alice" }))
            .expect("REST params should parse");
        assert_eq!(
            action,
            ExampleAction::Greet {
                name: Some("Alice".into())
            }
        );
    }

    #[test]
    fn missing_action_is_validation_error() {
        let error = ExampleAction::from_mcp_args(&json!({})).unwrap_err();
        assert!(error.to_string().contains("action is required"));
    }

    #[test]
    fn echo_rejects_missing_and_empty_message() {
        let missing = ExampleAction::from_mcp_args(&json!({ "action": "echo" })).unwrap_err();
        assert!(missing.to_string().contains("`message` is required"));

        let empty = ExampleAction::from_rest("echo", &json!({ "message": "" })).unwrap_err();
        assert!(empty.to_string().contains("`message` is required"));
    }

    #[test]
    fn string_params_reject_wrong_json_type() {
        let greet = ExampleAction::from_rest("greet", &json!({ "name": 42 })).unwrap_err();
        assert!(greet.to_string().contains("`name` must be a string"));

        let echo = ExampleAction::from_mcp_args(&json!({
            "action": "echo",
            "message": ["not", "a", "string"]
        }))
        .unwrap_err();
        assert!(echo.to_string().contains("`message` must be a string"));
    }

    #[test]
    fn unknown_action_mentions_help() {
        let error = ExampleAction::from_rest("missing", &json!({})).unwrap_err();
        assert!(error.to_string().contains("action=help"));
    }
}

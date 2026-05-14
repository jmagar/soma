use anyhow::{anyhow, Result};
use serde_json::{json, Value};

use crate::app::ExampleService;

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
                name: string_param(params, "name"),
            }),
            "echo" => {
                let message = string_param(params, "message")
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
        "actions": ["greet", "echo", "status", "help"],
        "mcp_only_actions": ["elicit_name"],
        "usage": "POST /v1/example with {\"action\": \"<action>\", \"params\": {...}}",
        "examples": {
            "greet":  {"action": "greet",  "params": {"name": "Alice"}},
            "echo":   {"action": "echo",   "params": {"message": "Hello!"}},
            "status": {"action": "status", "params": {}},
        }
    })
}

fn string_param(params: &Value, name: &str) -> Option<String> {
    params
        .get(name)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

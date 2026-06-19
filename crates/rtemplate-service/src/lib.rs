pub mod app;
pub mod example;

use anyhow::{anyhow, Result};
use rtemplate_contracts::{
    actions::{action_validation_error, rest_help, ExampleAction},
    errors::{ServiceError, ToolError},
};
use serde_json::Value;

pub use app::{ElicitedNameOutcome, ExampleService, ScaffoldIntent, ScaffoldIntentValidationError};
pub use example::ExampleClient;

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
        ExampleAction::ScaffoldIntent => Err(anyhow!(
            "action=scaffold_intent is only available over MCP because it requires elicitation"
        )),
    }
}

pub fn is_validation_error(error: &anyhow::Error) -> bool {
    classify_service_error(error).kind == rtemplate_contracts::errors::ServiceErrorKind::Validation
}

pub fn classify_service_error(error: &anyhow::Error) -> ServiceError {
    if let Some(error) = action_validation_error(error) {
        return ToolError::from_action_validation(error);
    }
    if let Some(error) = error.downcast_ref::<ScaffoldIntentValidationError>() {
        let mut tool_error =
            ToolError::validation(error.code(), error.to_string(), error.remediation());
        if let Some(field) = error.field() {
            tool_error = tool_error.with_field(field);
        }
        if let Some(expected_pattern) = error.expected_pattern() {
            tool_error = tool_error.with_expected_pattern(expected_pattern);
        }
        return tool_error;
    }
    ToolError::execution(error)
}

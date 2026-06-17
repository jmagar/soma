pub mod app;
pub mod example;

use anyhow::{anyhow, Result};
use rtemplate_contracts::actions::{rest_help, ExampleAction};
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
    rtemplate_contracts::actions::is_validation_error(error)
        || error
            .downcast_ref::<ScaffoldIntentValidationError>()
            .is_some()
}

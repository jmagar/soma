//! `example config` — thin CLI shim over the shared `config_store`.
//!
//! All registry, parsing, and filesystem logic lives in `crate::config_store`
//! (called via `ExampleService::config_*`). This file only parses CLI args and
//! prints the JSON result.

use anyhow::Result;
use serde_json::Value;

use crate::app::ExampleService;
use crate::config::ExampleConfig;
use crate::example::ExampleClient;

#[derive(Debug, PartialEq, Eq)]
pub enum ConfigCommand {
    List,
    Get { key: String },
    Set { key: String, value: String },
    Unset { key: String },
    Path,
}

pub fn run_config(cfg: &ExampleConfig, command: ConfigCommand) -> Result<()> {
    let service = ExampleService::new(ExampleClient::new(cfg)?);
    let result: Value = match command {
        ConfigCommand::List => service.config_list()?,
        ConfigCommand::Get { key } => service.config_get(&key)?,
        ConfigCommand::Set { key, value } => service.config_set(&key, &value)?,
        ConfigCommand::Unset { key } => service.config_unset(&key)?,
        ConfigCommand::Path => service.config_paths()?,
    };
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;

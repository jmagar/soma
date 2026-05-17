use anyhow::{anyhow, Result};
use serde_json::{json, Value};

use crate::app::ExampleService;

// ── Validation error type ─────────────────────────────────────────────────────

#[derive(Debug)]
pub enum ValidationError {
    MissingAction,
    MissingField { field: String },
    WrongType { field: String },
    NotAvailableOverRest { action: String },
    NotAvailableOverMcp { action: String },
    UnknownAction { action: String },
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingAction => write!(f, "action is required"),
            Self::MissingField { field } => {
                write!(f, "`{field}` is required and must not be empty")
            }
            Self::WrongType { field } => write!(f, "`{field}` must be a string"),
            Self::NotAvailableOverRest { action } => write!(
                f,
                "action={action} is not available over REST; use MCP or action=help for documentation"
            ),
            Self::NotAvailableOverMcp { action } => write!(
                f,
                "action={action} is not available over MCP; use the CLI or REST API instead"
            ),
            Self::UnknownAction { action } => write!(
                f,
                "unknown example action: {action}; use action=help for documentation"
            ),
        }
    }
}

impl std::error::Error for ValidationError {}

pub const READ_SCOPE: &str = "example:read";
pub const WRITE_SCOPE: &str = "example:write";
pub const DENY_SCOPE: &str = "example:__deny__";

/// Returns true if `token_scopes` satisfy `required`.
/// Write scope satisfies read (write ⊇ read).
/// Single source of truth — called from both REST and MCP enforcement paths.
pub fn scopes_satisfy(token_scopes: &[String], required: &str) -> bool {
    token_scopes
        .iter()
        .any(|s| s == required || (required == READ_SCOPE && s == WRITE_SCOPE))
}

/// Per-transport availability for an action.
///
/// Each transport is gated by a boolean so an action can be exposed to any
/// combination of CLI / REST / MCP. The CLI calls service methods directly,
/// so its flag exists for documentation and `help` output only — there is no
/// runtime check against it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActionSpec {
    pub name: &'static str,
    pub required_scope: Option<&'static str>,
    pub cli_enabled: bool,
    pub rest_enabled: bool,
    pub mcp_enabled: bool,
}

pub const ACTION_SPECS: &[ActionSpec] = &[
    ActionSpec {
        name: "greet",
        required_scope: Some(READ_SCOPE),
        cli_enabled: true,
        rest_enabled: true,
        mcp_enabled: true,
    },
    ActionSpec {
        name: "echo",
        required_scope: Some(READ_SCOPE),
        cli_enabled: true,
        rest_enabled: true,
        mcp_enabled: true,
    },
    ActionSpec {
        name: "status",
        required_scope: Some(READ_SCOPE),
        cli_enabled: true,
        rest_enabled: true,
        mcp_enabled: true,
    },
    ActionSpec {
        name: "elicit_name",
        required_scope: Some(READ_SCOPE),
        cli_enabled: false,
        rest_enabled: false,
        mcp_enabled: true,
    },
    ActionSpec {
        name: "scaffold_intent",
        required_scope: Some(READ_SCOPE),
        cli_enabled: false,
        rest_enabled: false,
        mcp_enabled: true,
    },
    ActionSpec {
        name: "help",
        required_scope: None,
        cli_enabled: true,
        rest_enabled: true,
        mcp_enabled: true,
    },
    // ── config_* actions ─────────────────────────────────────────────────────
    //
    // Disabled in MCP by default: a leaked bearer token with example:write
    // scope would otherwise let an MCP client overwrite secrets / auth flags
    // in `.env` and `config.toml`. CLI operators have local shell access
    // anyway, and REST is intended for human/script administrators authed
    // against the static bearer or OAuth — both higher-trust than an
    // arbitrary MCP client. Flip `mcp_enabled: true` here if your deployment
    // accepts the tradeoff.
    ActionSpec {
        name: "config_list",
        required_scope: Some(READ_SCOPE),
        cli_enabled: true,
        rest_enabled: true,
        mcp_enabled: false,
    },
    ActionSpec {
        name: "config_get",
        required_scope: Some(READ_SCOPE),
        cli_enabled: true,
        rest_enabled: true,
        mcp_enabled: false,
    },
    ActionSpec {
        name: "config_set",
        required_scope: Some(WRITE_SCOPE),
        cli_enabled: true,
        rest_enabled: true,
        mcp_enabled: false,
    },
    ActionSpec {
        name: "config_unset",
        required_scope: Some(WRITE_SCOPE),
        cli_enabled: true,
        rest_enabled: true,
        mcp_enabled: false,
    },
    ActionSpec {
        name: "config_path",
        required_scope: Some(READ_SCOPE),
        cli_enabled: true,
        rest_enabled: true,
        mcp_enabled: false,
    },
];

pub fn action_names() -> Vec<&'static str> {
    ACTION_SPECS.iter().map(|spec| spec.name).collect()
}

pub fn rest_action_names() -> Vec<&'static str> {
    ACTION_SPECS
        .iter()
        .filter(|spec| spec.rest_enabled)
        .map(|spec| spec.name)
        .collect()
}

pub fn mcp_action_names() -> Vec<&'static str> {
    ACTION_SPECS
        .iter()
        .filter(|spec| spec.mcp_enabled)
        .map(|spec| spec.name)
        .collect()
}

pub fn is_rest_action(action: &str) -> bool {
    ACTION_SPECS
        .iter()
        .any(|spec| spec.name == action && spec.rest_enabled)
}

pub fn is_mcp_action(action: &str) -> bool {
    ACTION_SPECS
        .iter()
        .any(|spec| spec.name == action && spec.mcp_enabled)
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
    ScaffoldIntent,
    ConfigList,
    ConfigGet { key: String },
    ConfigSet { key: String, value: String },
    ConfigUnset { key: String },
    ConfigPath,
}

impl ExampleAction {
    pub fn from_mcp_args(args: &Value) -> Result<Self> {
        let action = args
            .get("action")
            .and_then(Value::as_str)
            .ok_or(ValidationError::MissingAction)?;
        if !is_mcp_action(action) && action_exists(action) {
            return Err(ValidationError::NotAvailableOverMcp {
                action: action.to_owned(),
            }
            .into());
        }
        Self::from_params(action, args)
    }

    pub fn from_rest(action: &str, params: &Value) -> Result<Self> {
        if !is_rest_action(action) {
            return Err(ValidationError::NotAvailableOverRest {
                action: action.to_owned(),
            }
            .into());
        }
        Self::from_params(action, params)
    }

    fn from_params(action: &str, params: &Value) -> Result<Self> {
        match action {
            "greet" => Ok(Self::Greet {
                name: optional_string_param(params, "name")?,
            }),
            "echo" => {
                let message = optional_string_param(params, "message")?
                    .filter(|m| !m.is_empty())
                    .ok_or_else(|| ValidationError::MissingField {
                        field: "message".into(),
                    })?;
                Ok(Self::Echo { message })
            }
            "status" => Ok(Self::Status),
            "help" => Ok(Self::Help),
            "elicit_name" => Ok(Self::ElicitName),
            "scaffold_intent" => Ok(Self::ScaffoldIntent),
            "config_list" => Ok(Self::ConfigList),
            "config_path" => Ok(Self::ConfigPath),
            "config_get" => Ok(Self::ConfigGet {
                key: required_string_param(params, "key")?,
            }),
            "config_set" => Ok(Self::ConfigSet {
                key: required_string_param(params, "key")?,
                value: required_string_param(params, "value")?,
            }),
            "config_unset" => Ok(Self::ConfigUnset {
                key: required_string_param(params, "key")?,
            }),
            other => Err(ValidationError::UnknownAction {
                action: other.to_owned(),
            }
            .into()),
        }
    }
}

fn action_exists(action: &str) -> bool {
    ACTION_SPECS.iter().any(|spec| spec.name == action)
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
        ExampleAction::ScaffoldIntent => Err(anyhow!(
            "action=scaffold_intent is only available over MCP because it requires elicitation"
        )),
        ExampleAction::ConfigList => service.config_list(),
        ExampleAction::ConfigGet { key } => service.config_get(key),
        ExampleAction::ConfigSet { key, value } => service.config_set(key, value),
        ExampleAction::ConfigUnset { key } => service.config_unset(key),
        ExampleAction::ConfigPath => service.config_paths(),
    }
}

pub fn rest_help() -> Value {
    json!({
        "actions": rest_action_names(),
        "mcp_actions": mcp_action_names(),
        "usage": "POST /v1/example with {\"action\": \"<action>\", \"params\": {...}}",
        "examples": {
            "greet":  {"action": "greet",  "params": {"name": "Alice"}},
            "echo":   {"action": "echo",   "params": {"message": "Hello!"}},
            "status": {"action": "status", "params": {}},
            "config_set":  {"action": "config_set",  "params": {"key": "mcp.host", "value": "0.0.0.0"}},
            "config_get":  {"action": "config_get",  "params": {"key": "mcp.host"}},
        }
    })
}

fn optional_string_param(params: &Value, name: &str) -> Result<Option<String>> {
    match params.get(name) {
        None => Ok(None),
        Some(value) => value
            .as_str()
            .map(|s| Some(s.to_owned()))
            .ok_or_else(|| ValidationError::WrongType { field: name.into() }.into()),
    }
}

fn required_string_param(params: &Value, name: &str) -> Result<String> {
    optional_string_param(params, name)?
        .filter(|v| !v.is_empty())
        .ok_or_else(|| ValidationError::MissingField { field: name.into() }.into())
}

pub fn is_validation_error(error: &anyhow::Error) -> bool {
    error.downcast_ref::<ValidationError>().is_some()
}

#[cfg(test)]
#[path = "actions_tests.rs"]
mod tests;

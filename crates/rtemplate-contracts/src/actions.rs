use serde::Serialize;
use serde_json::{json, Value};

// ── Action error types ────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum ActionError {
    #[error(transparent)]
    Validation(#[from] ActionValidationError),
}

impl ActionError {
    pub fn as_validation(&self) -> Option<&ActionValidationError> {
        match self {
            Self::Validation(error) => Some(error),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ActionValidationError {
    #[error("action is required")]
    MissingAction,
    #[error("`{field}` is required and must not be empty")]
    MissingField { field: String },
    #[error("`{field}` must be a string")]
    WrongType { field: String },
    #[error(
        "action={action} is not available over REST; use MCP or action=help for documentation"
    )]
    NotAvailableOverRest { action: String },
    #[error("unknown example action: {action}; use action=help for documentation")]
    UnknownAction { action: String },
}

pub type ValidationError = ActionValidationError;

impl ActionValidationError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::MissingAction => "missing_action",
            Self::MissingField { .. } => "missing_field",
            Self::WrongType { .. } => "wrong_type",
            Self::NotAvailableOverRest { .. } => "not_available_over_rest",
            Self::UnknownAction { .. } => "unknown_action",
        }
    }

    pub fn field(&self) -> Option<&str> {
        match self {
            Self::MissingAction => Some("action"),
            Self::MissingField { field } | Self::WrongType { field } => Some(field.as_str()),
            Self::NotAvailableOverRest { .. } | Self::UnknownAction { .. } => Some("action"),
        }
    }

    pub fn bad_value(&self) -> Option<&str> {
        match self {
            Self::NotAvailableOverRest { action } | Self::UnknownAction { action } => {
                Some(action.as_str())
            }
            Self::MissingAction | Self::MissingField { .. } | Self::WrongType { .. } => None,
        }
    }

    pub fn remediation(&self) -> String {
        match self {
            Self::MissingAction => {
                format!(
                    "Set `action` to one of: {}. Use action=help for the full schema.",
                    action_names().join(", ")
                )
            }
            Self::MissingField { field } => {
                format!("Provide a non-empty `{field}` value, or use action=help for examples.")
            }
            Self::WrongType { field } => {
                format!("Pass `{field}` as a JSON string, or use action=help for examples.")
            }
            Self::NotAvailableOverRest { action } => {
                format!("Call action={action} through MCP, or call action=help over REST.")
            }
            Self::UnknownAction { .. } => {
                format!(
                    "Retry with one of the supported actions: {}. Use action=help for examples.",
                    action_names().join(", ")
                )
            }
        }
    }
}

pub const READ_SCOPE: &str = "example:read";
pub const WRITE_SCOPE: &str = "example:write";
pub const DENY_SCOPE: &str = "example:__deny__";

/// Returns true if `token_scopes` satisfy `required`.
/// Write scope satisfies read (write includes read).
/// Single source of truth - called from both REST and MCP enforcement paths.
pub fn scopes_satisfy(token_scopes: &[String], required: &str) -> bool {
    token_scopes
        .iter()
        .any(|s| s == required || (required == READ_SCOPE && s == WRITE_SCOPE))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionTransport {
    Any,
    McpOnly,
}

impl ActionTransport {
    pub fn mcp(self) -> bool {
        matches!(self, Self::Any | Self::McpOnly)
    }

    pub fn cli(self) -> bool {
        matches!(self, Self::Any)
    }

    pub fn rest(self) -> bool {
        matches!(self, Self::Any)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionCost {
    Cheap,
    Moderate,
    Expensive,
    Write,
}

impl ActionCost {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Cheap => "cheap",
            Self::Moderate => "moderate",
            Self::Expensive => "expensive",
            Self::Write => "write",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParamSpec {
    pub name: &'static str,
    pub ty: &'static str,
    pub required: bool,
    pub description: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActionSpec {
    pub name: &'static str,
    pub description: &'static str,
    pub required_scope: Option<&'static str>,
    pub transport: ActionTransport,
    pub destructive: bool,
    pub requires_admin: bool,
    pub cost: ActionCost,
    pub params: &'static [ParamSpec],
    pub returns: &'static str,
}

const GREET_PARAMS: &[ParamSpec] = &[ParamSpec {
    name: "name",
    ty: "string",
    required: false,
    description: "Name to greet. Omit to greet the world.",
}];

const ECHO_PARAMS: &[ParamSpec] = &[ParamSpec {
    name: "message",
    ty: "string",
    required: true,
    description: "Message to echo back. Must not be empty.",
}];

pub const ACTION_SPECS: &[ActionSpec] = &[
    ActionSpec {
        name: "greet",
        description: "Return a greeting.",
        required_scope: Some(READ_SCOPE),
        transport: ActionTransport::Any,
        destructive: false,
        requires_admin: false,
        cost: ActionCost::Cheap,
        params: GREET_PARAMS,
        returns: "Greeting",
    },
    ActionSpec {
        name: "echo",
        description: "Echo a message back unchanged.",
        required_scope: Some(READ_SCOPE),
        transport: ActionTransport::Any,
        destructive: false,
        requires_admin: false,
        cost: ActionCost::Cheap,
        params: ECHO_PARAMS,
        returns: "EchoResult",
    },
    ActionSpec {
        name: "status",
        description: "Return server status and configuration info.",
        required_scope: Some(READ_SCOPE),
        transport: ActionTransport::Any,
        destructive: false,
        requires_admin: false,
        cost: ActionCost::Cheap,
        params: &[],
        returns: "Status",
    },
    ActionSpec {
        name: "elicit_name",
        description: "Ask the MCP client to collect a name, then return a personalised greeting.",
        required_scope: Some(READ_SCOPE),
        transport: ActionTransport::McpOnly,
        destructive: false,
        requires_admin: false,
        cost: ActionCost::Cheap,
        params: &[],
        returns: "Greeting",
    },
    ActionSpec {
        name: "scaffold_intent",
        description: "Collect scaffold setup intent through MCP elicitation and return JSON for the scaffold-project skill.",
        required_scope: Some(READ_SCOPE),
        transport: ActionTransport::McpOnly,
        destructive: false,
        requires_admin: false,
        cost: ActionCost::Moderate,
        params: &[],
        returns: "ScaffoldIntentReport",
    },
    ActionSpec {
        name: "help",
        description: "Show the action reference.",
        required_scope: None,
        transport: ActionTransport::Any,
        destructive: false,
        requires_admin: false,
        cost: ActionCost::Cheap,
        params: &[],
        returns: "HelpPayload",
    },
];

pub fn action_names() -> Vec<&'static str> {
    ACTION_SPECS.iter().map(|spec| spec.name).collect()
}

pub fn is_known_action(action: &str) -> bool {
    ACTION_SPECS.iter().any(|spec| spec.name == action)
}

pub fn rest_action_names() -> Vec<&'static str> {
    ACTION_SPECS
        .iter()
        .filter(|spec| spec.transport.rest())
        .map(|spec| spec.name)
        .collect()
}

pub fn is_rest_action(action: &str) -> bool {
    action_spec(action)
        .map(|spec| spec.transport.rest())
        .unwrap_or(false)
}

pub fn mcp_only_action_names() -> Vec<&'static str> {
    ACTION_SPECS
        .iter()
        .filter(|spec| spec.transport == ActionTransport::McpOnly)
        .map(|spec| spec.name)
        .collect()
}

pub fn required_scope_for_action(action: &str) -> Option<&'static str> {
    action_spec(action)
        .map(|spec| spec.required_scope)
        .unwrap_or(Some(DENY_SCOPE))
}

pub fn action_spec(action: &str) -> Option<&'static ActionSpec> {
    ACTION_SPECS.iter().find(|spec| spec.name == action)
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SurfaceAvailability {
    pub mcp: bool,
    pub cli: bool,
    pub rest: bool,
    pub web_ui: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ParamDoc {
    pub name: String,
    pub ty: String,
    pub required: bool,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ActionDoc {
    pub service: String,
    pub action: String,
    pub description: String,
    pub destructive: bool,
    pub requires_admin: bool,
    pub cost: String,
    pub required_scope: Option<String>,
    pub params: Vec<ParamDoc>,
    pub returns: String,
    pub surface_availability: SurfaceAvailability,
    pub auth_posture: String,
    pub mcp_only_exception: Option<String>,
}

pub fn action_catalog() -> Vec<ActionDoc> {
    ACTION_SPECS
        .iter()
        .map(|spec| ActionDoc {
            service: "example".to_owned(),
            action: spec.name.to_owned(),
            description: spec.description.to_owned(),
            destructive: spec.destructive,
            requires_admin: spec.requires_admin,
            cost: spec.cost.as_str().to_owned(),
            required_scope: spec.required_scope.map(ToOwned::to_owned),
            params: spec
                .params
                .iter()
                .map(|param| ParamDoc {
                    name: param.name.to_owned(),
                    ty: param.ty.to_owned(),
                    required: param.required,
                    description: param.description.to_owned(),
                })
                .collect(),
            returns: spec.returns.to_owned(),
            surface_availability: SurfaceAvailability {
                mcp: spec.transport.mcp(),
                cli: spec.transport.cli(),
                rest: spec.transport.rest(),
                web_ui: false,
            },
            auth_posture: match spec.required_scope {
                Some(scope) => format!("requires `{scope}` on authenticated transports"),
                None => "public action; no action scope required".to_owned(),
            },
            mcp_only_exception: (spec.transport == ActionTransport::McpOnly)
                .then(|| "MCP-only because it requires client-rendered elicitation.".to_owned()),
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExampleAction {
    Greet { name: Option<String> },
    Echo { message: String },
    Status,
    Help,
    ElicitName,
    ScaffoldIntent,
}

impl ExampleAction {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Greet { .. } => "greet",
            Self::Echo { .. } => "echo",
            Self::Status => "status",
            Self::Help => "help",
            Self::ElicitName => "elicit_name",
            Self::ScaffoldIntent => "scaffold_intent",
        }
    }

    pub fn from_mcp_args(args: &Value) -> anyhow::Result<Self> {
        let action = match args.get("action") {
            None => return Err(action_error(ValidationError::MissingAction)),
            Some(Value::String(action)) => action.as_str(),
            Some(_) => {
                return Err(action_error(ValidationError::WrongType {
                    field: "action".into(),
                }));
            }
        };
        Self::from_params(action, args)
    }

    pub fn from_rest(action: &str, params: &Value) -> anyhow::Result<Self> {
        if action.is_empty() {
            return Err(action_error(ValidationError::MissingAction));
        }
        if action_spec(action)
            .map(|spec| spec.transport == ActionTransport::McpOnly)
            .unwrap_or(false)
        {
            return Err(action_error(ValidationError::NotAvailableOverRest {
                action: action.to_owned(),
            }));
        }
        Self::from_params(action, params)
    }

    fn from_params(action: &str, params: &Value) -> anyhow::Result<Self> {
        match action {
            "greet" => Ok(Self::Greet {
                name: optional_string_param(params, "name")?,
            }),
            "echo" => {
                let message = optional_string_param(params, "message")?
                    .filter(|m| !m.is_empty())
                    .ok_or_else(|| {
                        action_error(ValidationError::MissingField {
                            field: "message".into(),
                        })
                    })?;
                Ok(Self::Echo { message })
            }
            "status" => Ok(Self::Status),
            "help" => Ok(Self::Help),
            "elicit_name" => Ok(Self::ElicitName),
            "scaffold_intent" => Ok(Self::ScaffoldIntent),
            other => Err(action_error(ValidationError::UnknownAction {
                action: other.to_owned(),
            })),
        }
    }
}

pub fn rest_help() -> Value {
    json!({
        "actions": rest_action_names(),
        "mcp_only_actions": mcp_only_action_names(),
        "catalog": action_catalog(),
        "preferred_rest_style": "direct_routes",
        "usage": "Use direct REST routes such as POST /v1/echo or GET /v1/status. POST /v1/example remains as a deprecated compatibility action envelope.",
        "examples": {
            "greet":  {"method": "POST", "path": "/v1/greet",  "body": {"name": "Alice"}},
            "echo":   {"method": "POST", "path": "/v1/echo",   "body": {"message": "Hello!"}},
            "status": {"method": "GET", "path": "/v1/status"},
            "legacy_envelope": {"method": "POST", "path": "/v1/example", "body": {"action": "echo", "params": {"message": "Hello!"}}},
        }
    })
}

fn optional_string_param(params: &Value, name: &str) -> Result<Option<String>, ValidationError> {
    match params.get(name) {
        None => Ok(None),
        Some(value) => value
            .as_str()
            .map(|s| Some(s.to_owned()))
            .ok_or_else(|| ValidationError::WrongType { field: name.into() }),
    }
}

fn action_error(error: ValidationError) -> anyhow::Error {
    error.into()
}

pub fn action_validation_error(error: &anyhow::Error) -> Option<&ActionValidationError> {
    if let Some(error) = error.downcast_ref::<ActionError>() {
        return error.as_validation();
    }
    error.downcast_ref::<ActionValidationError>()
}

pub fn is_validation_error(error: &anyhow::Error) -> bool {
    action_validation_error(error).is_some()
}

#[cfg(test)]
#[path = "actions_tests.rs"]
mod tests;

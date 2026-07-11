use async_trait::async_trait;
use serde_json::{json, Map, Value};
use soma_contracts::{
    actions::{ActionTransport, SomaAction, ACTION_SPECS},
    providers::{
        CliOverlay, DocsOverlay, McpOverlay, PaletteOverlay, ProviderCatalog, ProviderIdentity,
        ProviderKind, ProviderManifest, ProviderTool, RestOverlay,
    },
};

use crate::{
    dispatch_action,
    provider_errors::ProviderError,
    provider_registry::{Provider, ProviderCall, ProviderOutput},
    SomaService,
};

#[derive(Clone)]
pub struct StaticRustProvider {
    service: SomaService,
    catalog: ProviderCatalog,
}

impl StaticRustProvider {
    pub fn new(service: SomaService) -> Self {
        Self {
            service,
            catalog: static_catalog(),
        }
    }

    pub fn catalog_static() -> ProviderCatalog {
        static_catalog()
    }
}

#[async_trait]
impl Provider for StaticRustProvider {
    fn catalog(&self) -> ProviderCatalog {
        self.catalog.clone()
    }

    async fn call(&self, call: ProviderCall) -> Result<ProviderOutput, ProviderError> {
        let action = SomaAction::from_rest(&call.action, &call.params)
            .or_else(|_| SomaAction::from_mcp_args(&action_params(&call.action, &call.params)))
            .map_err(|error| {
                ProviderError::validation(
                    "static-rust",
                    call.action.clone(),
                    "invalid_static_action_input",
                    error.to_string(),
                )
            })?;
        let value = match action {
            SomaAction::Help => crate::execute_service_action(&self.service, &action)
                .await
                .map_err(|error| ProviderError::execution("static-rust", call.action, error))?,
            SomaAction::ElicitName | SomaAction::ScaffoldIntent => {
                return Err(ProviderError::validation(
                    "static-rust",
                    call.action,
                    "mcp_peer_required",
                    "elicitation actions require a live MCP peer",
                ));
            }
            other => dispatch_action(&self.service, &other, surface_label(call.surface))
                .await
                .map_err(|error| ProviderError::execution("static-rust", call.action, error))?,
        };
        Ok(ProviderOutput::json(value))
    }
}

fn static_catalog() -> ProviderCatalog {
    ProviderManifest {
        schema_version: 1,
        provider: ProviderIdentity {
            name: "static-rust".to_owned(),
            kind: ProviderKind::StaticRust,
            title: Some("Built-in Rust actions".to_owned()),
            description: Some("Native service actions compiled into Soma.".to_owned()),
            homepage: None,
            source: None,
            version: None,
            enabled: Some(true),
        },
        tools: ACTION_SPECS.iter().map(static_tool).collect(),
        prompts: Vec::new(),
        resources: Vec::new(),
        tasks: Vec::new(),
        elicitation: Vec::new(),
        env: Vec::new(),
        capabilities: Default::default(),
        docs: Some(DocsOverlay {
            when_to_use: Some(
                "Use for Soma built-in Rust actions, scaffold intent collection, MCP elicitation flows, and CLI/REST action reference."
                    .to_owned(),
            ),
            examples: Vec::new(),
            troubleshooting: Vec::new(),
        }),
        plugin: None,
        ui: None,
        meta: json!({}),
    }
}

fn static_tool(spec: &soma_contracts::actions::ActionSpec) -> ProviderTool {
    ProviderTool {
        name: spec.name.to_owned(),
        description: spec.description.to_owned(),
        title: None,
        input_schema: action_input_schema(spec),
        output_schema: None,
        scope: spec.required_scope.map(ToOwned::to_owned),
        destructive: spec.destructive,
        requires_admin: spec.requires_admin,
        cost: Some(spec.cost.as_str().to_owned()),
        env: Vec::new(),
        limits: None,
        mcp: Some(McpOverlay {
            enabled: spec.transport.mcp(),
            title: None,
            annotations: json!({}),
        }),
        rest: spec.rest_path.map(|path| RestOverlay {
            enabled: spec.transport.rest(),
            method: spec.rest_method.map(ToOwned::to_owned),
            path: Some(path.to_owned()),
            tags: vec!["soma".to_owned()],
            summary: Some(spec.description.to_owned()),
            description: Some(spec.description.to_owned()),
            deprecated: false,
            path_params: json!({}),
            query_params: json!({}),
            request_body_schema: None,
        }),
        cli: spec.cli.map(|cli| CliOverlay {
            enabled: spec.transport.cli(),
            command: Some(cli.command.to_owned()),
            aliases: Vec::new(),
            about: Some(cli.description.to_owned()),
            long_about: Some(cli.usage.to_owned()),
            hidden: false,
            flags: cli
                .flags
                .iter()
                .map(|flag| {
                    json!({
                        "name": flag.name,
                        "value_name": flag.value_name,
                        "required": flag.required,
                        "description": flag.description,
                    })
                })
                .collect(),
            default_output: None,
            interactive: false,
        }),
        palette: Some(PaletteOverlay {
            enabled: spec.transport != ActionTransport::McpOnly,
            category: Some("Example".to_owned()),
            icon: None,
            tone: Some("neutral".to_owned()),
            arg_mode: Some("schema".to_owned()),
            result_view: Some("json".to_owned()),
            aurora_blocks: Vec::new(),
        }),
        ui: None,
        examples: Vec::new(),
        meta: json!({
            "returns": spec.returns,
            "cli_usage": spec.cli.map(|cli| cli.usage),
            "scaffold_fallback": if spec.name == "scaffold_intent" {
                json!({
                    "recommended_skill": "scaffold-project",
                    "instructions": "Ask the user for the scaffold fields manually, then create the same JSON shape documented by the scaffold-project skill. Do not mutate files until the user approves the plan."
                })
            } else {
                Value::Null
            },
        }),
    }
}

fn action_input_schema(spec: &soma_contracts::actions::ActionSpec) -> Value {
    let mut properties = Map::new();
    let mut required = Vec::new();
    for param in spec.params {
        let json_type = match param.ty {
            "integer" => "integer",
            "number" => "number",
            "boolean" => "boolean",
            "object" => "object",
            "array" => "array",
            _ => "string",
        };
        let mut schema = json!({
            "type": json_type,
            "description": param.description,
        });
        if param.required && json_type == "string" {
            schema["minLength"] = json!(1);
        }
        properties.insert(param.name.to_owned(), schema);
        if param.required {
            required.push(Value::String(param.name.to_owned()));
        }
    }
    let mut schema = json!({
        "type": "object",
        "additionalProperties": false,
        "properties": properties,
    });
    if !required.is_empty() {
        schema["required"] = Value::Array(required);
    }
    schema
}

fn action_params(action: &str, params: &Value) -> Value {
    let mut params = params.clone();
    if let Value::Object(map) = &mut params {
        map.insert("action".to_owned(), Value::String(action.to_owned()));
    }
    params
}

fn surface_label(surface: crate::provider_registry::ProviderSurface) -> &'static str {
    match surface {
        crate::provider_registry::ProviderSurface::Mcp => "mcp",
        crate::provider_registry::ProviderSurface::Rest => "rest",
        crate::provider_registry::ProviderSurface::Cli => "cli",
        crate::provider_registry::ProviderSurface::Palette => "palette",
    }
}

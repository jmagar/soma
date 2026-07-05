use std::{process::Stdio, sync::Arc, time::Duration};

use async_trait::async_trait;
use rmcp::{
    model::CallToolRequestParams,
    transport::{ConfigureCommandExt, TokioChildProcess},
    ServiceExt,
};
use rtemplate_contracts::providers::{ProviderCatalog, ProviderTool};
use serde_json::{json, Map, Value};
use tokio::process::Command;

use crate::{
    provider_errors::ProviderError,
    provider_registry::{Provider, ProviderCall, ProviderOutput},
};

#[derive(Clone)]
pub struct McpProvider {
    catalog: ProviderCatalog,
}

impl McpProvider {
    pub fn new(catalog: ProviderCatalog) -> Self {
        Self { catalog }
    }

    pub fn arc(catalog: ProviderCatalog) -> Arc<Self> {
        Arc::new(Self::new(catalog))
    }
}

#[async_trait]
impl Provider for McpProvider {
    fn catalog(&self) -> ProviderCatalog {
        self.catalog.clone()
    }

    async fn call(&self, call: ProviderCall) -> Result<ProviderOutput, ProviderError> {
        let runtime = McpRuntime::from_catalog(&self.catalog, &call)?;
        let tool = self.tool(&call)?;
        let upstream = UpstreamTool::from_tool(tool, &call);
        let params = upstream.params(call.params);
        let timeout = runtime.timeout();

        let fut = async {
            let (transport, _stderr) =
                TokioChildProcess::builder(Command::new(&runtime.command).configure(|cmd| {
                    cmd.args(&runtime.args)
                        .env_clear()
                        .envs(runtime.env.iter().map(|(key, value)| (key, value)))
                        .stderr(Stdio::piped());
                    if let Some(cwd) = &runtime.cwd {
                        cmd.current_dir(cwd);
                    }
                }))
                .spawn()
                .map_err(|error| {
                    ProviderError::execution(
                        &self.catalog.provider.name,
                        call.action.clone(),
                        error,
                    )
                })?;
            let service = ().serve(transport).await.map_err(|error| {
                ProviderError::execution(&self.catalog.provider.name, call.action.clone(), error)
            })?;
            let result = service
                .call_tool(CallToolRequestParams::new(upstream.name.clone()).with_arguments(params))
                .await
                .map_err(|error| {
                    ProviderError::execution(
                        &self.catalog.provider.name,
                        call.action.clone(),
                        error,
                    )
                });
            let _ = service.cancel().await;
            result
        };

        let result = tokio::time::timeout(timeout, fut).await.map_err(|_| {
            ProviderError::new(
                "mcp_provider_timeout",
                &self.catalog.provider.name,
                Some(call.action.clone()),
                format!("upstream MCP tool call exceeded {}ms", timeout.as_millis()),
                "Increase provider.meta.mcp.timeout_ms or fix the upstream MCP server.",
            )
        })??;

        Ok(ProviderOutput::json(
            result
                .structured_content
                .unwrap_or_else(|| json!({ "content": result.content })),
        ))
    }
}

impl McpProvider {
    fn tool(&self, call: &ProviderCall) -> Result<&ProviderTool, ProviderError> {
        self.catalog
            .tools
            .iter()
            .find(|tool| tool.name == call.action)
            .ok_or_else(|| {
                ProviderError::validation(
                    &self.catalog.provider.name,
                    &call.action,
                    "unknown_mcp_action",
                    format!("MCP provider has no action `{}`", call.action),
                )
            })
    }
}

struct McpRuntime {
    command: String,
    args: Vec<String>,
    cwd: Option<String>,
    env: Vec<(String, String)>,
    timeout_ms: u64,
}

impl McpRuntime {
    fn from_catalog(catalog: &ProviderCatalog, call: &ProviderCall) -> Result<Self, ProviderError> {
        let meta = catalog
            .meta
            .get("mcp")
            .or_else(|| catalog.meta.get("runtime"))
            .ok_or_else(|| {
                ProviderError::validation(
                    &catalog.provider.name,
                    &call.action,
                    "missing_mcp_runtime",
                    "MCP provider requires provider.meta.mcp runtime config",
                )
            })?;
        let stdio = meta.get("stdio").unwrap_or(meta);
        let command = stdio
            .get("command")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                ProviderError::validation(
                    &catalog.provider.name,
                    &call.action,
                    "missing_mcp_command",
                    "MCP provider stdio runtime requires command",
                )
            })?
            .to_owned();
        let args = string_array(stdio.get("args")).map_err(|message| {
            ProviderError::validation(
                &catalog.provider.name,
                &call.action,
                "invalid_mcp_args",
                message,
            )
        })?;
        let cwd = stdio
            .get("cwd")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned);
        let env = env_pairs(stdio.get("env")).map_err(|message| {
            ProviderError::validation(
                &catalog.provider.name,
                &call.action,
                "invalid_mcp_env",
                message,
            )
        })?;
        let timeout_ms = meta
            .get("timeout_ms")
            .or_else(|| stdio.get("timeout_ms"))
            .and_then(Value::as_u64)
            .unwrap_or(10_000);
        Ok(Self {
            command,
            args,
            cwd,
            env,
            timeout_ms,
        })
    }

    fn timeout(&self) -> Duration {
        Duration::from_millis(self.timeout_ms)
    }
}

struct UpstreamTool {
    name: String,
    static_args: Map<String, Value>,
}

impl UpstreamTool {
    fn from_tool(tool: &ProviderTool, call: &ProviderCall) -> Self {
        let meta = tool.meta.get("mcp");
        let name = meta
            .and_then(|value| value.get("upstream_tool"))
            .and_then(Value::as_str)
            .unwrap_or(&tool.name)
            .to_owned();
        let static_args = meta
            .and_then(|value| value.get("static_args"))
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        tracing::debug!(
            provider_action = %call.action,
            upstream_tool = %name,
            "proxying MCP provider tool call"
        );
        Self { name, static_args }
    }

    fn params(&self, call_params: Value) -> Map<String, Value> {
        let mut params = self.static_args.clone();
        if let Value::Object(map) = call_params {
            params.extend(map);
        }
        params
    }
}

fn string_array(value: Option<&Value>) -> Result<Vec<String>, String> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let Some(values) = value.as_array() else {
        return Err("args must be an array of strings".to_owned());
    };
    values
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(ToOwned::to_owned)
                .ok_or_else(|| "args must be an array of strings".to_owned())
        })
        .collect()
}

fn env_pairs(value: Option<&Value>) -> Result<Vec<(String, String)>, String> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let Some(values) = value.as_object() else {
        return Err("env must be an object of string values".to_owned());
    };
    values
        .iter()
        .map(|(key, value)| {
            value
                .as_str()
                .map(|value| (key.clone(), value.to_owned()))
                .ok_or_else(|| "env must be an object of string values".to_owned())
        })
        .collect()
}

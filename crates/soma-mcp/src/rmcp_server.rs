//! `SomaRmcpServer` — the `ServerHandler` implementation.
//!
//! This is the adapter between the rmcp crate and your application. It:
//!   - Advertises tools, resources, and prompts to MCP clients
//!   - Enforces auth scopes on every call
//!   - Delegates business logic to `tools.rs` → `app.rs` → `soma.rs`
//!
//! **Customize**: rename `SomaRmcpServer`. Update action metadata in
//! `src/actions.rs` to keep schemas, scope rules, and dispatch in sync.

use std::time::Instant;

use rmcp::{
    model::{
        CallToolRequestParams, CallToolResult, GetPromptRequestParams, GetPromptResult,
        Implementation, ListPromptsResult, ListResourceTemplatesResult, ListResourcesResult,
        ListToolsResult, PaginatedRequestParams, ReadResourceRequestParams, ReadResourceResult,
        ResourceContents, ResourceTemplate, ServerCapabilities, ServerInfo,
    },
    service::{Peer, RequestContext},
    ErrorData, RoleServer, ServerHandler,
};
use serde_json::{Map, Value};

use soma_contracts::errors::ServiceErrorKind;

use soma_runtime::server::AppState;

use super::{
    conformance, gateway_proxy, prompts,
    response_paging::{
        response_page_request, strip_response_page_params, tool_result_from_cached_page,
        tool_result_from_json,
    },
    rmcp_adapters::{
        empty_action_as_none, provider_error_payload, refresh_file_providers,
        resource_contents_from_output, resource_read_error, rmcp_resource_from_catalog_resource,
        rmcp_tool_definitions, schema_resource, tool_definitions_for_state, tool_error_result,
        unknown_tool_error, SCHEMA_RESOURCE_URI,
    },
    rmcp_auth::{
        gateway_oauth_subject, protected_route_scope, protected_scope_allows_service,
        provider_auth_mode, provider_principal, require_auth_context,
    },
    tools::execute_tool,
};

// ── server ────────────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct SomaRmcpServer {
    state: AppState,
}

pub fn rmcp_server(state: AppState) -> SomaRmcpServer {
    SomaRmcpServer { state }
}

impl ServerHandler for SomaRmcpServer {
    // ── tools ─────────────────────────────────────────────────────────────────

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        let auth = require_auth_context(&self.state, &context)?;
        let route_scope = protected_route_scope(&context);
        let soma_allowed = protected_scope_allows_service(route_scope, "soma");
        let mut tools = Vec::new();
        if soma_allowed {
            refresh_file_providers(&self.state)?;
            tools.extend(rmcp_tool_definitions(&self.state)?);
        }
        let gateway_subject = gateway_oauth_subject(auth);
        tools.extend(
            gateway_proxy::list_tools_for_subject_and_scope(
                &self.state.gateway,
                Some(gateway_subject.as_ref()),
                route_scope,
            )
            .await?,
        );
        if soma_allowed && self.state.config.conformance_fixtures {
            tools.extend(conformance::tool_definitions());
        }
        tracing::debug!(tool_count = tools.len(), "MCP tools listed");
        Ok(ListToolsResult {
            tools,
            ..Default::default()
        })
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let tool_name = request.name.to_string();

        // Extract action before scope check so a missing action returns the
        // more useful "action is required" validation error, not DENY_SCOPE.
        let action_opt: Option<String> = request
            .arguments
            .as_ref()
            .and_then(|m| m.get("action"))
            .and_then(Value::as_str)
            .map(ToOwned::to_owned);

        let response_page = response_page_request(request.arguments.as_ref())?;
        let auth = require_auth_context(&self.state, &context)?;
        let route_scope = protected_route_scope(&context);
        let soma_allowed = protected_scope_allows_service(route_scope, "soma");
        if soma_allowed && self.state.config.conformance_fixtures {
            if let Some(result) = conformance::call_tool(&tool_name) {
                return Ok(result);
            }
        }
        if tool_name != "soma" {
            let gateway_subject = gateway_oauth_subject(auth);
            if let Some(result) = gateway_proxy::call_tool_for_subject_and_scope(
                &self.state.gateway,
                &tool_name,
                request.arguments,
                Some(gateway_subject.as_ref()),
                route_scope,
            )
            .await
            {
                return Ok(result);
            }
            return Err(unknown_tool_error(&tool_name));
        }
        if !soma_allowed {
            return Err(unknown_tool_error(&tool_name));
        }
        let action: String = action_opt.unwrap_or_default();
        if let Some(cursor) = response_page.cursor().map(str::to_owned) {
            return tool_result_from_cached_page(
                &self.state.response_pages,
                &cursor,
                response_page,
                &tool_name,
                empty_action_as_none(&action),
            );
        }

        let mut arguments = request
            .arguments
            .map(Value::Object)
            .unwrap_or_else(|| Value::Object(Map::new()));
        strip_response_page_params(&mut arguments);
        let continuation_args = arguments.as_object().cloned();

        // Clone the peer so we can pass it to the tool dispatcher.
        // The peer is needed for elicitation (asking the client for user input).
        let peer: Peer<RoleServer> = context.peer.clone();
        let principal = provider_principal(auth);
        let auth_mode = provider_auth_mode(&self.state.auth_policy);

        let started = Instant::now();
        tracing::info!(tool = %tool_name, action = %action, "MCP tool execution started");

        match execute_tool(
            &self.state,
            &tool_name,
            arguments,
            &peer,
            principal,
            auth_mode,
        )
        .await
        {
            Ok(result) => {
                tracing::info!(
                    tool = %tool_name,
                    elapsed_ms = started.elapsed().as_millis(),
                    "MCP tool execution completed"
                );
                tool_result_from_json(
                    result,
                    &self.state.response_pages,
                    response_page,
                    &tool_name,
                    empty_action_as_none(&action),
                    continuation_args.as_ref(),
                )
            }
            Err(error) => {
                let tool_error = soma_service::classify_service_error(&error);
                if tool_error.kind == ServiceErrorKind::Validation {
                    tracing::warn!(
                        tool = %tool_name,
                        elapsed_ms = started.elapsed().as_millis(),
                        "MCP tool rejected invalid params"
                    );
                } else {
                    tracing::error!(
                        tool = %tool_name,
                        elapsed_ms = started.elapsed().as_millis(),
                        service_error_kind = %tool_error.kind.as_str(),
                        error = %error,
                        "MCP tool execution failed"
                    );
                }
                tool_error_result(
                    provider_error_payload(&error, &tool_name, empty_action_as_none(&action))
                        .unwrap_or_else(|| {
                            tool_error.to_mcp_payload(&tool_name, empty_action_as_none(&action))
                        }),
                )
            }
        }
    }

    // ── resources ─────────────────────────────────────────────────────────────

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, ErrorData> {
        let auth = require_auth_context(&self.state, &context)?;
        let route_scope = protected_route_scope(&context);
        let soma_allowed = protected_scope_allows_service(route_scope, "soma");
        let mut resources = Vec::new();
        if soma_allowed {
            refresh_file_providers(&self.state)?;
            let snapshot = self.state.provider_registry.snapshot();
            resources.push(schema_resource());
            resources.extend(
                snapshot
                    .exact_resources()
                    .map(rmcp_resource_from_catalog_resource),
            );
        }
        let gateway_subject = gateway_oauth_subject(auth);
        resources.extend(
            gateway_proxy::list_resources_for_subject_and_scope(
                &self.state.gateway,
                Some(gateway_subject.as_ref()),
                route_scope,
            )
            .await?,
        );
        if soma_allowed && self.state.config.conformance_fixtures {
            resources.extend(conformance::resources());
        }
        Ok(ListResourcesResult {
            resources,
            ..Default::default()
        })
    }

    async fn list_resource_templates(
        &self,
        _request: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> Result<ListResourceTemplatesResult, ErrorData> {
        require_auth_context(&self.state, &context)?;
        let route_scope = protected_route_scope(&context);
        let soma_allowed = protected_scope_allows_service(route_scope, "soma");
        if !soma_allowed {
            return Ok(ListResourceTemplatesResult::default());
        }
        refresh_file_providers(&self.state)?;
        let snapshot = self.state.provider_registry.snapshot();
        let mut resource_templates: Vec<ResourceTemplate> = snapshot
            .dynamic_resource_templates()
            .iter()
            .map(|(_, template)| {
                let mut built =
                    ResourceTemplate::new(template.uri_template(), template.name.clone())
                        .with_description(template.description.clone());
                if let Some(mime_type) = &template.mime_type {
                    built = built.with_mime_type(mime_type.clone());
                }
                built
            })
            .collect();
        if self.state.config.conformance_fixtures {
            resource_templates.extend(conformance::resource_templates());
        }
        Ok(ListResourceTemplatesResult {
            resource_templates,
            ..Default::default()
        })
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResult, ErrorData> {
        let auth = require_auth_context(&self.state, &context)?;
        let route_scope = protected_route_scope(&context);
        let soma_allowed = protected_scope_allows_service(route_scope, "soma");
        if soma_allowed && self.state.config.conformance_fixtures {
            if let Some(result) = conformance::read_resource(&request.uri) {
                return Ok(result);
            }
        }
        if soma_allowed {
            refresh_file_providers(&self.state)?;
        }
        if soma_allowed && request.uri == SCHEMA_RESOURCE_URI {
            let schema = tool_definitions_for_state(&self.state);
            let text = serde_json::to_string_pretty(&schema).map_err(|e| {
                ErrorData::internal_error(format!("serialization error: {e}"), None)
            })?;
            return Ok(ReadResourceResult::new(vec![ResourceContents::text(
                text,
                SCHEMA_RESOURCE_URI,
            )
            .with_mime_type("application/json")]));
        }
        let gateway_subject = gateway_oauth_subject(auth);
        if let Some(result) = gateway_proxy::read_resource_for_subject_and_scope(
            &self.state.gateway,
            &request.uri,
            Some(gateway_subject.as_ref()),
            route_scope,
        )
        .await?
        {
            return Ok(result);
        }
        if !soma_allowed {
            return Err(ErrorData::invalid_params(
                format!("unknown resource: {}", request.uri),
                None,
            ));
        }

        let principal = provider_principal(auth);
        let auth_mode = provider_auth_mode(&self.state.auth_policy);
        let output = self
            .state
            .provider_registry
            .read_resource(&request.uri, &principal, auth_mode)
            .await
            .map_err(|error| resource_read_error(&request.uri, &error))?;
        Ok(ReadResourceResult::new(vec![
            resource_contents_from_output(&request.uri, output),
        ]))
    }

    // ── prompts ───────────────────────────────────────────────────────────────

    async fn list_prompts(
        &self,
        _request: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> Result<ListPromptsResult, ErrorData> {
        let auth = require_auth_context(&self.state, &context)?;
        let route_scope = protected_route_scope(&context);
        let soma_allowed = protected_scope_allows_service(route_scope, "soma");
        let mut result = if soma_allowed {
            let mut result = prompts::list_prompts();
            refresh_file_providers(&self.state)?;
            let snapshot = self.state.provider_registry.snapshot();
            result
                .prompts
                .extend(prompts::provider_prompts(&snapshot.catalogs));
            result
        } else {
            ListPromptsResult::default()
        };
        let gateway_subject = gateway_oauth_subject(auth);
        result.prompts.extend(
            gateway_proxy::list_prompts_for_subject_and_scope(
                &self.state.gateway,
                Some(gateway_subject.as_ref()),
                route_scope,
            )
            .await?,
        );
        if soma_allowed && self.state.config.conformance_fixtures {
            result.prompts.extend(conformance::prompts());
        }
        Ok(result)
    }

    async fn get_prompt(
        &self,
        request: GetPromptRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<GetPromptResult, ErrorData> {
        let auth = require_auth_context(&self.state, &context)?;
        let route_scope = protected_route_scope(&context);
        let soma_allowed = protected_scope_allows_service(route_scope, "soma");
        if soma_allowed && self.state.config.conformance_fixtures {
            if let Some(result) = conformance::get_prompt(request.clone()) {
                return Ok(result);
            }
        }
        if soma_allowed {
            refresh_file_providers(&self.state)?;
            let snapshot = self.state.provider_registry.snapshot();
            match prompts::get_provider_prompt(
                &snapshot.catalogs,
                &request,
                provider_auth_mode(&self.state.auth_policy),
                &provider_principal(auth),
            ) {
                prompts::ProviderPromptLookup::Found(result) => return Ok(result),
                prompts::ProviderPromptLookup::ScopeDenied { required_scope } => {
                    return Err(ErrorData::invalid_request(
                        format!(
                            "forbidden: prompt `{}` requires scope `{required_scope}`",
                            request.name
                        ),
                        None,
                    ));
                }
                prompts::ProviderPromptLookup::NotFound => {}
            }
        }
        let gateway_subject = gateway_oauth_subject(auth);
        if let Some(result) = gateway_proxy::get_prompt_for_subject_and_scope(
            &self.state.gateway,
            request.name.as_ref(),
            request.arguments.clone(),
            Some(gateway_subject.as_ref()),
            route_scope,
        )
        .await?
        {
            return Ok(result);
        }
        if !soma_allowed {
            return Err(ErrorData::invalid_params("prompt not found", None));
        }
        prompts::get_prompt(request).map_err(|e| ErrorData::invalid_params(e.to_string(), None))
    }

    // ── server info ───────────────────────────────────────────────────────────

    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .enable_prompts()
                .build(),
        )
        .with_server_info(Implementation::new(
            self.state.config.server_name.clone(),
            env!("CARGO_PKG_VERSION"),
        ))
        .with_instructions(SERVER_INSTRUCTIONS)
    }
}

const SERVER_INSTRUCTIONS: &str = "\
Soma is a batteries-included RMCP runtime for shipping provider-backed MCP servers. \
It exposes one action-dispatched `soma` tool plus first-class MCP prompt and resource surfaces. \
Homepage: https://soma.dinglebear.ai. Repository: https://github.com/jmagar/soma. \
Node package: soma-rmcp. Binary: soma. \
Config home: ~/.soma or SOMA_HOME. License: MIT. Author: dinglebear.ai. \
Use drop-in providers to add tools, prompts, and resources without rewriting transport, auth, \
schema, paging, config, Docker, plugin, or release plumbing. A new server comes online by adding \
provider files under providers/tools, providers/prompts, providers/resources, or another configured \
provider source. Clients should discover `soma://schema/mcp-tool` before invoking actions, call \
`status` or `help` to inspect available providers, and send JSON action arguments matching the \
advertised schema. Responses are structured JSON; large payloads may be paged through Soma's \
resource paging flow.";

#[cfg(test)]
#[path = "rmcp_server_tests.rs"]
mod tests;

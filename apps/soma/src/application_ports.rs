use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;

use soma_application::{
    ApplicationPorts, ExecutionContext, GatewayExecuteRequest, GatewayPort, GatewayPromptRoute,
    GatewayReloadRequest, GatewayResourceRoute, GatewayRouteScope, GatewayToolRoute, PortError,
    SomaApplication,
};
use soma_contracts::{
    actions::{scopes_satisfy, READ_SCOPE},
    scopes::has_admin_scope,
};
use soma_domain::AuthorizationMode;
use soma_gateway::gateway::dispatch::{
    dispatch_gateway_action, GatewayAccess, GatewayDispatchError,
};
use soma_gateway::gateway::{manager::GatewayManagerError, protected_routes::ProtectedRouteScope};
use soma_runtime::server::AppState;
use soma_runtime::server::GatewayProductState;

pub(crate) fn application_for_state(state: &AppState) -> Arc<SomaApplication> {
    let ports = ApplicationPorts::unavailable()
        .with_gateway(Arc::new(GatewayApplicationPort::new(state.gateway.clone())));
    Arc::new(SomaApplication::new(
        Arc::new(state.service.clone()),
        Arc::new(state.provider_registry.clone()),
        ports,
    ))
}

pub(crate) fn authorization_mode(state: &AppState) -> AuthorizationMode {
    match &state.auth_policy {
        soma_runtime::server::AuthPolicy::LoopbackDev => AuthorizationMode::LoopbackDev,
        soma_runtime::server::AuthPolicy::TrustedGatewayUnscoped => {
            AuthorizationMode::TrustedGateway
        }
        soma_runtime::server::AuthPolicy::Mounted { .. } => AuthorizationMode::Mounted,
    }
}

pub(crate) fn mcp_state_for_state(state: &AppState) -> soma_mcp::McpState {
    soma_mcp::McpState::new(
        application_for_state(state),
        state.config.clone(),
        authorization_mode(state),
        state.response_pages.clone(),
    )
}

pub(crate) struct GatewayApplicationPort {
    gateway: GatewayProductState,
}

impl GatewayApplicationPort {
    pub(crate) fn new(gateway: GatewayProductState) -> Self {
        Self { gateway }
    }

    async fn dispatch(
        &self,
        action: &str,
        params: Value,
        context: &ExecutionContext,
    ) -> Result<Value, PortError> {
        dispatch_gateway_action(&self.gateway, gateway_access(context), action, params)
            .await
            .map_err(|error| gateway_port_error(action, error))
    }
}

#[async_trait]
impl GatewayPort for GatewayApplicationPort {
    async fn status(&self, context: &ExecutionContext) -> Result<Value, PortError> {
        self.dispatch("gateway.list", serde_json::json!({}), context)
            .await
    }

    async fn reload(
        &self,
        request: GatewayReloadRequest,
        context: &ExecutionContext,
    ) -> Result<Value, PortError> {
        self.dispatch("gateway.reload", request.config, context)
            .await
    }

    async fn execute(
        &self,
        request: GatewayExecuteRequest,
        context: &ExecutionContext,
    ) -> Result<Value, PortError> {
        self.dispatch(&request.action, request.params, context)
            .await
    }

    async fn list_mcp_tools(
        &self,
        scope: Option<&GatewayRouteScope>,
        context: &ExecutionContext,
    ) -> Result<Vec<GatewayToolRoute>, PortError> {
        let scope = scope.map(protected_route_scope);
        self.gateway
            .tool_routes_for_subject_and_scope(Some(gateway_subject(context)), scope.as_ref())
            .await
            .map(|routes| {
                routes
                    .into_iter()
                    .map(|route| GatewayToolRoute {
                        name: route.name,
                        description: route.descriptor.description,
                        input_schema: route.descriptor.input_schema,
                        output_schema: route.descriptor.output_schema,
                        destructive: route.descriptor.destructive,
                    })
                    .collect()
            })
            .map_err(|error| gateway_manager_port_error("tools/list", error))
    }

    async fn call_mcp_tool(
        &self,
        name: &str,
        params: Value,
        scope: Option<&GatewayRouteScope>,
        context: &ExecutionContext,
    ) -> Result<Option<Value>, PortError> {
        let scope = scope.map(protected_route_scope);
        self.gateway
            .call_mcp_tool_for_subject_and_scope(
                name,
                params,
                Some(gateway_subject(context)),
                scope.as_ref(),
            )
            .await
            .map_err(|error| gateway_manager_port_error("tools/call", error))
    }

    async fn list_mcp_resources(
        &self,
        scope: Option<&GatewayRouteScope>,
        context: &ExecutionContext,
    ) -> Result<Vec<GatewayResourceRoute>, PortError> {
        let scope = scope.map(protected_route_scope);
        self.gateway
            .resource_routes_for_subject_and_scope(Some(gateway_subject(context)), scope.as_ref())
            .await
            .map(|routes| {
                routes
                    .into_iter()
                    .map(|route| GatewayResourceRoute {
                        uri: route.uri,
                        native_uri: route.native_uri,
                        name: route.descriptor.name,
                    })
                    .collect()
            })
            .map_err(|error| gateway_manager_port_error("resources/list", error))
    }

    async fn read_mcp_resource(
        &self,
        uri: &str,
        scope: Option<&GatewayRouteScope>,
        context: &ExecutionContext,
    ) -> Result<Option<Value>, PortError> {
        let scope = scope.map(protected_route_scope);
        self.gateway
            .read_mcp_resource_for_subject_and_scope(
                uri,
                Some(gateway_subject(context)),
                scope.as_ref(),
            )
            .await
            .map_err(|error| gateway_manager_port_error("resources/read", error))
    }

    async fn list_mcp_prompts(
        &self,
        scope: Option<&GatewayRouteScope>,
        context: &ExecutionContext,
    ) -> Result<Vec<GatewayPromptRoute>, PortError> {
        let scope = scope.map(protected_route_scope);
        self.gateway
            .prompt_routes_for_subject_and_scope(Some(gateway_subject(context)), scope.as_ref())
            .await
            .map(|routes| {
                routes
                    .into_iter()
                    .map(|route| GatewayPromptRoute {
                        name: route.name,
                        description: route.descriptor.description,
                    })
                    .collect()
            })
            .map_err(|error| gateway_manager_port_error("prompts/list", error))
    }

    async fn get_mcp_prompt(
        &self,
        name: &str,
        arguments: Option<serde_json::Map<String, Value>>,
        scope: Option<&GatewayRouteScope>,
        context: &ExecutionContext,
    ) -> Result<Option<Value>, PortError> {
        let scope = scope.map(protected_route_scope);
        self.gateway
            .get_mcp_prompt_for_subject_and_scope(
                name,
                arguments,
                Some(gateway_subject(context)),
                scope.as_ref(),
            )
            .await
            .map_err(|error| gateway_manager_port_error("prompts/get", error))
    }
}

fn gateway_subject(context: &ExecutionContext) -> &str {
    const SHARED_GATEWAY_SUBJECT: &str = "gateway";
    if !matches!(context.authorization_mode, AuthorizationMode::Mounted) {
        return SHARED_GATEWAY_SUBJECT;
    }
    let Some(principal) = context.principal.as_ref() else {
        return SHARED_GATEWAY_SUBJECT;
    };
    if principal.issuer.as_deref() == Some("local") || has_admin_scope(&principal.scopes.to_vec()) {
        SHARED_GATEWAY_SUBJECT
    } else {
        &principal.subject
    }
}

fn protected_route_scope(scope: &GatewayRouteScope) -> ProtectedRouteScope {
    ProtectedRouteScope {
        upstreams: scope.upstreams.clone(),
        services: scope.services.clone(),
        expose_code_mode: scope.expose_code_mode,
    }
}

fn gateway_access(context: &ExecutionContext) -> GatewayAccess {
    if !matches!(context.authorization_mode, AuthorizationMode::Mounted) {
        return GatewayAccess {
            read: true,
            admin: true,
        };
    }
    let scopes = context
        .principal
        .as_ref()
        .map(|principal| principal.scopes.to_vec())
        .unwrap_or_default();
    let admin = has_admin_scope(&scopes);
    GatewayAccess {
        read: admin || scopes_satisfy(&scopes, READ_SCOPE),
        admin,
    }
}

fn gateway_port_error(action: &str, error: GatewayDispatchError) -> PortError {
    let structured = error.structured(action);
    PortError {
        code: structured.code.to_owned(),
        message: error.to_string(),
        retryable: matches!(structured.kind, "runtime"),
        remediation: structured.remediation.to_owned(),
    }
}

fn gateway_manager_port_error(operation: &str, error: GatewayManagerError) -> PortError {
    PortError {
        code: "gateway_proxy_failed".to_owned(),
        message: format!("{operation} failed: {error}"),
        retryable: true,
        remediation: "Check the gateway upstream configuration and retry.".to_owned(),
    }
}

#[cfg(test)]
#[path = "application_ports_tests.rs"]
mod tests;

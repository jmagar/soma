//! Constructs `SomaApplication`'s ports from `soma-integrations` adapters.
//!
//! This module holds only composition: it builds `soma_integrations`
//! adapters and wires them into `ApplicationPorts` / `SomaRuntime`. The
//! adapters' implementation logic lives in `soma-integrations` (plan section
//! 3.20, PR 11's acceptance criterion).

use std::sync::Arc;

use soma_application::{ApplicationPorts, SomaApplication};
#[cfg(feature = "mcp")]
use soma_runtime::server::AppState;
use soma_runtime::server::{GatewayProductState, SomaRuntime};
use soma_service::{ProviderRegistry, SomaService};

pub(crate) fn runtime_for_components(
    service: SomaService,
    provider_registry: ProviderRegistry,
    gateway: GatewayProductState,
) -> Arc<SomaRuntime> {
    let ports = ApplicationPorts::unavailable()
        .with_gateway(Arc::new(soma_integrations::GatewayApplicationPort::new(
            gateway.clone(),
        )))
        .with_codemode(Arc::new(
            soma_integrations::CodeModeApplicationPort::default(),
        ));
    let application = Arc::new(SomaApplication::new(
        Arc::new(service),
        Arc::new(provider_registry),
        ports,
    ));
    Arc::new(SomaRuntime::new(application, gateway))
}

#[cfg(feature = "mcp")]
pub(crate) fn authorization_mode(state: &AppState) -> soma_domain::AuthorizationMode {
    match &state.auth_policy {
        soma_runtime::server::AuthPolicy::LoopbackDev => {
            soma_domain::AuthorizationMode::LoopbackDev
        }
        soma_runtime::server::AuthPolicy::TrustedGatewayUnscoped => {
            soma_domain::AuthorizationMode::TrustedGateway
        }
        soma_runtime::server::AuthPolicy::Mounted { .. } => soma_domain::AuthorizationMode::Mounted,
    }
}

#[cfg(feature = "mcp")]
pub(crate) fn mcp_state_for_state(state: &AppState) -> soma_mcp::McpState {
    soma_mcp::McpState::new(
        state.application_handle(),
        state.config.clone(),
        authorization_mode(state),
        state.response_pages.clone(),
    )
}

#[cfg(all(test, feature = "mcp"))]
#[path = "application_ports_tests.rs"]
mod tests;

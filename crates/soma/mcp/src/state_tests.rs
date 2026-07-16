use soma_domain::{AuthorizationMode, Surface};

use crate::testing::loopback_state;

#[test]
fn execution_context_is_mcp_scoped_with_unique_request_ids() {
    let state = loopback_state();

    let first = state.execution_context(None, None);
    let second = state.execution_context(None, None);

    assert_eq!(first.surface, Surface::Mcp);
    assert_eq!(first.authorization_mode, AuthorizationMode::LoopbackDev);
    assert_ne!(first.request_id, second.request_id);
}

#[test]
fn configured_server_name_is_isolated_to_mcp_state() {
    let state = loopback_state().with_server_name("custom-soma");

    assert_eq!(state.config().server_name, "custom-soma");
}

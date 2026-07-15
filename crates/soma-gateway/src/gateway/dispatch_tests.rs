use crate::config::GatewayConfig;
use crate::gateway::manager::GatewayManager;

use super::*;

const READ_ACCESS: GatewayAccess = GatewayAccess {
    read: true,
    admin: false,
};
const ADMIN_ACCESS: GatewayAccess = GatewayAccess {
    read: true,
    admin: true,
};

fn default_manager() -> GatewayManager {
    GatewayManager::new(GatewayConfig::default()).unwrap()
}

#[test]
fn read_access_can_list_but_cannot_admin_test() {
    let manager = default_manager();

    dispatch_gateway_action(&manager, READ_ACCESS, "gateway.list", serde_json::json!({})).unwrap();
    let error = dispatch_gateway_action(
        &manager,
        READ_ACCESS,
        "gateway.test",
        serde_json::json!({"command": "node"}),
    )
    .unwrap_err();

    assert!(matches!(error, GatewayDispatchError::AdminRequired));
}

#[test]
fn admin_spawn_actions_run_spawn_validation() {
    let manager = default_manager();
    let error = dispatch_gateway_action(
        &manager,
        ADMIN_ACCESS,
        "gateway.test",
        serde_json::json!({"command": "/tmp/x/node"}),
    )
    .unwrap_err();

    assert!(matches!(error, GatewayDispatchError::SpawnValidation));
}

#[test]
fn admin_spawn_actions_validate_args_and_env() {
    let manager = default_manager();

    let bad_env = dispatch_gateway_action(
        &manager,
        ADMIN_ACCESS,
        "gateway.add",
        serde_json::json!({"name": "bad", "command": "node", "env": {"LD_PRELOAD": "x.so"}}),
    )
    .unwrap_err();
    assert!(matches!(bad_env, GatewayDispatchError::SpawnValidation));

    let bad_arg = dispatch_gateway_action(
        &manager,
        ADMIN_ACCESS,
        "gateway.add",
        serde_json::json!({"name": "bad", "command": "node", "args": ["--disable-spawn-guard"]}),
    )
    .unwrap_err();
    assert!(matches!(bad_arg, GatewayDispatchError::SpawnValidation));
}

#[test]
fn admin_actions_mutate_gateway_config() {
    let manager = default_manager();

    let added = dispatch_gateway_action(
        &manager,
        ADMIN_ACCESS,
        "gateway.add",
        serde_json::json!({"name": "demo", "url": "https://example.com/mcp"}),
    )
    .unwrap();
    assert_eq!(added["added"], true);
    assert_eq!(
        dispatch_gateway_action(
            &manager,
            ADMIN_ACCESS,
            "gateway.list",
            serde_json::json!({})
        )
        .unwrap()["upstream_count"],
        1
    );

    let removed = dispatch_gateway_action(
        &manager,
        ADMIN_ACCESS,
        "gateway.remove",
        serde_json::json!({"name": "demo"}),
    )
    .unwrap();
    assert_eq!(removed["removed"], "demo");
    assert!(manager.discover().unwrap().is_empty());
}

#[test]
fn unknown_actions_fail_closed() {
    let manager = default_manager();

    let error = dispatch_gateway_action(
        &manager,
        ADMIN_ACCESS,
        "gateway.typo",
        serde_json::json!({}),
    )
    .unwrap_err();

    assert!(matches!(error, GatewayDispatchError::UnknownAction));
    assert_eq!(
        error.structured("gateway.typo").to_json()["code"],
        "unknown_action"
    );
}

#[test]
fn structured_errors_keep_stable_gateway_shape() {
    let error = GatewayDispatchError::Params(ParamsError::MustBeObject);
    let structured = error.structured("gateway.add").to_json();

    assert_eq!(structured["code"], "invalid_param");
    assert_eq!(structured["action"], "gateway.add");
}

use serde_json::{json, Value};
use thiserror::Error;

use crate::dispatch_helpers::{structured_error, GatewayStructuredError};
use crate::gateway::catalog::{GatewayAction, GatewayActionCatalog};
use crate::gateway::manager::GatewayManager;
use crate::gateway::params::{
    object_params, required_string_param, test_upstream_config_from_params,
    upstream_config_from_params, ParamsError,
};
use crate::process::guard::SpawnGuard;
use crate::process::stdio::StdioProcessSpec;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GatewayAccess {
    pub read: bool,
    pub admin: bool,
}

#[derive(Debug, Error)]
pub enum GatewayDispatchError {
    #[error("gateway admin access required")]
    AdminRequired,
    #[error(transparent)]
    Params(#[from] ParamsError),
    #[error("unknown gateway action")]
    UnknownAction,
    #[error("gateway action is not implemented")]
    NotImplemented,
    #[error("spawn validation failed")]
    SpawnValidation,
    #[error(transparent)]
    Manager(#[from] crate::gateway::manager::GatewayManagerError),
}

impl GatewayDispatchError {
    #[must_use]
    pub fn structured(&self, action: &str) -> GatewayStructuredError {
        match self {
            Self::AdminRequired => structured_error(
                action,
                "admin_required",
                "authorization",
                "use a principal with gateway admin access",
            ),
            Self::Params(_) => structured_error(
                action,
                "invalid_param",
                "validation",
                "pass an object with valid gateway action parameters",
            ),
            Self::UnknownAction => structured_error(
                action,
                "unknown_action",
                "validation",
                "use one of the advertised gateway actions",
            ),
            Self::NotImplemented => structured_error(
                action,
                "not_implemented",
                "unsupported",
                "this gateway action is advertised but not implemented in this build",
            ),
            Self::SpawnValidation => structured_error(
                action,
                "spawn_validation_failed",
                "validation",
                "use an allowed command and safe environment",
            ),
            Self::Manager(error) => manager_error_shape(action, error),
        }
    }
}

pub fn dispatch_gateway_action(
    manager: &GatewayManager,
    access: GatewayAccess,
    action_name: &str,
    params: Value,
) -> Result<Value, GatewayDispatchError> {
    let catalog = GatewayActionCatalog::standard();
    let action = catalog
        .get(action_name)
        .ok_or(GatewayDispatchError::UnknownAction)?;
    enforce_access(action, access)?;
    if action.spawn_validation_required {
        validate_spawn_params(&params)?;
    }
    match action_name {
        "gateway.list" => Ok(crate::gateway::view_models::gateway_list_view(manager)?),
        "gateway.config.view" => Ok(crate::gateway::view_models::gateway_config_view(manager)),
        "gateway.add" => {
            let view = manager.add_upstream(upstream_config_from_params(&params)?)?;
            Ok(json!({"added": true, "config": view}))
        }
        "gateway.update" => {
            let view = manager.update_upstream(upstream_config_from_params(&params)?)?;
            Ok(json!({"updated": true, "config": view}))
        }
        "gateway.remove" => {
            let params = object_params(&params)?;
            let name = required_string_param(params, "name")?;
            let view = manager.remove_upstream(&name)?;
            Ok(json!({"removed": name, "config": view}))
        }
        "gateway.reload" => {
            let view = manager.reload_from_store()?;
            Ok(json!({"reloaded": true, "config": view}))
        }
        "gateway.test" | "gateway.import.approve" => Err(GatewayDispatchError::NotImplemented),
        _ => Err(GatewayDispatchError::UnknownAction),
    }
}

fn enforce_access(
    action: GatewayAction,
    access: GatewayAccess,
) -> Result<(), GatewayDispatchError> {
    if action.admin_required && !access.admin {
        return Err(GatewayDispatchError::AdminRequired);
    }
    if !(action.admin_required || access.read || access.admin) {
        return Err(GatewayDispatchError::AdminRequired);
    }
    Ok(())
}

fn validate_spawn_params(params: &Value) -> Result<(), GatewayDispatchError> {
    let config = test_upstream_config_from_params(params)?;
    if let Some(command) = config.command {
        let spec = StdioProcessSpec {
            command,
            args: config.args,
            env: config.env,
        };
        spec.validate(&SpawnGuard::default())
            .map_err(|_| GatewayDispatchError::SpawnValidation)?;
    }
    Ok(())
}

fn manager_error_shape(
    action: &str,
    error: &crate::gateway::manager::GatewayManagerError,
) -> GatewayStructuredError {
    use crate::gateway::manager::GatewayManagerError;
    use crate::upstream::UpstreamError;

    match error {
        GatewayManagerError::GatewayReloading => structured_error(
            action,
            "gateway_reloading",
            "runtime",
            "retry after the gateway reload completes",
        ),
        GatewayManagerError::StoreNotMounted => structured_error(
            action,
            "store_not_mounted",
            "configuration",
            "start Soma with a mounted gateway config store",
        ),
        GatewayManagerError::UpstreamExists(_) => structured_error(
            action,
            "upstream_exists",
            "validation",
            "use gateway.update or remove the existing upstream first",
        ),
        GatewayManagerError::UpstreamMissing(_) => structured_error(
            action,
            "upstream_missing",
            "validation",
            "configure the upstream before operating on it",
        ),
        GatewayManagerError::Config(_) => structured_error(
            action,
            "invalid_config",
            "validation",
            "fix the gateway configuration and retry",
        ),
        GatewayManagerError::Upstream(UpstreamError::UnknownUpstream { .. }) => structured_error(
            action,
            "unknown_upstream",
            "validation",
            "configure the upstream before calling it",
        ),
        GatewayManagerError::Upstream(UpstreamError::NotExposed { .. }) => structured_error(
            action,
            "not_exposed",
            "authorization",
            "choose a tool/resource/prompt exposed by the upstream policy",
        ),
        GatewayManagerError::Upstream(UpstreamError::NotRoutable { .. }) => structured_error(
            action,
            "not_routable",
            "runtime",
            "wait for the upstream to connect or fix its configuration",
        ),
        GatewayManagerError::Upstream(UpstreamError::Unsupported { .. }) => structured_error(
            action,
            "unsupported_transport",
            "unsupported",
            "this gateway build cannot route that upstream transport yet",
        ),
        GatewayManagerError::Upstream(UpstreamError::ResponseTooLarge { .. }) => structured_error(
            action,
            "response_too_large",
            "limits",
            "narrow the request or lower the upstream response size",
        ),
        GatewayManagerError::Upstream(UpstreamError::ParamsMustBeObject) => structured_error(
            action,
            "invalid_param",
            "validation",
            "pass tool params as a JSON object",
        ),
    }
}

#[cfg(test)]
#[path = "dispatch_tests.rs"]
mod tests;

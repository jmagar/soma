//! Resolves [`crate::api::ApiSourceFamily::Hybrid`] capabilities to a
//! concrete official or internal action.
//!
//! Callers can force a side with `params.prefer` (`"official"` or
//! `"internal"`); otherwise the presence of a `siteId` parameter is taken as
//! a signal the caller wants the official API's site-scoped shape.

use serde_json::{json, Value};

use crate::error::{Result, UnifiError};

/// Resolves `action` to a concrete `(target_action, params)` pair.
///
/// # Errors
/// Returns [`UnifiError::HybridRouting`] if `params.prefer` is set to
/// something other than `"official"`/`"internal"`, or if `action` has no
/// mapping for the resolved side.
pub fn resolve(action: &str, params: &Value) -> Result<(&'static str, Value)> {
    let prefer = params
        .get("prefer")
        .and_then(Value::as_str)
        .map(str::to_ascii_lowercase);
    let has_site_id = params.get("siteId").is_some();
    let target = match prefer.as_deref() {
        Some("official") => official_target(action),
        Some("internal") => internal_target(action),
        Some(other) => {
            return Err(UnifiError::HybridRouting(format!(
                "unknown hybrid preference: {other}"
            )))
        }
        None if has_site_id => official_target(action),
        None => internal_target(action),
    };
    let Some(target) = target else {
        return Err(UnifiError::HybridRouting(format!(
            "unknown hybrid action: {action}"
        )));
    };
    Ok((target, normalize_params(params)))
}

fn official_target(action: &str) -> Option<&'static str> {
    match action {
        "list_clients" => Some("official_list_clients"),
        "list_devices" => Some("official_list_devices"),
        "list_networks" => Some("official_list_networks"),
        "list_wifi" => Some("official_list_wifi"),
        "get_system_info" => Some("official_get_info"),
        _ => None,
    }
}

fn internal_target(action: &str) -> Option<&'static str> {
    match action {
        "list_clients" => Some("clients"),
        "list_devices" => Some("devices"),
        "list_networks" => Some("unifi_list_networks"),
        "list_wifi" => Some("wlans"),
        "get_system_info" => Some("sysinfo"),
        _ => None,
    }
}

fn normalize_params(params: &Value) -> Value {
    let mut value = params.clone();
    if let Some(object) = value.as_object_mut() {
        object.remove("prefer");
    }
    if value.is_null() {
        json!({})
    } else {
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_defaults_to_internal_without_a_site_id() {
        let (target, params) = resolve("list_clients", &json!({})).unwrap();

        assert_eq!(target, "clients");
        assert_eq!(params, json!({}));
    }

    #[test]
    fn resolve_prefers_official_when_a_site_id_is_present() {
        let (target, _) = resolve("list_clients", &json!({ "siteId": "abc" })).unwrap();

        assert_eq!(target, "official_list_clients");
    }

    #[test]
    fn resolve_honors_an_explicit_preference_over_the_site_id_heuristic() {
        let (target, _) = resolve(
            "list_clients",
            &json!({ "siteId": "abc", "prefer": "internal" }),
        )
        .unwrap();

        assert_eq!(target, "clients");
    }

    #[test]
    fn resolve_strips_the_prefer_field_from_the_forwarded_params() {
        let (_, params) =
            resolve("list_clients", &json!({ "prefer": "internal", "limit": 5 })).unwrap();

        assert_eq!(params, json!({ "limit": 5 }));
    }

    #[test]
    fn resolve_errors_on_an_unknown_preference() {
        let err = resolve("list_clients", &json!({ "prefer": "both" })).unwrap_err();

        assert!(
            matches!(err, UnifiError::HybridRouting(msg) if msg.contains("unknown hybrid preference"))
        );
    }

    #[test]
    fn resolve_errors_on_an_unmapped_action() {
        let err = resolve("not_a_real_action", &json!({})).unwrap_err();

        assert!(
            matches!(err, UnifiError::HybridRouting(msg) if msg.contains("unknown hybrid action"))
        );
    }
}

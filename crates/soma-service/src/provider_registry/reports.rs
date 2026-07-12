use serde_json::{json, Value};

use super::RegistrySnapshot;

impl RegistrySnapshot {
    pub fn validation_summary(&self) -> Value {
        let actions = self.action_names();
        json!({
            "schema_version": 1,
            "ok": true,
            "provider_fingerprint": self.fingerprint.clone(),
            "provider_count": self.catalogs.len(),
            "action_count": actions.len(),
            "compiled_validator_count": self.compiled_validator_count,
            "actions": actions
        })
    }

    pub fn inspection_report(&self) -> Value {
        let providers = self
            .catalogs
            .iter()
            .map(|catalog| {
                let kind = catalog.provider.kind.as_str();
                let tools = catalog
                    .tools
                    .iter()
                    .map(|tool| {
                        json!({
                            "name": tool.name.clone(),
                            "description": tool.description.clone(),
                            "scope": tool.scope.clone(),
                            "destructive": tool.destructive,
                            "requires_admin": tool.requires_admin,
                            "surfaces": {
                                "mcp": tool.mcp.as_ref().map(|mcp| mcp.enabled).unwrap_or(true),
                                "rest": tool.rest.as_ref().map(|rest| rest.enabled).unwrap_or(false),
                                "cli": tool.cli.as_ref().map(|cli| cli.enabled).unwrap_or(false),
                                "palette": tool.palette.as_ref().map(|palette| palette.enabled).unwrap_or(true)
                            },
                            "limits": tool.limits.clone(),
                            "env": tool.env.clone(),
                        })
                    })
                    .collect::<Vec<_>>();
                json!({
                    "name": catalog.provider.name.clone(),
                    "kind": kind,
                    "title": catalog.provider.title.clone(),
                    "enabled": catalog.provider.enabled.unwrap_or(true),
                    "version": catalog.provider.version.clone(),
                    "source": catalog.provider.source.clone(),
                    "declared_capabilities": catalog.capabilities.clone(),
                    "runtime_security": provider_runtime_security(kind),
                    "tools": tools,
                })
            })
            .collect::<Vec<_>>();
        json!({
            "schema_version": 1,
            "provider_fingerprint": self.fingerprint.clone(),
            "compiled_validator_count": self.compiled_validator_count,
            "actions": self.action_names(),
            "providers": providers,
        })
    }
}

fn provider_runtime_security(kind: &str) -> Value {
    match kind {
        "wasm" => json!({
            "runtime": "wasmtime",
            "trust": "sandboxed",
            "capability_enforcement": "registry broker enforces declared host capabilities before dispatch"
        }),
        "ai-sdk" | "python" | "langchain" | "llamaindex" => json!({
            "runtime": "sidecar-process",
            "trust": "trusted-local-code",
            "capability_enforcement": "registry broker enforces declared host capabilities before dispatch"
        }),
        "openapi" | "mcp" => json!({
            "runtime": "remote-or-upstream",
            "trust": "upstream-service",
            "capability_enforcement": "registry broker enforces declared host capabilities before dispatch"
        }),
        _ => json!({
            "runtime": "in-process",
            "trust": "trusted-binary",
            "capability_enforcement": "registry broker enforces declared host capabilities before dispatch"
        }),
    }
}

#[cfg(test)]
#[path = "reports_tests.rs"]
mod tests;

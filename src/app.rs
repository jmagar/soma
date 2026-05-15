//! Business service layer.
//!
//! **All business logic lives here.** CLI and MCP are thin shims that call into this.
//!
//! `ExampleService` owns an `ExampleClient` and exposes typed methods.
//! If you need caching, retries, data transformation, or validation, do it here —
//! never in `cli.rs` or `mcp/tools.rs`.

use anyhow::Result;
use serde_json::{json, Value};

use crate::example::ExampleClient;

// Unit tests live in a sidecar file — see src/app_tests.rs for the pattern.
#[cfg(test)]
#[path = "app_tests.rs"]
mod tests;

/// The service layer — wraps the transport client and adds business logic.
///
/// **Template**: rename this to `MyServiceService` (or whatever fits).
/// Add any fields you need: caches, config, metrics, etc.
#[derive(Clone)]
pub struct ExampleService {
    client: ExampleClient,
}

#[derive(Debug, Clone)]
pub struct ScaffoldIntent {
    pub display_name: String,
    pub crate_name: String,
    pub binary_name: String,
    pub server_category: String,
    pub env_prefix: String,
    pub auth_kind: String,
    pub host: String,
    pub port: u16,
    pub mcp_transport: String,
    pub mcp_primitives: String,
    pub deployment: String,
    pub plugins: String,
    pub publish_mcp: bool,
    pub crawl_urls: String,
    pub crawl_repos: String,
    pub crawl_search_topics: String,
}

impl ExampleService {
    pub fn new(client: ExampleClient) -> Self {
        Self { client }
    }

    /// Return a greeting for `name`, defaulting to "World".
    pub async fn greet(&self, name: Option<&str>) -> Result<Value> {
        self.client.greet(name).await
    }

    /// Echo `message` back unchanged.
    pub async fn echo(&self, message: &str) -> Result<Value> {
        self.client.echo(message).await
    }

    /// Return the server status.
    pub async fn status(&self) -> Result<Value> {
        self.client.status().await
    }

    /// Convert elicited scaffold requirements into the handoff contract consumed by the skill.
    pub fn scaffold_intent(&self, input: ScaffoldIntent) -> Value {
        let category = normalize_category(&input.server_category);
        let required_surfaces = if category == "application-platform" {
            vec!["api", "cli", "mcp", "web"]
        } else {
            vec!["mcp", "cli"]
        };
        let service_name = input.binary_name.trim().replace('-', "_");
        let env_prefix = input.env_prefix.trim().to_ascii_uppercase();

        json!({
            "kind": "rmcp_template_scaffold_intent",
            "schema_version": 1,
            "server_category": category,
            "required_surfaces": required_surfaces,
            "project": {
                "display_name": input.display_name.trim(),
                "crate_name": input.crate_name.trim(),
                "binary_name": input.binary_name.trim(),
                "service_name": service_name,
                "env_prefix": env_prefix,
            },
            "upstream": {
                "base_url_env": format!("{env_prefix}_API_URL"),
                "auth_kind": normalize_auth_kind(&input.auth_kind),
            },
            "runtime": {
                "host": normalize_host(&input.host),
                "port": input.port,
                "mcp_transport": normalize_transport(&input.mcp_transport),
            },
            "mcp_primitives": normalize_primitives(&input.mcp_primitives),
            "deployment": normalize_deployment(&input.deployment),
            "plugins": normalize_plugins(&input.plugins),
            "publish_mcp": input.publish_mcp,
            "crawl_docs": {
                "urls": split_csv(&input.crawl_urls),
                "repos": split_csv(&input.crawl_repos),
                "search_topics": split_csv(&input.crawl_search_topics),
            },
            "handoff": {
                "recommended_skill": "scaffold-project",
                "instructions": "Create an approval-first scaffold plan from this JSON. Do not mutate files until the user approves the plan.",
            },
            "policy": {
                "business_action_minimum_surfaces": ["mcp", "cli"],
                "upstream_client_surfaces": ["mcp", "cli"],
                "application_platform_surfaces": ["api", "cli", "mcp", "web"],
            }
        })
    }
}

fn normalize_category(category: &str) -> &'static str {
    let normalized = category.trim().to_ascii_lowercase();
    if normalized.contains("application") || normalized.contains("platform") {
        "application-platform"
    } else {
        "upstream-client"
    }
}

fn normalize_auth_kind(value: &str) -> &'static str {
    match value.trim().to_ascii_lowercase().as_str() {
        "none" => "none",
        "api-key" | "apikey" | "api_key" | "api key" | "key" => "api-key",
        "bearer" | "token" => "bearer",
        "oauth" => "oauth",
        "both" => "both",
        _ => "other",
    }
}

fn normalize_host(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        "127.0.0.1".to_owned()
    } else {
        trimmed.to_owned()
    }
}

fn normalize_transport(value: &str) -> &'static str {
    match value.trim().to_ascii_lowercase().as_str() {
        "stdio" => "stdio",
        "http" | "streamable-http" | "streamable_http" => "http",
        _ => "dual",
    }
}

fn normalize_deployment(value: &str) -> &'static str {
    match value.trim().to_ascii_lowercase().as_str() {
        "systemd" => "systemd",
        "docker" | "container" | "containers" => "docker",
        _ => "none",
    }
}

fn normalize_primitives(value: &str) -> Vec<String> {
    let requested = split_csv(value);
    let mut primitives = Vec::new();
    for item in requested {
        let primitive = match item.to_ascii_lowercase().as_str() {
            "tools" | "tool" => Some("tools"),
            "resources" | "resource" => Some("resources"),
            "prompts" | "prompt" => Some("prompts"),
            "elicitation" | "elicit" => Some("elicitation"),
            _ => None,
        };
        if let Some(primitive) = primitive {
            let primitive = primitive.to_owned();
            if !primitives.contains(&primitive) {
                primitives.push(primitive);
            }
        }
    }
    if primitives.is_empty() {
        primitives.push("tools".to_owned());
    }
    primitives
}

fn normalize_plugins(value: &str) -> Vec<String> {
    let requested = split_csv(value);
    let mut plugins = Vec::new();
    for item in requested {
        let plugin = match item.to_ascii_lowercase().as_str() {
            "claude" | "claude-code" | "claude_code" => Some("claude"),
            "codex" => Some("codex"),
            "gemini" => Some("gemini"),
            "none" => None,
            _ => None,
        };
        if let Some(plugin) = plugin {
            let plugin = plugin.to_owned();
            if !plugins.contains(&plugin) {
                plugins.push(plugin);
            }
        }
    }
    plugins
}

fn split_csv(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

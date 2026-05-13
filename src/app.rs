//! Business service layer.
//!
//! **All business logic lives here.** CLI and MCP are thin shims that call into this.
//!
//! `ExampleService` owns an `ExampleClient` and exposes typed methods.
//! If you need caching, retries, data transformation, or validation, do it here —
//! never in `cli.rs` or `mcp/tools.rs`.

use anyhow::Result;
use serde_json::Value;

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
}

//! Projection of gateway routes into concrete `rmcp::model` types.
//!
//! [`super::mcp_routes`] returns loose route/descriptor structs (reused
//! directly from `soma-mcp-proxy`). A caller that wants to hand these routes
//! to its own inbound MCP clients — rather than just tracking route names —
//! needs real `rmcp::model` objects. This module builds those directly on
//! `soma-mcp-server`'s generic descriptor projection, so the gateway does not
//! reach for `rmcp` server-side types on its own; it composes the role crate
//! that owns that conversion.

use rmcp::model::{Prompt, Resource, Tool};
use soma_mcp_server::protocol::{
    prompt_from_descriptor, resource_from_descriptor, tool_from_descriptor,
};

use super::{GatewayManager, GatewayManagerError};

impl GatewayManager {
    pub async fn rmcp_tool_routes(&self) -> Result<Vec<Tool>, GatewayManagerError> {
        self.rmcp_tool_routes_for_subject(None).await
    }

    pub async fn rmcp_tool_routes_for_subject(
        &self,
        subject: Option<&str>,
    ) -> Result<Vec<Tool>, GatewayManagerError> {
        Ok(self
            .tool_routes_for_subject(subject)
            .await?
            .into_iter()
            .map(|route| {
                tool_from_descriptor(
                    route.name,
                    route.descriptor.description,
                    route.descriptor.input_schema,
                    route.descriptor.output_schema,
                    route.descriptor.destructive,
                )
            })
            .collect())
    }

    pub async fn rmcp_resource_routes(&self) -> Result<Vec<Resource>, GatewayManagerError> {
        self.rmcp_resource_routes_for_subject(None).await
    }

    pub async fn rmcp_resource_routes_for_subject(
        &self,
        subject: Option<&str>,
    ) -> Result<Vec<Resource>, GatewayManagerError> {
        Ok(self
            .resource_routes_for_subject(subject)
            .await?
            .into_iter()
            .map(|route| {
                let name = route
                    .descriptor
                    .name
                    .unwrap_or_else(|| route.native_uri.clone());
                resource_from_descriptor(route.uri, name)
            })
            .collect())
    }

    pub async fn rmcp_prompt_routes(&self) -> Result<Vec<Prompt>, GatewayManagerError> {
        self.rmcp_prompt_routes_for_subject(None).await
    }

    pub async fn rmcp_prompt_routes_for_subject(
        &self,
        subject: Option<&str>,
    ) -> Result<Vec<Prompt>, GatewayManagerError> {
        Ok(self
            .prompt_routes_for_subject(subject)
            .await?
            .into_iter()
            .map(|route| {
                prompt_from_descriptor(route.name, route.descriptor.description.as_deref())
            })
            .collect())
    }
}

#[cfg(test)]
#[path = "mcp_projection_tests.rs"]
mod tests;

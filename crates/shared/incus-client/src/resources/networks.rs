//! Network CRUD.

use serde::Deserialize;

use crate::error::{Error, Result};
use crate::operations::{operation_from_envelope, Operation};
use crate::transport::{Client, Method};

#[derive(Debug, Clone, Deserialize)]
pub struct Network {
    pub name: String,
    #[serde(rename = "type")]
    pub network_type: String,
    pub managed: bool,
    pub status: String,
    #[serde(default)]
    pub config: serde_json::Value,
}

impl Client {
    pub async fn list_networks(&self, recursion: bool) -> Result<Vec<serde_json::Value>> {
        let recursion_value = recursion.to_string();
        let query = [("recursion", recursion_value.as_str())];
        let envelope = self
            .request(Method::Get, "/1.0/networks", &query, None, None)
            .await?;
        match envelope {
            crate::transport::IncusEnvelope::Sync { metadata, .. } => {
                Ok(serde_json::from_value(metadata)?)
            }
            other => Err(Error::InvalidResponse(format!(
                "expected a sync list response, got {other:?}"
            ))),
        }
    }

    pub async fn get_network(&self, name: &str) -> Result<Network> {
        let path = format!("/1.0/networks/{name}");
        let envelope = self.request(Method::Get, &path, &[], None, None).await?;
        match envelope {
            crate::transport::IncusEnvelope::Sync { metadata, .. } => {
                Ok(serde_json::from_value(metadata)?)
            }
            other => Err(Error::InvalidResponse(format!(
                "expected a sync network response, got {other:?}"
            ))),
        }
    }

    /// Always async, per the crate-wide mutation-return convention (some
    /// network backends, e.g. OVN, provision asynchronously; treat every
    /// resource type uniformly rather than special-casing this one).
    pub async fn create_network(&self, params: &serde_json::Value) -> Result<Operation> {
        let envelope = self
            .request(Method::Post, "/1.0/networks", &[], Some(params), None)
            .await?;
        operation_from_envelope(envelope)
    }

    pub async fn update_network(
        &self,
        name: &str,
        new_definition: &serde_json::Value,
    ) -> Result<Operation> {
        let path = format!("/1.0/networks/{name}");
        let envelope = self
            .request(Method::Put, &path, &[], Some(new_definition), None)
            .await?;
        operation_from_envelope(envelope)
    }

    pub async fn delete_network(&self, name: &str) -> Result<Operation> {
        let path = format!("/1.0/networks/{name}");
        let envelope = self.request(Method::Delete, &path, &[], None, None).await?;
        operation_from_envelope(envelope)
    }
}

#[cfg(test)]
#[path = "networks_tests.rs"]
mod tests;

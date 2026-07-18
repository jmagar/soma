//! Storage pool and volume CRUD. Volumes are scoped under a pool
//! (`/1.0/storage-pools/{pool}/volumes`) - Incus has no global cross-pool
//! volumes endpoint, so "list all volumes across all pools" is inherently a
//! list-pools-then-list-volumes-per-pool fan-out on the caller's part, not a
//! gap in this crate.

use serde::Deserialize;

use crate::error::{Error, Result};
use crate::operations::{operation_from_envelope, Operation};
use crate::transport::{Client, Method};

#[derive(Debug, Clone, Deserialize)]
pub struct StoragePool {
    pub name: String,
    pub driver: String,
    pub status: String,
    #[serde(default)]
    pub config: serde_json::Value,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StorageVolume {
    pub name: String,
    #[serde(rename = "type")]
    pub volume_type: String,
    pub content_type: String,
    #[serde(default)]
    pub config: serde_json::Value,
}

impl Client {
    pub async fn list_storage_pools(&self, recursion: bool) -> Result<Vec<serde_json::Value>> {
        let recursion_value = recursion.to_string();
        let query = [("recursion", recursion_value.as_str())];
        let envelope = self
            .request(Method::Get, "/1.0/storage-pools", &query, None, None)
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

    pub async fn get_storage_pool(&self, name: &str) -> Result<StoragePool> {
        let path = format!("/1.0/storage-pools/{name}");
        let envelope = self.request(Method::Get, &path, &[], None, None).await?;
        match envelope {
            crate::transport::IncusEnvelope::Sync { metadata, .. } => {
                Ok(serde_json::from_value(metadata)?)
            }
            other => Err(Error::InvalidResponse(format!(
                "expected a sync storage pool response, got {other:?}"
            ))),
        }
    }

    pub async fn create_storage_pool(&self, params: &serde_json::Value) -> Result<Operation> {
        let envelope = self
            .request(Method::Post, "/1.0/storage-pools", &[], Some(params), None)
            .await?;
        operation_from_envelope(envelope)
    }

    pub async fn delete_storage_pool(&self, name: &str) -> Result<Operation> {
        let path = format!("/1.0/storage-pools/{name}");
        let envelope = self.request(Method::Delete, &path, &[], None, None).await?;
        operation_from_envelope(envelope)
    }

    /// Lists volumes within one pool. See the module doc comment for why
    /// there's no "list all volumes across all pools" convenience method.
    ///
    /// Per the Incus REST API, `recursion = false` returns an array of bare
    /// URL strings (not typed volume objects), so - like every other
    /// `list_*` method in this crate - this returns untyped
    /// `serde_json::Value`s rather than `Vec<StorageVolume>`. Use
    /// `recursion = true` to get full volume objects in one call, or
    /// `get_storage_volume` to fetch one volume's full object by name.
    pub async fn list_storage_volumes(
        &self,
        pool_name: &str,
        recursion: bool,
    ) -> Result<Vec<serde_json::Value>> {
        let recursion_value = recursion.to_string();
        let query = [("recursion", recursion_value.as_str())];
        let path = format!("/1.0/storage-pools/{pool_name}/volumes");
        let envelope = self.request(Method::Get, &path, &query, None, None).await?;
        match envelope {
            crate::transport::IncusEnvelope::Sync { metadata, .. } => {
                Ok(serde_json::from_value(metadata)?)
            }
            other => Err(Error::InvalidResponse(format!(
                "expected a sync list response, got {other:?}"
            ))),
        }
    }

    pub async fn create_storage_volume(
        &self,
        pool_name: &str,
        params: &serde_json::Value,
    ) -> Result<Operation> {
        let path = format!("/1.0/storage-pools/{pool_name}/volumes");
        let envelope = self
            .request(Method::Post, &path, &[], Some(params), None)
            .await?;
        operation_from_envelope(envelope)
    }

    pub async fn delete_storage_volume(
        &self,
        pool_name: &str,
        volume_type: &str,
        volume_name: &str,
    ) -> Result<Operation> {
        let path = format!("/1.0/storage-pools/{pool_name}/volumes/{volume_type}/{volume_name}");
        let envelope = self.request(Method::Delete, &path, &[], None, None).await?;
        operation_from_envelope(envelope)
    }
}

#[cfg(test)]
#[path = "storage_tests.rs"]
mod tests;

//! Image CRUD.

use serde::Deserialize;

use crate::error::{Error, Result};
use crate::operations::{operation_from_envelope, Operation};
use crate::transport::{Client, Method};

#[derive(Debug, Clone, Deserialize)]
pub struct Image {
    pub fingerprint: String,
    pub public: bool,
    pub filename: String,
    pub size: i64,
    pub architecture: String,
    pub created_at: String,
    pub uploaded_at: String,
    #[serde(default)]
    pub properties: serde_json::Value,
}

impl Client {
    /// `recursion = true` fetches every image's full object in one call;
    /// `recursion = false` returns lightweight fingerprint/URL references.
    pub async fn list_images(&self, recursion: bool) -> Result<Vec<serde_json::Value>> {
        let recursion_value = recursion.to_string();
        let query = [("recursion", recursion_value.as_str())];
        let envelope = self
            .request(Method::Get, "/1.0/images", &query, None, None)
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

    pub async fn get_image(&self, fingerprint: &str) -> Result<Image> {
        let path = format!("/1.0/images/{fingerprint}");
        let envelope = self.request(Method::Get, &path, &[], None, None).await?;
        match envelope {
            crate::transport::IncusEnvelope::Sync { metadata, .. } => {
                Ok(serde_json::from_value(metadata)?)
            }
            other => Err(Error::InvalidResponse(format!(
                "expected a sync image response, got {other:?}"
            ))),
        }
    }

    /// Always async: image import/creation is documented as a long-running
    /// operation (fetching/unpacking a source, which can be a remote URL or
    /// a large upload).
    pub async fn create_image(&self, params: &serde_json::Value) -> Result<Operation> {
        let envelope = self
            .request(Method::Post, "/1.0/images", &[], Some(params), None)
            .await?;
        operation_from_envelope(envelope)
    }

    pub async fn update_image(
        &self,
        fingerprint: &str,
        new_definition: &serde_json::Value,
    ) -> Result<Operation> {
        let path = format!("/1.0/images/{fingerprint}");
        let envelope = self
            .request(Method::Put, &path, &[], Some(new_definition), None)
            .await?;
        operation_from_envelope(envelope)
    }

    pub async fn delete_image(&self, fingerprint: &str) -> Result<Operation> {
        let path = format!("/1.0/images/{fingerprint}");
        let envelope = self.request(Method::Delete, &path, &[], None, None).await?;
        operation_from_envelope(envelope)
    }
}

#[cfg(test)]
#[path = "images_tests.rs"]
mod tests;

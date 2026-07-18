//! Project CRUD.

use serde::Deserialize;

use crate::error::{Error, Result};
use crate::operations::{operation_from_envelope, Operation};
use crate::transport::{
    precondition_failed_or, sync_metadata, Client, Method, RecursionQuery, WithEtag,
};

#[derive(Debug, Clone, Deserialize)]
pub struct Project {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub config: serde_json::Value,
}

impl Client {
    pub async fn list_projects(&self, recursion: bool) -> Result<Vec<serde_json::Value>> {
        let recursion_query = RecursionQuery::new(recursion);
        let envelope = self
            .request(
                Method::Get,
                "/1.0/projects",
                &recursion_query.as_query(),
                None,
                None,
            )
            .await?;
        Ok(serde_json::from_value(sync_metadata(envelope, "list")?)?)
    }

    /// Fetches one project by name, along with its ETag for use as a later
    /// `If-Match` precondition.
    pub async fn get_project(&self, name: &str) -> Result<WithEtag<Project>> {
        let path = format!("/1.0/projects/{name}");
        let envelope = self.request(Method::Get, &path, &[], None, None).await?;
        match envelope {
            crate::transport::IncusEnvelope::Sync { metadata, etag } => Ok(WithEtag {
                value: serde_json::from_value(metadata)?,
                etag,
            }),
            other => Err(Error::InvalidResponse(format!(
                "expected a sync project response, got {other:?}"
            ))),
        }
    }

    /// Always async, per the crate-wide mutation-return convention.
    pub async fn create_project(&self, params: &serde_json::Value) -> Result<Operation> {
        let envelope = self
            .request(Method::Post, "/1.0/projects", &[], Some(params), None)
            .await?;
        operation_from_envelope(envelope)
    }

    /// Full replacement update (PUT). `etag`, if provided, is sent as
    /// `If-Match` for optimistic concurrency; a stale ETag surfaces as
    /// `Error::PreconditionFailed`, not the generic `Error::Api`.
    pub async fn update_project(
        &self,
        name: &str,
        new_definition: &serde_json::Value,
        etag: Option<&str>,
    ) -> Result<Operation> {
        let path = format!("/1.0/projects/{name}");
        let envelope = self
            .request(Method::Put, &path, &[], Some(new_definition), etag)
            .await
            .map_err(|err| precondition_failed_or(err, name))?;
        operation_from_envelope(envelope)
    }

    pub async fn delete_project(&self, name: &str) -> Result<Operation> {
        let path = format!("/1.0/projects/{name}");
        let envelope = self.request(Method::Delete, &path, &[], None, None).await?;
        operation_from_envelope(envelope)
    }
}

#[cfg(test)]
#[path = "projects_tests.rs"]
mod tests;

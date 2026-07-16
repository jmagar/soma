use serde::Serialize;
use serde_json::Value;

use crate::ProviderSurface;

#[derive(Debug, Clone)]
pub struct ProviderCall {
    pub provider: String,
    pub action: String,
    pub params: Value,
    pub surface: ProviderSurface,
    pub snapshot_id: String,
}

impl ProviderCall {
    pub fn new(action: impl Into<String>, arguments: Value) -> Self {
        Self {
            provider: String::new(),
            action: action.into(),
            params: arguments,
            surface: ProviderSurface::Internal,
            snapshot_id: String::new(),
        }
    }

    #[must_use]
    pub fn with_surface(mut self, surface: ProviderSurface) -> Self {
        self.surface = surface;
        self
    }

    pub fn tool(&self) -> &str {
        &self.action
    }

    pub fn arguments(&self) -> &Value {
        &self.params
    }

    pub fn execution_envelope(&self) -> ProviderExecutionEnvelope {
        ProviderExecutionEnvelope {
            schema_version: 1,
            provider: self.provider.clone(),
            action: self.action.clone(),
            params: self.params.clone(),
            surface: self.surface,
            snapshot_id: self.snapshot_id.clone(),
        }
    }

    pub fn execution_payload(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec(&self.execution_envelope())
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ProviderExecutionEnvelope {
    pub schema_version: u32,
    pub provider: String,
    pub action: String,
    pub params: Value,
    pub surface: ProviderSurface,
    pub snapshot_id: String,
}

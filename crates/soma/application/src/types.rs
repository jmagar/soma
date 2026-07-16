use serde::{Deserialize, Serialize};
use serde_json::Value;
use soma_contracts::providers::ProviderCatalog;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ExecuteActionRequest {
    pub action: String,
    #[serde(default)]
    pub params: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ExecuteActionResponse {
    pub output: Value,
    pub request_id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GatewayReloadRequest {
    #[serde(default)]
    pub config: Value,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GatewayExecuteRequest {
    pub action: String,
    #[serde(default)]
    pub params: Value,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CodeModeExecuteRequest {
    pub source: String,
    #[serde(default)]
    pub input: Value,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OpenApiExecuteRequest {
    pub operation: String,
    #[serde(default)]
    pub params: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct OperationResponse {
    pub output: Value,
    pub request_id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ReadResourceRequest {
    pub uri: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ResourceContent {
    Text {
        text: String,
        mime_type: Option<String>,
    },
    Blob {
        blob_base64: String,
        mime_type: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CatalogSnapshot {
    pub id: String,
    pub fingerprint: String,
    pub catalogs: Vec<ProviderCatalog>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DoctorReport {
    pub ready: bool,
    pub status: Option<Value>,
    pub problems: Vec<String>,
}

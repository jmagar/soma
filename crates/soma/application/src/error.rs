use serde::Serialize;
use soma_service::ProviderError;

use crate::PortError;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ApplicationError {
    pub code: String,
    pub message: String,
    pub retryable: bool,
    pub remediation: String,
}

impl ApplicationError {
    pub fn new(
        code: impl Into<String>,
        message: impl Into<String>,
        retryable: bool,
        remediation: impl Into<String>,
    ) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            retryable,
            remediation: remediation.into(),
        }
    }

    pub(crate) fn legacy(operation: &str, error: impl std::fmt::Display) -> Self {
        let diagnostic = soma_service::provider_errors::redact_public(&error.to_string());
        Self::new(
            "legacy_operation_failed",
            format!("{operation} failed: {diagnostic}"),
            false,
            "Check Soma service status and retry.",
        )
    }

    pub(crate) fn not_found(kind: &str, name: &str) -> Self {
        Self::new(
            format!("{kind}_not_found"),
            format!("unknown {kind} `{name}`"),
            false,
            format!("List available {kind}s and retry with a known name."),
        )
    }
}

impl From<ProviderError> for ApplicationError {
    fn from(error: ProviderError) -> Self {
        Self::new(
            error.code.to_string(),
            error.message.to_string(),
            error.retryable,
            error.remediation.to_string(),
        )
    }
}

impl From<PortError> for ApplicationError {
    fn from(error: PortError) -> Self {
        let message = soma_service::provider_errors::redact_public(&error.message);
        Self::new(error.code, message, error.retryable, error.remediation)
    }
}

impl std::fmt::Display for ApplicationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for ApplicationError {}

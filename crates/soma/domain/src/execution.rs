use serde::{Deserialize, Serialize};

const MAX_REQUEST_ID_BYTES: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Surface {
    Mcp,
    Rest,
    Cli,
    Palette,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AuthorizationMode {
    LoopbackDev,
    TrustedGateway,
    Mounted,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Confirmation {
    #[default]
    Missing,
    Confirmed,
}

impl Confirmation {
    pub fn is_confirmed(self) -> bool {
        matches!(self, Self::Confirmed)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RequestId(String);

impl RequestId {
    pub fn new(value: impl Into<String>) -> Result<Self, RequestIdError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(RequestIdError::Empty);
        }
        if value.len() > MAX_REQUEST_ID_BYTES {
            return Err(RequestIdError::TooLong {
                actual: value.len(),
                maximum: MAX_REQUEST_ID_BYTES,
            });
        }
        if value.chars().any(char::is_control) {
            return Err(RequestIdError::ControlCharacter);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RequestIdError {
    Empty,
    TooLong { actual: usize, maximum: usize },
    ControlCharacter,
}

impl std::fmt::Display for RequestIdError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => f.write_str("request id must not be empty"),
            Self::TooLong { actual, maximum } => {
                write!(f, "request id is {actual} bytes; maximum is {maximum}")
            }
            Self::ControlCharacter => f.write_str("request id must not contain control characters"),
        }
    }
}

impl std::error::Error for RequestIdError {}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraceContext {
    pub traceparent: Option<String>,
    pub tracestate: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::{Confirmation, RequestId};

    #[test]
    fn request_ids_reject_empty_and_control_characters() {
        assert!(RequestId::new("  ").is_err());
        assert!(RequestId::new("request\n1").is_err());
        assert_eq!(RequestId::new("request-1").unwrap().as_str(), "request-1");
    }

    #[test]
    fn confirmation_defaults_to_missing() {
        assert!(!Confirmation::default().is_confirmed());
        assert!(Confirmation::Confirmed.is_confirmed());
    }
}

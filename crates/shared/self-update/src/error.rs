use std::path::PathBuf;

/// Errors returned by update policy and transaction operations.
#[derive(Debug, thiserror::Error)]
pub enum UpdateError {
    #[error("invalid update directive: {0}")]
    InvalidDirective(&'static str),
    #[error("invalid SHA-256 digest: {0}")]
    InvalidDigest(String),
    #[error("invalid update policy: {0}")]
    InvalidPolicy(&'static str),
    #[error("I/O operation failed for {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

pub type Result<T> = std::result::Result<T, UpdateError>;

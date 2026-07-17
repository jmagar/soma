use std::path::PathBuf;

/// Connection configuration for [`crate::Client`].
///
/// This epic only supports a local Unix-socket target. A `remote(url)`
/// constructor for the mutual-TLS transport is intentionally absent - see
/// the crate root doc comment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientConfig {
    pub(crate) socket_path: PathBuf,
}

impl ClientConfig {
    /// Configure a client that connects to the Incus daemon over the given
    /// Unix domain socket path (e.g. `/var/lib/incus/unix.socket`).
    #[must_use]
    pub fn unix_socket(path: impl Into<PathBuf>) -> Self {
        Self {
            socket_path: path.into(),
        }
    }
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;

use crate::protocol::RequestId;

/// Errors returned by [`crate::CodexAppServerClient`].
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to spawn `{command}`: {source}")]
    Spawn {
        command: String,
        #[source]
        source: std::io::Error,
    },

    #[error("transport I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("failed to (de)serialize a protocol message: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("the app-server connection closed before a response arrived")]
    TransportClosed,

    #[error("no response after {after:?}")]
    Timeout { after: std::time::Duration },

    #[error("app-server returned a JSON-RPC error (code {code}): {message}")]
    Rpc {
        code: i64,
        message: String,
        data: Option<serde_json::Value>,
    },

    #[error("received a response for unknown or already-resolved request id {0:?}")]
    UnknownRequestId(RequestId),

    #[error("child process exited before initialize completed")]
    ChildExited,
}

pub type Result<T> = std::result::Result<T, Error>;

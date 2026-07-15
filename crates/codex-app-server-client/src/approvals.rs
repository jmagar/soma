use crate::protocol::ServerRequest;
use crate::{PendingServerRequest, Result};

#[derive(Clone, Debug, PartialEq)]
pub enum ServerRequestReply {
    Result(serde_json::Value),
    Error {
        code: i64,
        message: String,
        data: Option<serde_json::Value>,
    },
}

impl ServerRequestReply {
    pub fn send(self, request: PendingServerRequest) -> Result<()> {
        match self {
            Self::Result(result) => request.respond(result),
            Self::Error {
                code,
                message,
                data,
            } => {
                request.respond_error(code, message, data);
                Ok(())
            }
        }
    }
}

pub trait ApprovalHandler: Send + Sync {
    fn handle(&self, request: &ServerRequest) -> ServerRequestReply;
}

#[derive(Clone, Debug)]
pub struct DenyAllApprovalHandler {
    code: i64,
    message: String,
}

impl Default for DenyAllApprovalHandler {
    fn default() -> Self {
        Self {
            code: -32000,
            message: "codex-app-server-client: no approval handler accepted this request"
                .to_owned(),
        }
    }
}

impl DenyAllApprovalHandler {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            ..Self::default()
        }
    }
}

impl ApprovalHandler for DenyAllApprovalHandler {
    fn handle(&self, _request: &ServerRequest) -> ServerRequestReply {
        ServerRequestReply::Error {
            code: self.code,
            message: self.message.clone(),
            data: None,
        }
    }
}

pub struct FnApprovalHandler<F>(F);

impl<F> FnApprovalHandler<F>
where
    F: Fn(&ServerRequest) -> ServerRequestReply + Send + Sync,
{
    pub fn new(handler: F) -> Self {
        Self(handler)
    }
}

impl<F> ApprovalHandler for FnApprovalHandler<F>
where
    F: Fn(&ServerRequest) -> ServerRequestReply + Send + Sync,
{
    fn handle(&self, request: &ServerRequest) -> ServerRequestReply {
        (self.0)(request)
    }
}

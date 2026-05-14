//! MCP protocol layer — tool dispatch, schemas, prompts, and server handler.
//!
//! This module is strictly MCP concerns: the `ServerHandler` impl, tool schemas,
//! prompt templates, and dispatch shims. Application state lives in `crate::server`.

mod prompts;
pub mod rmcp_server;
mod schemas;
mod tools;

pub use rmcp_server::{
    allowed_origins, rmcp_server, streamable_http_config, streamable_http_service,
    ExampleRmcpServer,
};

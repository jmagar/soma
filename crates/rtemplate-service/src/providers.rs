pub mod ai_sdk;
pub mod filesystem;
pub mod mcp;
pub mod openapi;
pub mod static_rust;
pub mod wasm;

#[cfg(test)]
#[path = "providers/filesystem_tests.rs"]
mod filesystem_tests;

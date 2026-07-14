//! Generated types for the Codex app-server v2 JSON-RPC protocol.
//!
//! Everything in this module is generated at build time by `build.rs` (via
//! `typify`) from `schema/protocol.schema.json`. See the crate README for how
//! that schema was derived and how to regenerate it against a newer `codex`
//! CLI version.
#![allow(
    clippy::all,
    dead_code,
    non_camel_case_types,
    non_snake_case,
    missing_docs
)]

include!(concat!(env!("OUT_DIR"), "/protocol_generated.rs"));

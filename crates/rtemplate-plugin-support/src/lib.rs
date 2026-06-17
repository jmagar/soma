//! Plugin setup support crate.
//!
//! The concrete setup command implementation currently lives in `rtemplate-cli`
//! because it shares CLI command parsing and diagnostics. This crate is kept as
//! the reusable home for plugin hook contracts as they are extracted.

pub use rtemplate_contracts::env_registry;

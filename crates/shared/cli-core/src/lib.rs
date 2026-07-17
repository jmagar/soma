//! Reusable CLI plumbing: output formats, table/JSON rendering, confirmation
//! I/O, terminal/color capability policy, shell completions, and progress
//! helpers.
//!
//! This crate is transport- and product-neutral. It has no knowledge of
//! Soma commands, action names, scopes, or exit-code policy — a consuming
//! CLI supplies its own parser and wording and calls into these primitives
//! for the generic mechanics.

#![forbid(unsafe_code)]

pub mod color;
pub mod common_args;
pub mod completion;
pub mod confirmation;
pub mod error;
pub mod json;
pub mod output;
pub mod progress;
pub mod table;
pub mod terminal;

//! Concrete outbound transport for a remote Soma HTTP server.
//!
//! `soma-client` owns HTTP request construction, remote response decoding, and
//! transport-level retries/timeouts for talking to a deployed `soma serve`
//! REST API. It does not decide *when* a request should go upstream (that is
//! application policy) and it has no CLI, provider registry, or validation
//! logic of its own — see plan section 3.19.

mod client;

pub use client::SomaClient;

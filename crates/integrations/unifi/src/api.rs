//! Path/URL builders for the two UniFi controller APIs this crate speaks.

pub mod internal;
pub mod official;
pub mod path;

/// Which controller API a [`crate::capabilities::Capability`] is served from.
///
/// `#[non_exhaustive]`: callers only ever read this (via a capability's
/// [`source`](crate::capabilities::Capability::source) field or a `match`),
/// never construct it — a future 4th source family should not be a
/// downstream semver break.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ApiSourceFamily {
    /// UniFi's documented `/proxy/network/integration` REST API.
    Official,
    /// The controller's own internal (undocumented, but stable) web-UI API.
    Internal,
    /// Resolves to [`Official`](Self::Official) or [`Internal`](Self::Internal)
    /// at call time — see [`crate::actions::hybrid`].
    Hybrid,
}

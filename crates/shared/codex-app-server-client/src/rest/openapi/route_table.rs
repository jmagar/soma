//! The single source-of-truth route table: which HTTP method + path pairs
//! exist, how each is gated by [`RestRouterOptions`](crate::rest::types::RestRouterOptions),
//! and the concrete probe request used to verify each is actually mounted.
//! See [`super::paths`] for the OpenAPI operation bodies built from this
//! table, and `super::tests` (in `super::super`, i.e. `openapi.rs`) for the
//! coverage test that issues real HTTP requests against [`ROUTES`].

/// How a route is gated by [`RestRouterOptions`](crate::rest::types::RestRouterOptions).
///
/// Mirrors the two independent opt-in flags `routes.rs` actually checks
/// (`enable_text_turn_route`, `enable_bridge_routes`) so a spec reader can
/// tell, per route, which router constructor(s) mount it - a spec that
/// implied every route is always present (as a flat OpenAPI document with
/// no such distinction would) misrepresents `rest::router()`'s
/// intentionally non-executing default.
#[derive(Clone, Copy)]
pub(super) enum RouteGate {
    /// Mounted by every router constructor, including the bare `rest::router()`.
    Always,
    /// Mounted only when `RestRouterOptions::enable_text_turn_route` is
    /// `true` - `rest::text_turn_router()` or `RestRouterOptions::text_turn()`/
    /// `::trusted_bridge()`.
    TextTurn,
    /// Mounted only when `RestRouterOptions::enable_bridge_routes` is `true`
    /// - `rest::trusted_bridge_router()` or `RestRouterOptions::trusted_bridge()`.
    Bridge,
}

impl RouteGate {
    pub(super) fn note(self) -> &'static str {
        match self {
            RouteGate::Always => "",
            RouteGate::TextTurn => {
                " Only mounted when `RestRouterOptions::enable_text_turn_route` is `true` \
                 (`rest::text_turn_router()`, or `RestRouterOptions::text_turn()` / \
                 `::trusted_bridge()`) - the bare `rest::router()` does not expose this route."
            }
            RouteGate::Bridge => {
                " Only mounted when `RestRouterOptions::enable_bridge_routes` is `true` \
                 (`rest::trusted_bridge_router()`, or `RestRouterOptions::trusted_bridge()`) - \
                 neither `rest::router()` nor `rest::text_turn_router()` exposes this route. \
                 **Never mount this behind a public/untrusted boundary without your own \
                 authentication layer** (e.g. `rest::bearer_auth`) - see this document's \
                 top-level description."
            }
        }
    }
}

/// One documented HTTP method + path pair, and the concrete probe request
/// used to verify it's actually mounted.
///
/// This is the single shared table [`openapi_spec`](super::openapi_spec) and
/// the coverage test in `super::tests::every_documented_route_is_actually_mounted`
/// both read - deliberately, so there is exactly one place to edit when
/// `routes.rs` gains, loses, or moves a route. See that test's doc comment
/// for what this table does and does not protect against (axum 0.8's
/// `Router` has no public route-enumeration API, which is *why* a shared
/// table is the chosen fallback instead of introspecting the live router).
pub(super) struct RouteDef {
    /// Lowercase HTTP method, doubling as the OpenAPI `paths.<path>.<method>` key.
    pub(super) method: &'static str,
    /// OpenAPI-style path template (`{param}` placeholders), also the `paths` object key.
    pub(super) path_template: &'static str,
    /// A concrete, requestable path with real path-parameter values plugged
    /// in, used only by the coverage test to issue an actual HTTP request.
    /// Legitimately dead in a non-test build (nothing outside
    /// `#[cfg(test)] mod tests` reads it) - not dead in the sense
    /// `#[warn(dead_code)]` exists to catch.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) probe_path: &'static str,
    pub(super) gate: RouteGate,
}

pub(super) const ROUTES: &[RouteDef] = &[
    RouteDef {
        method: "get",
        path_template: "/health",
        probe_path: "/health",
        gate: RouteGate::Always,
    },
    RouteDef {
        method: "get",
        path_template: "/v1/health",
        probe_path: "/v1/health",
        gate: RouteGate::Always,
    },
    RouteDef {
        method: "get",
        path_template: "/v1/compatibility",
        probe_path: "/v1/compatibility",
        gate: RouteGate::Always,
    },
    RouteDef {
        method: "post",
        path_template: "/v1/text-turn",
        probe_path: "/v1/text-turn",
        gate: RouteGate::TextTurn,
    },
    RouteDef {
        method: "post",
        path_template: "/v1/call/{method}",
        probe_path: "/v1/call/thread/start",
        gate: RouteGate::Bridge,
    },
    RouteDef {
        method: "get",
        path_template: "/v1/sessions",
        probe_path: "/v1/sessions",
        gate: RouteGate::Bridge,
    },
    RouteDef {
        method: "post",
        path_template: "/v1/sessions",
        probe_path: "/v1/sessions",
        gate: RouteGate::Bridge,
    },
    RouteDef {
        method: "delete",
        path_template: "/v1/sessions/{sessionId}",
        probe_path: "/v1/sessions/session-test",
        gate: RouteGate::Bridge,
    },
    RouteDef {
        method: "post",
        path_template: "/v1/sessions/{sessionId}/call/{method}",
        probe_path: "/v1/sessions/session-test/call/thread/start",
        gate: RouteGate::Bridge,
    },
    RouteDef {
        method: "get",
        path_template: "/v1/sessions/{sessionId}/events",
        probe_path: "/v1/sessions/session-test/events",
        gate: RouteGate::Bridge,
    },
    RouteDef {
        method: "get",
        path_template: "/v1/sessions/{sessionId}/events/stream",
        probe_path: "/v1/sessions/session-test/events/stream",
        gate: RouteGate::Bridge,
    },
    RouteDef {
        method: "post",
        path_template: "/v1/sessions/{sessionId}/requests/{requestKey}/result",
        probe_path: "/v1/sessions/session-test/requests/request-test/result",
        gate: RouteGate::Bridge,
    },
    RouteDef {
        method: "post",
        path_template: "/v1/sessions/{sessionId}/requests/{requestKey}/error",
        probe_path: "/v1/sessions/session-test/requests/request-test/error",
        gate: RouteGate::Bridge,
    },
];

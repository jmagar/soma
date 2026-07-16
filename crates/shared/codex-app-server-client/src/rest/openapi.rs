//! OpenAPI 3.1.0 document for the `rest` module's HTTP surface.
//!
//! [`openapi_spec`] hand-builds a `serde_json::Value` describing every route
//! in [`super::routes`] rather than deriving it from the Rust types via a
//! schema-generation crate (`schemars`, `utoipa`, ...). That's a deliberate
//! trade-off, not an oversight: this crate promises zero path-dependencies
//! and a minimal, audited `crates.io` dependency footprint (see
//! `README.md`), and every schema-derive crate considered pulls in either a
//! proc-macro-heavy dependency tree or its own opinionated JSON Schema
//! dialect that doesn't map onto OpenAPI 3.1 cleanly (`serde`'s
//! `rename_all_fields` internally-tagged enums in particular - see
//! [`RestEventResponse`](super::types::RestEventResponse) - have no clean
//! derive-crate story as of this writing). Hand-writing means every schema
//! below is transcribed by reading `src/rest/types.rs` and `src/compat.rs`
//! directly; the tests at the bottom of this file exist specifically to
//! catch that transcription drifting from the real wire format or the real
//! mounted routes.
//!
//! # Determinism
//!
//! This crate does not enable `serde_json`'s `preserve_order` feature, so in
//! an ordinary standalone build `serde_json::Map` is backed by a
//! `BTreeMap` and serializes with sorted keys automatically. But Cargo
//! unifies feature flags across an entire build's unit graph: when this
//! crate is built as part of the full `soma` workspace (rather than in
//! isolation with `cargo test -p codex-app-server-client`), sibling crates
//! that *do* enable `preserve_order` (see `crates/shared/openapi/Cargo.toml`,
//! `crates/shared/codemode/Cargo.toml`) flip `serde_json::Map` to an
//! insertion-order-preserving `IndexMap` for every crate in that build,
//! including this one - see `xtask/Cargo.toml`'s comment on the same
//! incident for prior art. [`obj`] below builds every JSON object in this
//! module by sorting its entries before insertion, so
//! [`openapi_spec`]'s serialized output is byte-identical either way.
//! [`serde_json::json!`] is still used freely for arrays and scalar leaves,
//! where element order is meaningful (arrays) or there's nothing to order
//! (leaves) - only object-shaped literals go through [`obj`].

use serde_json::{json, Value};

/// Builds a JSON object from `entries`, sorting by key first so the result
/// is identical regardless of whether the ambient build's `serde_json::Map`
/// happens to preserve insertion order or not. See the module docs'
/// "Determinism" section for why this matters here specifically. Every
/// object-shaped value in this module is built through this function.
fn obj(mut entries: Vec<(&'static str, Value)>) -> Value {
    entries.sort_by(|a, b| a.0.cmp(b.0));
    let mut map = serde_json::Map::with_capacity(entries.len());
    for (key, value) in entries {
        map.insert(key.to_owned(), value);
    }
    Value::Object(map)
}

/// A `{"$ref": "#/components/schemas/<name>"}` pointer.
fn schema_ref(name: &str) -> Value {
    obj(vec![(
        "$ref",
        json!(format!("#/components/schemas/{name}")),
    )])
}

/// An unconstrained JSON Schema (no `type` keyword: matches any JSON value),
/// used for the crate's several `serde_json::Value`-typed fields where the
/// real shape is whatever the underlying `codex app-server` JSON-RPC method
/// happens to return - this crate deliberately does not attempt to model
/// that per-method surface (see README.md on the typed [`protocol`](crate::protocol)
/// layer being the place that happens, not the REST adapter).
fn any_value_schema(description: &str) -> Value {
    obj(vec![("description", json!(description))])
}

fn string_schema(description: &str) -> Value {
    obj(vec![
        ("type", json!("string")),
        ("description", json!(description)),
    ])
}

/// A `string` schema whose Rust field is `Option<String>` with no
/// `skip_serializing_if`, meaning it is always present on the wire but may
/// be JSON `null` - OpenAPI 3.1 models that as `"type": ["string", "null"]`
/// rather than the 3.0-era `nullable: true` (dropped in 3.1 in favor of
/// JSON Schema's own type-array convention).
fn nullable_string_schema(description: &str) -> Value {
    obj(vec![
        ("type", json!(["string", "null"])),
        ("description", json!(description)),
    ])
}

fn integer_schema(description: &str) -> Value {
    obj(vec![
        ("type", json!("integer")),
        ("description", json!(description)),
    ])
}

fn nonneg_integer_schema(description: &str) -> Value {
    obj(vec![
        ("type", json!("integer")),
        ("minimum", json!(0)),
        ("description", json!(description)),
    ])
}

fn array_schema(items: Value, description: &str) -> Value {
    obj(vec![
        ("type", json!("array")),
        ("items", items),
        ("description", json!(description)),
    ])
}

/// Builds an `object` JSON Schema. `additional_properties` is `Some(false)`
/// exactly when the Rust struct carries `#[serde(deny_unknown_fields)]`;
/// `None` leaves `additionalProperties` unset (permissive default) for
/// structs that don't - getting this wrong in either direction would
/// misrepresent what the route layer actually accepts, which is the whole
/// point of this file (see the module docs).
fn object_schema(
    properties: Vec<(&'static str, Value)>,
    required: &[&'static str],
    additional_properties: Option<bool>,
) -> Value {
    let mut entries = vec![("type", json!("object")), ("properties", obj(properties))];
    if !required.is_empty() {
        entries.push(("required", json!(required)));
    }
    if let Some(allowed) = additional_properties {
        entries.push(("additionalProperties", json!(allowed)));
    }
    obj(entries)
}

/// A JSON `application/json` request body wrapper.
fn json_request_body(description: &str, schema_name: &str, required: bool) -> Value {
    obj(vec![
        ("description", json!(description)),
        ("required", json!(required)),
        (
            "content",
            obj(vec![(
                "application/json",
                obj(vec![("schema", schema_ref(schema_name))]),
            )]),
        ),
    ])
}

/// A `200` (or other success) `application/json` response.
fn json_response(description: &str, schema_name: &str) -> (&'static str, Value) {
    (
        "200",
        obj(vec![
            ("description", json!(description)),
            (
                "content",
                obj(vec![(
                    "application/json",
                    obj(vec![("schema", schema_ref(schema_name))]),
                )]),
            ),
        ]),
    )
}

/// A `text/event-stream` success response for the one SSE route.
fn sse_response(description: &str) -> (&'static str, Value) {
    (
        "200",
        obj(vec![
            ("description", json!(description)),
            (
                "content",
                obj(vec![(
                    "text/event-stream",
                    obj(vec![
                        ("schema", schema_ref("RestEventResponse")),
                        (
                            "description",
                            json!(
                                "One SSE `data:` frame per event, `event:` set to the payload's \
                                 own `event` discriminant (`notification` | `request` | \
                                 `closed` | `timeout`). See `RestEventResponse` - the frame body \
                                 is that exact JSON shape, not wrapped further."
                            ),
                        ),
                    ]),
                )]),
            ),
        ]),
    )
}

/// A non-2xx `application/json` error response using [`RestErrorResponse`](super::types::RestErrorResponse).
fn error_response(status: &'static str, description: &str) -> (&'static str, Value) {
    (status, json_response(description, "RestErrorResponse").1)
}

/// The `401` response documented on every operation except the two health
/// routes. Not one of the codes [`super::routes::rest_error_response`]
/// emits - it comes from the *optional* [`super::auth::bearer_auth`] layer,
/// which is not mounted by default. Listed here (rather than omitted, since
/// strictly nothing in `router_with_options` alone can 401) because a spec
/// consumer integrating against a real deployment needs to know 401 is
/// possible the moment an operator opts into `bearer_auth` - see this
/// document's top-level `info.description`.
fn unauthorized_response() -> (&'static str, Value) {
    (
        "401",
        obj(vec![
            (
                "description",
                json!(
                    "Missing or invalid `Authorization: Bearer <token>` header. Only returned \
                     when the router is wrapped in `rest::bearer_auth(...)` - the base router \
                     has no built-in auth and never returns this on its own."
                ),
            ),
            (
                "content",
                obj(vec![(
                    "application/json",
                    obj(vec![("schema", schema_ref("RestErrorResponse"))]),
                )]),
            ),
        ]),
    )
}

fn session_id_param() -> Value {
    obj(vec![
        ("name", json!("sessionId")),
        ("in", json!("path")),
        ("required", json!(true)),
        ("schema", obj(vec![("type", json!("string"))])),
        (
            "description",
            json!(
                "REST bridge session identifier returned as `sessionId` by `POST /v1/sessions` \
                 (the built-in `CodexRestBackend` mints values shaped like \
                 `session-<uuid-v4-simple>`, but that format is not a contract - callers must \
                 treat it as an opaque token)."
            ),
        ),
    ])
}

/// The `{method}` path parameter. Deliberately documented as *not* a
/// conventional single-segment OpenAPI path parameter: the underlying axum
/// route uses a `{*method}` catch-all (see `routes.rs`), because a real
/// `codex app-server` JSON-RPC method name is namespaced with a literal `/`
/// (`thread/start`, `config/read`, ...). Most OpenAPI tooling assumes
/// `{param}` matches exactly one path segment with no `/`; that assumption
/// is false for this parameter, which is exactly the "represent that
/// honestly" case called out in this crate's REST adapter notes. There is
/// no strictly-correct OpenAPI 3.1 way to express "one path parameter that
/// itself may contain literal slashes" - this documents the true behavior
/// in prose rather than picking a technically-valid-but-misleading schema.
fn method_param() -> Value {
    obj(vec![
        ("name", json!("method")),
        ("in", json!("path")),
        ("required", json!(true)),
        ("schema", obj(vec![("type", json!("string"))])),
        (
            "description",
            json!(
                "Full `codex app-server` JSON-RPC method name, e.g. `thread/start` or \
                 `config/read`. IMPORTANT: this is captured by an axum `{*method}` wildcard, \
                 not a conventional single-segment path parameter - the value legitimately \
                 contains literal `/` characters, so naive path-templating clients that escape \
                 `/` in path parameters will build the wrong URL. A leading/trailing `/` on the \
                 captured value is trimmed server-side before use."
            ),
        ),
    ])
}

fn request_key_param() -> Value {
    obj(vec![
        ("name", json!("requestKey")),
        ("in", json!("path")),
        ("required", json!(true)),
        ("schema", obj(vec![("type", json!("string"))])),
        (
            "description",
            json!(
                "Opaque key returned as `requestKey` on a `\"event\": \"request\"` payload from \
                 `GET .../events` or `GET .../events/stream`. Answers exactly one pending \
                 server-originated request and is single-use - a second reply attempt with the \
                 same key returns `404`."
            ),
        ),
    ])
}

fn timeout_ms_param() -> Value {
    obj(vec![
        ("name", json!("timeoutMs")),
        ("in", json!("query")),
        ("required", json!(false)),
        (
            "schema",
            obj(vec![("type", json!("integer")), ("minimum", json!(0))]),
        ),
        (
            "description",
            json!(
                "Long-poll budget in milliseconds. Defaults to, and is clamped down to, the \
                 server's configured `RestLimits::max_poll_timeout` (default 30000ms, overridable \
                 via `CODEX_APP_SERVER_REST_MAX_POLL_TIMEOUT_MS`) - a caller-requested value \
                 above that ceiling is silently lowered to it, never rejected."
            ),
        ),
    ])
}

/// How a route is gated by [`RestRouterOptions`](super::types::RestRouterOptions).
///
/// Mirrors the two independent opt-in flags `routes.rs` actually checks
/// (`enable_text_turn_route`, `enable_bridge_routes`) so a spec reader can
/// tell, per route, which router constructor(s) mount it - a spec that
/// implied every route is always present (as a flat OpenAPI document with
/// no such distinction would) misrepresents `rest::router()`'s
/// intentionally non-executing default.
#[derive(Clone, Copy)]
enum RouteGate {
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
    fn note(self) -> &'static str {
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
/// This is the single shared table [`openapi_spec`] and the coverage test
/// in [`tests::every_documented_route_is_actually_mounted`] both read -
/// deliberately, so there is exactly one place to edit when `routes.rs`
/// gains, loses, or moves a route. See that test's doc comment for what
/// this table does and does not protect against (axum 0.8's `Router` has no
/// public route-enumeration API, which is *why* a shared table is the
/// chosen fallback instead of introspecting the live router).
struct RouteDef {
    /// Lowercase HTTP method, doubling as the OpenAPI `paths.<path>.<method>` key.
    method: &'static str,
    /// OpenAPI-style path template (`{param}` placeholders), also the `paths` object key.
    path_template: &'static str,
    /// A concrete, requestable path with real path-parameter values plugged
    /// in, used only by the coverage test to issue an actual HTTP request.
    /// Legitimately dead in a non-test build (nothing outside
    /// `#[cfg(test)] mod tests` reads it) - not dead in the sense
    /// `#[warn(dead_code)]` exists to catch.
    #[cfg_attr(not(test), allow(dead_code))]
    probe_path: &'static str,
    gate: RouteGate,
}

const ROUTES: &[RouteDef] = &[
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

/// Builds the full OpenAPI operation object for one [`RouteDef`]. Keyed on
/// `(method, path_template)` and panics on an unmapped combination -
/// deliberately, so adding a row to [`ROUTES`] without also writing its
/// operation body fails loudly at spec-build time (every call to
/// `openapi_spec()`, including in tests) instead of silently emitting a
/// route with no operation.
fn operation_for(route: &RouteDef) -> Value {
    let auth_note = unauthorized_response();
    match (route.method, route.path_template) {
        ("get", "/health") | ("get", "/v1/health") => obj(vec![
            ("summary", json!("Liveness probe")),
            (
                "description",
                json!("Always returns `200`. Never requires authentication, even when `rest::bearer_auth` is layered on (see `BearerAuthLayer::allow_unauthenticated_health`, default `true`)."),
            ),
            ("operationId", json!(format!("get{}", route.path_template.replace(['/', '-'], "_")))),
            (
                "responses",
                obj(vec![json_response(
                    "The process is up.",
                    "RestHealthResponse",
                )]),
            ),
        ]),
        ("get", "/v1/compatibility") => obj(vec![
            ("summary", json!("Schema/installed-version compatibility report")),
            (
                "description",
                json!(
                    "Reports the vendored protocol schema's Codex version, the locally \
                     installed `codex --version` (if any), and a generated method-count \
                     summary. Requires authentication when `rest::bearer_auth` is layered on \
                     (unlike `/health`) since it reveals environment details."
                ),
            ),
            ("operationId", json!("getV1Compatibility")),
            (
                "responses",
                obj(vec![
                    json_response("Compatibility report.", "CompatibilityReport"),
                    error_response(
                        "500",
                        "The backend's compatibility check failed (e.g. the check task panicked or was cancelled).",
                    ),
                    auth_note.clone(),
                ]),
            ),
        ]),
        ("post", "/v1/text-turn") => obj(vec![
            ("summary", json!("One-shot text turn")),
            (
                "description",
                json!(format!(
                    "Starts a fresh, ephemeral Codex session, sends one text prompt, waits for \
                     turn completion (bounded by `RestLimits::max_text_turn_duration`), and \
                     returns the assistant's message plus the latest diff and any turn errors. \
                     Does not preserve session state across calls - use the stateful bridge \
                     routes (`POST /v1/sessions`, ...) for multi-turn conversations.{}",
                    route.gate.note()
                )),
            ),
            ("operationId", json!("postV1TextTurn")),
            (
                "requestBody",
                json_request_body(
                    "The prompt and optional model/approval/client overrides.",
                    "RestTextTurnRequest",
                    true,
                ),
            ),
            (
                "responses",
                obj(vec![
                    json_response("The turn reached a terminal state.", "RestTextTurnResponse"),
                    error_response("400", "Malformed JSON body, or `prompt` is empty/whitespace-only."),
                    error_response(
                        "403",
                        "`approvalPolicy: \"allow_all\"` or a `client.command`/`extraArgs`/`config` \
                         override was requested without `RestRouterOptions::with_unsafe_client_options(true)`.",
                    ),
                    error_response(
                        "429",
                        "`RestLimits::max_one_shot_concurrency` concurrent one-shot calls are already in flight.",
                    ),
                    error_response(
                        "413",
                        "Accumulated turn output exceeded `RestLimits::max_text_turn_output_bytes`; the turn was interrupted.",
                    ),
                    error_response(
                        "502",
                        "The underlying `codex app-server` process failed, disconnected, or returned a JSON-RPC error while starting the thread or turn.",
                    ),
                    error_response(
                        "504",
                        "The turn did not reach a terminal state within `RestLimits::max_text_turn_duration`; it was interrupted.",
                    ),
                    auth_note.clone(),
                ]),
            ),
        ]),
        ("post", "/v1/call/{method}") => obj(vec![
            ("summary", json!("One-shot raw JSON-RPC bridge call")),
            (
                "description",
                json!(format!(
                    "Starts a fresh, short-lived Codex session, calls the named app-server \
                     method once, and returns its raw JSON-RPC result. Does not preserve turn \
                     state, stream events, or let you answer server-originated requests \
                     afterward - use the stateful bridge (`POST /v1/sessions` + \
                     `POST /v1/sessions/{{sessionId}}/call/{{method}}`) for that.{}",
                    route.gate.note()
                )),
            ),
            ("operationId", json!("postV1CallMethod")),
            ("parameters", json!([method_param()])),
            (
                "requestBody",
                json_request_body(
                    "JSON-RPC params for `method`, plus optional client overrides.",
                    "RestCallBody",
                    true,
                ),
            ),
            (
                "responses",
                obj(vec![
                    json_response("The call returned a result.", "RestCallResponse"),
                    error_response("400", "Malformed JSON body, or the `{method}` path segment was empty."),
                    error_response(
                        "403",
                        "A `client.command`/`extraArgs`/`config` override was requested without \
                         `RestRouterOptions::with_unsafe_client_options(true)`.",
                    ),
                    error_response(
                        "429",
                        "`RestLimits::max_one_shot_concurrency` concurrent one-shot calls are already in flight.",
                    ),
                    error_response(
                        "502",
                        "The underlying `codex app-server` process failed, disconnected, or the call itself returned a JSON-RPC error.",
                    ),
                    auth_note.clone(),
                ]),
            ),
        ]),
        ("get", "/v1/sessions") => obj(vec![
            ("summary", json!("List active bridge sessions")),
            ("description", json!(format!("Lists session IDs for currently open stateful bridge sessions.{}", route.gate.note()))),
            ("operationId", json!("getV1Sessions")),
            (
                "responses",
                obj(vec![
                    json_response("Active sessions.", "RestListSessionsResponse"),
                    auth_note.clone(),
                ]),
            ),
        ]),
        ("post", "/v1/sessions") => obj(vec![
            ("summary", json!("Start a stateful bridge session")),
            (
                "description",
                json!(format!(
                    "Spawns a persistent `codex app-server` process and returns a `sessionId` \
                     plus its raw `initialize` response. Use the returned `sessionId` with \
                     `POST /v1/sessions/{{sessionId}}/call/{{method}}` to drive it.{}",
                    route.gate.note()
                )),
            ),
            ("operationId", json!("postV1Sessions")),
            (
                "requestBody",
                json_request_body("Optional client overrides.", "RestSessionCreateRequest", false),
            ),
            (
                "responses",
                obj(vec![
                    json_response("Session created.", "RestSessionCreateResponse"),
                    error_response("400", "Malformed JSON body."),
                    error_response(
                        "403",
                        "A `client.command`/`extraArgs`/`config` override was requested without \
                         `RestRouterOptions::with_unsafe_client_options(true)`.",
                    ),
                    error_response(
                        "429",
                        "`RestLimits::max_sessions` concurrently open bridge sessions are already open.",
                    ),
                    error_response(
                        "502",
                        "The `codex app-server` process failed to spawn or initialize.",
                    ),
                    auth_note.clone(),
                ]),
            ),
        ]),
        ("delete", "/v1/sessions/{sessionId}") => obj(vec![
            ("summary", json!("Drop a bridge session")),
            (
                "description",
                json!(format!(
                    "Removes the session and terminates its owned `codex app-server` process \
                     once no client clones remain.{}",
                    route.gate.note()
                )),
            ),
            ("operationId", json!("deleteV1SessionsBySessionId")),
            ("parameters", json!([session_id_param()])),
            (
                "responses",
                obj(vec![
                    json_response("Session removed.", "RestStatusResponse"),
                    error_response("404", "No session with this `sessionId` is open."),
                    auth_note.clone(),
                ]),
            ),
        ]),
        ("post", "/v1/sessions/{sessionId}/call/{method}") => obj(vec![
            ("summary", json!("Call a method on an existing bridge session")),
            (
                "description",
                json!(format!(
                    "Calls any app-server method on an already-open session, preserving \
                     thread/turn state across calls (e.g. `thread/start` then `turn/start`). \
                     Unlike the one-shot `POST /v1/call/{{method}}`, the request body's `client` \
                     field is rejected (`400`) here - client overrides only apply at session \
                     creation or on one-shot calls, not per-call on an existing session.{}",
                    route.gate.note()
                )),
            ),
            ("operationId", json!("postV1SessionsBySessionIdCallMethod")),
            ("parameters", json!([session_id_param(), method_param()])),
            (
                "requestBody",
                json_request_body(
                    "JSON-RPC params for `method`. `client` must be omitted or null.",
                    "RestCallBody",
                    true,
                ),
            ),
            (
                "responses",
                obj(vec![
                    json_response("The call returned a result.", "RestCallResponse"),
                    error_response(
                        "400",
                        "Malformed JSON body, the `{method}` path segment was empty, or a non-null `client` was supplied.",
                    ),
                    error_response("404", "No session with this `sessionId` is open."),
                    error_response(
                        "429",
                        "`RestLimits::max_session_call_concurrency` (global) or \
                         `RestLimits::max_session_call_concurrency_per_session` (this session) is already saturated.",
                    ),
                    error_response(
                        "502",
                        "The underlying `codex app-server` process failed, disconnected, or the call itself returned a JSON-RPC error.",
                    ),
                    auth_note.clone(),
                ]),
            ),
        ]),
        ("get", "/v1/sessions/{sessionId}/events") => obj(vec![
            ("summary", json!("Long-poll the next session event")),
            (
                "description",
                json!(format!(
                    "Long-polls for the next server notification or server-originated request \
                     on this session, up to `timeoutMs` (clamped to \
                     `RestLimits::max_poll_timeout`). Returns `{{\"event\": \"timeout\"}}` if \
                     nothing arrived in time - that is a normal `200`, not an error, and callers \
                     are expected to poll again. At most one poll (via this route or its SSE \
                     counterpart below) may be active per session at a time.{}",
                    route.gate.note()
                )),
            ),
            ("operationId", json!("getV1SessionsBySessionIdEvents")),
            ("parameters", json!([session_id_param(), timeout_ms_param()])),
            (
                "responses",
                obj(vec![
                    json_response(
                        "One event (`notification` | `request` | `closed` | `timeout`).",
                        "RestEventResponse",
                    ),
                    error_response("404", "No session with this `sessionId` is open."),
                    error_response(
                        "409",
                        "An event poll (long-poll or SSE stream) is already active for this session.",
                    ),
                    error_response(
                        "410",
                        "A server-originated request arrived but its reply deadline had already \
                         passed before it could be surfaced; the app-server was sent an error reply automatically.",
                    ),
                    error_response(
                        "429",
                        "This session already holds `RestLimits::max_pending_requests_per_session` \
                         un-replied-to server requests.",
                    ),
                    auth_note.clone(),
                ]),
            ),
        ]),
        ("get", "/v1/sessions/{sessionId}/events/stream") => obj(vec![
            ("summary", json!("Server-Sent Events counterpart to the long-poll route")),
            (
                "description",
                json!(format!(
                    "Same events as `GET /v1/sessions/{{sessionId}}/events`, streamed as \
                     Server-Sent Events instead of returned one at a time: repeatedly polls the \
                     backend and forwards every resolved event (including `timeout`, as an \
                     application-level heartbeat) as its own `data:` frame until a `closed` \
                     event or a backend error ends the stream. IMPORTANT: because the `200` \
                     status and `text/event-stream` content type are committed the moment the \
                     stream opens, a backend error that would be `404`/`409`/`410`/`429` on the \
                     long-poll route instead ends this stream with a terminal `event: error` \
                     frame carrying the same `RestErrorResponse` body (minus the HTTP status, \
                     since none can be sent at that point) - the *only* HTTP-level error status \
                     this route can return is `409`, and only synchronously before the stream \
                     opens, when another poll is already active for this session. At most one \
                     poll (via this route or its long-poll counterpart above) may be active per \
                     session at a time. Keep-alive comment frames are sent every \
                     `RestLimits::sse_keep_alive_interval` while no real event is ready.{}",
                    route.gate.note()
                )),
            ),
            ("operationId", json!("getV1SessionsBySessionIdEventsStream")),
            ("parameters", json!([session_id_param(), timeout_ms_param()])),
            (
                "responses",
                obj(vec![
                    sse_response("An SSE stream of events for this session."),
                    error_response(
                        "409",
                        "An event poll (long-poll or SSE stream) is already active for this session. \
                         This is the only backend-originated error this route can report as an HTTP \
                         status - see the operation description.",
                    ),
                    auth_note.clone(),
                ]),
            ),
        ]),
        ("post", "/v1/sessions/{sessionId}/requests/{requestKey}/result") => obj(vec![
            ("summary", json!("Reply to a pending server-originated request with a result")),
            (
                "description",
                json!(format!(
                    "Answers a pending server-originated request (surfaced via a `\"event\": \
                     \"request\"` payload from the events routes above) with a successful \
                     JSON-RPC result.{}",
                    route.gate.note()
                )),
            ),
            ("operationId", json!("postV1SessionsBySessionIdRequestsByRequestKeyResult")),
            ("parameters", json!([session_id_param(), request_key_param()])),
            (
                "requestBody",
                json_request_body("The JSON-RPC result value.", "RestRequestReplyResultRequest", true),
            ),
            (
                "responses",
                obj(vec![
                    json_response("The reply was delivered.", "RestRequestReplyResponse"),
                    error_response("400", "Malformed JSON body."),
                    error_response(
                        "404",
                        "No session with this `sessionId` is open, or no pending request matches `requestKey`.",
                    ),
                    error_response(
                        "410",
                        "`requestKey` matched a request that has since expired (past \
                         `RestLimits::pending_request_ttl` or the app-server's own reply deadline) \
                         or was already answered.",
                    ),
                    auth_note.clone(),
                ]),
            ),
        ]),
        ("post", "/v1/sessions/{sessionId}/requests/{requestKey}/error") => obj(vec![
            ("summary", json!("Reply to a pending server-originated request with an error")),
            (
                "description",
                json!(format!(
                    "Answers a pending server-originated request (surfaced via a `\"event\": \
                     \"request\"` payload from the events routes above) with a JSON-RPC error.{}",
                    route.gate.note()
                )),
            ),
            ("operationId", json!("postV1SessionsBySessionIdRequestsByRequestKeyError")),
            ("parameters", json!([session_id_param(), request_key_param()])),
            (
                "requestBody",
                json_request_body("The JSON-RPC error code/message/data.", "RestErrorReplyRequest", true),
            ),
            (
                "responses",
                obj(vec![
                    json_response("The reply was delivered.", "RestRequestReplyResponse"),
                    error_response("400", "Malformed JSON body."),
                    error_response(
                        "404",
                        "No session with this `sessionId` is open, or no pending request matches `requestKey`.",
                    ),
                    error_response(
                        "410",
                        "`requestKey` matched a request that has since expired (past \
                         `RestLimits::pending_request_ttl` or the app-server's own reply deadline) \
                         or was already answered.",
                    ),
                    auth_note.clone(),
                ]),
            ),
        ]),
        (method, path) => unreachable!(
            "openapi.rs::operation_for has no operation body mapped for `{method} {path}` - \
             add one alongside the new ROUTES entry"
        ),
    }
}

/// Builds `paths`, grouping [`ROUTES`] entries that share a `path_template`
/// (`/v1/sessions` mounts both `GET` and `POST`) into a single path-item object.
fn build_paths() -> Value {
    let mut by_path: Vec<(&'static str, Vec<&RouteDef>)> = Vec::new();
    for route in ROUTES {
        match by_path
            .iter_mut()
            .find(|(path, _)| *path == route.path_template)
        {
            Some((_, routes)) => routes.push(route),
            None => by_path.push((route.path_template, vec![route])),
        }
    }
    let entries = by_path
        .into_iter()
        .map(|(path, routes)| {
            let operations = routes
                .into_iter()
                .map(|route| (route.method, operation_for(route)))
                .collect();
            (path, obj(operations))
        })
        .collect();
    obj(entries)
}

/// Builds `components.schemas` for every request/response body type in
/// [`super::types`] plus [`crate::compat`]. Field names and required-ness
/// are transcribed straight from each type's `serde` attributes - see the
/// module docs for why this is hand-written rather than derived, and this
/// file's `tests` module for how that transcription is checked.
fn build_schemas() -> Value {
    let mut entries: Vec<(&'static str, Value)> = vec![
        (
            "RestHealthResponse",
            object_schema(
                vec![("status", string_schema("Always `\"ok\"`."))],
                &["status"],
                None,
            ),
        ),
        (
            "RestApprovalPolicy",
            obj(vec![
                ("type", json!("string")),
                ("enum", json!(["deny_all", "read_only", "allow_all"])),
                ("default", json!("deny_all")),
                (
                    "description",
                    json!(
                        "Approval policy preset applied while collecting turn events for \
                         `POST /v1/text-turn`. `allow_all` requires \
                         `RestRouterOptions::with_unsafe_client_options(true)` and is rejected \
                         with `403` otherwise."
                    ),
                ),
            ]),
        ),
        (
            "RestClientOptions",
            object_schema(
                vec![
                    ("name", nullable_string_schema("Client name reported to the app-server's `initialize` call. Defaults to a per-route value (e.g. `codex_app_server_rest`) when omitted.")),
                    ("version", nullable_string_schema("Client version reported to `initialize`. Defaults to this crate's own version when omitted.")),
                    (
                        "command",
                        nullable_string_schema(
                            "Override the `codex` executable path/name to spawn. Requires \
                             `RestRouterOptions::with_unsafe_client_options(true)`; rejected with \
                             `403` otherwise, since it lets the caller choose an arbitrary host executable.",
                        ),
                    ),
                    (
                        "extraArgs",
                        array_schema(
                            string_schema("One extra CLI argument."),
                            "Extra arguments passed to the spawned `codex app-server` process. \
                             Requires `RestRouterOptions::with_unsafe_client_options(true)`; \
                             rejected with `403` otherwise. Omitted from the wire format when empty.",
                        ),
                    ),
                    (
                        "config",
                        obj(vec![
                            ("type", json!("object")),
                            ("additionalProperties", json!({"type": "string"})),
                            (
                                "description",
                                json!(
                                    "Extra `-c key=value` app-server config overrides. Requires \
                                     `RestRouterOptions::with_unsafe_client_options(true)`; rejected \
                                     with `403` otherwise. Omitted from the wire format when empty."
                                ),
                            ),
                        ]),
                    ),
                    (
                        "callTimeoutMs",
                        integer_schema("Per-call JSON-RPC timeout override in milliseconds. Defaults to `codex_app_server_client::DEFAULT_CALL_TIMEOUT` (120s) when omitted."),
                    ),
                ],
                &[],
                Some(false),
            ),
        ),
        (
            "RestTextTurnRequest",
            object_schema(
                vec![
                    ("prompt", string_schema("The text prompt to send. Rejected with `400` if empty or whitespace-only.")),
                    ("model", nullable_string_schema("Model override for the ephemeral thread. Uses the app-server's own default when omitted.")),
                    ("approvalPolicy", schema_ref("RestApprovalPolicy")),
                    ("client", schema_ref("RestClientOptions")),
                ],
                &["prompt"],
                Some(false),
            ),
        ),
        (
            "RestTextTurnResponse",
            object_schema(
                vec![
                    ("threadId", string_schema("The ephemeral thread's id.")),
                    ("turnId", string_schema("The turn's id.")),
                    ("turnStatus", nullable_string_schema("The turn's terminal status (e.g. `\"completed\"`), or `null` if it could not be determined.")),
                    ("agentMessage", string_schema("Concatenated assistant message text observed for the turn. Empty string if none.")),
                    ("latestDiff", nullable_string_schema("The most recent unified diff observed for the turn, or `null` if none.")),
                    (
                        "errors",
                        array_schema(
                            any_value_schema("One turn-level error event, in the app-server's own shape."),
                            "Turn error events observed while collecting the turn. Empty array if none.",
                        ),
                    ),
                ],
                &["threadId", "turnId", "turnStatus", "agentMessage", "latestDiff", "errors"],
                None,
            ),
        ),
        (
            "RestCallBody",
            object_schema(
                vec![
                    ("params", any_value_schema("JSON-RPC params for the target method. Defaults to `null` when omitted.")),
                    ("client", schema_ref("RestClientOptions")),
                ],
                &[],
                Some(false),
            ),
        ),
        (
            "RestCallResponse",
            object_schema(
                vec![
                    ("method", string_schema("The method that was called (echoes the path parameter).")),
                    ("result", any_value_schema("The raw JSON-RPC result.")),
                ],
                &["method", "result"],
                None,
            ),
        ),
        (
            "RestSessionCreateRequest",
            object_schema(vec![("client", schema_ref("RestClientOptions"))], &[], Some(false)),
        ),
        (
            "RestSessionCreateResponse",
            object_schema(
                vec![
                    ("sessionId", string_schema("Opaque session identifier for use in subsequent bridge calls.")),
                    ("initializeResponse", any_value_schema("The raw app-server `initialize` response.")),
                ],
                &["sessionId", "initializeResponse"],
                None,
            ),
        ),
        (
            "RestSessionSummary",
            object_schema(vec![("sessionId", string_schema("Opaque session identifier."))], &["sessionId"], None),
        ),
        (
            "RestListSessionsResponse",
            object_schema(
                vec![(
                    "sessions",
                    array_schema(schema_ref("RestSessionSummary"), "Currently open bridge sessions."),
                )],
                &["sessions"],
                None,
            ),
        ),
        (
            "RestStatusResponse",
            object_schema(vec![("status", string_schema("A short status word, e.g. `\"deleted\"`."))], &["status"], None),
        ),
        ("RestEventResponse", build_rest_event_response_schema()),
        (
            "RestRequestReplyResultRequest",
            object_schema(vec![("result", any_value_schema("The JSON-RPC result to send back to the app-server."))], &["result"], Some(false)),
        ),
        (
            "RestErrorReplyRequest",
            object_schema(
                vec![
                    ("code", integer_schema("JSON-RPC error code sent back to the app-server.")),
                    ("message", string_schema("JSON-RPC error message sent back to the app-server.")),
                    ("data", any_value_schema("Optional JSON-RPC error data. Omitted (defaults to `null`) when not supplied.")),
                ],
                &["code", "message"],
                Some(false),
            ),
        ),
        (
            "RestRequestReplyResponse",
            object_schema(vec![("status", string_schema("Always `\"ok\"` on success."))], &["status"], None),
        ),
        (
            "RestErrorResponse",
            object_schema(
                vec![
                    ("error", string_schema("Short machine-readable error kind, e.g. `\"not_found\"`, `\"rate_limited\"`, `\"json_rpc_error\"`.")),
                    ("message", string_schema("Human-readable error message.")),
                    ("code", integer_schema("JSON-RPC error code, present only for `error: \"json_rpc_error\"` (a JSON-RPC error propagated from the app-server). Omitted otherwise.")),
                    ("data", any_value_schema("JSON-RPC error data, present only for `error: \"json_rpc_error\"` when the app-server supplied one. Omitted otherwise.")),
                ],
                &["error", "message"],
                None,
            ),
        ),
        (
            "CompatibilityReport",
            // NOTE: unlike every REST-specific type above, `CompatibilityReport` and
            // `SurfaceSummary` (in `src/compat.rs`) carry no `#[serde(rename_all = "camelCase")]`
            // - they predate the REST adapter and serialize with their literal Rust
            // (snake_case) field names. Getting this right is the entire point of this file.
            object_schema(
                vec![
                    ("schema_codex_version", string_schema("The Codex version this crate's vendored protocol schema was generated from (`schema/CODEX_VERSION.txt`).")),
                    ("installed_codex_version", nullable_string_schema("Output of the local `codex --version`, or `null` if `codex` is not on `PATH` or the check failed.")),
                    ("surface", schema_ref("SurfaceSummary")),
                ],
                &["schema_codex_version", "installed_codex_version", "surface"],
                None,
            ),
        ),
        (
            "SurfaceSummary",
            object_schema(
                vec![
                    ("client_requests", nonneg_integer_schema("Number of client->server request methods in the vendored schema.")),
                    ("server_requests", nonneg_integer_schema("Number of server->client request methods in the vendored schema.")),
                    ("server_notifications", nonneg_integer_schema("Number of server->client notification methods in the vendored schema.")),
                    ("client_notifications", nonneg_integer_schema("Number of client->server notification methods in the vendored schema.")),
                ],
                &["client_requests", "server_requests", "server_notifications", "client_notifications"],
                None,
            ),
        ),
    ];
    // The `RestEventResponse` union's variants are registered as real named
    // schemas so its `discriminator.mapping` refs resolve; see
    // `rest_event_response_variant_schemas`. `obj` sorts by key, so appending
    // here (rather than interleaving them alphabetically above) doesn't affect
    // the emitted key order.
    entries.extend(rest_event_response_variant_schemas());
    obj(entries)
}

/// The four `RestEventResponse` variants, as `(component schema name, `event`
/// tag value)` pairs.
///
/// Single source of truth for both the `components/schemas` entries
/// ([`rest_event_response_variant_schemas`]) and the parent union's
/// `oneOf`/`discriminator.mapping` ([`build_rest_event_response_schema`]).
/// Those three lists have to agree exactly - OpenAPI requires every
/// `discriminator.mapping` target to resolve to a real schema - and deriving
/// all of them from one table is what keeps a new variant from being added to
/// the `oneOf` while its mapping silently dangles.
const REST_EVENT_RESPONSE_VARIANTS: &[(&str, &str)] = &[
    ("RestEventResponseNotification", "notification"),
    ("RestEventResponseRequest", "request"),
    ("RestEventResponseClosed", "closed"),
    ("RestEventResponseTimeout", "timeout"),
];

/// The per-variant `components/schemas` entries backing
/// [`build_rest_event_response_schema`]'s `oneOf` refs.
///
/// These are registered as real named schemas rather than inlined into the
/// `oneOf` array specifically so `discriminator.mapping` has something to
/// point at: a mapping whose `$ref` names a schema that exists only inline is
/// unresolvable, and spec-compliant generators reject the whole document
/// rather than the one keyword. Naming them also gives generated clients real
/// per-variant types instead of an anonymous union member.
fn rest_event_response_variant_schemas() -> Vec<(&'static str, Value)> {
    vec![
        (
            "RestEventResponseNotification",
            object_schema(
                vec![
                    ("event", obj(vec![("const", json!("notification"))])),
                    (
                        "notification",
                        any_value_schema(
                            "The raw app-server JSON-RPC notification (has its own `method`/`params`).",
                        ),
                    ),
                ],
                &["event", "notification"],
                None,
            ),
        ),
        (
            "RestEventResponseRequest",
            object_schema(
                vec![
                    ("event", obj(vec![("const", json!("request"))])),
                    ("requestKey", string_schema("Opaque key for replying via the `.../requests/{requestKey}/result` or `/error` routes.")),
                    ("requestId", any_value_schema("The JSON-RPC request id (typically a number or string) as sent by the app-server.")),
                    ("method", string_schema("The server-originated JSON-RPC method name, e.g. `currentTime/read`.")),
                    ("request", any_value_schema("The raw server-originated JSON-RPC request (`id`, `method`, `params`).")),
                ],
                &["event", "requestKey", "requestId", "method", "request"],
                None,
            ),
        ),
        (
            "RestEventResponseClosed",
            object_schema(
                vec![("event", obj(vec![("const", json!("closed"))]))],
                &["event"],
                None,
            ),
        ),
        (
            "RestEventResponseTimeout",
            object_schema(
                vec![("event", obj(vec![("const", json!("timeout"))]))],
                &["event"],
                None,
            ),
        ),
    ]
}

/// `RestEventResponse` is an internally-tagged enum
/// (`#[serde(tag = "event", rename_all = "snake_case", rename_all_fields = "camelCase")]`),
/// modeled here as `oneOf` over one `$ref` per variant, discriminated by the
/// shared `event` property, per OpenAPI 3.1's `discriminator` keyword.
fn build_rest_event_response_schema() -> Value {
    obj(vec![
        (
            "description",
            json!(
                "One session event. Internally tagged on `event`: `notification` (a server \
                 notification arrived), `request` (a server-originated request arrived and \
                 awaits a reply), `closed` (the session's transport closed), or `timeout` (no \
                 event arrived within the poll budget - a normal outcome, not an error)."
            ),
        ),
        (
            "oneOf",
            Value::Array(
                REST_EVENT_RESPONSE_VARIANTS
                    .iter()
                    .map(|(schema_name, _)| schema_ref(schema_name))
                    .collect(),
            ),
        ),
        (
            "discriminator",
            obj(vec![
                ("propertyName", json!("event")),
                (
                    "mapping",
                    obj(REST_EVENT_RESPONSE_VARIANTS
                        .iter()
                        .map(|(schema_name, tag)| {
                            (*tag, json!(format!("#/components/schemas/{schema_name}")))
                        })
                        .collect()),
                ),
            ]),
        ),
    ])
}

/// Builds the full OpenAPI 3.1.0 document for the `rest` module.
///
/// Deterministic: every object in the returned [`serde_json::Value`] is
/// built through [`obj`], so `serde_json::to_string_pretty(&openapi_spec())`
/// is byte-identical run to run and build to build - see the module docs'
/// "Determinism" section. `tests::openapi_spec_matches_checked_in_file`
/// pins that output against the checked-in `openapi.json`.
pub fn openapi_spec() -> Value {
    obj(vec![
        ("openapi", json!("3.1.0")),
        (
            "info",
            obj(vec![
                ("title", json!("codex-app-server-client REST adapter")),
                ("version", json!(env!("CARGO_PKG_VERSION"))),
                (
                    "description",
                    json!(
                        "HTTP surface for the optional `rest` feature of `codex-app-server-client` \
                         - a portable adapter around local `codex app-server` JSON-RPC processes. \
                         This is only an adapter: it does not authenticate callers, authorize \
                         requests, sandbox clients, or make the upstream app-server safe to expose \
                         on a network by itself.\n\n\
                         **Routes are opt-in per router constructor** - see each operation's \
                         description for which of `rest::router()` (health/compat only), \
                         `rest::text_turn_router()`, or `rest::trusted_bridge_router()` mounts it. \
                         `rest::router_with_options`/`_with_backend*` let a host application mix \
                         and match via `RestRouterOptions`.\n\n\
                         **Authentication is opt-in and not part of the base router.** Wrap any \
                         router in `rest::bearer_auth(token)` (a `tower` `Layer`) to require an \
                         `Authorization: Bearer <token>` header on every request except (by \
                         default) `GET /health` and `GET /v1/health` - see \
                         `BearerAuthLayer::allow_unauthenticated_health`. Operations below that can \
                         return `401` note that it only applies once this layer is added; the base \
                         router never returns `401` on its own. A caller that presents the one \
                         configured token gets everything the mounted router exposes - this is \
                         transport auth only, not per-session or per-method authorization."
                    ),
                ),
            ]),
        ),
        (
            "servers",
            json!([
                obj(vec![
                    ("url", json!("http://127.0.0.1:43210")),
                    ("description", json!("Default loopback bind address used by the `rest_server` example and the `codex-app-server-rest` binary's `text-turn` mode.")),
                ]),
            ]),
        ),
        (
            "components",
            obj(vec![
                (
                    "securitySchemes",
                    obj(vec![(
                        "bearerAuth",
                        obj(vec![
                            ("type", json!("http")),
                            ("scheme", json!("bearer")),
                            (
                                "description",
                                json!(
                                    "Opt-in via `rest::bearer_auth(token)`; not required by the base \
                                     router. See this document's top-level `info.description`."
                                ),
                            ),
                        ]),
                    )]),
                ),
                ("schemas", build_schemas()),
            ]),
        ),
        ("paths", build_paths()),
    ])
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeSet, fs, path::PathBuf};

    use axum::{
        body::{to_bytes, Body},
        http::{header, Method, Request},
    };
    use tower::ServiceExt;

    use super::*;
    use crate::{
        rest::{
            router_with_backend_and_options, RestBackend, RestFuture, RestRouterOptions,
            RestTextTurnResponse,
        },
        CompatibilityReport,
    };

    /// Path to the checked-in spec, relative to this crate's manifest.
    fn checked_in_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("openapi.json")
    }

    fn rendered() -> String {
        // `to_string_pretty` (not `to_string`) so the checked-in file is
        // human-diffable in review, same rationale as any other checked-in
        // generated JSON in this repo.
        let mut text = serde_json::to_string_pretty(&openapi_spec())
            .expect("openapi_spec() must always serialize");
        text.push('\n');
        text
    }

    /// Pins `openapi_spec()`'s serialized output against the checked-in
    /// `openapi.json`. This crate has no `xtask` (see README.md: zero
    /// path-dependencies on anything else in the workspace, including
    /// tooling crates), so unlike `docs/generated/openapi.json` elsewhere in
    /// this repo, regeneration is an env-var-gated test run rather than an
    /// `xtask` subcommand:
    ///
    /// ```sh
    /// CODEX_REST_OPENAPI_WRITE=1 cargo test -p codex-app-server-client --features rest openapi_spec_matches_checked_in_file
    /// ```
    ///
    /// then review the diff and commit `openapi.json` alongside the change
    /// that caused it.
    #[test]
    fn openapi_spec_matches_checked_in_file() {
        let rendered = rendered();
        if std::env::var_os("CODEX_REST_OPENAPI_WRITE").is_some() {
            fs::write(checked_in_path(), &rendered).expect("failed to write openapi.json");
            return;
        }
        let checked_in = fs::read_to_string(checked_in_path()).unwrap_or_else(|error| {
            panic!(
                "failed to read {}: {error}\n\n\
                 Generate it with:\n\
                 CODEX_REST_OPENAPI_WRITE=1 cargo test -p codex-app-server-client --features rest openapi_spec_matches_checked_in_file",
                checked_in_path().display()
            )
        });
        assert!(
            rendered == checked_in,
            "openapi_spec() no longer matches the checked-in openapi.json.\n\n\
             Regenerate it with:\n\
             CODEX_REST_OPENAPI_WRITE=1 cargo test -p codex-app-server-client --features rest openapi_spec_matches_checked_in_file\n\n\
             then review the diff and commit crates/shared/codex-app-server-client/openapi.json."
        );
    }

    /// Every `$ref` and `discriminator.mapping` target in the document must
    /// name a schema that actually exists under `components/schemas`.
    ///
    /// This exists because it caught a real bug: `RestEventResponse`'s
    /// `discriminator.mapping` pointed at four `RestEventResponse*` names
    /// whose schemas were only ever built inline inside the `oneOf` array and
    /// never registered as components. The document still round-tripped as
    /// JSON and every other test passed - but a spec-compliant generator
    /// (`openapi-typescript`, via Redocly) rejects the whole document when it
    /// can't resolve a mapping ref, which defeats the point of publishing the
    /// spec at all. A dangling ref is invisible to `serde_json`, so it needs
    /// its own assertion.
    #[test]
    fn every_schema_ref_resolves_to_a_real_component() {
        let spec = openapi_spec();
        let defined: BTreeSet<String> = spec["components"]["schemas"]
            .as_object()
            .expect("components/schemas is an object")
            .keys()
            .cloned()
            .collect();

        let mut refs = Vec::new();
        collect_schema_refs(&spec, &mut refs);
        assert!(
            !refs.is_empty(),
            "found no schema refs at all - collect_schema_refs is not walking the document"
        );

        let dangling: Vec<&String> = refs
            .iter()
            .filter(|name| !defined.contains(*name))
            .collect();
        assert!(
            dangling.is_empty(),
            "openapi_spec() references schemas that do not exist under components/schemas: \
             {dangling:?}\n\nDefined schemas: {defined:?}"
        );
    }

    /// Walks the whole document collecting every `#/components/schemas/<name>`
    /// target, from both `$ref` values and `discriminator.mapping` values
    /// (the latter are plain strings, not `$ref` objects, which is exactly why
    /// they were able to dangle unnoticed).
    fn collect_schema_refs(value: &Value, out: &mut Vec<String>) {
        const PREFIX: &str = "#/components/schemas/";
        match value {
            Value::Object(map) => {
                for (key, child) in map {
                    match (key.as_str(), child) {
                        ("$ref", Value::String(target)) => {
                            if let Some(name) = target.strip_prefix(PREFIX) {
                                out.push(name.to_owned());
                            }
                        }
                        ("mapping", Value::Object(mapping)) => {
                            for target in mapping.values().filter_map(Value::as_str) {
                                if let Some(name) = target.strip_prefix(PREFIX) {
                                    out.push(name.to_owned());
                                }
                            }
                        }
                        _ => collect_schema_refs(child, out),
                    }
                }
            }
            Value::Array(items) => {
                for item in items {
                    collect_schema_refs(item, out);
                }
            }
            _ => {}
        }
    }

    /// A `RestBackend` that answers every call immediately without spawning
    /// a real `codex` process - the two mandatory trait methods
    /// (`compatibility_report`, `run_text_turn`) get trivial canned
    /// responses; every other method falls back to the trait's own default
    /// impl (see `src/rest/types.rs`), which already returns a
    /// properly-JSON-shaped `RestError::NotFound`/empty-list/etc for
    /// anything it doesn't implement - exactly the shape a real backend
    /// would return for "session not found", which is what
    /// `every_documented_route_is_actually_mounted` needs: a JSON response
    /// distinguishable from axum's own no-route-matched fallback, without
    /// depending on `codex` being installed in the test environment.
    struct MinimalBackend;

    impl RestBackend for MinimalBackend {
        fn compatibility_report(&self) -> RestFuture<CompatibilityReport> {
            Box::pin(async { Ok(CompatibilityReport::from_installed_version(None)) })
        }

        fn run_text_turn(
            &self,
            _request: super::super::types::RestTextTurnRequest,
        ) -> RestFuture<RestTextTurnResponse> {
            Box::pin(async { Ok(RestTextTurnResponse::default()) })
        }
    }

    /// Request bodies for the routes that require one. Kept next to (not
    /// merged into) [`ROUTES`] because a body is a probe-test concern only -
    /// `openapi_spec()` itself never needs a concrete instance, only the
    /// schema.
    fn probe_body(route: &RouteDef) -> Option<Value> {
        match (route.method, route.path_template) {
            ("post", "/v1/text-turn") => Some(json!({"prompt": "hello"})),
            ("post", "/v1/call/{method}") => Some(json!({})),
            ("post", "/v1/sessions") => Some(json!({})),
            ("post", "/v1/sessions/{sessionId}/call/{method}") => Some(json!({})),
            ("post", "/v1/sessions/{sessionId}/requests/{requestKey}/result") => {
                Some(json!({"result": {}}))
            }
            ("post", "/v1/sessions/{sessionId}/requests/{requestKey}/error") => {
                Some(json!({"code": -32000, "message": "denied"}))
            }
            _ => None,
        }
    }

    /// The coverage test for bead g0qf.2's "route-coverage" requirement.
    ///
    /// axum 0.8's `Router` has no public API to enumerate its own routes, so
    /// this can't diff "routes the live router actually has" against
    /// "routes `openapi_spec()` documents" by introspection. Instead it
    /// takes the documented, honest fallback: [`ROUTES`] is the *one* table
    /// both `openapi_spec()` (via [`build_paths`]/[`operation_for`]) and this
    /// test read, and this test proves each entry in it is real by actually
    /// issuing an HTTP request for it against a live
    /// `trusted_bridge_router()` (the superset router - it mounts every gate
    /// in [`RouteGate`]) and checking the response could only have come from
    /// that route's real handler, not axum's built-in no-route-matched
    /// fallback:
    ///
    /// - every route except the SSE stream one always answers with a
    ///   `Content-Type: application/json` body on this crate's own success
    ///   *and* error paths (including extraction failures like a malformed
    ///   body - see `invalid_json`/`invalid_request` in `routes.rs`, both of
    ///   which still go through `Json(...)`), so any non-JSON body is proof
    ///   the request never reached a real handler.
    /// - the SSE stream route commits `Content-Type: text/event-stream` the
    ///   moment the response starts, before the backend is even polled once
    ///   (see that operation's description above), so a non-SSE content
    ///   type there is the same tell.
    ///
    /// What this test does *not* catch: a route added to `routes.rs` and
    /// never added to `ROUTES` here. axum's opacity makes that direction
    /// fundamentally unverifiable without either vendoring axum's internal
    /// route-matching table or maintaining a second independent list (which
    /// just moves the drift problem rather than solving it) - see this
    /// file's module docs and `RouteDef`'s doc comment. `cargo xtask
    /// check-*`-style CI has no hook into this crate (zero
    /// workspace-crate path-dependencies - see README.md), so the practical
    /// mitigation is procedural: touch `ROUTES` in the same diff that
    /// touches `routes.rs`'s `.route(...)` calls.
    #[tokio::test]
    async fn every_documented_route_is_actually_mounted() {
        let app =
            router_with_backend_and_options(MinimalBackend, RestRouterOptions::trusted_bridge());

        for route in ROUTES {
            let method = Method::from_bytes(route.method.to_ascii_uppercase().as_bytes())
                .unwrap_or_else(|error| panic!("invalid method `{}`: {error}", route.method));
            let mut builder = Request::builder().method(method).uri(route.probe_path);
            let request = match probe_body(route) {
                Some(body) => {
                    builder = builder.header(header::CONTENT_TYPE, "application/json");
                    builder.body(Body::from(body.to_string())).unwrap()
                }
                None => builder.body(Body::empty()).unwrap(),
            };

            let response = app.clone().oneshot(request).await.unwrap_or_else(|error| {
                panic!("{} {} failed: {error}", route.method, route.probe_path)
            });
            let content_type = response
                .headers()
                .get(header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok())
                .unwrap_or_default()
                .to_owned();
            let status = response.status();
            // Drain the body so a hung/oversized stream would fail the test
            // rather than the test process, and so `content_type` above
            // (already read from headers, which arrive before any body) is
            // the only thing this assertion needs.
            let _ = to_bytes(response.into_body(), usize::MAX).await;

            let is_sse_route = route.path_template.ends_with("/events/stream");
            if is_sse_route {
                assert!(
                    content_type.starts_with("text/event-stream"),
                    "{} {} returned Content-Type {content_type:?} (status {status}), not \
                     text/event-stream - openapi_spec() documents this as the SSE route but \
                     trusted_bridge_router() did not route the probe request to it",
                    route.method,
                    route.probe_path,
                );
            } else {
                assert!(
                    content_type.starts_with("application/json"),
                    "{} {} returned Content-Type {content_type:?} (status {status}), not JSON - \
                     openapi_spec() documents `{} {}` as a mounted route, but \
                     trusted_bridge_router() did not route the probe request to a real handler \
                     (axum's own no-route-matched fallback never returns application/json)",
                    route.method,
                    route.probe_path,
                    route.method,
                    route.path_template,
                );
            }
        }
    }

    /// Cheap structural sanity check that every [`ROUTES`] entry has a
    /// matching `paths.<template>.<method>` entry in `openapi_spec()`, and
    /// vice versa. This is *not* the coverage test bead g0qf.2 asks for -
    /// [`every_documented_route_is_actually_mounted`] above is - since
    /// `build_paths` mechanically derives `paths` from `ROUTES`, so by
    /// construction this can only fail if `operation_for`'s match arms and
    /// `ROUTES`'s entries fall out of sync with each other (a bug this
    /// module could introduce internally), not if `routes.rs` drifts from
    /// either. Kept anyway because it's a real, cheap regression guard for
    /// that internal-consistency failure mode, and its assertion messages
    /// are far more direct than tracing a panic out of `operation_for`.
    #[test]
    fn openapi_spec_paths_match_routes_table_exactly() {
        let spec = openapi_spec();
        let paths = spec
            .get("paths")
            .and_then(Value::as_object)
            .expect("openapi_spec() must have an object `paths`");

        for route in ROUTES {
            let path_item = paths.get(route.path_template).unwrap_or_else(|| {
                panic!(
                    "ROUTES has `{} {}` but openapi_spec()'s paths has no entry for `{}`",
                    route.method, route.probe_path, route.path_template
                )
            });
            assert!(
                path_item.get(route.method).is_some(),
                "ROUTES has `{} {}` but openapi_spec()'s path item for `{}` has no `{}` operation",
                route.method,
                route.probe_path,
                route.path_template,
                route.method,
            );
        }

        for (path, item) in paths {
            let methods = item.as_object().unwrap_or_else(|| {
                panic!("openapi_spec()'s path item for `{path}` is not an object")
            });
            for method in methods.keys() {
                assert!(
                    ROUTES
                        .iter()
                        .any(|route| route.path_template == path && route.method == method),
                    "openapi_spec() documents `{method} {path}` but ROUTES has no matching entry"
                );
            }
        }
    }
}

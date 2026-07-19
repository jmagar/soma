# Changelog

All notable changes to this crate will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this crate adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
This crate is not yet published to crates.io (`publish = false`) — everything
below is still `[Unreleased]`; it becomes `[0.1.0] - YYYY-MM-DD` on first
publish.

## [Unreleased]

### Added

- Initial extraction from `unifi-rmcp`'s `crates/unifi`: `UnifiClient`
  (pooled HTTP client, 8 named endpoints), `UnifiService` (the facade
  embedders should depend on), `ActionDispatcher`/`ActionRequest` (dynamic
  dispatch across 244 actions), the capability catalog (`capabilities`,
  built from two bundled `data/*.json` inventories), and `UnifiError`
  (`thiserror`-derived, no `anyhow`).
- `UnifiConfig::request_timeout` — configurable per-request timeout
  (previously a hardcoded 30s constant in `http.rs`), with a public
  `DEFAULT_REQUEST_TIMEOUT` constant.
- `UnifiError::RateLimited` — HTTP 429, with a best-effort parsed
  `retry_after: Option<Duration>` from the response's `Retry-After` header.
- `ActionRequest::new(action, params)` constructor.
- `tests/action_dispatch.rs` — end-to-end wiremock tests driving
  `ActionDispatcher::execute` (the individual pieces were unit-tested, but
  the dispatcher itself previously had no direct coverage).
- A catalog-wide invariant test
  (`no_dispatchable_mutating_action_shares_a_get_path_with_a_read_only_action`)
  so the "mutating action silently maps to a read-only path" bug class (see
  Fixed, below) can't recur unnoticed.
- Per-crate `LICENSE` (MIT) — the workspace-root `LICENSE` isn't packaged by
  `cargo package`, since it lives outside this crate's directory.
- Per-crate `CHANGELOG.md` (this file).

### Changed

- `UnifiError`, `ApiSourceFamily`, `AuthScope`, `Capability`, and
  `ActionRequest` are all now `#[non_exhaustive]` — this crate is meant to
  be published and consumed externally, so adding a variant/field to any of
  them later must not be a downstream semver break. `ActionRequest` is the
  one type callers are meant to construct, hence its dedicated `new`
  constructor above.
- `#![warn(missing_docs)]` → `#![deny(missing_docs)]`.
- `lib.rs`'s crate-level docs are now `#![doc = include_str!("../README.md")]`
  instead of a separate, shorter `//!` summary — the README is the crate's
  entire rustdoc landing page, and every one of its code examples is a real
  doctest under `cargo test --doc` instead of only the one that used to live
  in `lib.rs`.
- `tracing` instrumentation (span + ok/error logging) moved from
  `UnifiClient`'s private `get()` helper — which only wrapped the 8 named
  methods — down into `http::request_json`, the one function every dispatch
  path (named methods and all ~236 dynamically-dispatched actions) actually
  calls. Previously, only named-method calls were observable via `tracing`.
- `OfficialNetworkApi::path()` now passes already-qualified
  `/proxy/network|protect/integration/` Connector paths through unchanged
  instead of re-prefixing them.
- Dropped the inherited `homepage.workspace = true` `Cargo.toml` field — it
  resolved to soma's own product site, not anything UniFi-related.

### Fixed

- 21 mutating internal-API actions (client unblock/rename/authorize/etc.,
  toggle WLAN/firewall-policy/traffic-route/QoS-rule/OON-policy/port-forward,
  update network/AP-group/device-radio, set outlet state, reorder firewall
  policies) were declared with a `GET` method against a path identical to an
  existing read-only listing endpoint — dispatching one silently re-ran the
  read and reported success without performing the intended mutation.
  Disabled (`runtime: false`) until each real endpoint is confirmed; see the
  `evidence` field on the affected entries in
  `data/unifi_internal_endpoint_models.json`.
- The `alarms` legacy-alias catalog entry's path was corrected to
  `/rest/alarm`, matching what `UnifiClient::alarms()` actually calls (it
  had silently diverged from `/stat/alarm`).
- Two `normalize_internal_request` overrides that routed
  `unifi_get_traffic_flow_statistics` and `unifi_get_gateway_settings` to
  the wrong endpoint were removed.
- `actions/hybrid::resolve` now rejects a non-string `params.prefer` instead
  of silently falling through, and treats a present-but-null `params.siteId`
  as absent rather than as a truthy site-scoping signal.
- `http::request_json` now checks the response status before attempting a
  strict JSON decode, so a non-JSON error body (an HTML error page, plain
  text) maps to `UnexpectedStatus` with a best-effort-captured body instead
  of an opaque `Decode` error that dropped the status code.

### Removed

- `.cookie_store(true)` on the pooled `reqwest::Client`, and reqwest's
  `cookies` feature flag. Verified empirically against a real UniFi
  controller: authenticated requests (official and internal API) never
  receive a `Set-Cookie` header, so the cookie jar was dead configuration,
  not a defense-in-depth measure. Removes six transitive dependencies from
  the workspace entirely: `cookie`, `cookie_store`, `document-features`,
  `litrs`, `psl-types`, `publicsuffix`.

### Security

- `UnifiConfig::skip_tls_verify` now defaults to `false` (verify by
  default) — a self-signed local controller must opt in explicitly instead
  of TLS verification being silently skipped.

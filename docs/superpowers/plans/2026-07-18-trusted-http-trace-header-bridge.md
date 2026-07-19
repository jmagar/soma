# Trusted HTTP Trace-Header Bridge Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Finish the `rmcp-template-mdei` epic (GH #76 next slice) by implementing beads `.2`–`.5`: typed Soma config for trusted HTTP trace-header extraction, wiring `rmcp-traces/http` into `soma-mcp`'s `call_tool` handler, gating browser CORS on that config, and adding live smoke tests plus docs with outbound non-propagation proof.

**Architecture:** `rmcp-traces` (crate `crates/shared/traces`, already implements `.1`) stays a leaf/platform crate — `soma-config`/`soma-runtime` never depend on it. A new `SOMA_MCP_TRACE_HEADERS` typed enum (`off` default / `trusted` / `trusted-with-baggage`) lives in `soma-config`; `soma-runtime::server::resolve_auth_policy_kind` gates non-`off` modes on a real trust boundary (`LoopbackDev` or `TrustedGatewayUnscoped` only — bearer/OAuth auth is never a trace-header trust boundary); `soma-mcp` maps the typed config to `rmcp_traces::http::HttpTracePolicy` inside a new `trace_resolution` module and consumes it in `SomaRmcpServer::call_tool` after auth; `apps/soma/src/http.rs`'s `cors_layer` gates the browser-facing CORS allow-header list on the same typed config (CORS is transport permission only, never the trust decision).

**Tech Stack:** Rust 2021 workspace, `rmcp` 2.2.0 (`RequestParamsMeta`, `RequestContext.extensions` carrying `http::request::Parts`), Axum 0.8 + `tower-http` CORS, `reqwest` (client-side test harness), `tracing`/`tracing-subscriber` for structured logging, `soma_test_support::{tracing_test_lock, SharedBuf}` for log-capture assertions, `cargo xtask` for the live smoke harness.

## Global Constraints

- `rmcp-traces` (leaf crate) must **never** gain a dependency on `soma-config`, `soma-runtime`, `soma-mcp`, or any other Soma product crate — verified by `cargo xtask check-architecture` / the existing `crates/shared/traces` `layer = "shared"` metadata.
- Default trace-header mode is `off`; every existing deployment's behavior is unchanged unless an operator explicitly sets `SOMA_MCP_TRACE_HEADERS`.
- Bearer/OAuth authentication is **not** a trace-header trust boundary. Only `AuthPolicyKind::LoopbackDev` and `AuthPolicyKind::TrustedGatewayUnscoped` may run a non-`off` trace-header mode; `MountedBearer`/`MountedOAuth` must fail config validation at startup with a clear, actionable error.
- RMCP `_meta` (`traceparent`/`tracestate`/`baggage` keys) always wins over HTTP headers. When `_meta` carries any trace key, HTTP header **values** must never be parsed, joined, counted, or logged — only safe presence booleans (`http_trace_headers_present`, `trace_context_conflict`) may be recorded.
- `off` mode must do **zero** HTTP header lookup (no `RequestContext.extensions` access), zero `Meta`/`Parts` clone, and zero env/config string matching on the request hot path — the mode is read once as a typed `Copy` enum.
- CORS is transport permission only, never a trust decision — the CORS allow-header list stays a static exact allow-list (`HeaderName::from_static`, no `Any`, no reflection, no per-request synthesis), gated at router-construction time on the same typed config that gates extraction.
- No outbound trace-header propagation in this epic. Prove (with a runtime regression test) that inbound `traceparent`/`tracestate`/`baggage` headers are never forwarded to Soma's deployed upstream API (`SomaClient`) or to gateway-proxied MCP HTTP providers.
- Run `cargo fmt`, `cargo clippy --all-targets --all-features -- -D warnings`, and the relevant `cargo test -p <crate>` after every task before moving on — this workspace's CI gate treats clippy warnings as failures.
- Every new/changed action, config key, or CLI-visible behavior needs a `CHANGELOG.md` entry under `[Unreleased]` (see `CLAUDE.md`'s "How to add an action" convention, adapted here for a config/runtime change rather than a new MCP action).

---

## File Structure

| File | Responsibility |
|------|-----------------|
| `crates/soma/config/src/config.rs` | Add `TraceHeaderMode` enum + `McpConfig.trace_headers` field + `SOMA_MCP_TRACE_HEADERS` env parsing |
| `crates/soma/config/src/config_tests.rs` | Unit tests for the new enum/field/env parsing |
| `crates/soma/config/src/env_registry.rs` | Register `SOMA_MCP_TRACE_HEADERS` in the canonical env-var table |
| `crates/soma/config/src/env_registry_tests.rs` | Assert the new spec entry exists with the right shape |
| `crates/soma/runtime/src/server.rs` | Add `validate_trace_headers_trust` and wire it into `resolve_auth_policy_kind` (split into a public wrapper + private `_unchecked` helper) |
| `crates/soma/runtime/src/server_tests.rs` | Trust-boundary validation matrix |
| `crates/soma/cli/src/doctor/checks.rs` | Special-case trace-header-trust failures in `check_auth_config`'s remediation text |
| `crates/soma/cli/src/setup.rs` | Give trace-header-trust failures their own `SetupFailure.code` |
| `crates/soma/mcp/Cargo.toml` | Enable `rmcp-traces/http` under the existing `http` feature |
| `crates/soma/mcp/src/trace_resolution.rs` (new) | Pure, unit-testable trace resolution: `TraceResolution` struct (no stored conflict field — derived at the log call site), `resolve_trace_resolution`, `meta_has_any_trace_key` |
| `crates/soma/mcp/src/trace_resolution_tests.rs` (new) | Full behavior matrix: off/trusted/trusted-with-baggage, `_meta` vs HTTP conflict, invalid headers, orphan `tracestate` |
| `crates/soma/mcp/src/lib.rs` | Register the new `trace_resolution` module |
| `crates/soma/mcp/src/rmcp_server.rs` | Wire `trace_resolution` into `call_tool` only, via a new sibling `execution_context_with_trace` — the existing `execution_context()` and its 5 non-`call_tool` call sites are untouched |
| `apps/soma/src/lib.rs` | Add `testing::{loopback_state_with_mcp_config, trusted_gateway_state_with_mcp_config, bearer_state_with_mcp_config}` test helpers |
| `apps/soma/tests/mcp_trace_headers.rs` (new) | Real streamable-HTTP round trip proving header consumption end to end, across `LoopbackDev`/`TrustedGatewayUnscoped`/mounted-auth-failure, via shared `ServerHandle`/`TracingCapture` test helpers |
| `apps/soma/src/http.rs` | Gate `cors_layer`'s allow-header list on `McpConfig.trace_headers` |
| `apps/soma/src/http_tests.rs` | CORS preflight tests per mode, including mixed-case request headers |
| `apps/soma/tests/api_routes.rs` | Outbound non-propagation regression test for the gateway-proxied MCP HTTP path (`protected_routes_proxy.rs::forwarded_mcp_headers()`) |
| `crates/soma/client/src/client_tests.rs` | Outbound non-propagation regression test for `SomaClient`'s deployed-API path |
| `docs/TRACE_CONTEXT.md` (new) | Durable operator guide: modes, trust boundary, examples, outbound-propagation scope, stdio note |
| `xtask/src/trace_headers_smoke.rs` (new) | `cargo xtask test-trace-headers` — builds `soma` once, spawns it directly per scenario |
| `xtask/src/scripts_lane_a.rs` | Widen `AuthSmokeResults` to `pub(crate)` so the new smoke command reuses it |
| `xtask/src/main.rs` | Register the new xtask subcommand |
| `scripts/test-trace-headers.sh` (new) | Thin wrapper, mirrors `scripts/test-mcp-auth.sh` |
| `CHANGELOG.md` | `[Unreleased]` entries for the new config key and behavior |

---

## Task 1: `TraceHeaderMode` config enum and `SOMA_MCP_TRACE_HEADERS` env parsing

**Files:**
- Modify: `crates/soma/config/src/config.rs`
- Test: `crates/soma/config/src/config_tests.rs`

**Interfaces:**
- Produces: `pub enum TraceHeaderMode { Off, Trusted, TrustedWithBaggage }` (derives `Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq`; `#[serde(rename_all = "kebab-case")]`; `#[default] Off`), field `pub trace_headers: TraceHeaderMode` on `McpConfig`.
- Consumes: nothing new (mirrors the existing `AuthMode`/`SOMA_MCP_AUTH_MODE` pattern in the same file).

- [ ] **Step 1: Write the failing tests**

Add to `crates/soma/config/src/config_tests.rs` (append near the bottom, before the closing of the file):

```rust
// ── TraceHeaderMode / SOMA_MCP_TRACE_HEADERS ──────────────────────────────────

#[test]
fn trace_headers_default_to_off() {
    assert_eq!(McpConfig::default().trace_headers, TraceHeaderMode::Off);
}

#[test]
#[serial]
fn trace_headers_env_parses_all_three_values() {
    let previous = std::env::var_os("SOMA_MCP_TRACE_HEADERS");

    for (raw, expected) in [
        ("off", TraceHeaderMode::Off),
        ("trusted", TraceHeaderMode::Trusted),
        ("trusted-with-baggage", TraceHeaderMode::TrustedWithBaggage),
    ] {
        std::env::set_var("SOMA_MCP_TRACE_HEADERS", raw);
        let config = Config::load().expect("config should load");
        assert_eq!(
            config.mcp.trace_headers, expected,
            "SOMA_MCP_TRACE_HEADERS={raw:?} should parse to {expected:?}"
        );
    }

    match previous {
        Some(value) => std::env::set_var("SOMA_MCP_TRACE_HEADERS", value),
        None => std::env::remove_var("SOMA_MCP_TRACE_HEADERS"),
    }
}

#[test]
#[serial]
fn trace_headers_env_rejects_invalid_value() {
    let previous = std::env::var_os("SOMA_MCP_TRACE_HEADERS");
    std::env::set_var("SOMA_MCP_TRACE_HEADERS", "bogus");

    let error = Config::load().expect_err("invalid SOMA_MCP_TRACE_HEADERS should be rejected");
    assert!(error.to_string().contains("SOMA_MCP_TRACE_HEADERS"));

    match previous {
        Some(value) => std::env::set_var("SOMA_MCP_TRACE_HEADERS", value),
        None => std::env::remove_var("SOMA_MCP_TRACE_HEADERS"),
    }
}

#[test]
fn trace_header_mode_serde_uses_kebab_case() {
    assert_eq!(
        serde_json::to_value(TraceHeaderMode::TrustedWithBaggage).unwrap(),
        serde_json::json!("trusted-with-baggage")
    );
    assert_eq!(
        serde_json::from_value::<TraceHeaderMode>(serde_json::json!("trusted")).unwrap(),
        TraceHeaderMode::Trusted
    );
}

#[test]
fn trace_headers_toml_file_config_parses_all_three_values() {
    // Exercises the same `toml::from_str::<Config>` path `Config::load()` uses
    // for `config.toml`, without needing filesystem/cwd scaffolding — proves
    // file config (not just env) parses `mcp.trace_headers`.
    for (raw, expected) in [
        ("off", TraceHeaderMode::Off),
        ("trusted", TraceHeaderMode::Trusted),
        ("trusted-with-baggage", TraceHeaderMode::TrustedWithBaggage),
    ] {
        let toml_str = format!("[mcp]\ntrace_headers = \"{raw}\"\n");
        let config: Config = toml::from_str(&toml_str).expect("toml should parse");
        assert_eq!(config.mcp.trace_headers, expected, "raw TOML value {raw:?}");
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p soma-config trace_header -- --test-threads=1`
Expected: FAIL — `TraceHeaderMode` and `McpConfig.trace_headers` do not exist yet (compile error).

- [ ] **Step 3: Implement the enum, field, and env parsing**

In `crates/soma/config/src/config.rs`, add the enum right after `AuthMode`'s definition (after line 158's closing brace, before the `// ── defaults ──` section):

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum TraceHeaderMode {
    /// No HTTP trace-header extraction. Default — safe for every deployment.
    #[default]
    Off,
    /// Extract `traceparent`/`tracestate` from inbound HTTP headers after auth.
    /// Baggage is never extracted in this mode.
    Trusted,
    /// Like `Trusted`, but also extracts validated `baggage`. Baggage can carry
    /// sensitive user/session/application data — enable deliberately.
    TrustedWithBaggage,
}
```

Add the field to `McpConfig` (after `pub allowed_origins: Vec<String>,`, before `pub auth: AuthConfig,`):

```rust
    /// Trusted HTTP trace-header extraction mode (SOMA_MCP_TRACE_HEADERS).
    /// Only meaningful when the resolved auth policy is a real trust boundary
    /// (loopback bind or a trusted gateway) — see
    /// `soma_runtime::server::resolve_auth_policy_kind`.
    pub trace_headers: TraceHeaderMode,
```

Add it to `impl Default for McpConfig` (inside the `Self { ... }` literal, after `allowed_origins: Vec::new(),`):

```rust
            trace_headers: TraceHeaderMode::default(),
```

Add env parsing in `Config::load()`, right after the existing `SOMA_MCP_AUTH_MODE` block (after its closing `}` around line 373, before the `// Upstream service config` comment):

```rust
        if let Ok(v) = std::env::var("SOMA_MCP_TRACE_HEADERS") {
            if !v.is_empty() {
                config.mcp.trace_headers = match v.to_lowercase().as_str() {
                    "off" => TraceHeaderMode::Off,
                    "trusted" => TraceHeaderMode::Trusted,
                    "trusted-with-baggage" => TraceHeaderMode::TrustedWithBaggage,
                    other => {
                        return Err(anyhow::anyhow!(
                            "invalid SOMA_MCP_TRACE_HEADERS {:?}: must be \"off\", \"trusted\", \
                             or \"trusted-with-baggage\"",
                            other
                        ));
                    }
                };
            }
        }
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p soma-config trace_header -- --test-threads=1`
Expected: PASS (4 new tests). Also run `cargo test -p soma-config` in full to confirm no regression.

- [ ] **Step 5: Lint and format**

Run: `cargo fmt -p soma-config && cargo clippy -p soma-config --all-targets -- -D warnings`
Expected: clean.

- [ ] **Step 6: Commit**

```bash
git add crates/soma/config/src/config.rs crates/soma/config/src/config_tests.rs
git commit -m "feat(config): add SOMA_MCP_TRACE_HEADERS typed config"
```

---

## Task 2: Register `SOMA_MCP_TRACE_HEADERS` in the env-var registry

**Files:**
- Modify: `crates/soma/config/src/env_registry.rs`
- Test: `crates/soma/config/src/env_registry_tests.rs`

**Interfaces:**
- Consumes: `EnvKeySpec`, `spec()`, `EnvClassification::KeepEnv`, `RuntimePlacement::Both`, `LegacyBehavior::Canonical` (all already defined in this file).
- Produces: a new entry in `ENV_KEY_SPECS` discoverable via `spec_for("SOMA_MCP_TRACE_HEADERS")`.

- [ ] **Step 1: Write the failing test**

Append to `crates/soma/config/src/env_registry_tests.rs`:

```rust
#[test]
fn trace_headers_env_is_registered_and_maps_to_mcp_config() {
    let spec = spec_for("SOMA_MCP_TRACE_HEADERS").expect("SOMA_MCP_TRACE_HEADERS should be registered");
    assert_eq!(spec.toml_destination, Some("mcp.trace_headers"));
    assert!(!spec.secret, "trace-header mode is not a secret");
    assert_eq!(spec.classification, EnvClassification::KeepEnv);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p soma-config trace_headers_env_is_registered`
Expected: FAIL — `spec_for` returns `None`.

- [ ] **Step 3: Add the registry entry**

In `crates/soma/config/src/env_registry.rs`, add a new `spec(...)` entry to `ENV_KEY_SPECS` (append just before the closing `];`, after the `SOMA_MCP_ALLOWED_ORIGINS` entry):

```rust
    spec(
        "SOMA_MCP_TRACE_HEADERS",
        EnvClassification::KeepEnv,
        RuntimePlacement::Both,
        Some("mcp.trace_headers"),
        LegacyBehavior::Canonical,
        false,
        Some("CLAUDE_PLUGIN_OPTION_TRACE_HEADERS"),
    ),
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p soma-config trace_headers_env_is_registered`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/soma/config/src/env_registry.rs crates/soma/config/src/env_registry_tests.rs
git commit -m "feat(config): register SOMA_MCP_TRACE_HEADERS in the env registry"
```

---

## Task 3: Startup trust-boundary validation (`resolve_auth_policy_kind`)

**Files:**
- Modify: `crates/soma/runtime/src/server.rs`
- Test: `crates/soma/runtime/src/server_tests.rs`
- Modify: `crates/soma/cli/src/doctor/checks.rs` (Step 6)
- Modify: `crates/soma/cli/src/setup.rs` (Step 6)

**Interfaces:**
- Consumes: `soma_config::TraceHeaderMode` (Task 1), `AuthPolicyKind` (existing).
- Produces: `resolve_auth_policy_kind` now also rejects a non-`off` `trace_headers` mode unless the resolved kind is `LoopbackDev` or `TrustedGatewayUnscoped`. New private fn `validate_trace_headers_trust(mcp: &McpConfig, kind: AuthPolicyKind) -> Result<()>`.

- [ ] **Step 1: Write the failing tests**

Append to `crates/soma/runtime/src/server_tests.rs`:

```rust
#[test]
fn off_trace_headers_do_not_require_a_trust_boundary() {
    let mut config = config("0.0.0.0");
    config.mcp.api_token = Some("secret".into());
    // trace_headers defaults to Off — MountedBearer must still resolve fine.
    assert_eq!(
        resolve_auth_policy_kind(&config, false).unwrap(),
        AuthPolicyKind::MountedBearer
    );
}

#[test]
fn trusted_trace_headers_reject_mounted_bearer() {
    let mut config = config("0.0.0.0");
    config.mcp.api_token = Some("secret".into());
    config.mcp.trace_headers = TraceHeaderMode::Trusted;
    let error = resolve_auth_policy_kind(&config, false).unwrap_err();
    assert!(
        error.to_string().contains("not a trace-header trust boundary"),
        "error was: {error}"
    );
}

#[cfg(feature = "auth")]
#[test]
fn trusted_with_baggage_rejects_mounted_oauth() {
    let mut config = config("0.0.0.0");
    config.mcp.auth = AuthConfig {
        mode: AuthMode::OAuth,
        ..AuthConfig::default()
    };
    config.mcp.trace_headers = TraceHeaderMode::TrustedWithBaggage;
    let error = resolve_auth_policy_kind(&config, false).unwrap_err();
    assert!(error.to_string().contains("not a trace-header trust boundary"));
}

#[test]
fn trusted_trace_headers_allowed_on_loopback() {
    let mut config = config("127.0.0.1");
    config.mcp.trace_headers = TraceHeaderMode::TrustedWithBaggage;
    assert_eq!(
        resolve_auth_policy_kind(&config, false).unwrap(),
        AuthPolicyKind::LoopbackDev
    );
}

#[test]
fn trusted_trace_headers_allowed_on_trusted_gateway_unscoped() {
    let mut config = config("0.0.0.0");
    config.mcp.no_auth = true;
    config.mcp.trace_headers = TraceHeaderMode::Trusted;
    assert_eq!(
        resolve_auth_policy_kind(&config, true).unwrap(),
        AuthPolicyKind::TrustedGatewayUnscoped
    );
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p soma-runtime trace_headers -- --test-threads=1`
Expected: FAIL — `config.mcp.trace_headers` field compiles (from Task 1) but every non-`off` case currently resolves successfully with no rejection, so `trusted_trace_headers_reject_mounted_bearer` and the OAuth variant fail their `unwrap_err()`.

- [ ] **Step 3: Implement the validation**

In `crates/soma/runtime/src/server.rs`, update the import line:

```rust
use soma_config::{AuthMode, Config, McpConfig, TraceHeaderMode};
```

Rename the existing `resolve_auth_policy_kind` body into a private `_unchecked` helper and add a thin public wrapper that validates trace-header trust. Replace:

```rust
pub fn resolve_auth_policy_kind(config: &Config, trusted_gateway: bool) -> Result<AuthPolicyKind> {
    validate_public_url(config)?;

    if config.mcp.is_loopback() {
        return Ok(AuthPolicyKind::LoopbackDev);
    }
```

with:

```rust
pub fn resolve_auth_policy_kind(config: &Config, trusted_gateway: bool) -> Result<AuthPolicyKind> {
    validate_public_url(config)?;
    let kind = resolve_auth_policy_kind_unchecked(config, trusted_gateway)?;
    validate_trace_headers_trust(&config.mcp, kind)?;
    Ok(kind)
}

fn resolve_auth_policy_kind_unchecked(
    config: &Config,
    trusted_gateway: bool,
) -> Result<AuthPolicyKind> {
    if config.mcp.is_loopback() {
        return Ok(AuthPolicyKind::LoopbackDev);
    }
```

Leave the rest of the old function body (the `has_token`/`has_oauth`/`no_auth` branches through the final `anyhow::bail!`) exactly as-is under the new `resolve_auth_policy_kind_unchecked` name — only the function signature line and the first two lines changed.

Add the new validation function right after `resolve_auth_policy_kind_unchecked`'s closing brace, before `fn validate_public_url`:

```rust
/// Bearer/OAuth authentication is not a trace-header trust boundary: a
/// client presenting a valid token says nothing about whether an upstream
/// gateway/proxy stripped or overwrote inbound `traceparent`/`tracestate`/
/// `baggage` headers from untrusted clients before the request reached this
/// server. Only a real transport-level trust boundary (loopback bind, or an
/// explicitly trusted upstream gateway/proxy) may enable HTTP trace-header
/// extraction.
fn validate_trace_headers_trust(mcp: &McpConfig, kind: AuthPolicyKind) -> Result<()> {
    if mcp.trace_headers == TraceHeaderMode::Off {
        return Ok(());
    }
    match kind {
        AuthPolicyKind::LoopbackDev | AuthPolicyKind::TrustedGatewayUnscoped => Ok(()),
        AuthPolicyKind::MountedBearer | AuthPolicyKind::MountedOAuth => {
            anyhow::bail!(
                "Refusing to start with SOMA_MCP_TRACE_HEADERS={:?} on a {:?} deployment.\n\
                 \n\
                 Bearer/OAuth authentication is not a trace-header trust boundary — a caller \
                 presenting a valid token says nothing about whether an upstream gateway or \
                 proxy stripped or overwrote inbound traceparent/tracestate/baggage headers \
                 from untrusted clients before the request reached this server.\n\
                 \n\
                 Choose one of:\n\
                 1. Set SOMA_MCP_TRACE_HEADERS=off (default; disables HTTP trace-header extraction).\n\
                 2. Bind to loopback:  SOMA_MCP_HOST=127.0.0.1\n\
                 3. Deploy behind a trusted proxy that strips/overwrites inbound trace headers \
                    from untrusted clients, and set SOMA_NOAUTH=true.",
                mcp.trace_headers,
                kind,
            );
        }
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p soma-runtime` (full crate, to also catch any regression in the pre-existing `resolve_auth_policy_kind` tests since the function was split).
Expected: PASS — all previously-passing tests still pass, plus the 5 new ones.

- [ ] **Step 5: Lint and format**

Run: `cargo fmt -p soma-runtime && cargo clippy -p soma-runtime --all-targets --all-features -- -D warnings`
Expected: clean (run with `--all-features` too, since `AuthMode`/`AuthConfig` are used inside a `#[cfg(feature = "auth")]` test).

- [ ] **Step 6: Fix `doctor`'s auth-failure remediation text**

`crates/soma/cli/src/doctor/checks.rs::check_auth_config` (lines 301-331) already calls the validating `resolve_auth_policy_kind` and surfaces its `Err` via `DoctorCheck::fail`, but it unconditionally appends a generic 4-item auth-setup fix list after the error message:

```rust
        Err(error) => DoctorCheck::fail(
            "auth",
            "Auth mode",
            format!(
                "{error}\n    \
                 Fix ONE of:\n    \
                 1. Bind to loopback:    SOMA_MCP_HOST=127.0.0.1\n    \
                 2. Set a bearer token:  SOMA_MCP_TOKEN=$(openssl rand -hex 32)\n    \
                 3. Enable OAuth:        SOMA_MCP_AUTH_MODE=oauth\n    \
                 4. Upstream gateway:    SOMA_NOAUTH=true\n    \
                 CUSTOMIZE: Replace SOMA_ prefix with your service prefix."
            ),
        ),
```

When `validate_trace_headers_trust` (Step 3) is what actually failed, the resolved `AuthPolicyKind` was already fine (e.g. a valid bearer token *is* configured) — this generic list is then actively wrong advice, since it tells the operator to "fix" an auth setup that isn't broken. The trace-header error already carries its own complete remediation. Replace the `Err` arm with:

```rust
        Err(error) => {
            let message = if error.to_string().contains("SOMA_MCP_TRACE_HEADERS") {
                // Auth policy itself resolved fine — this is a trace-header
                // trust-boundary rejection (see `validate_trace_headers_trust`
                // in soma-runtime). The error already carries its own
                // complete remediation; the generic auth-setup fix list below
                // would be wrong advice here, since auth isn't the problem.
                error.to_string()
            } else {
                format!(
                    "{error}\n    \
                     Fix ONE of:\n    \
                     1. Bind to loopback:    SOMA_MCP_HOST=127.0.0.1\n    \
                     2. Set a bearer token:  SOMA_MCP_TOKEN=$(openssl rand -hex 32)\n    \
                     3. Enable OAuth:        SOMA_MCP_AUTH_MODE=oauth\n    \
                     4. Upstream gateway:    SOMA_NOAUTH=true\n    \
                     CUSTOMIZE: Replace SOMA_ prefix with your service prefix."
                )
            };
            DoctorCheck::fail("auth", "Auth mode", message)
        }
```

`crates/soma/cli/src/setup.rs::check_auth` (around line 254) already uses bare `error.to_string()` with no appended generic list, so it needs no text fix — but its `SetupFailure.code` is a flat `"invalid_auth_policy"` for every `resolve_auth_policy_kind` failure. Give trace-header failures their own code so downstream tooling (and `setup`'s own tests) can distinguish the two failure classes:

```rust
fn check_auth(config: &Config, report: &mut SetupReport) {
    if let Err(error) = resolve_auth_policy_kind(config, config.mcp.trusted_gateway) {
        let code = if error.to_string().contains("SOMA_MCP_TRACE_HEADERS") {
            "invalid_trace_headers_trust"
        } else {
            "invalid_auth_policy"
        };
        report.blocking_failures.push(SetupFailure {
            code,
            message: error.to_string(),
        });
        return;
    }
```

Add a test to `crates/soma/cli/src/doctor/checks_tests.rs` (or wherever `check_auth_config`'s existing tests live — locate via `grep -rn "check_auth_config" crates/soma/cli/src` before writing the test, since this plan has not yet read that test file):

```rust
#[test]
fn trace_header_trust_failure_omits_the_generic_auth_fix_list() {
    let mut config = test_config_with_bearer_token(); // reuse whatever helper the existing tests use
    config.mcp.trace_headers = TraceHeaderMode::Trusted;
    let check = check_auth_config(&config);
    assert!(!check.passed);
    assert!(check.detail.contains("not a trace-header trust boundary"));
    assert!(
        !check.detail.contains("Fix ONE of"),
        "trace-header failures should not show the generic auth fix list: {}",
        check.detail
    );
}
```

(Adapt the config-construction helper name/shape to whatever `checks.rs`'s existing tests already use — read the file first.)

- [ ] **Step 7: Run tests to verify they pass**

Run: `cargo test -p soma-cli` (covers both `doctor` and `setup` checks).
Expected: PASS.

- [ ] **Step 8: Lint and format**

Run: `cargo fmt -p soma-cli && cargo clippy -p soma-cli --all-targets --all-features -- -D warnings`
Expected: clean.

- [ ] **Step 9: Commit**

```bash
git add crates/soma/runtime/src/server.rs crates/soma/runtime/src/server_tests.rs crates/soma/cli/src/doctor/checks.rs crates/soma/cli/src/setup.rs
git commit -m "feat(runtime): reject trusted trace-header modes on bearer/OAuth deployments"
```

This closes bead `rmcp-template-mdei.2`. Run `bd update rmcp-template-mdei.2 --claim` before Task 1 and `bd close rmcp-template-mdei.2 --reason "typed SOMA_MCP_TRACE_HEADERS config + startup trust-boundary validation shipped"` after Task 3's commit, before starting Task 4.

---

## Task 4: Enable `rmcp-traces/http` under `soma-mcp`'s `http` feature

**Files:**
- Modify: `crates/soma/mcp/Cargo.toml`

**Interfaces:**
- Produces: `rmcp_traces::http::{HttpTracePolicy, HttpTraceExtraction, extract_http_trace}` become compilable from `soma-mcp` when built with `--features http`, and *not* compilable (module absent) without it.

- [ ] **Step 1: Change the Cargo feature**

In `crates/soma/mcp/Cargo.toml`, change:

```toml
http = ["dep:axum", "rmcp/transport-streamable-http-server", "soma-mcp-server/http"]
```

to:

```toml
http = ["dep:axum", "rmcp/transport-streamable-http-server", "soma-mcp-server/http", "rmcp-traces/http"]
```

- [ ] **Step 2: Verify both feature combinations still compile**

Run: `cargo check -p soma-mcp --no-default-features --features stdio` (stdio-only, no `http` — must succeed with no `rmcp_traces::http` references anywhere yet, since Task 5 hasn't landed).
Expected: PASS.

Run: `cargo check -p soma-mcp --features http`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/soma/mcp/Cargo.toml
git commit -m "feat(soma-mcp): wire rmcp-traces/http behind the existing http feature"
```

---

## Task 5: `trace_resolution` module — pure resolution logic + unit tests

**Files:**
- Create: `crates/soma/mcp/src/trace_resolution.rs`
- Create: `crates/soma/mcp/src/trace_resolution_tests.rs`
- Modify: `crates/soma/mcp/src/lib.rs`

**Interfaces:**
- Consumes: `rmcp::model::Meta`, `rmcp_traces::{TraceSummary, TraceTrust, TRACEPARENT_KEY, TRACESTATE_KEY, BAGGAGE_KEY}`, `rmcp_traces::http::{HttpTracePolicy, extract_http_trace}` (feature-gated), `soma_config::TraceHeaderMode`, `soma_domain::TraceContext`, `soma_mcp_server::trace::{trace_summary_from_meta, raw_trace_fields_from_meta}` (already used pre-refactor in `rmcp_server.rs`).
- Produces:
  - `pub(crate) struct TraceResolution { pub summary: TraceSummary, pub trace_context: Option<TraceContext>, pub http_trace_headers_present: bool }`
  - `pub(crate) fn trace_context_from_meta(meta: &Meta) -> Option<TraceContext>`
  - `pub(crate) fn resolve_trace_resolution(mode: TraceHeaderMode, meta: &Meta, headers: Option<&::http::HeaderMap>) -> TraceResolution` — pure, no I/O, fully unit-testable.
  - `pub(crate) fn meta_has_any_trace_key(meta: &Meta) -> bool` — exposed (not just private) so `rmcp_server.rs` can compute the `trace_context_conflict` log field inline at the `call_tool` call site (Task 6) instead of `TraceResolution` carrying it as a redundant, hand-maintained field. It is always exactly `resolution.http_trace_headers_present && meta_has_any_trace_key(&context.meta)` — storing it separately risked one branch of `resolve_trusted` silently drifting out of sync with that invariant on a future edit.

- [ ] **Step 1: Write the failing tests**

Create `crates/soma/mcp/src/trace_resolution_tests.rs`:

```rust
use super::*;
use ::http::{HeaderMap, HeaderValue};
use rmcp::model::Meta;
use soma_config::TraceHeaderMode;

const VALID_TRACEPARENT: &str = "00-0af7651916cd43dd8448eb211c80319c-00f067aa0ba902b7-01";
const OTHER_TRACEPARENT: &str = "00-11112222333344445555666677778888-1111222233334444-01";

fn headers_with(pairs: &[(&'static str, &str)]) -> HeaderMap {
    let mut headers = HeaderMap::new();
    for (name, value) in pairs {
        headers.insert(*name, HeaderValue::from_str(value).expect("valid header value"));
    }
    headers
}

// ── Off mode ───────────────────────────────────────────────────────────────

#[test]
fn off_mode_ignores_http_headers_even_when_present() {
    let meta = Meta::new();
    let headers = headers_with(&[("traceparent", VALID_TRACEPARENT)]);

    let resolution = resolve_trace_resolution(TraceHeaderMode::Off, &meta, Some(&headers));

    assert!(resolution.summary.trace_id_prefix().is_none());
    assert!(!resolution.http_trace_headers_present);
    assert!(resolution.trace_context.is_none());
}

#[test]
fn off_mode_still_summarizes_meta_traceparent() {
    let mut meta = Meta::new();
    meta.set_traceparent(VALID_TRACEPARENT);

    let resolution = resolve_trace_resolution(TraceHeaderMode::Off, &meta, None);

    assert_eq!(resolution.summary.trace_id_prefix(), Some("0af76519"));
    assert!(!resolution.http_trace_headers_present);
}

// ── Trusted mode: no _meta, headers present ──────────────────────────────────
//
// These tests assert real HTTP header parsing, which only exists under the
// `http` Cargo feature (see `resolve_trusted`'s two `#[cfg]` variants in
// trace_resolution.rs) — gate them accordingly so `cargo test -p soma-mcp
// --no-default-features --features stdio` doesn't try to run assertions that
// only hold when `rmcp-traces/http` is actually compiled in.

#[cfg(feature = "http")]
#[test]
fn trusted_mode_extracts_traceparent_and_tracestate_from_headers() {
    let meta = Meta::new();
    let headers = headers_with(&[
        ("traceparent", VALID_TRACEPARENT),
        ("tracestate", "vendor=value"),
    ]);

    let resolution = resolve_trace_resolution(TraceHeaderMode::Trusted, &meta, Some(&headers));

    assert_eq!(resolution.summary.trace_id_prefix(), Some("0af76519"));
    assert!(resolution.summary.has_tracestate());
    assert!(resolution.http_trace_headers_present);
    let trace_context = resolution.trace_context.expect("trace context should be set");
    assert_eq!(trace_context.traceparent.as_deref(), Some(VALID_TRACEPARENT));
    assert_eq!(trace_context.tracestate.as_deref(), Some("vendor=value"));
}

#[cfg(feature = "http")]
#[test]
fn trusted_mode_strips_baggage_even_when_present() {
    let meta = Meta::new();
    let headers = headers_with(&[
        ("traceparent", VALID_TRACEPARENT),
        ("baggage", "region=us-east-1"),
    ]);

    let resolution = resolve_trace_resolution(TraceHeaderMode::Trusted, &meta, Some(&headers));

    assert_eq!(resolution.summary.baggage_member_count(), 0);
}

#[cfg(feature = "http")]
#[test]
fn trusted_with_baggage_mode_summarizes_baggage_safely() {
    let meta = Meta::new();
    let headers = headers_with(&[
        ("traceparent", VALID_TRACEPARENT),
        ("baggage", "region=us-east-1,accessToken=super-secret-token"),
    ]);

    let resolution =
        resolve_trace_resolution(TraceHeaderMode::TrustedWithBaggage, &meta, Some(&headers));

    assert_eq!(resolution.summary.baggage_member_count(), 2);
    assert_eq!(resolution.summary.sensitive_baggage_member_count(), 1);
}

#[test]
fn trusted_mode_with_no_headers_falls_back_to_meta_only() {
    let meta = Meta::new();
    let resolution = resolve_trace_resolution(TraceHeaderMode::Trusted, &meta, None);
    assert!(resolution.summary.trace_id_prefix().is_none());
    assert!(!resolution.http_trace_headers_present);
}

// Not `#[cfg(feature = "http")]`-gated: under the `http`-enabled path,
// `extract_http_trace` requires a valid `traceparent` before it will even
// look at `tracestate`, so `tracestate` alone yields the same "ignored"
// result the `stdio`-only fallback also produces — this assertion holds
// identically either way. (`xtask test-trace-headers`, Task 12, previously
// duplicated this exact business rule as a live-smoke check; this unit test
// replaces that duplication — `rmcp-traces` bead `.1` already proved the
// underlying `TraceStateRequiresTraceParent` parsing rule, so this test only
// needs to prove `soma-mcp`'s wiring respects it, not re-derive it.)
#[test]
fn trusted_mode_ignores_tracestate_without_a_valid_traceparent() {
    let meta = Meta::new();
    let headers = headers_with(&[("tracestate", "vendor=value")]);

    let resolution = resolve_trace_resolution(TraceHeaderMode::Trusted, &meta, Some(&headers));

    assert!(!resolution.summary.has_tracestate());
    assert!(resolution.trace_context.is_none());
}

#[cfg(feature = "http")]
#[test]
fn trusted_mode_records_invalid_reason_for_malformed_traceparent() {
    let meta = Meta::new();
    let headers = headers_with(&[("traceparent", "not-a-valid-traceparent")]);

    let resolution = resolve_trace_resolution(TraceHeaderMode::Trusted, &meta, Some(&headers));

    assert!(resolution.summary.trace_id_prefix().is_none());
    assert!(resolution.summary.invalid_count() > 0);
    assert!(resolution.trace_context.is_none());
}

// ── _meta vs HTTP conflict ────────────────────────────────────────────────────
//
// Conflict-flag assertions below also require the `http` feature: the
// stdio-only fallback never inspects `headers` at all, so it can never set
// `http_trace_headers_present`/`trace_context_conflict` to `true`.

#[cfg(feature = "http")]
#[test]
fn meta_traceparent_wins_and_http_extraction_is_skipped() {
    let mut meta = Meta::new();
    meta.set_traceparent(VALID_TRACEPARENT);
    let headers = headers_with(&[("traceparent", OTHER_TRACEPARENT)]);

    let resolution = resolve_trace_resolution(TraceHeaderMode::Trusted, &meta, Some(&headers));

    // Meta wins: the summary reflects the _meta traceparent, not the header one.
    assert_eq!(resolution.summary.trace_id_prefix(), Some("0af76519"));
    assert!(resolution.http_trace_headers_present, "presence flag should still be set");
    // The conflict itself is asserted via `meta_has_any_trace_key(&meta) && resolution.http_trace_headers_present`
    // — see the `meta_has_any_trace_key` tests below; `TraceResolution` does
    // not carry a separate `trace_context_conflict` field.
    assert!(meta_has_any_trace_key(&meta));
}

#[cfg(feature = "http")]
#[test]
fn meta_baggage_key_alone_triggers_conflict_detection_without_traceparent() {
    let mut meta = Meta::new();
    meta.set_baggage("region=us-east-1");
    let headers = headers_with(&[("traceparent", VALID_TRACEPARENT)]);

    let resolution =
        resolve_trace_resolution(TraceHeaderMode::TrustedWithBaggage, &meta, Some(&headers));

    assert!(resolution.http_trace_headers_present);
    assert!(meta_has_any_trace_key(&meta));
    // The HTTP baggage value must never be counted when _meta already carries a trace key.
    assert_eq!(resolution.summary.baggage_member_count(), 0);
}

#[test]
fn meta_trace_key_present_but_no_headers_means_no_conflict() {
    // Conflict = `http_trace_headers_present && meta_has_any_trace_key(&meta)`.
    // With no headers at all, `http_trace_headers_present` is false, so there
    // is no conflict even though `_meta` carries a trace key.
    let mut meta = Meta::new();
    meta.set_traceparent(VALID_TRACEPARENT);

    let resolution = resolve_trace_resolution(TraceHeaderMode::Trusted, &meta, None);

    assert!(!resolution.http_trace_headers_present);
    assert!(meta_has_any_trace_key(&meta), "meta itself does carry a trace key");
}

// ── meta_has_any_trace_key ────────────────────────────────────────────────────

#[test]
fn meta_has_any_trace_key_detects_each_key_independently() {
    assert!(!meta_has_any_trace_key(&Meta::new()));

    let mut traceparent_only = Meta::new();
    traceparent_only.set_traceparent(VALID_TRACEPARENT);
    assert!(meta_has_any_trace_key(&traceparent_only));

    let mut tracestate_only = Meta::new();
    tracestate_only.set_tracestate("vendor=value");
    assert!(meta_has_any_trace_key(&tracestate_only));

    let mut baggage_only = Meta::new();
    baggage_only.set_baggage("region=us-east-1");
    assert!(meta_has_any_trace_key(&baggage_only));
}

// ── trace_context_from_meta ────────────────────────────────────────────────────

#[test]
fn trace_context_from_meta_is_none_without_a_valid_traceparent() {
    let meta = Meta::new();
    assert!(trace_context_from_meta(&meta).is_none());
}

#[test]
fn trace_context_from_meta_returns_traceparent_and_tracestate() {
    let mut meta = Meta::new();
    meta.set_traceparent(VALID_TRACEPARENT);
    meta.set_tracestate("vendor=value");

    let context = trace_context_from_meta(&meta).expect("trace context should be present");
    assert_eq!(context.traceparent.as_deref(), Some(VALID_TRACEPARENT));
    assert_eq!(context.tracestate.as_deref(), Some("vendor=value"));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p soma-mcp --features http trace_resolution`
Expected: FAIL — compile error, `crate::trace_resolution` module does not exist yet.

- [ ] **Step 3: Implement `trace_resolution.rs`**

Create `crates/soma/mcp/src/trace_resolution.rs`:

```rust
//! Maps Soma's typed [`TraceHeaderMode`] config onto `rmcp-traces`' request-side
//! trace extraction, for both RMCP `_meta` (always available) and, when
//! trusted, inbound HTTP headers (only under the `http` Cargo feature).
//!
//! This is the one place in `soma-mcp` that turns a trust-boundary decision
//! (already made by `soma-runtime::server::resolve_auth_policy_kind`) into
//! actual header parsing. `_meta` always wins over HTTP headers: when `_meta`
//! carries any trace key, HTTP header values are never parsed, joined,
//! counted, or logged — only safe presence booleans are recorded.

use rmcp::model::Meta;
use rmcp_traces::{TraceSummary, TraceTrust, BAGGAGE_KEY, TRACEPARENT_KEY, TRACESTATE_KEY};
use soma_config::TraceHeaderMode;
use soma_domain::TraceContext;

#[cfg(test)]
#[path = "trace_resolution_tests.rs"]
mod tests;

/// Everything a `call_tool` invocation needs to log and propagate about trace
/// metadata for exactly one authenticated call.
pub(crate) struct TraceResolution {
    pub summary: TraceSummary,
    pub trace_context: Option<TraceContext>,
    /// Set only when trace-header extraction is enabled (non-`Off` mode) *and*
    /// at least one of `traceparent`/`tracestate`/`baggage` was present on the
    /// inbound HTTP request — regardless of whether it was used.
    ///
    /// There is deliberately no separate `trace_context_conflict` field here:
    /// a `_meta`/HTTP conflict is exactly `http_trace_headers_present &&
    /// meta_has_any_trace_key(meta)` — always derivable from this field plus
    /// the `Meta` the caller already has in hand, so callers (`call_tool` in
    /// `rmcp_server.rs`) compute it inline instead of `TraceResolution`
    /// carrying a second, hand-maintained boolean that could silently drift
    /// out of sync with this one on a future edit to `resolve_trusted`.
    pub http_trace_headers_present: bool,
}

impl TraceResolution {
    pub(crate) fn from_meta_only(meta: &Meta) -> Self {
        Self {
            summary: soma_mcp_server::trace::trace_summary_from_meta(meta, TraceTrust::Untrusted),
            trace_context: trace_context_from_meta(meta),
            http_trace_headers_present: false,
        }
    }
}

/// Recover a product-level [`TraceContext`] (bounded `traceparent`/`tracestate`
/// only — never baggage) from RMCP request `_meta`, gated on a validated
/// trace id. Untrusted by definition: `_meta` is caller-controlled.
pub(crate) fn trace_context_from_meta(meta: &Meta) -> Option<TraceContext> {
    let fields = soma_mcp_server::trace::raw_trace_fields_from_meta(meta, TraceTrust::Untrusted)?;
    Some(TraceContext {
        traceparent: fields.traceparent,
        tracestate: fields.tracestate,
    })
}

/// Whether RMCP `_meta` already carries any trace key. Exposed at
/// `pub(crate)` (not private) so `rmcp_server.rs::call_tool` can compute the
/// `trace_context_conflict` log field as `resolution.http_trace_headers_present
/// && meta_has_any_trace_key(&context.meta)` at the log call site, rather
/// than `TraceResolution` duplicating that boolean as a stored field.
pub(crate) fn meta_has_any_trace_key(meta: &Meta) -> bool {
    meta.get(TRACEPARENT_KEY).is_some()
        || meta.get(TRACESTATE_KEY).is_some()
        || meta.get(BAGGAGE_KEY).is_some()
}

/// Resolve trace metadata for one authenticated `call_tool` invocation.
///
/// `headers` should be `None` whenever the caller has not already confirmed
/// `mode != TraceHeaderMode::Off` — callers own the "off mode does zero HTTP
/// lookup" guarantee by not fetching `RequestContext.extensions` at all in
/// that case (see `rmcp_server.rs::trace_resolution_for_call`).
pub(crate) fn resolve_trace_resolution(
    mode: TraceHeaderMode,
    meta: &Meta,
    headers: Option<&::http::HeaderMap>,
) -> TraceResolution {
    match mode {
        TraceHeaderMode::Off => TraceResolution::from_meta_only(meta),
        TraceHeaderMode::Trusted | TraceHeaderMode::TrustedWithBaggage => {
            resolve_trusted(mode, meta, headers)
        }
    }
}

#[cfg(feature = "http")]
fn resolve_trusted(
    mode: TraceHeaderMode,
    meta: &Meta,
    headers: Option<&::http::HeaderMap>,
) -> TraceResolution {
    if meta_has_any_trace_key(meta) {
        let mut resolution = TraceResolution::from_meta_only(meta);
        resolution.http_trace_headers_present =
            headers.is_some_and(headers_have_any_trace_key);
        return resolution;
    }
    let Some(headers) = headers else {
        return TraceResolution::from_meta_only(meta);
    };
    let policy = rmcp_traces::http::HttpTracePolicy {
        trust: TraceTrust::Trusted,
        limits: Default::default(),
        include_baggage: matches!(mode, TraceHeaderMode::TrustedWithBaggage),
    };
    let extraction = rmcp_traces::http::extract_http_trace(headers, policy);
    TraceResolution {
        trace_context: trace_context_from_http_extraction(&extraction),
        summary: extraction.summary,
        http_trace_headers_present: true,
    }
}

#[cfg(feature = "http")]
fn headers_have_any_trace_key(headers: &::http::HeaderMap) -> bool {
    headers.contains_key(TRACEPARENT_KEY)
        || headers.contains_key(TRACESTATE_KEY)
        || headers.contains_key(BAGGAGE_KEY)
}

#[cfg(feature = "http")]
fn trace_context_from_http_extraction(
    extraction: &rmcp_traces::http::HttpTraceExtraction,
) -> Option<TraceContext> {
    extraction.summary.trace_id_prefix()?;
    Some(TraceContext {
        traceparent: extraction.meta.get_traceparent().map(ToOwned::to_owned),
        tracestate: extraction
            .summary
            .has_tracestate()
            .then(|| extraction.meta.get_tracestate().map(ToOwned::to_owned))
            .flatten(),
    })
}

// Defensive fallback: config validation (Task 3) already prevents a non-`off`
// trace-header mode from being reachable on a build compiled without the
// `http` feature (there is no HTTP transport to source headers from), but
// keep the crate compiling either way rather than depending on that
// invariant holding across every future call site.
#[cfg(not(feature = "http"))]
fn resolve_trusted(
    _mode: TraceHeaderMode,
    meta: &Meta,
    _headers: Option<&::http::HeaderMap>,
) -> TraceResolution {
    TraceResolution::from_meta_only(meta)
}
```

- [ ] **Step 4: Register the module**

In `crates/soma/mcp/src/lib.rs`, add (alphabetically, after `state`, before `tools`):

```rust
mod trace_resolution;
```

(No `pub`/`pub(crate) use` re-export needed yet — `rmcp_server.rs` will reach it via `super::trace_resolution` in Task 6.)

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p soma-mcp --features http trace_resolution`
Expected: PASS (14 new tests — all of them, since `--features http` compiles the 6 `#[cfg(feature = "http")]`-gated tests too).

Run: `cargo test -p soma-mcp --no-default-features --features stdio trace_resolution`
Expected: PASS (8 tests — the 6 tests that assert real HTTP-extraction behavior are `#[cfg(feature = "http")]`-gated in Step 1 and do not run here). This confirms the `#[cfg(not(feature = "http"))] resolve_trusted` fallback compiles and the `Off`-mode / no-headers / meta-only / `meta_has_any_trace_key` tests still pass without the `http` feature.

- [ ] **Step 6: Lint and format**

Run: `cargo fmt -p soma-mcp && cargo clippy -p soma-mcp --all-targets --features http -- -D warnings && cargo clippy -p soma-mcp --all-targets --no-default-features --features stdio -- -D warnings`
Expected: clean under both feature combinations.

- [ ] **Step 7: Commit**

```bash
git add crates/soma/mcp/src/trace_resolution.rs crates/soma/mcp/src/trace_resolution_tests.rs crates/soma/mcp/src/lib.rs
git commit -m "feat(soma-mcp): add trace_resolution module for trusted HTTP trace headers"
```

---

## Task 6: Wire `trace_resolution` into `SomaRmcpServer::call_tool`

**Files:**
- Modify: `crates/soma/mcp/src/rmcp_server.rs`

**Interfaces:**
- Consumes: `trace_resolution::{TraceResolution, resolve_trace_resolution, meta_has_any_trace_key}` (Task 5).
- Produces: a new `execution_context_with_trace(state, auth, trace: Option<TraceContext>)` function used only by `call_tool`. **The existing `execution_context(state, request, auth)` function and its 5 non-`call_tool` call sites (`list_tools`, `list_resources`, `read_resource`, `list_prompts`, `get_prompt`) are left completely untouched** — an earlier draft of this plan changed that shared function's signature and mechanically edited all 6 call sites for a change that only `call_tool` needed; adding one small sibling function instead means the diff for those 5 methods is zero lines, and their existing behavior/tests cannot regress because nothing about them changed. `call_tool` computes `TraceResolution` exactly once, after auth succeeds, and logs `http_trace_headers_present` plus an inline-computed `trace_context_conflict` alongside the existing trace fields — `trace_context_conflict` is not a stored field (see Task 5), it is `resolution.http_trace_headers_present && trace_resolution::meta_has_any_trace_key(&context.meta)`, computed once at the log call site.

- [ ] **Step 1: Remove the now-superseded `TraceSummary`-only helpers**

In `crates/soma/mcp/src/rmcp_server.rs`, delete these two functions entirely (they are replaced by `trace_resolution`):

```rust
fn trace_summary_from_context(context: &RequestContext<RoleServer>) -> TraceSummary {
    trace_summary_from_meta(&context.meta)
}

fn trace_summary_from_meta(meta: &rmcp::model::Meta) -> TraceSummary {
    soma_mcp_server::trace::trace_summary_from_meta(meta, TraceTrust::Untrusted)
}
```

**Do not delete `trace_context_from_meta`** — the existing (unchanged) `execution_context()` still calls it internally for the 5 non-`call_tool` methods, so it must stay exactly as-is:

```rust
fn trace_context_from_meta(meta: &rmcp::model::Meta) -> Option<TraceContext> {
    let fields = soma_mcp_server::trace::raw_trace_fields_from_meta(meta, TraceTrust::Untrusted)?;
    Some(TraceContext {
        traceparent: fields.traceparent,
        tracestate: fields.tracestate,
    })
}
```

(Yes, `trace_resolution.rs` (Task 5) has its own near-identical private helper for the same purpose — that one builds `TraceResolution.trace_context` internally and is not exposed outside the module. The two are not the same function reachable from two names; keeping `rmcp_server.rs`'s copy avoids widening `trace_resolution`'s public surface for a caller that, after this task, no longer exists.)

`TraceSummary` becomes unused after removing the two functions above (the macro's `.method()` calls on `$trace_resolution.summary` resolve via the value's inferred type, not via an imported type name, so no import is needed for the macro itself). `TraceTrust` stays used by the retained `trace_context_from_meta`. Change the import line from:

```rust
use rmcp_traces::{TraceSummary, TraceTrust};
```

to:

```rust
use rmcp_traces::TraceTrust;
```

Add `trace_resolution` to the `use super::{...}` block near the top of the file — change:

```rust
use super::{
    gateway_proxy, prompts,
    protocol_errors::{application_error_payload, tool_error_result, unknown_tool_error},
    rmcp_auth::{
        principal, protected_route_scope, protected_scope_allows_service, require_auth_context,
        AuthContext,
    },
    schemas::tool_definitions_for_catalogs as tool_definitions,
    state::McpState,
    tools::execute_tool,
    ACTION_DISCRIMINATOR_FIELD,
};
```

to:

```rust
use super::{
    gateway_proxy, prompts,
    protocol_errors::{application_error_payload, tool_error_result, unknown_tool_error},
    rmcp_auth::{
        principal, protected_route_scope, protected_scope_allows_service, require_auth_context,
        AuthContext,
    },
    schemas::tool_definitions_for_catalogs as tool_definitions,
    state::McpState,
    tools::execute_tool,
    trace_resolution,
    ACTION_DISCRIMINATOR_FIELD,
};
```

- [ ] **Step 2: Update the `trace_summary_event!` macro**

Replace the macro definition:

```rust
macro_rules! trace_summary_event {
    ($level:ident, $trace_summary:expr, $message:literal, $($field:tt)*) => {
        tracing::$level!(
            $($field)*
            trace_id_prefix = ?$trace_summary.trace_id_prefix(),
            span_id_prefix = ?$trace_summary.span_id_prefix(),
            trace_sampled = ?$trace_summary.sampled(),
            trace_trust = ?$trace_summary.trust(),
            has_tracestate = $trace_summary.has_tracestate(),
            baggage_member_count = $trace_summary.baggage_member_count(),
            sensitive_baggage_member_count = $trace_summary.sensitive_baggage_member_count(),
            trace_invalid_count = $trace_summary.invalid_count(),
            trace_invalid_reasons = ?$trace_summary.invalid_reasons(),
            $message
        );
    };
}
```

with:

```rust
macro_rules! trace_summary_event {
    ($level:ident, $trace_resolution:expr, $trace_context_conflict:expr, $message:literal, $($field:tt)*) => {
        tracing::$level!(
            $($field)*
            trace_id_prefix = ?$trace_resolution.summary.trace_id_prefix(),
            span_id_prefix = ?$trace_resolution.summary.span_id_prefix(),
            trace_sampled = ?$trace_resolution.summary.sampled(),
            trace_trust = ?$trace_resolution.summary.trust(),
            has_tracestate = $trace_resolution.summary.has_tracestate(),
            baggage_member_count = $trace_resolution.summary.baggage_member_count(),
            sensitive_baggage_member_count = $trace_resolution.summary.sensitive_baggage_member_count(),
            trace_invalid_count = $trace_resolution.summary.invalid_count(),
            trace_invalid_reasons = ?$trace_resolution.summary.invalid_reasons(),
            http_trace_headers_present = $trace_resolution.http_trace_headers_present,
            trace_context_conflict = $trace_context_conflict,
            $message
        );
    };
}
```

`trace_context_conflict` becomes an explicit second positional macro argument (computed once in `call_tool`, Step 3) rather than a field read off `$trace_resolution` — `TraceResolution` does not store it (see Task 5's rationale: it is always exactly `http_trace_headers_present && meta_has_any_trace_key(&meta)`, so storing it separately risked drift).

- [ ] **Step 3: Update `call_tool`**

Replace:

```rust
        let trace_summary = trace_summary_from_context(&context);
        let route_scope = protected_route_scope(&context);
        let execution_context = execution_context(&self.state, &context, auth);
```

with:

```rust
        let trace_resolution = trace_resolution_for_call(&self.state, &context);
        let trace_context_conflict = trace_resolution.http_trace_headers_present
            && trace_resolution::meta_has_any_trace_key(&context.meta);
        let route_scope = protected_route_scope(&context);
        let execution_context = execution_context_with_trace(
            &self.state,
            auth,
            trace_resolution.trace_context.clone(),
        );
```

Every remaining reference to `trace_summary` inside `call_tool` (the 6 `trace_summary_event!(..., trace_summary, "...", ...)` call sites) must be renamed to pass BOTH `trace_resolution` and `trace_context_conflict` as the macro's first two expression arguments — e.g. `trace_summary_event!(warn, trace_summary, "MCP tool rejected unknown tool", ...)` becomes `trace_summary_event!(warn, trace_resolution, trace_context_conflict, "MCP tool rejected unknown tool", ...)`. This is a plain find-and-replace of the identifier plus inserting the new argument, within `call_tool`'s body only (do not touch other methods — they don't use this macro).

- [ ] **Step 4: Add `execution_context_with_trace` and `trace_resolution_for_call` — leave `execution_context()` untouched**

`execution_context()` (used by `list_tools`, `list_resources`, `read_resource`, `list_prompts`, `get_prompt`) needs **zero changes** — it already computes exactly what those 5 methods need (meta-only trace context, via the retained `trace_context_from_meta`), and none of those 5 methods are in scope for HTTP trace-header extraction (the epic's "per authenticated tool call" language, and bead `.3`'s "compute exactly one `TraceSummary` per authenticated tool call", both point at `call_tool` specifically). Do not edit `execution_context()` or any of its 5 call sites.

Add two new functions near `execution_context()` (after it, before `trace_context_from_meta`):

```rust
/// Like `execution_context`, but takes an already-resolved trace context
/// instead of deriving one from `request.meta` internally. Used only by
/// `call_tool`, which may resolve trace context from trusted HTTP headers
/// (see `trace_resolution_for_call`) rather than `_meta` alone. A future
/// handler added alongside `call_tool` should default to `execution_context`
/// (meta-only) unless it specifically needs HTTP-aware trace resolution too.
fn execution_context_with_trace(
    state: &McpState,
    auth: Option<&AuthContext>,
    trace: Option<TraceContext>,
) -> ExecutionContext {
    state.execution_context(Some(principal(auth)), trace)
}

/// Resolve trace metadata for one authenticated `call_tool` invocation. `Off`
/// mode returns without ever touching `RequestContext.extensions` — no HTTP
/// header lookup, no `Parts` clone, no config string matching on the hot path.
fn trace_resolution_for_call(
    state: &McpState,
    context: &RequestContext<RoleServer>,
) -> trace_resolution::TraceResolution {
    let mode = state.config().trace_headers;
    if mode == soma_config::TraceHeaderMode::Off {
        return trace_resolution::TraceResolution::from_meta_only(&context.meta);
    }
    let headers = context
        .extensions
        .get::<http::request::Parts>()
        .map(|parts| &parts.headers);
    trace_resolution::resolve_trace_resolution(mode, &context.meta, headers)
}
```

- [ ] **Step 5: Run existing tests to verify nothing regressed**

Run: `cargo test -p soma-mcp --features http`
Expected: PASS — all pre-existing tests in `rmcp_server_tests.rs`, `mcp_tests.rs`, etc. still pass unchanged (behavior for `Off` mode, the default, is identical to before this refactor).

Run: `cargo test -p soma-mcp --no-default-features --features stdio`
Expected: PASS.

- [ ] **Step 6: Lint and format**

Run: `cargo fmt -p soma-mcp && cargo clippy -p soma-mcp --all-targets --features http -- -D warnings && cargo clippy -p soma-mcp --all-targets --no-default-features --features stdio -- -D warnings`
Expected: clean. (Step 1 already narrowed the `rmcp_traces` import to just `TraceTrust`, dropping the now-unused `TraceSummary`. `use soma_domain::{token_limit::MAX_RESPONSE_BYTES, TraceContext};` stays exactly as-is — both `execution_context()` and the new `execution_context_with_trace()` still name `TraceContext`.)

- [ ] **Step 7: Commit**

```bash
git add crates/soma/mcp/src/rmcp_server.rs
git commit -m "feat(soma-mcp): consume trusted HTTP trace headers in call_tool"
```

---

## Task 7: Test helper — `loopback_state_with_mcp_config`

**Files:**
- Modify: `apps/soma/src/lib.rs`

**Interfaces:**
- Produces: `pub fn loopback_state_with_mcp_config(config: McpConfig) -> AppState` in the `testing` module (same visibility/feature-gating as the existing `loopback_state()`).

- [ ] **Step 1: Add the helper**

In `apps/soma/src/lib.rs`'s `pub mod testing { ... }` block, add right after `pub fn loopback_state_with_registry(...)`:

```rust
    /// `AppState` with no auth (loopback trust boundary) and a caller-supplied
    /// `McpConfig` — use when a test needs to vary MCP-level config (e.g.
    /// trace-header mode) while keeping the loopback trust boundary.
    pub fn loopback_state_with_mcp_config(config: McpConfig) -> AppState {
        let service = stub_service();
        let provider_registry =
            soma_application::static_provider_registry(service.clone()).expect("static registry");
        state(
            config,
            AuthPolicy::LoopbackDev,
            service,
            provider_registry,
            empty_gateway_product_state(),
        )
    }
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo check -p soma --features full,test-support`
Expected: PASS (no test yet consumes it — Task 8 will).

- [ ] **Step 3: Commit**

Do not commit yet — fold this into Task 8's commit, since it has no independent test coverage on its own. Proceed directly to Task 8.

---

## Task 8: Integration test — real streamable-HTTP round trip

**Files:**
- Create: `apps/soma/tests/mcp_trace_headers.rs`

**Interfaces:**
- Consumes: `soma::testing::{loopback_state_with_mcp_config, trusted_gateway_state_with_mcp_config, bearer_state_with_mcp_config}` (Task 7 + this task's Step 1), `soma::server::{router, AppState}`, `soma_config::{McpConfig, TraceHeaderMode}`, `soma_test_support::{tracing_test_lock, SharedBuf}`, `rmcp::transport::StreamableHttpClientTransport`, `reqwest::Client`.
- Produces: end-to-end proof that inbound HTTP `traceparent`/`tracestate`/`baggage` headers reach `call_tool` through the real `/mcp` route and produce the expected safe log fields — the acceptance-criterion test that "direct construction of extensions is not sufficient as the only proof." Two shared test helpers (`ServerHandle`, `TracingCapture`) so all 5 tests in this file share one spawn/capture path instead of each hand-rolling it.

- [ ] **Step 1: Add the two remaining `AppState` test helpers**

Bead `.3`'s acceptance criteria name three deployment shapes trace-header consumption must be proven under: `LoopbackDev` (Task 7's `loopback_state_with_mcp_config`), `TrustedGatewayUnscoped` ("LoopbackDev and TrustedGatewayUnscoped may return no authenticated principal while still allowing tool execution... tests must prove config gates HTTP header consumption in these modes too"), and a defense-in-depth proof that mounted-auth failure never leaks trace fields ("Mounted auth failure and pre-auth paging rejection tests include HTTP traceparent, tracestate, and baggage headers and assert no trace fields... reach logs"). Add the remaining two helpers to `apps/soma/src/lib.rs`'s `testing` module, right after `loopback_state_with_mcp_config` (Task 7):

```rust
    /// `AppState` with `AuthPolicy::TrustedGatewayUnscoped` and a
    /// caller-supplied `McpConfig` — the deployment shape where an upstream
    /// gateway/proxy is trusted to enforce auth *and* strip untrusted inbound
    /// trace headers before traffic reaches Soma.
    pub fn trusted_gateway_state_with_mcp_config(config: McpConfig) -> AppState {
        let service = stub_service();
        let provider_registry =
            soma_application::static_provider_registry(service.clone()).expect("static registry");
        state(
            config,
            AuthPolicy::TrustedGatewayUnscoped,
            service,
            provider_registry,
            empty_gateway_product_state(),
        )
    }

    /// `AppState` requiring a static bearer token, with a caller-supplied
    /// `McpConfig` — use only for defense-in-depth tests that deliberately
    /// construct a combination `resolve_auth_policy_kind` would reject at
    /// startup (see Task 3: `MountedBearer` + non-`off` trace headers), to
    /// prove the request path itself is safe even if that startup guard were
    /// ever bypassed. This directly satisfies bead `rmcp-template-mdei.3`'s
    /// written acceptance criterion for mounted-auth-failure coverage — it is
    /// not incidental extra testing.
    pub fn bearer_state_with_mcp_config(token: &str, mut config: McpConfig) -> AppState {
        config.api_token = Some(token.to_string());
        let service = stub_service();
        let provider_registry =
            soma_application::static_provider_registry(service.clone()).expect("static registry");
        state(
            config,
            mounted_test_policy(),
            service,
            provider_registry,
            empty_gateway_product_state(),
        )
    }
```

- [ ] **Step 2: Write the test file**

Create `apps/soma/tests/mcp_trace_headers.rs`:

```rust
//! Real Streamable HTTP round trip proving trusted HTTP trace-header
//! extraction works through the actual `/mcp` route — not just the pure
//! `trace_resolution::resolve_trace_resolution` unit tests in
//! `crates/soma/mcp/src/trace_resolution_tests.rs`. Mirrors the server-spawn
//! pattern from `mcp_http_roundtrip.rs` and the tracing-capture pattern from
//! `dispatch_logging.rs`.
#![cfg(feature = "mcp-http")]

use std::net::TcpListener as StdTcpListener;

use reqwest::header::{HeaderMap, HeaderValue};
use rmcp::{
    model::CallToolRequestParams, service::ServiceExt, transport::StreamableHttpClientTransport,
};
use serde_json::json;
use soma::server::AppState;
use soma_config::{McpConfig, TraceHeaderMode};
use soma_test_support::{tracing_test_lock, SharedBuf};

const VALID_TRACEPARENT: &str = "00-0af7651916cd43dd8448eb211c80319c-00f067aa0ba902b7-01";

/// Real axum-served `/mcp` endpoint on a loopback TCP port, bound to a
/// caller-supplied `AppState` so every test in this file (`LoopbackDev`,
/// `TrustedGatewayUnscoped`, `Mounted`) shares one spawn path. Aborts its
/// spawned task on `Drop`, not just on the happy path — an earlier draft of
/// this test file called `server_handle.abort()` as the last line of each
/// test body, which never ran if an earlier `?` (e.g. a slow first-connect
/// erroring out) short-circuited the function first.
struct ServerHandle {
    port: u16,
    join: Option<tokio::task::JoinHandle<()>>,
}

impl ServerHandle {
    async fn spawn(state: AppState) -> anyhow::Result<Self> {
        let std_listener = StdTcpListener::bind("127.0.0.1:0")?;
        let port = std_listener.local_addr()?.port();
        std_listener.set_nonblocking(true)?;
        let listener = tokio::net::TcpListener::from_std(std_listener)?;

        let app = soma::server::router(state);
        let join = tokio::spawn(async move {
            if let Err(err) = axum::serve(listener, app.into_make_service()).await {
                eprintln!("trace-header test server exited with error: {err}");
            }
        });
        Ok(Self { port, join: Some(join) })
    }

    fn port(&self) -> u16 {
        self.port
    }
}

impl Drop for ServerHandle {
    fn drop(&mut self) {
        if let Some(join) = self.join.take() {
            join.abort();
        }
    }
}

/// Bundles the tracing-capture subscriber setup every test in this file
/// needs. `tracing_test_lock()` is intentionally kept as a separate `let
/// _lock = tracing_test_lock();` at each call site (not folded in here) —
/// see `dispatch_logging.rs`'s comment: it must be held across the whole
/// `#[tokio::test(flavor = "current_thread")]` body, including awaits.
struct TracingCapture {
    buf: SharedBuf,
    guard: tracing::subscriber::DefaultGuard,
}

impl TracingCapture {
    fn start() -> Self {
        let buf = SharedBuf::new();
        let subscriber = tracing_subscriber::fmt()
            .with_writer(buf.writer())
            .with_ansi(false)
            .without_time()
            .finish();
        let guard = tracing::subscriber::set_default(subscriber);
        Self { buf, guard }
    }

    /// Restore the previous default subscriber, then return everything
    /// captured. Drop-before-read matches `dispatch_logging.rs`'s existing
    /// `drop(guard); let logs = buf.contents();` ordering.
    fn finish(self) -> String {
        drop(self.guard);
        self.buf.contents()
    }
}

fn client_with_headers(headers: &[(&str, &str)]) -> reqwest::Client {
    let mut header_map = HeaderMap::new();
    for (name, value) in headers {
        header_map.insert(*name, HeaderValue::from_str(value).expect("valid header value"));
    }
    reqwest::Client::builder()
        .default_headers(header_map)
        .build()
        .expect("reqwest client should build")
}

async fn call_status(port: u16, client: reqwest::Client) -> anyhow::Result<()> {
    let url = format!("http://127.0.0.1:{port}/mcp");
    let transport = StreamableHttpClientTransport::with_client(
        client,
        rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig::with_uri(
            url,
        ),
    );
    let service = ().serve(transport).await?;
    service
        .call_tool(
            CallToolRequestParams::new("soma")
                .with_arguments(json!({"action": "status"}).as_object().unwrap().clone()),
        )
        .await?;
    service.cancel().await?;
    Ok(())
}

#[allow(clippy::await_holding_lock)]
#[tokio::test(flavor = "current_thread")]
async fn off_mode_never_reports_http_trace_headers_present() -> anyhow::Result<()> {
    let _lock = tracing_test_lock();
    let capture = TracingCapture::start();

    let server = ServerHandle::spawn(soma::testing::loopback_state_with_mcp_config(
        McpConfig::default(),
    ))
    .await?;
    call_status(
        server.port(),
        client_with_headers(&[("traceparent", VALID_TRACEPARENT)]),
    )
    .await?;

    let logs = capture.finish();
    assert!(
        logs.contains("http_trace_headers_present=false"),
        "logs were: {logs}"
    );
    Ok(())
}

#[allow(clippy::await_holding_lock)]
#[tokio::test(flavor = "current_thread")]
async fn trusted_mode_extracts_traceparent_from_a_real_http_request() -> anyhow::Result<()> {
    let _lock = tracing_test_lock();
    let capture = TracingCapture::start();

    let config = McpConfig {
        trace_headers: TraceHeaderMode::Trusted,
        ..McpConfig::default()
    };
    let server = ServerHandle::spawn(soma::testing::loopback_state_with_mcp_config(config)).await?;
    call_status(
        server.port(),
        client_with_headers(&[
            ("traceparent", VALID_TRACEPARENT),
            ("baggage", "region=us-east-1"),
        ]),
    )
    .await?;

    let logs = capture.finish();
    assert!(logs.contains("http_trace_headers_present=true"), "logs were: {logs}");
    assert!(logs.contains("trace_id_prefix=Some(\"0af76519\")"), "logs were: {logs}");
    // Trusted (without baggage) must strip baggage even though the header was sent.
    assert!(logs.contains("baggage_member_count=0"), "logs were: {logs}");
    assert!(
        !logs.contains("us-east-1"),
        "raw baggage value must never reach logs: {logs}"
    );
    Ok(())
}

#[allow(clippy::await_holding_lock)]
#[tokio::test(flavor = "current_thread")]
async fn trusted_with_baggage_mode_summarizes_baggage_without_leaking_raw_values() -> anyhow::Result<()>
{
    let _lock = tracing_test_lock();
    let capture = TracingCapture::start();

    let config = McpConfig {
        trace_headers: TraceHeaderMode::TrustedWithBaggage,
        ..McpConfig::default()
    };
    let server = ServerHandle::spawn(soma::testing::loopback_state_with_mcp_config(config)).await?;
    call_status(
        server.port(),
        client_with_headers(&[
            ("traceparent", VALID_TRACEPARENT),
            ("baggage", "accessToken=super-secret-value"),
        ]),
    )
    .await?;

    let logs = capture.finish();
    assert!(logs.contains("baggage_member_count=1"), "logs were: {logs}");
    assert!(logs.contains("sensitive_baggage_member_count=1"), "logs were: {logs}");
    assert!(
        !logs.contains("super-secret-value"),
        "raw baggage value must never reach logs: {logs}"
    );
    assert!(
        !logs.contains("accessToken"),
        "sensitive baggage KEY name must never reach logs either, only the derived count: {logs}"
    );
    Ok(())
}

#[allow(clippy::await_holding_lock)]
#[tokio::test(flavor = "current_thread")]
async fn trusted_gateway_unscoped_extracts_trace_headers_with_no_principal() -> anyhow::Result<()> {
    let _lock = tracing_test_lock();
    let capture = TracingCapture::start();

    let config = McpConfig {
        trace_headers: TraceHeaderMode::Trusted,
        ..McpConfig::default()
    };
    let server =
        ServerHandle::spawn(soma::testing::trusted_gateway_state_with_mcp_config(config)).await?;
    call_status(
        server.port(),
        client_with_headers(&[("traceparent", VALID_TRACEPARENT)]),
    )
    .await?;

    let logs = capture.finish();
    assert!(logs.contains("trace_id_prefix=Some(\"0af76519\")"), "logs were: {logs}");
    Ok(())
}

#[allow(clippy::await_holding_lock)]
#[tokio::test(flavor = "current_thread")]
async fn mounted_auth_failure_never_emits_trace_fields_even_with_headers_present() -> anyhow::Result<()>
{
    let _lock = tracing_test_lock();
    let capture = TracingCapture::start();

    // Deliberately constructs the combination Task 3's startup validation
    // rejects (MountedBearer + non-off trace headers) to prove the request
    // path is defense-in-depth safe even if that guard were bypassed — see
    // `bearer_state_with_mcp_config`'s doc comment for why this test exists.
    let config = McpConfig {
        trace_headers: TraceHeaderMode::TrustedWithBaggage,
        ..McpConfig::default()
    };
    let server = ServerHandle::spawn(soma::testing::bearer_state_with_mcp_config(
        "expected-token",
        config,
    ))
    .await?;

    // No Authorization header at all -> AuthLayer rejects with 401 before
    // the request ever reaches SomaRmcpServer::call_tool.
    let response = client_with_headers(&[
        ("traceparent", VALID_TRACEPARENT),
        ("tracestate", "vendor=value"),
        ("baggage", "accessToken=super-secret-value"),
    ])
    .post(format!("http://127.0.0.1:{}/mcp", server.port()))
    .header("Content-Type", "application/json")
    .header("Accept", "application/json, text/event-stream")
    .body(r#"{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}"#)
    .send()
    .await?;
    assert_eq!(response.status(), 401);

    let logs = capture.finish();
    assert!(
        !logs.contains("trace_id_prefix"),
        "auth-rejected request must never reach trace-summary logging: {logs}"
    );
    assert!(
        !logs.contains("super-secret-value"),
        "raw baggage must never reach logs even on an unrelated auth-failure path: {logs}"
    );
    Ok(())
}
```

- [ ] **Step 3: Run the test to verify it fails first (red)**

Temporarily verify the test harness itself is sound by checking it against the current `main` behavior — since Tasks 1–7 already landed by this point in plan execution, this test should actually pass immediately. If executing this plan strictly task-by-task, skip the "expect failure" sub-step here (Tasks 1–7 already made this green) and go straight to Step 4.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p soma --features full,test-support --test mcp_trace_headers`
Expected: PASS (5 tests).

- [ ] **Step 5: Run the full existing HTTP test suite to confirm no regression**

Run: `cargo test -p soma --features full,test-support --test mcp_http_roundtrip --test api_routes`
Expected: PASS — unchanged.

- [ ] **Step 6: Lint and format**

Run: `cargo fmt -p soma && cargo clippy -p soma --all-targets --features full,test-support -- -D warnings`
Expected: clean.

- [ ] **Step 7: Commit**

```bash
git add apps/soma/src/lib.rs apps/soma/tests/mcp_trace_headers.rs
git commit -m "test(soma): add real streamable-HTTP round trip for trusted trace headers"
```

This closes bead `rmcp-template-mdei.3`. Run `bd update rmcp-template-mdei.3 --claim` before Task 4, and `bd close rmcp-template-mdei.3 --reason "call_tool consumes trusted HTTP trace headers after auth; real /mcp round-trip tests cover off/trusted/trusted-with-baggage, TrustedGatewayUnscoped, and mounted-auth-failure paths"` here.

---

## Task 9: Gate browser CORS on trace-header trust config

**Files:**
- Modify: `apps/soma/src/http.rs`
- Test: `apps/soma/src/http_tests.rs`

**Interfaces:**
- Consumes: `soma_config::{McpConfig, TraceHeaderMode}` (already in scope in `http.rs` via `config: &soma_config::McpConfig`).
- Produces: `cors_layer()`'s allow-header list now includes `traceparent`/`tracestate` when `mode == Trusted`, and additionally `baggage` when `mode == TrustedWithBaggage`; unchanged (no trace headers) when `mode == Off`.

- [ ] **Step 1: Write the failing tests**

Append to `apps/soma/src/http_tests.rs` (after `cors_preflight_allows_mcp_protocol_headers`):

```rust
async fn preflight_allow_headers(state: AppState, requested_headers: &str) -> String {
    let response = router(state)
        .oneshot(
            Request::builder()
                .method(axum::http::Method::OPTIONS)
                .uri("/mcp")
                .header(axum::http::header::ORIGIN, "http://127.0.0.1:40060")
                .header(axum::http::header::ACCESS_CONTROL_REQUEST_METHOD, "POST")
                .header(axum::http::header::ACCESS_CONTROL_REQUEST_HEADERS, requested_headers)
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should respond");

    response
        .headers()
        .get(axum::http::header::ACCESS_CONTROL_ALLOW_HEADERS)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_ascii_lowercase()
}

#[tokio::test]
async fn off_mode_denies_trace_header_preflight() {
    let allow_headers =
        preflight_allow_headers(crate::testing::loopback_state(), "TraceParent").await;
    assert!(
        !allow_headers.contains("traceparent"),
        "off mode must not allow traceparent, got: {allow_headers:?}"
    );
}

#[tokio::test]
async fn trusted_mode_allows_traceparent_and_tracestate_but_not_baggage() {
    let state = crate::testing::loopback_state_with_mcp_config(soma_config::McpConfig {
        trace_headers: soma_config::TraceHeaderMode::Trusted,
        ..soma_config::McpConfig::default()
    });
    let allow_headers =
        preflight_allow_headers(state, "TraceParent, TraceState, Baggage").await;

    assert!(allow_headers.contains("traceparent"), "got: {allow_headers:?}");
    assert!(allow_headers.contains("tracestate"), "got: {allow_headers:?}");
    assert!(
        !allow_headers.contains("baggage"),
        "trusted (without baggage) must not allow the baggage header, got: {allow_headers:?}"
    );
}

#[tokio::test]
async fn trusted_with_baggage_mode_allows_all_three_trace_headers() {
    let state = crate::testing::loopback_state_with_mcp_config(soma_config::McpConfig {
        trace_headers: soma_config::TraceHeaderMode::TrustedWithBaggage,
        ..soma_config::McpConfig::default()
    });
    let allow_headers =
        preflight_allow_headers(state, "TraceParent, TraceState, Baggage").await;

    for required in ["traceparent", "tracestate", "baggage"] {
        assert!(
            allow_headers.contains(required),
            "CORS allow-headers must include {required}, got: {allow_headers:?}"
        );
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p soma --features full,test-support --lib http_tests::trusted_mode_allows -- --test-threads=1`
Expected: FAIL — `trusted_mode_allows_traceparent_and_tracestate_but_not_baggage` fails because `cors_layer` does not yet gate on `trace_headers`.

- [ ] **Step 3: Implement CORS gating**

In `apps/soma/src/http.rs`, replace the `cors_layer` function body's header-list construction. Change:

```rust
    generic_cors_layer(
        origins,
        vec![Method::POST, Method::GET],
        vec![
            axum::http::header::AUTHORIZATION,
            axum::http::header::CONTENT_TYPE,
            axum::http::header::ACCEPT,
            // MCP protocol headers: Mcp-Protocol-Version (2025-06-18+) and the
            // draft (2026-07-28 / SEP-2243) Mcp-Method, Mcp-Name, and x-mcp-header.
            // Permitting them lets browser-based MCP clients clear CORS preflight.
            HeaderName::from_static("mcp-protocol-version"),
            HeaderName::from_static("mcp-method"),
            HeaderName::from_static("mcp-name"),
            HeaderName::from_static("x-mcp-header"),
        ],
    )
}
```

to:

```rust
    let mut headers = vec![
        axum::http::header::AUTHORIZATION,
        axum::http::header::CONTENT_TYPE,
        axum::http::header::ACCEPT,
        // MCP protocol headers: Mcp-Protocol-Version (2025-06-18+) and the
        // draft (2026-07-28 / SEP-2243) Mcp-Method, Mcp-Name, and x-mcp-header.
        // Permitting them lets browser-based MCP clients clear CORS preflight.
        HeaderName::from_static("mcp-protocol-version"),
        HeaderName::from_static("mcp-method"),
        HeaderName::from_static("mcp-name"),
        HeaderName::from_static("x-mcp-header"),
    ];
    headers.extend(trace_header_cors_allow_list(config.trace_headers));

    generic_cors_layer(origins, vec![Method::POST, Method::GET], headers)
}

/// CORS is transport permission only — it lets a browser *send* these
/// headers, it is never the trust decision (that stays owned by
/// `soma_runtime::server::resolve_auth_policy_kind` and
/// `soma_mcp::trace_resolution`). Static exact allow-list per mode, computed
/// once at router-construction time — no `Any`, no reflection, no
/// per-request synthesis.
fn trace_header_cors_allow_list(mode: soma_config::TraceHeaderMode) -> Vec<HeaderName> {
    match mode {
        soma_config::TraceHeaderMode::Off => Vec::new(),
        soma_config::TraceHeaderMode::Trusted => vec![
            HeaderName::from_static("traceparent"),
            HeaderName::from_static("tracestate"),
        ],
        soma_config::TraceHeaderMode::TrustedWithBaggage => vec![
            HeaderName::from_static("traceparent"),
            HeaderName::from_static("tracestate"),
            HeaderName::from_static("baggage"),
        ],
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p soma --features full,test-support --lib http_tests::`
Expected: PASS — all `http_tests` module tests, including the 3 new ones and the pre-existing `cors_preflight_allows_mcp_protocol_headers`.

- [ ] **Step 5: Lint and format**

Run: `cargo fmt -p soma && cargo clippy -p soma --all-targets --features full,test-support -- -D warnings`
Expected: clean.

- [ ] **Step 6: Commit**

```bash
git add apps/soma/src/http.rs apps/soma/src/http_tests.rs
git commit -m "feat(soma): gate browser CORS trace headers on the trust config"
```

This closes bead `rmcp-template-mdei.4`. Run `bd update rmcp-template-mdei.4 --claim` before Step 1, and `bd close rmcp-template-mdei.4 --reason "CORS allow-headers gated on SOMA_MCP_TRACE_HEADERS; mixed-case preflight tests added"` here.

---

## Task 10: Outbound non-propagation regression tests

**Files:**
- Modify: `apps/soma/tests/api_routes.rs`
- Modify: `crates/soma/client/src/client_tests.rs`

**Interfaces:**
- Produces: two new tests proving inbound `traceparent`/`tracestate`/`baggage` headers are never forwarded outbound — one for the gateway-proxied MCP HTTP path (`crates/soma/runtime/src/protected_routes_proxy.rs`, the one outbound surface in this codebase that forwards a real, non-empty set of inbound state: a fixed 5-header allow-list via `forwarded_mcp_headers()`, plus a separately-handled upstream bearer token via `bearer_token_env`), one for `SomaClient`'s deployed-upstream-API path (which, unlike the proxy, has no header-forwarding code at all to audit).

- [ ] **Step 1: Write the failing test for the gateway-proxied MCP HTTP path**

Append to `apps/soma/tests/api_routes.rs` (near the other `protected_route_proxy_*` tests):

```rust
#[tokio::test]
async fn protected_route_proxy_does_not_forward_inbound_trace_headers() {
    let seen_headers: Arc<Mutex<Vec<HeaderMap>>> = Arc::new(Mutex::new(Vec::new()));
    let backend = header_capturing_backend_server(seen_headers.clone()).await;
    std::env::set_var("SOMA_TEST_UPSTREAM_TOKEN", "Bearer upstream-secret");
    let temp = tempfile::tempdir().unwrap();
    let state = oauth_state_with_gateway(
        &temp,
        protected_gateway_config(Some(backend), Some("SOMA_TEST_UPSTREAM_TOKEN")),
    )
    .await;
    let token = protected_route_token(&state, "https://mcp.example.com/media", "soma:read");

    let response = router(state)
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/media")
                .header(header::HOST, "mcp.example.com")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .header("traceparent", "00-0af7651916cd43dd8448eb211c80319c-00f067aa0ba902b7-01")
                .header("tracestate", "vendor=value")
                .header("baggage", "region=us-east-1")
                .body(Body::from(r#"{"jsonrpc":"2.0","id":1}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    std::env::remove_var("SOMA_TEST_UPSTREAM_TOKEN");
    assert_eq!(response.status(), StatusCode::OK);

    let seen = seen_headers.lock().await;
    assert_eq!(seen.len(), 1, "backend should have received exactly one proxied request");
    for name in ["traceparent", "tracestate", "baggage"] {
        assert!(
            !seen[0].contains_key(name),
            "outbound proxied request must not carry {name}, got headers: {:?}",
            seen[0]
        );
    }
}

async fn header_capturing_backend_server(seen_headers: Arc<Mutex<Vec<HeaderMap>>>) -> String {
    let app = axum::Router::new().route(
        "/mcp",
        post(move |headers: HeaderMap, _body: Bytes| {
            let seen_headers = seen_headers.clone();
            async move {
                seen_headers.lock().await.push(headers);
                (StatusCode::OK, "proxied").into_response()
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    format!("http://{addr}/mcp")
}
```

- [ ] **Step 2: Run test to verify it currently passes (this is a regression guard, not new behavior)**

Run: `cargo test -p soma --features full,test-support --test api_routes protected_route_proxy_does_not_forward_inbound_trace_headers`
Expected: PASS immediately. The gateway-proxied MCP HTTP path (`crates/soma/runtime/src/protected_routes_proxy.rs`) is not "no header-forwarding code exists" the way `SomaClient`/the OpenAPI provider adapter are (Step 3/Task 11) — it genuinely does forward a fixed, explicit set of inbound headers to the upstream (see `forwarded_mcp_headers()`, `crates/soma/runtime/src/protected_routes_proxy.rs:222-230`):

```rust
fn forwarded_mcp_headers() -> [HeaderName; 5] {
    [
        header::ACCEPT,
        header::CONTENT_TYPE,
        HeaderName::from_static("mcp-protocol-version"),
        HeaderName::from_static("mcp-session-id"),
        HeaderName::from_static("last-event-id"),
    ]
}
```

That is the real, load-bearing invariant this test protects: forwarding is a fixed 5-header allow-list built at compile time, `traceparent`/`tracestate`/`baggage` are not members of it, and the proxy code loops over exactly this list (`crates/soma/runtime/src/protected_routes_proxy.rs:69-73`) rather than reflecting/forwarding arbitrary inbound headers. This test exists to catch a *future* regression to that allow-list, not to drive new code — there is no Step 3 implementation needed for this half of the task. (An earlier draft of this plan justified this test by grepping `crates/soma/application/src/providers/remote.rs`, `crates/soma/client/src/client.rs`, and `crates/shared/provider-adapters/src/openapi.rs` for header-handling code and finding none — that grep is real but targets the wrong files for *this* test; none of those three is the gateway-proxy code path. Keep that grep's finding for Step 3/Task 11's `SomaClient` case instead, where it does apply.)

- [ ] **Step 3: Write the failing test for `SomaClient`'s deployed-API path**

Append to `crates/soma/client/src/client_tests.rs` (near `mock_deployed_api`/`push_observed`):

Add a field to `ObservedRequest`:

```rust
#[derive(Debug, Clone)]
struct ObservedRequest {
    path: String,
    body: Value,
    bearer: String,
    trace_headers_present: bool,
}
```

Update `push_observed` to compute it:

```rust
fn push_observed(observed: &ObservedRequests, headers: &HeaderMap, path: &str, body: Value) {
    let bearer = headers
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    let trace_headers_present = headers.get("traceparent").is_some()
        || headers.get("tracestate").is_some()
        || headers.get("baggage").is_some();
    observed
        .lock()
        .expect("observed requests should lock")
        .push(ObservedRequest {
            path: path.to_owned(),
            body,
            bearer,
            trace_headers_present,
        });
}
```

Add the test (near the other `mock_deployed_api`-based tests):

```rust
#[tokio::test]
async fn deployed_api_status_call_never_sends_trace_headers() {
    let observed: ObservedRequests = Arc::new(Mutex::new(Vec::new()));
    let (base_url, _handle) = mock_deployed_api(observed.clone()).await;
    let client = SomaClient::new(&SomaConfig {
        api_url: base_url,
        api_key: "test-key".into(),
        ..SomaConfig::default()
    })
    .expect("client should build");

    client.status().await.expect("status should succeed");

    let seen = observed.lock().expect("observed requests should lock");
    assert_eq!(seen.len(), 1);
    assert!(
        !seen[0].trace_headers_present,
        "SomaClient must never emit trace headers on its own — it takes no \
         trace/header parameter on any outbound method"
    );
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p soma-client --features client deployed_api_status_call_never_sends_trace_headers`
Expected: PASS immediately (same rationale as Step 2 — `SomaClient`'s methods take no trace/context parameter, so there is no code path that could set these headers; this is a regression guard).

- [ ] **Step 5: Run full test suites to confirm no regression from the `ObservedRequest` field addition**

Run: `cargo test -p soma-client --features client`
Expected: PASS — `push_observed` is the only construction site for `ObservedRequest`, so adding a field there does not affect any other test.

- [ ] **Step 6: Lint and format**

Run: `cargo fmt -p soma-client -p soma && cargo clippy -p soma-client --all-targets --features client -- -D warnings && cargo clippy -p soma --all-targets --features full,test-support -- -D warnings`
Expected: clean.

- [ ] **Step 7: Commit**

```bash
git add apps/soma/tests/api_routes.rs crates/soma/client/src/client_tests.rs
git commit -m "test: add outbound non-propagation regression guards for trace headers"
```

---

## Task 11: `docs/TRACE_CONTEXT.md`

**Files:**
- Create: `docs/TRACE_CONTEXT.md`
- Modify: `docs/CLAUDE.md` (add the new file to the "Files in This Directory" table)

**Interfaces:** none (documentation only).

- [ ] **Step 1: Write the doc**

Create `docs/TRACE_CONTEXT.md`:

```markdown
# Trace Context

Soma's MCP surface can bridge inbound W3C `traceparent`/`tracestate`/`baggage`
metadata into request-scoped trace context, from two sources:

1. **RMCP `_meta`** (`traceparent`/`tracestate`/`baggage` keys) — always
   available, on every transport (stdio and HTTP). Untrusted by definition:
   `_meta` is caller-controlled. Summarized via `rmcp-traces`
   (`crates/shared/traces`) into a redacted `TraceSummary` — raw baggage
   values are never logged.
2. **Inbound HTTP headers** — only available on the HTTP transport, and only
   when explicitly enabled via `SOMA_MCP_TRACE_HEADERS` on a real trust
   boundary. `_meta` always wins: if `_meta` already carries any trace key,
   HTTP header values are never parsed, joined, counted, or logged — only
   safe presence booleans (`http_trace_headers_present`,
   `trace_context_conflict`) are recorded.

## `SOMA_MCP_TRACE_HEADERS`

| Value | Behavior |
|---|---|
| `off` (default) | No HTTP trace-header extraction. Zero HTTP header lookup on the request hot path. Safe for every deployment. |
| `trusted` | Extract `traceparent`/`tracestate` from inbound HTTP headers after auth. Baggage is never extracted. |
| `trusted-with-baggage` | Like `trusted`, but also extracts validated `baggage`. **Baggage can carry sensitive user/session/application data** — enable deliberately, not by default. |

```bash
# config.toml
[mcp]
trace_headers = "trusted"
```

```bash
# env
SOMA_MCP_TRACE_HEADERS=trusted-with-baggage
```

## Trust boundary — this is the load-bearing rule

**Bearer/OAuth authentication is not a trace-header trust boundary.** A
client presenting a valid token says nothing about whether an upstream
gateway or proxy stripped or overwrote inbound `traceparent`/`tracestate`/
`baggage` headers from *other*, untrusted clients before the request reached
this server. A non-`off` `SOMA_MCP_TRACE_HEADERS` is therefore only valid
when the resolved auth policy is one of:

- **`LoopbackDev`** — bound to `127.0.0.1`/`::1`/`localhost`. The bind itself
  is the trust boundary.
- **`TrustedGatewayUnscoped`** — `SOMA_NOAUTH=true` on a non-loopback bind,
  meaning an upstream gateway/proxy is expected to enforce both
  authentication *and* header hygiene (stripping/overwriting inbound trace
  headers from untrusted clients) before traffic reaches Soma.

`MountedBearer` and `MountedOAuth` deployments reject any non-`off`
`SOMA_MCP_TRACE_HEADERS` at startup with a clear error — see
`soma_runtime::server::resolve_auth_policy_kind` /
`validate_trace_headers_trust`.

## CORS

CORS is transport permission only — it lets a browser *send* the
`traceparent`/`tracestate`/`baggage` headers, it is never the trust decision.
The browser-facing `Access-Control-Allow-Headers` list is gated on the same
`SOMA_MCP_TRACE_HEADERS` config, as a static exact allow-list computed once
at router construction (`apps/soma/src/http.rs::trace_header_cors_allow_list`)
— never `Any`, never header reflection.

## Outbound propagation is out of scope

This epic (GH #76 slice `rmcp-template-mdei`) is inbound-only. Inbound trace
headers are never forwarded to:

- Soma's deployed upstream API (`SomaClient`, `crates/soma/client`)
- The OpenAPI provider adapter (`crates/shared/provider-adapters/src/openapi.rs`)
- Gateway-proxied MCP HTTP providers (`crates/soma/runtime/src/protected_routes_proxy.rs`)

`SomaClient` and the OpenAPI provider adapter are safe by construction: their
outbound methods take no trace/header parameter, so there is no code path
that could forward an inbound header. The gateway-proxied MCP HTTP path is
different — it genuinely forwards some inbound state (a fixed, explicit
5-header allow-list via `protected_routes_proxy.rs::forwarded_mcp_headers()`
— `accept`, `content-type`, `mcp-protocol-version`, `mcp-session-id`,
`last-event-id` — plus a separately-handled resolved upstream bearer token).
`traceparent`/`tracestate`/`baggage` are not members of that allow-list. A
runtime regression test
(`apps/soma/tests/api_routes.rs::protected_route_proxy_does_not_forward_inbound_trace_headers`)
proves it stays that way.

Outbound propagation (attaching Soma's own trace context to *its* outbound
calls) is deferred to a future slice.

## A note on stdio

`validate_trace_headers_trust` (and `resolve_auth_policy_kind` generally) is
only invoked on the HTTP transport's startup path (`apps/soma/src/bootstrap.rs::http_auth_policy`).
Stdio mode always runs as `AuthPolicy::LoopbackDev` directly and never calls
it. This is safe — `LoopbackDev` is always an allowed trust boundary, so the
validation would pass trivially even if it ran — but it means a
misconfigured `SOMA_MCP_TRACE_HEADERS` on a stdio-only deployment does not
surface a startup error. It is also inert there in practice: stdio has no
HTTP transport, so `RequestContext.extensions` never carries `http::request::Parts`
and trace-header extraction never has a header source to read from.

## Live smoke

`cargo xtask test-trace-headers` (thin wrapper: `scripts/test-trace-headers.sh`)
starts a bounded, self-contained local server per scenario (`off`, `trusted`,
`trusted-with-baggage`) with an isolated `SOMA_HOME`, exercises the trust
matrix and the negative cases that are meaningfully different over a real
wire (duplicate `traceparent` headers, non-visible-ASCII header values), and
asserts only safe fields ever reach the captured log. `tracestate` without a
valid `traceparent` is covered as a pure unit test instead (see
`crates/soma/mcp/src/trace_resolution_tests.rs::trusted_mode_ignores_tracestate_without_a_valid_traceparent`)
since it needs no real transport to prove.
```

- [ ] **Step 2: Register the new doc in `docs/CLAUDE.md`**

In `docs/CLAUDE.md`'s "Files in This Directory" table, add a row (alphabetically near `PATTERNS.md`):

```markdown
| `TRACE_CONTEXT.md` | Trace-header trust config, modes, CORS gating, outbound-propagation scope | The trace-header trust boundary, `SOMA_MCP_TRACE_HEADERS` modes, or outbound-propagation scope change |
```

- [ ] **Step 3: Verify doc conventions**

Run: `python3 scripts/check-stale-claims.py docs/TRACE_CONTEXT.md` (if this script accepts a path argument; otherwise run the project's full doc-check target) — confirm env var names (`SOMA_MCP_TRACE_HEADERS`), file paths, and function names referenced in the doc match the actual code from Tasks 1–10.
Expected: no stale-claim findings against this new file.

- [ ] **Step 4: Commit**

```bash
git add docs/TRACE_CONTEXT.md docs/CLAUDE.md
git commit -m "docs: add TRACE_CONTEXT.md for the trusted HTTP trace-header bridge"
```

---

## Task 12: Live smoke — `cargo xtask test-trace-headers`

**Files:**
- Modify: `xtask/src/scripts_lane_a.rs` (widen `AuthSmokeResults` visibility)
- Create: `xtask/src/trace_headers_smoke.rs`
- Modify: `xtask/src/main.rs`
- Modify: `xtask/src/workspace_commands.rs` (help text)
- Create: `scripts/test-trace-headers.sh`

**Interfaces:**
- Produces: `cargo xtask test-trace-headers` — builds the `soma` binary **once**, then spawns it directly (not via `cargo run`) once per scenario (isolated `SOMA_HOME`, `SOMA_RUNTIME_MODE=local`, an unused port, a startup timeout, deterministic cleanup via a drop guard), exercises `off`/`trusted`/`trusted-with-baggage` plus the negative cases that need a real wire, and asserts only safe fields ever appear in the captured stdout/stderr log. Reuses `xtask::scripts_lane_a::AuthSmokeResults` for pass/fail reporting instead of defining a parallel type.

Three review findings converged on the same root cause here and are fixed together: the original draft spawned `cargo run --bin soma --features full -- serve` fresh per scenario, which (a) let a cold-cache first-time *compile* eat into the 15s startup-health timeout, indistinguishable from an actual startup regression; (b) meant `ServerGuard::drop`'s `child.kill()` killed the `cargo` parent process, not the `soma` grandchild it spawns — an orphaned no-auth loopback listener could survive even the normal `?`/Drop cleanup path, not just a hard interrupt; and (c) triple-compiled the same crate for no reason. Building once via `cargo build --message-format=json` (to reliably locate the produced binary path regardless of target-dir configuration) and spawning that binary path directly for each scenario has no `cargo` parent at all, so `child.kill()` genuinely terminates the one process it started — this fixes all three at once, it isn't three separate patches.

- [ ] **Step 1: Make `AuthSmokeResults` reusable across xtask smoke commands**

In `xtask/src/scripts_lane_a.rs`, widen `AuthSmokeResults`'s visibility so a sibling module can reuse the same pass/fail counter shape instead of `trace_headers_smoke.rs` defining a parallel one:

```rust
#[derive(Default)]
pub(crate) struct AuthSmokeResults {
    pub(crate) pass: usize,
    pub(crate) fail: usize,
}

impl AuthSmokeResults {
    pub(crate) fn pass(&mut self, label: &str) {
        println!("PASS  {label}");
        self.pass += 1;
    }

    pub(crate) fn fail(&mut self, label: &str) {
        eprintln!("FAIL  {label}");
        self.fail += 1;
    }
}
```

Both the two fields (`pass`, `fail` — read as plain counters by `trace_headers_smoke.rs`) and the two methods (`pass(&mut self, label)`, `fail(&mut self, label)` — called to record one outcome) go `pub(crate)`. Rust disambiguates `results.pass` (field read) from `results.pass("label")` (method call) by the trailing parens, so both can share the name without conflict — but read carefully: `trace_headers_smoke.rs` calls the *method* `results.pass(&format!(...))` to record each check, and reads the *field* `results.pass` (no parens) only in the final summary line. (Only the `struct`/`impl`/field/method visibility changes from private to `pub(crate)` — `test_mcp_auth`'s existing usage of `AuthSmokeResults` is unaffected, since `pub(crate)` is a superset of module-private within the same crate.)

Run: `cargo check -p xtask` to confirm `test_mcp_auth` still compiles unchanged.

- [ ] **Step 2: Implement the xtask command**

Create `xtask/src/trace_headers_smoke.rs`:

```rust
//! Bounded, self-contained live smoke for the trusted HTTP trace-header
//! bridge (`SOMA_MCP_TRACE_HEADERS`). Unlike `test_mcp_auth` (which curls an
//! already-running server), this smoke builds and starts its own local
//! `soma` process per scenario — required because each scenario needs a
//! different `SOMA_MCP_TRACE_HEADERS` value, and the trust-boundary
//! validation (Task 3) is a startup-time check. The binary is built exactly
//! once (`build_soma_binary`) and spawned directly (no `cargo run` wrapper)
//! for each of the three scenarios, so `STARTUP_TIMEOUT` below measures only
//! process startup, and `ServerGuard::drop`'s `child.kill()` terminates the
//! actual `soma` process with no intervening `cargo` parent to orphan it.

use std::{
    io::Read,
    net::TcpListener,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

use anyhow::{bail, Context, Result};

use crate::scripts_lane_a::AuthSmokeResults;

// Startup-only now that the binary is prebuilt — no compile time is folded
// into this window (see this module's doc comment).
const STARTUP_TIMEOUT: Duration = Duration::from_secs(15);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

pub fn test_trace_headers(_args: &[String]) -> Result<()> {
    let binary = build_soma_binary()?;
    let mut results = AuthSmokeResults::default();

    run_scenario(&binary, &mut results, "off", "off", |port, home| {
        off_mode_checks(port, home)
    })?;
    run_scenario(&binary, &mut results, "trusted", "trusted", |port, home| {
        trusted_mode_checks(port, home, false)
    })?;
    run_scenario(
        &binary,
        &mut results,
        "trusted-with-baggage",
        "trusted-with-baggage",
        |port, home| trusted_mode_checks(port, home, true),
    )?;

    // `.pass`/`.fail` here are field reads (no parens) — the summary count,
    // not the `pass(&mut self, label)`/`fail(&mut self, label)` recording
    // methods `run_scenario` calls per-check above.
    println!("\n{} passed, {} failed", results.pass, results.fail);
    if results.fail == 0 {
        Ok(())
    } else {
        bail!("{} trace-header smoke check(s) failed", results.fail)
    }
}

/// Build the `soma` binary once, ahead of every scenario, and return its
/// actual produced path — parsed from `--message-format=json` rather than
/// assumed as `target/debug/soma`, so this keeps working under a custom
/// `CARGO_TARGET_DIR` or workspace-level target-dir override.
fn build_soma_binary() -> Result<PathBuf> {
    println!("==> Building soma binary (once, shared across all scenarios)...");
    let output = Command::new("cargo")
        .args(["build", "--bin", "soma", "--features", "full", "--message-format=json"])
        .output()
        .context("run cargo build")?;
    if !output.status.success() {
        bail!("cargo build failed:\n{}", String::from_utf8_lossy(&output.stderr));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        let Ok(message) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if message["reason"] == "compiler-artifact"
            && message["target"]["name"] == "soma"
            && message["executable"].is_string()
        {
            let path = message["executable"]
                .as_str()
                .context("executable path should be a string")?;
            return Ok(PathBuf::from(path));
        }
    }
    bail!("cargo build did not report a soma binary artifact")
}

fn run_scenario(
    binary: &Path,
    results: &mut AuthSmokeResults,
    label: &str,
    trace_headers_env: &str,
    checks: impl FnOnce(u16, &Path) -> Result<Vec<(String, bool)>>,
) -> Result<()> {
    println!("==> Scenario: SOMA_MCP_TRACE_HEADERS={trace_headers_env}");
    let home = tempfile::tempdir().context("create isolated SOMA_HOME")?;
    // `free_port()` has an inherent TOCTOU race (see its doc comment) — this
    // smoke is a manual/local diagnostic, not a CI gate (see Task 12's final
    // Step, and the plan's Final Verification section), so a rare collision
    // costs a re-run rather than a flaky required check. Not retried here.
    let port = free_port()?;
    let mut server = ServerGuard::spawn(binary, home.path(), port, trace_headers_env)?;
    server.wait_for_health(port)?;

    let outcomes = checks(port, home.path())?;
    let log = server.captured_log();
    for (check_label, passed) in outcomes {
        if passed {
            results.pass(&format!("{label}: {check_label}"));
        } else {
            results.fail(&format!("{label}: {check_label}"));
        }
    }
    assert_log_has_no_raw_baggage(&log, results, label);
    Ok(())
}

fn off_mode_checks(port: u16, _home: &Path) -> Result<Vec<(String, bool)>> {
    let mut checks = Vec::new();
    let status = curl_status(
        port,
        &[("traceparent", "00-0af7651916cd43dd8448eb211c80319c-00f067aa0ba902b7-01")],
    )?;
    checks.push(("status call succeeds with traceparent header present".to_owned(), status == 200));

    let preflight = curl_preflight(port, "TraceParent")?;
    checks.push((
        "off mode CORS preflight denies traceparent".to_owned(),
        !preflight.to_ascii_lowercase().contains("traceparent"),
    ));
    Ok(checks)
}

fn trusted_mode_checks(port: u16, _home: &Path, baggage_enabled: bool) -> Result<Vec<(String, bool)>> {
    let mut checks = Vec::new();

    let status = curl_status(
        port,
        &[
            ("traceparent", "00-0af7651916cd43dd8448eb211c80319c-00f067aa0ba902b7-01"),
            ("tracestate", "vendor=value"),
        ],
    )?;
    checks.push(("valid traceparent+tracestate call succeeds".to_owned(), status == 200));

    // Negative: duplicate traceparent header must not crash the server. Two
    // entries with the same header name in `curl_status`'s headers slice
    // become two separate `-H "traceparent: ..."` flags on the same curl
    // invocation — no separate helper needed for this. (`tracestate` without
    // a valid `traceparent`, and the underlying "duplicate header" parsing
    // rule itself, are unit-tested in `trace_resolution_tests.rs` and
    // `rmcp-traces`' own `http_propagation.rs` respectively — this live
    // check only proves the real transport surfaces a duplicate header to
    // `soma-mcp`'s code the same way a synthetic `HeaderMap` does.)
    let dup_status = curl_status(
        port,
        &[
            ("traceparent", "00-0af7651916cd43dd8448eb211c80319c-00f067aa0ba902b7-01"),
            ("traceparent", "00-11112222333344445555666677778888-1111222233334444-01"),
        ],
    )?;
    checks.push(("duplicate traceparent is rejected without a server error".to_owned(), dup_status == 200));

    // Negative: non-visible-ASCII header value must not crash the server.
    // Kept live (not moved to a unit test) because `HeaderValue::from_str`
    // in a synthetic unit test may reject bytes curl would send raw over
    // the wire — this is the one negative case that genuinely needs a real
    // transport to prove.
    let non_ascii_status = curl_status(port, &[("traceparent", "00-\u{00e9}-invalid-01")])?;
    checks.push(("non-ASCII traceparent value does not error".to_owned(), non_ascii_status == 200));

    let preflight = curl_preflight(port, "TraceParent, TraceState, Baggage")?;
    let preflight_lower = preflight.to_ascii_lowercase();
    checks.push(("trusted mode CORS preflight allows traceparent".to_owned(), preflight_lower.contains("traceparent")));
    checks.push(("trusted mode CORS preflight allows tracestate".to_owned(), preflight_lower.contains("tracestate")));
    checks.push((
        "baggage CORS allowance matches mode".to_owned(),
        preflight_lower.contains("baggage") == baggage_enabled,
    ));

    if baggage_enabled {
        let baggage_status =
            curl_status(port, &[("traceparent", "00-0af7651916cd43dd8448eb211c80319c-00f067aa0ba902b7-01"), ("baggage", "region=us-east-1")])?;
        checks.push(("baggage-enabled call succeeds".to_owned(), baggage_status == 200));
    }

    Ok(checks)
}

fn assert_log_has_no_raw_baggage(log: &str, results: &mut AuthSmokeResults, scenario: &str) {
    // "region=us-east-1" and "super-secret" would only appear if a raw
    // baggage/tracestate value leaked into the log — the smoke never sends
    // a value containing this exact needle unless checking for its absence.
    let leaked = log.contains("region=us-east-1") && log.contains("baggage_member_count");
    if leaked {
        results.fail(&format!("{scenario}: raw baggage value leaked into server log"));
    } else {
        results.pass(&format!("{scenario}: no raw baggage value in server log"));
    }
}

// ── HTTP helpers (curl, to avoid adding a new xtask dependency) ──────────────

fn curl_status(port: u16, headers: &[(&str, &str)]) -> Result<u16> {
    let mut cmd = Command::new("curl");
    cmd.args([
        "-s",
        "-o",
        "/dev/null",
        "-w",
        "%{http_code}",
        "-X",
        "POST",
        "-H",
        "Content-Type: application/json",
        "-H",
        "Accept: application/json, text/event-stream",
        "--max-time",
        &REQUEST_TIMEOUT.as_secs().to_string(),
    ]);
    for (name, value) in headers {
        cmd.args(["-H", &format!("{name}: {value}")]);
    }
    cmd.args([
        "-d",
        r#"{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}"#,
        &format!("http://127.0.0.1:{port}/mcp"),
    ]);
    let output = cmd.output().context("run curl")?;
    String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse::<u16>()
        .context("parse curl status code")
}

fn curl_preflight(port: u16, requested_headers: &str) -> Result<String> {
    let output = Command::new("curl")
        .args([
            "-s",
            "-i",
            "-X",
            "OPTIONS",
            "-H",
            &format!("Origin: http://127.0.0.1:{port}"),
            "-H",
            "Access-Control-Request-Method: POST",
            "-H",
            &format!("Access-Control-Request-Headers: {requested_headers}"),
            &format!("http://127.0.0.1:{port}/mcp"),
        ])
        .output()
        .context("run curl preflight")?;
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Bind an ephemeral port and immediately release it for the spawned server
/// to rebind. This has an inherent TOCTOU race — something else could grab
/// the same port between this function returning and `ServerGuard::spawn`
/// binding it — but it is bounded to this manual/local smoke tool (never
/// wired into CI; see Task 12's final step), so a rare collision costs a
/// re-run of `cargo xtask test-trace-headers`, not a flaky required check.
fn free_port() -> Result<u16> {
    let listener = TcpListener::bind("127.0.0.1:0").context("bind ephemeral port")?;
    Ok(listener.local_addr()?.port())
}

// ── server lifecycle ──────────────────────────────────────────────────────────

struct ServerGuard {
    child: Child,
    log_path: PathBuf,
}

impl ServerGuard {
    /// Spawn the prebuilt `soma` binary directly — no `cargo run` wrapper —
    /// so `Drop`'s `child.kill()` terminates the actual server process, not
    /// a `cargo` parent that would leave the real listener orphaned.
    fn spawn(binary: &Path, home: &Path, port: u16, trace_headers: &str) -> Result<Self> {
        let log_path = home.join("server.log");
        let log_file = std::fs::File::create(&log_path).context("create server log file")?;
        let child = Command::new(binary)
            .arg("serve")
            .env("SOMA_HOME", home)
            .env("SOMA_MCP_HOST", "127.0.0.1")
            .env("SOMA_MCP_PORT", port.to_string())
            .env("SOMA_MCP_NO_AUTH", "true")
            .env("SOMA_MCP_TRACE_HEADERS", trace_headers)
            .env("SOMA_RUNTIME_MODE", "local")
            .env("RUST_LOG", "soma_mcp=info,soma=info")
            .stdout(Stdio::from(log_file.try_clone().context("clone log handle")?))
            .stderr(Stdio::from(log_file))
            .spawn()
            .context("spawn soma serve")?;
        Ok(Self { child, log_path })
    }

    fn wait_for_health(&mut self, port: u16) -> Result<()> {
        let deadline = Instant::now() + STARTUP_TIMEOUT;
        loop {
            if Instant::now() > deadline {
                bail!("server did not become healthy within {STARTUP_TIMEOUT:?}");
            }
            let status = Command::new("curl")
                .args([
                    "-s",
                    "-o",
                    "/dev/null",
                    "-w",
                    "%{http_code}",
                    "--max-time",
                    "1",
                    &format!("http://127.0.0.1:{port}/health"),
                ])
                .output();
            if let Ok(output) = status {
                if String::from_utf8_lossy(&output.stdout).trim() == "200" {
                    return Ok(());
                }
            }
            std::thread::sleep(Duration::from_millis(200));
        }
    }

    fn captured_log(&self) -> String {
        let mut contents = String::new();
        if let Ok(mut file) = std::fs::File::open(&self.log_path) {
            let _ = file.read_to_string(&mut contents);
        }
        contents
    }
}

impl Drop for ServerGuard {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
```

- [ ] **Step 3: Register the xtask command**

In `xtask/src/main.rs`, add a new match arm right after the existing `Some("test-mcp-auth") => scripts_lane_a::test_mcp_auth(&args[1..]),` line:

```rust
        Some("test-trace-headers") => trace_headers_smoke::test_trace_headers(&args[1..]),
```

Add the module declaration near the other `mod` declarations at the top of `xtask/src/main.rs` (alongside `mod scripts_lane_a;`):

```rust
mod trace_headers_smoke;
```

In `xtask/src/workspace_commands.rs`, add a line to the help text near the existing `test-mcp-auth` entry:

```
  test-trace-headers    Bounded live smoke for SOMA_MCP_TRACE_HEADERS (off/trusted/trusted-with-baggage)
```

- [ ] **Step 4: Check `xtask`'s dependencies**

`xtask/src/scripts_lane_a.rs` already uses `anyhow` and `tempfile`, and `xtask/Cargo.toml` already lists `serde_json` (used by `build_soma_binary`'s `cargo build --message-format=json` parsing) — verify all three are already `xtask` dependencies:

Run: `grep -E "^(anyhow|tempfile|serde_json) =" xtask/Cargo.toml`
Expected: all three present. If any is missing, add it to `xtask/Cargo.toml`'s `[dependencies]` matching the version already pinned in the workspace root `Cargo.toml`'s `[workspace.dependencies]`.

- [ ] **Step 5: Create the thin wrapper script**

Create `scripts/test-trace-headers.sh`:

```bash
#!/usr/bin/env bash
# Thin wrapper. Canonical implementation: cargo xtask test-trace-headers.
set -euo pipefail

cargo xtask test-trace-headers "$@"
```

Run: `chmod +x scripts/test-trace-headers.sh`

- [ ] **Step 6: Run the smoke locally**

Run: `cargo xtask test-trace-headers`
Expected: all scenarios PASS. `build_soma_binary()` compiles once up front (the slow part — full cold build of the `full`-feature binary can take a few minutes; a warm/incremental rebuild is much faster), then all three scenarios spawn the prebuilt binary directly and each add well under a second of process-startup + curl-check time. If any curl-based check fails, read the printed `FAIL` line and the corresponding scenario's captured log at the temp `SOMA_HOME`/`server.log` path (the `TempDir` drops — and is deleted — at the end of each `run_scenario` call; add a one-line diagnostic print of `home.path()` before that drop if a failure needs deeper investigation, or `tempfile::Builder::new().keep(true)` for a debugging session — this is expected local debugging, not a plan step).

- [ ] **Step 7: Lint and format**

Run: `cargo fmt -p xtask && cargo clippy -p xtask --all-targets -- -D warnings`
Expected: clean.

- [ ] **Step 8: Commit**

```bash
git add xtask/src/trace_headers_smoke.rs xtask/src/main.rs xtask/src/workspace_commands.rs xtask/src/scripts_lane_a.rs scripts/test-trace-headers.sh
git commit -m "feat(xtask): add bounded live smoke for trusted HTTP trace headers"
```

Not wired into any CI workflow — this is a manual/local diagnostic tool, listed in the plan's Final Verification section as a one-off check, not a required PR gate. Each invocation does a real `cargo build`, so adding it to `ci.yml` would add a full compile pass to every PR; do not do that as a "helpful" follow-up without a separate, deliberate decision.

---

## Task 13: CHANGELOG and bead closeout

**Files:**
- Modify: `CHANGELOG.md`

**Interfaces:** none.

- [ ] **Step 1: Add the `[Unreleased]` entries**

In `CHANGELOG.md`, under `## [Unreleased]` → `### Added`, add (near the existing `rmcp-traces` platform-crate entry from bead `.1`):

```markdown
- Add `SOMA_MCP_TRACE_HEADERS` (`off` default / `trusted` / `trusted-with-baggage`)
  typed config gating trusted HTTP `traceparent`/`tracestate`/`baggage`
  extraction on a real trust boundary (loopback bind or `SOMA_NOAUTH=true`
  behind a trusted gateway) — bearer/OAuth authentication is never treated as
  a trace-header trust boundary and rejects any non-`off` value at startup.
  `soma-mcp`'s `call_tool` consumes trusted headers via a new
  `trace_resolution` module (RMCP `_meta` always wins over HTTP headers, and
  a `_meta`/HTTP conflict is recorded only as safe presence booleans, never
  by parsing the losing-source HTTP value); browser CORS allow-headers are
  gated on the same config as a static exact allow-list. See
  `docs/TRACE_CONTEXT.md`, `cargo xtask test-trace-headers`, and epic
  `rmcp-template-mdei`.
```

- [ ] **Step 2: Verify version-sync gates**

Run: `cargo xtask check-version-sync`
Expected: clean — this task touches no version-bearing files (`Cargo.toml` package versions, `server.json`, `docs/generated/openapi.json`), only `[Unreleased]` prose.

- [ ] **Step 3: Commit**

```bash
git add CHANGELOG.md
git commit -m "docs(changelog): note trusted HTTP trace-header bridge under Unreleased"
```

- [ ] **Step 4: Close the epic**

Run `bd update rmcp-template-mdei.5 --claim` before Task 11, and `bd close rmcp-template-mdei.5 --reason "docs, live smoke, and outbound non-propagation proof shipped"` here. Then `bd close rmcp-template-mdei --reason "all five slices (.1-.5) of the trusted HTTP trace-header bridge shipped"` to close the epic itself.

---

## Final Verification

- [ ] Run the full workspace gate: `cargo fmt --all --check && cargo clippy --workspace --all-targets --all-features -- -D warnings && cargo test --workspace --all-features`
- [ ] Run `cargo test --workspace --no-default-features` (or the narrowest stdio-only feature combination CI exercises) to confirm no crate accidentally pulled `rmcp-traces/http` into a build that should not have it.
- [ ] Run `cargo xtask check-architecture` (or the closest existing equivalent — see `Justfile`'s `patterns-check`/`patterns-strict` targets) to confirm `rmcp-traces` gained no dependency on any Soma product/runtime/shim crate.
- [ ] Run `cargo xtask test-trace-headers` one more time end to end.
- [ ] `bd swarm validate rmcp-template-mdei` should now show 5/5 complete.

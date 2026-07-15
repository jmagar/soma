# CIMD Authorization-Server Support Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let `soma-auth`'s `/authorize` endpoint accept OAuth Client ID Metadata Documents (CIMD) as an alternative to Dynamic Client Registration, per the MCP draft authorization spec, with an SSRF-hardened fetcher that holds the same redirect-URI trust boundary DCR already enforces.

**Architecture:** A new `cimd` module (`crates/soma-auth/src/cimd.rs` + `crates/soma-auth/src/cimd/{ssrf,document}.rs`) provides an SSRF-hardened HTTPS fetcher and a `ClientMetadataDocument` type with a single-flight, TTL-and-negative-cached, size-bounded fetch pipeline. `authorize()` in `authorize.rs` gains a `resolve_client_redirect_uris()` helper that detects an `https://`-shaped `client_id`, fetches+validates its metadata document instead of querying the DCR-registered-clients table, and — critically — filters the document's `redirect_uris` through the exact same `is_allowed_redirect_uri` check DCR-registered clients are already held to, rather than trusting them outright. AS metadata advertises `client_id_metadata_document_supported: true`. DCR (`/register`) is untouched — this is a new parallel path, not a replacement.

**Tech Stack:** Rust, reqwest (existing dep, `.resolve()` for DNS-pinned fetches, `.no_proxy()`, `redirect::Policy::none()`, streaming `.chunk()` reads, `response.remote_addr()` for post-connect peer re-validation), tokio (`net` feature, newly enabled, for `tokio::net::lookup_host` + `tokio::time::timeout`), dashmap (existing dep, for the metadata-document cache + per-key single-flight locks), serde_json, axum/wiremock for tests (existing dev-deps).

## Revision note (this is v2 of this plan)

This plan was revised after a four-agent engineering review (architecture, simplicity, security, performance) found one **critical** security gap and several **high**-severity implementation defects in the original design. Summary of what changed and why, so the rationale isn't lost:

- **Critical — redirect_uri allowlist bypass (architecture review):** the original Task 4 trusted a CIMD document's `redirect_uris` outright. Since `client_id` is an arbitrary attacker-hosted URL, the attacker also controls the JSON served there — including `redirect_uris`. Without re-applying the same `is_allowed_redirect_uri` check DCR already enforces, any public HTTPS server can declare `redirect_uris: ["https://attacker.evil/steal-code"]` and have it trusted, making CIMD strictly weaker than DCR at exactly the point DCR exists to protect. **Fixed by filtering the document's `redirect_uris` through `is_allowed_redirect_uri` before use** (Task 4), rather than deferring to a not-yet-built consent UI.
- **High — no `.no_proxy()` (security review, source-confirmed against reqwest 0.12.28):** without it, ambient `HTTPS_PROXY`/`ALL_PROXY` env vars silently route the fetch through a proxy that re-resolves the hostname itself, completely bypassing the `.resolve()` SSRF pin. **Fixed** (Task 3).
- **High — no post-connect peer re-validation (security review):** the original plan's own text claimed parity with the reference SSRF pattern (`labby-primitives::ssrf`'s caller) on this point, but the reference actually does re-validate the connected peer's IP against the pin — the plan omitted it while claiming otherwise. **Fixed** by threading the pinned address through and checking `response.remote_addr()` against it (Task 3).
- **High — decorative size cap (architecture + security review, independently):** `response.bytes().await` buffers the entire body before the `MAX_DOCUMENT_BYTES` check ever runs, so a hostile server can exhaust memory regardless of the nominal cap. **Fixed** by streaming with a running cap (Task 3).
- **High — acceptance tests could not pass as written (simplicity + security review, independently):** the original Task 4 tests pointed `client_id` at a plain-HTTP `wiremock` server, but `is_cimd_client_id` requires `https://` and the SSRF guard blocks loopback — every proposed test would have failed immediately, and the fully-assembled guarded fetch path had zero real coverage. **Fixed** by splitting the fetch into a DNS-resolution step and a separately-testable `fetch_via_pinned_address` step that tests can call directly with `wiremock`'s real bound address, and by testing the redirect-URI-allowlist business logic in isolation from the network fetch (Task 3, Task 4).
- **High — `AuthState` field would break the crate's supported no-features build (architecture review, empirically verified with `cargo check -p soma-auth --no-default-features`):** the new cache field must be `#[cfg(feature = "http-axum")]`-gated, and this plan's own verification steps must include a reduced-feature check to catch a regression here (Task 4).
- **Medium — several fixes bundled together:** unbounded DNS resolution time (no timeout), unbounded cache growth (attacker-controlled cardinality, no eviction), no single-flight fetch coordination (reusing this crate's own existing `upstream/cache.rs`/`upstream/refresh.rs` patterns), an SSRF error-message oracle (internal errors echoed verbatim to an anonymous caller), incomplete IP denylist coverage (IPv4-compatible IPv6, NAT64/6to4/Teredo, `0.0.0.0/8`, multicast), a dropped query/fragment rejection relative to the reference pattern, and dead code (a `Cache-Control: max-age` parse that was computed and then discarded). All fixed below; see each task for specifics.

---

## Global Constraints

- `soma-auth` must remain self-contained: no path/git dependency outside the `soma` workspace. The SSRF guard is written from scratch in this crate (adapted from `~/workspace/lab/crates/labby-primitives/src/ssrf.rs` and its caller `~/workspace/lab/crates/labby-apis/src/acp_registry/installer.rs` as *reference patterns only* — do not import or path-depend on either).
- DCR (`POST /register`, `register_client()` in `authorize.rs`) must keep working completely unchanged. This plan adds a new path at `GET /authorize`; it does not touch `register_client()`.
- **CIMD-sourced `redirect_uris` MUST pass the same `is_allowed_redirect_uri` check DCR-registered clients are held to.** This is not optional hardening — it is the fix for the critical finding above. Do not trust a CIMD document's `redirect_uris` outright at any point in this plan.
- All new code lives behind the existing `http-axum` feature gate (matches how `authorize`/`metadata`/`routes` are already gated in `crates/soma-auth/src/lib.rs`). The new `AuthState` field this plan adds must ALSO be `#[cfg(feature = "http-axum")]`-gated, even though `state.rs` itself is an ungated module — verify this doesn't regress `cargo check -p soma-auth` (no features) as part of every task's verification gate from Task 4 onward.
- Every new public function needs unit test coverage at the density already established in this crate. Tests must actually be able to pass — do not write a test against a code path that structurally cannot reach it (e.g. a plain-HTTP mock server against an HTTPS-only, loopback-blocking pipeline). Where a code path (DNS resolution, a real public HTTPS target) cannot be exercised hermetically in CI, say so explicitly in the task rather than writing a test that silently can't pass.
- `cargo build --workspace`, `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo clippy -p soma-auth --all-features --all-targets -- -D warnings`, `cargo check -p soma-auth` (no features — this must keep compiling), and `cargo fmt --all -- --check` must all stay green after every task.
- Follow `mod_module_files = "deny"` — `crates/soma-auth/src/cimd.rs` is the mod-root file (sibling to the `crates/soma-auth/src/cimd/` directory), never `cimd/mod.rs`. This mirrors the existing `crates/soma-auth/src/upstream.rs` + `crates/soma-auth/src/upstream/*.rs` structure exactly.
- Known, explicitly out-of-scope items (do not attempt in this plan; call out if you're tempted to add them):
  - **No consent/warning UI.** The MCP spec's CIMD security section says authorization servers "MUST clearly display the redirect URI hostname during authorization" and "SHOULD display additional warnings for `localhost`-only redirect URIs." `soma-auth`'s `/authorize` handler currently renders no consent page at all for the OAuth-client path — it redirects straight to Google. Building a consent/warning screen is a separate, larger feature. **This deferral is now safe** (unlike in the original plan) because the redirect-URI allowlist fix means CIMD cannot introduce a redirect target an operator hasn't already implicitly trusted via `allowed_client_redirect_uris`/loopback/native-app-scheme rules — the "MUST clearly display" requirement is a UX nicety on top of an already-enforced trust boundary here, not the only thing standing between an attacker and code theft.
  - **No HTTP conditional-request caching** (ETag / `If-None-Match`). The cache implemented here is a fixed-TTL cache (with a shorter negative-result TTL for failures), which satisfies "SHOULD cache metadata respecting HTTP cache headers" at a reasonable minimum bar without full RFC 7234 semantics. (The original plan attempted `Cache-Control: max-age` parsing but never actually wired the parsed value into the cache — that dead code is removed in this revision rather than fixed, since a fixed TTL is simpler and equally spec-compliant at the "SHOULD" bar.)
  - **No global-routable-address allowlist model.** The security review suggested switching from a private-range denylist to a "reject anything not globally routable" allowlist (more robust to future range additions, but needs an extra dependency since `Ipv4Addr::is_global`/`Ipv6Addr::is_global` are unstable in this crate's Rust edition). This plan instead extends the existing denylist to close the specific gaps identified (IPv4-compatible IPv6, NAT64/6to4/Teredo, `0.0.0.0/8`, multicast) — narrower in principle but zero new dependencies and consistent with the crate's existing pattern. Revisit if a future audit finds more gaps.
  - **No true end-to-end "real DNS + real public HTTPS + successful fetch" automated test.** This would require network access in CI, which is undesirable for hermeticity/flakiness reasons. Coverage strategy instead: (a) `ssrf::validate_url_shape` and `resolve_and_validate_address`'s *rejection* paths are fully unit-tested with literal/local hostnames (no network needed to prove a block); (b) the fetch-given-a-resolved-address pipeline (`fetch_via_pinned_address`/`fetch_document_at`) is fully tested against a local `wiremock` server, exercising every success/failure branch including the streaming size cap and redirect rejection; (c) the `/authorize`-level redirect-URI-allowlist business logic is unit-tested with an injected fetch result, decoupled from the network; (d) one true `/authorize`-level HTTP test exercises the full wire-up via an SSRF-*rejection* path (a `client_id` pointing at a private address), which requires no network since the rejection happens before any I/O. This is a deliberate, documented tradeoff, not a silent gap.

---

### Task 1: Enable the `tokio` `net` feature

**Files:**
- Modify: `crates/soma-auth/Cargo.toml`

**Interfaces:**
- Produces: `tokio::net::lookup_host` and `tokio::time::timeout` become available to the crate (used by Task 3).

- [ ] **Step 1: Add the `net` feature to the existing `tokio` dependency**

In `crates/soma-auth/Cargo.toml`, find:

```toml
tokio = { version = "1", features = ["rt-multi-thread", "macros", "time", "fs"] }
```

Change to:

```toml
tokio = { version = "1", features = ["rt-multi-thread", "macros", "time", "fs", "net"] }
```

(`"time"` is already present, which is what `tokio::time::timeout` needs — only `"net"` is new.)

- [ ] **Step 2: Verify it compiles**

Run: `cargo check -p soma-auth --all-features`
Expected: succeeds (no behavior change yet, just a new feature flag available).

- [ ] **Step 3: Commit**

```bash
git add crates/soma-auth/Cargo.toml Cargo.lock
git commit -m "build(soma-auth): enable tokio net feature for CIMD DNS resolution"
```

---

### Task 2: SSRF preflight guard (`cimd/ssrf.rs`)

**Files:**
- Create: `crates/soma-auth/src/cimd/ssrf.rs`
- Create: `docs/references/mcp/client-id-metadata-document.md` (captured spec snapshot — see Step 0)
- Test: `crates/soma-auth/src/cimd/ssrf.rs`, `#[cfg(test)] mod tests`

**Interfaces:**
- Produces:
  - `pub enum SsrfError { InvalidUrl(String), Blocked(String) }` with `pub fn kind(&self) -> &'static str` (`"invalid_param"` | `"ssrf_blocked"`)
  - `pub fn check_ip_not_private(ip: std::net::IpAddr, context: &str) -> Result<(), SsrfError>`
  - `pub fn validate_url_shape(url: &str) -> Result<url::Url, SsrfError>` — static-only checks (no DNS): must parse, scheme must be `https`, must have a host, must not include userinfo/query/fragment, host must not be a private-TLD-suffixed or textual-loopback name (trailing-dot-normalized), must contain a non-root path component, and if the host is an IP literal it must pass `check_ip_not_private`.
- Consumes: nothing (leaf module, only `std::net`, `url` — already a `soma-auth` dependency).

- [ ] **Step 0: Capture a citable spec snapshot before writing checks that claim to be spec-derived**

Fetch `https://modelcontextprotocol.io/specification/draft/basic/authorization/client-registration` and `https://datatracker.ietf.org/doc/html/draft-ietf-oauth-client-id-metadata-document-00` (at minimum its Section 6, "Security Considerations") and save a trimmed markdown copy of the requirements actually used by this plan (the `client_id` URL shape rule — "MUST use the https scheme and contain a path component" — and the SSRF/localhost-redirect/trust-policy considerations) to `docs/references/mcp/client-id-metadata-document.md`, following whatever format the existing files under `docs/references/mcp/` use (check `docs/references/mcp/conformance/` and `docs/references/mcp/schema/` first for the established convention). This gives the non-root-path constraint below (and the redirect-URI-allowlist decision in Task 4) a citable local source instead of paraphrase, per this repo's own docs policy of preferring `docs/references/mcp/` for MCP protocol behavior, especially for a fast-moving *draft*-labeled spec area like CIMD.

- [ ] **Step 1: Write the failing tests**

Create `crates/soma-auth/src/cimd/ssrf.rs`:

```rust
//! SSRF preflight guard for CIMD `client_id` URL fetches.
//!
//! This is a *static* preflight — it does not perform DNS resolution. It
//! rejects non-https schemes, userinfo, query/fragment components, a
//! missing or root-only path, private-TLD-suffixed hostnames, textual
//! loopback hostnames, and IP-literal hosts that fall in a private/
//! loopback/link-local/CGNAT/ULA/transition-mechanism/multicast range.
//! Callers that resolve a domain name to an IP address MUST additionally
//! run each resolved address through [`check_ip_not_private`] before
//! connecting, and MUST re-validate the actual TCP peer post-connect (see
//! `cimd::document::resolve_and_validate_address` and
//! `cimd::document::fetch_document_at`) — this module alone does not close
//! the DNS-rebinding/proxy-interception gap by itself.
//!
//! Adapted (not imported — this crate has no path dependency on the
//! sibling `lab` repo) from the equivalent guard in
//! `labby-primitives::ssrf` and its caller,
//! `labby-apis::acp_registry::installer`, which additionally documents and
//! implements post-connect peer re-validation as "the load-bearing line of
//! the SSRF TOCTOU / DNS-rebinding defense" — that additional layer is
//! implemented in `cimd::document`, not here (this module only covers the
//! static/pre-DNS portion of the reference's rigor).

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

/// Reason a CIMD `client_id` URL was rejected.
#[derive(Debug, Clone, thiserror::Error)]
pub enum SsrfError {
    /// URL could not be parsed, used a non-https scheme, lacked a host,
    /// carried forbidden userinfo/query/fragment, or lacked a path
    /// component.
    #[error("{0}")]
    InvalidUrl(String),
    /// Host resolved (or parsed) to a private/loopback/link-local/CGNAT/
    /// ULA/transition-mechanism/multicast address, or matched a
    /// private-TLD suffix denylist.
    #[error("{0}")]
    Blocked(String),
}

impl SsrfError {
    #[must_use]
    pub fn kind(&self) -> &'static str {
        match self {
            Self::InvalidUrl(_) => "invalid_param",
            Self::Blocked(_) => "ssrf_blocked",
        }
    }
}

/// Private-DNS suffix denylist applied to non-IP hosts. Belt-and-suspenders
/// on top of the resolved-IP checks (an internal name might resolve through
/// split-horizon DNS to a public-looking record at validation time).
pub const PRIVATE_TLD_SUFFIXES: &[&str] =
    &[".local", ".internal", ".lan", ".intranet", ".corp", ".home"];

/// Returns `true` for the IPv4 carrier-grade NAT range `100.64.0.0/10`.
#[must_use]
pub fn is_cgnat(ip: Ipv4Addr) -> bool {
    let octets = ip.octets();
    octets[0] == 100 && (64..=127).contains(&octets[1])
}

/// Returns `true` for `0.0.0.0/8` ("this network", broader than the single
/// unspecified address), IPv4 multicast (`224.0.0.0/4`), and the limited
/// broadcast address `255.255.255.255`.
#[must_use]
pub fn is_ipv4_reserved_broadcast_or_multicast(ip: Ipv4Addr) -> bool {
    ip.octets()[0] == 0 || ip.is_multicast() || ip.is_broadcast()
}

fn is_ipv6_link_local(ip: Ipv6Addr) -> bool {
    (ip.segments()[0] & 0xffc0) == 0xfe80
}

fn is_ipv6_ula(ip: Ipv6Addr) -> bool {
    (ip.segments()[0] & 0xfe00) == 0xfc00
}

/// Returns `true` for the deprecated IPv4-compatible IPv6 form (`::a.b.c.d`,
/// distinct from the IPv4-*mapped* `::ffff:a.b.c.d` form already handled
/// separately via `to_ipv4_mapped()`). Some stacks still route this form;
/// treat any non-loopback, non-unspecified address here as embedding a
/// target IPv4 address this guard has not yet validated.
#[must_use]
pub fn is_ipv6_ipv4_compatible(ip: Ipv6Addr) -> bool {
    let s = ip.segments();
    s[0] == 0 && s[1] == 0 && s[2] == 0 && s[3] == 0 && s[4] == 0 && s[5] == 0
        && !ip.is_loopback()
        && !ip.is_unspecified()
}

/// Returns `true` for IPv6 transition-mechanism prefixes that embed an
/// IPv4 address reachable through the mechanism — NAT64 (`64:ff9b::/96`),
/// 6to4 (`2002::/16`), and Teredo (`2001::/32`). These are more credible
/// SSRF bypasses than IPv6 documentation ranges and are blocked wholesale
/// (not selectively unwrapped) since this is a conservative preflight for
/// an OAuth Authorization Server fetching an attacker-supplied URL, not a
/// general-purpose network client that needs transition-mechanism support.
#[must_use]
pub fn is_ipv6_transition_mechanism(ip: Ipv6Addr) -> bool {
    let s = ip.segments();
    (s[0] == 0x0064 && s[1] == 0xff9b && s[2] == 0 && s[3] == 0 && s[4] == 0 && s[5] == 0)
        || s[0] == 0x2002
        || (s[0] == 0x2001 && s[1] == 0)
}

/// Reject an IP that targets private, loopback, link-local, CGNAT, ULA,
/// IPv4-mapped-private, IPv4-compatible, transition-mechanism, reserved,
/// multicast, or broadcast space. `context` is a non-secret label (redacted
/// URL or bare host) used only to build the error message.
///
/// # Errors
/// Returns [`SsrfError::Blocked`] when `ip` falls in any blocked range.
pub fn check_ip_not_private(ip: IpAddr, context: &str) -> Result<(), SsrfError> {
    let normalized = match ip {
        IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
            Some(v4) => IpAddr::V4(v4),
            None => IpAddr::V6(v6),
        },
        other => other,
    };

    let blocked = match normalized {
        IpAddr::V4(v4) => {
            v4.is_private()
                || v4.is_loopback()
                || v4.is_link_local()
                || v4.is_unspecified()
                || is_cgnat(v4)
                || is_ipv4_reserved_broadcast_or_multicast(v4)
        }
        IpAddr::V6(v6) => {
            v6.is_loopback()
                || v6.is_unspecified()
                || is_ipv6_link_local(v6)
                || is_ipv6_ula(v6)
                || is_ipv6_ipv4_compatible(v6)
                || is_ipv6_transition_mechanism(v6)
        }
    };

    if blocked {
        return Err(SsrfError::Blocked(format!(
            "`{context}` resolves to a private, loopback, link-local, CGNAT, ULA, transition-mechanism, reserved, multicast, or broadcast address {ip}; blocked to prevent SSRF"
        )));
    }

    Ok(())
}

fn check_host_not_private(host: &str) -> Result<(), SsrfError> {
    let host_lower = host.to_ascii_lowercase();
    let host_lower = host_lower.strip_suffix('.').unwrap_or(&host_lower);
    if host_lower == "localhost"
        || host_lower.starts_with("127.")
        || host_lower == "::1"
        || host_lower.contains("::ffff:")
        || host_lower == "0.0.0.0"
        || PRIVATE_TLD_SUFFIXES.iter().any(|s| host_lower.ends_with(s))
    {
        return Err(SsrfError::Blocked(format!(
            "host `{host}` is a local/loopback/private address"
        )));
    }
    Ok(())
}

fn redact_url(raw: &str) -> String {
    match url::Url::parse(raw) {
        Ok(mut url) => {
            let _ = url.set_username("");
            let _ = url.set_password(None);
            url.set_query(None);
            url.set_fragment(None);
            url.to_string()
        }
        Err(_) => "<invalid-url>".to_string(),
    }
}

/// Parse and statically validate a CIMD `client_id` URL: require https,
/// forbid userinfo/query/fragment, require a non-root path component,
/// require a host, and reject the private-TLD/loopback host denylist. If
/// the host is an IP literal it is additionally run through
/// [`check_ip_not_private`].
///
/// This performs **no DNS** — see the module doc for what callers must do
/// after resolving a domain-name host.
///
/// # Errors
/// Returns [`SsrfError`] when any static rule is violated.
pub fn validate_url_shape(url: &str) -> Result<url::Url, SsrfError> {
    let redacted = redact_url(url);
    let parsed = url::Url::parse(url)
        .map_err(|e| SsrfError::InvalidUrl(format!("invalid URL `{redacted}`: {e}")))?;

    if parsed.scheme() != "https" {
        return Err(SsrfError::InvalidUrl(format!(
            "URL `{redacted}` must use https to prevent SSRF"
        )));
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(SsrfError::InvalidUrl(format!(
            "URL `{redacted}` must not include userinfo"
        )));
    }
    if parsed.query().is_some() || parsed.fragment().is_some() {
        return Err(SsrfError::InvalidUrl(format!(
            "URL `{redacted}` must not include query or fragment components"
        )));
    }
    if parsed.path().is_empty() || parsed.path() == "/" {
        return Err(SsrfError::InvalidUrl(format!(
            "URL `{redacted}` must contain a path component (see docs/references/mcp/client-id-metadata-document.md)"
        )));
    }

    match parsed.host() {
        Some(url::Host::Domain(domain)) => check_host_not_private(domain)?,
        Some(url::Host::Ipv4(ip)) => check_ip_not_private(IpAddr::V4(ip), &redacted)?,
        Some(url::Host::Ipv6(ip)) => check_ip_not_private(IpAddr::V6(ip), &redacted)?,
        None => {
            return Err(SsrfError::InvalidUrl(format!(
                "URL `{redacted}` must include a host"
            )));
        }
    }

    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_private_ranges_exactly() {
        for ip in [
            "127.0.0.1",
            "10.1.2.3",
            "172.16.0.1",
            "192.168.1.1",
            "169.254.1.1",
            "169.254.169.254", // cloud metadata service
            "100.64.0.1",
            "100.127.255.255",
            "0.5.5.5",         // 0.0.0.0/8 "this network"
            "224.0.0.1",       // multicast
            "255.255.255.255", // limited broadcast
            "::1",
            "fe80::1",
            "fc00::1",
            "fd00::1",
            "::ffff:127.0.0.1",
            "::ffff:10.1.2.3",
            "::ffff:100.64.0.1",
            "::ffff:169.254.169.254",
            "::7f00:1",       // IPv4-compatible IPv6 form of 127.0.0.1
            "64:ff9b::7f00:1", // NAT64-embedded loopback
            "2002::1",         // 6to4
            "2001::1",         // Teredo
        ] {
            let parsed: IpAddr = ip.parse().expect(ip);
            let err = check_ip_not_private(parsed, "app.example.com").unwrap_err();
            assert_eq!(err.kind(), "ssrf_blocked", "{ip}");
        }
    }

    #[test]
    fn allows_public_addresses() {
        for ip in ["1.1.1.1", "8.8.8.8", "2606:4700:4700::1111"] {
            let parsed: IpAddr = ip.parse().expect(ip);
            check_ip_not_private(parsed, "app.example.com").expect(ip);
        }
    }

    #[test]
    fn rejects_non_https_as_invalid_param() {
        let err =
            validate_url_shape("http://app.example.com/oauth/client-metadata.json").unwrap_err();
        assert_eq!(err.kind(), "invalid_param");
    }

    #[test]
    fn rejects_missing_path_as_invalid_param() {
        let err = validate_url_shape("https://app.example.com").unwrap_err();
        assert_eq!(err.kind(), "invalid_param");
        let err_root = validate_url_shape("https://app.example.com/").unwrap_err();
        assert_eq!(err_root.kind(), "invalid_param");
    }

    #[test]
    fn rejects_userinfo() {
        let err = validate_url_shape("https://user@app.example.com/client.json").unwrap_err();
        assert_eq!(err.kind(), "invalid_param");
    }

    #[test]
    fn rejects_query_and_fragment() {
        let err_query =
            validate_url_shape("https://app.example.com/client.json?x=1").unwrap_err();
        assert_eq!(err_query.kind(), "invalid_param");
        let err_fragment =
            validate_url_shape("https://app.example.com/client.json#x").unwrap_err();
        assert_eq!(err_fragment.kind(), "invalid_param");
    }

    #[test]
    fn rejects_private_and_loopback_hosts_as_blocked() {
        for url in [
            "https://app.local/client.json",
            "https://127.0.0.1/client.json",
            "https://[::ffff:127.0.0.1]/client.json",
            "https://192.168.1.20/client.json",
        ] {
            let err = validate_url_shape(url).unwrap_err();
            assert_eq!(err.kind(), "ssrf_blocked", "{url}");
        }
    }

    #[test]
    fn rejects_bracketed_ipv6_literals() {
        for url in [
            "https://[::1]/client.json",
            "https://[fe80::1]/client.json",
            "https://[fc00::1]/client.json",
        ] {
            let err = validate_url_shape(url).unwrap_err();
            assert_eq!(err.kind(), "ssrf_blocked", "{url}");
        }
    }

    #[test]
    fn private_tld_suffixes_are_blocked() {
        for host_url in [
            "https://box.local/c.json",
            "https://svc.internal/c.json",
            "https://host.lan/c.json",
        ] {
            let err = validate_url_shape(host_url).unwrap_err();
            assert_eq!(err.kind(), "ssrf_blocked", "{host_url}");
        }
    }

    #[test]
    fn private_tld_suffix_bypass_via_trailing_dot_is_blocked() {
        // "svc.internal." (FQDN trailing dot) resolves identically to
        // "svc.internal" and must not bypass the suffix denylist.
        let err = validate_url_shape("https://svc.internal./c.json").unwrap_err();
        assert_eq!(err.kind(), "ssrf_blocked");
    }

    #[test]
    fn allows_valid_public_https_url_with_path() {
        let parsed = validate_url_shape("https://app.example.com/oauth/client-metadata.json")
            .expect("should validate");
        assert_eq!(parsed.host_str(), Some("app.example.com"));
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail (module doesn't exist yet, `cimd.rs` mod-root not wired)**

This file alone won't compile as part of the crate yet (Task 3 wires the mod tree). Verify the file itself is syntactically self-consistent by running `rustfmt --check crates/soma-auth/src/cimd/ssrf.rs` — expect no output. Full compilation happens once Task 3 wires `lib.rs`.

- [ ] **Step 3: Commit** (bundled with Task 3's mod-root wiring, since this file can't compile standalone — see Task 3 Step 4)

---

### Task 3: `ClientMetadataDocument` fetch + validate + single-flight cache (`cimd/document.rs`) and mod-root wiring

**Files:**
- Create: `crates/soma-auth/src/cimd/document.rs`
- Create: `crates/soma-auth/src/cimd.rs` (mod root)
- Modify: `crates/soma-auth/src/lib.rs`
- Test: `crates/soma-auth/src/cimd/document.rs`, `#[cfg(test)] mod tests`

**Interfaces:**
- Consumes: `cimd::ssrf::{validate_url_shape, check_ip_not_private, SsrfError}` (Task 2)
- Produces:
  - `pub struct ClientMetadataDocument { pub client_id: String, pub client_name: String, pub redirect_uris: Vec<String> }` (deserializes extra unknown fields silently — no `deny_unknown_fields`)
  - `pub enum CimdError { Ssrf(ssrf::SsrfError), DnsResolutionFailed(String, String), DnsBlocked(String), Fetch(String), PeerMismatch { expected: SocketAddr, actual: SocketAddr }, InvalidDocument(String), ClientIdMismatch { document_client_id: String, requested_url: String } }`, `Clone + Debug`, with `pub fn kind(&self) -> &'static str`
  - `pub fn is_cimd_client_id(client_id: &str) -> bool` — cheap `starts_with("https://")` check
  - `pub async fn resolve_and_validate_address(host: &str, port: u16) -> Result<SocketAddr, CimdError>` — real DNS resolution via `tokio::net::lookup_host`, bounded by a `DNS_TIMEOUT`, rejects the *entire* resolved set if *any* resolved address is private (stricter than "first public wins" — matches the reference pattern's stance), returns the first address otherwise
  - `pub(crate) async fn fetch_via_pinned_address(url: &str, host: &str, addr: SocketAddr) -> Result<ClientMetadataDocument, CimdError>` — builds an address-pinned, no-proxy, no-redirect `reqwest::Client` and delegates to `fetch_document_at`. **This is the test seam**: tests call it directly with a local `wiremock` server's real bound address, with no DNS involved, while still exercising pinning/no-proxy/redirect-policy/peer-recheck/streaming-cap/document-validation.
  - `pub(crate) async fn fetch_document_at(client: &reqwest::Client, url: &str, pinned_addr: SocketAddr) -> Result<ClientMetadataDocument, CimdError>` — given an already-pinned client and the address it's pinned to, issues `GET url`, re-validates the actual TCP peer against `pinned_addr` post-connect, streams the body with a running byte cap, parses JSON, validates required fields + non-empty `redirect_uris`, validates `document.client_id == url` byte-for-byte
  - `pub async fn fetch_and_validate_client_metadata(cache: &DocumentCache, url: &str) -> Result<ClientMetadataDocument, CimdError>` — the production entry point: single-flight-locked cache lookup (including a short negative-result cooldown for cached failures), else `ssrf::validate_url_shape` -> DNS resolve+validate (for domain hosts) or direct IP-literal use -> `fetch_via_pinned_address` -> cache the result (success or failure)
  - `pub struct DocumentCache { .. }` with `pub fn new() -> Self`, backed by `dashmap::DashMap` for both cached results and per-key single-flight locks (mirrors `crate::upstream::cache::OauthClientCache`'s `build_locks` pattern), positive TTL 300s, negative (failure) TTL 60s, capped at `MAX_CACHE_ENTRIES` with a sweep-on-insert eviction of expired entries

- [ ] **Step 1: Write the failing tests**

Create `crates/soma-auth/src/cimd/document.rs`:

```rust
//! Fetch, validate, and cache OAuth Client ID Metadata Documents (CIMD).
//!
//! Split into independently testable layers:
//! 1. [`ssrf::validate_url_shape`] — static URL checks, no network (tested
//!    in isolation in `cimd::ssrf`).
//! 2. [`resolve_and_validate_address`] — real DNS resolution, bounded by a
//!    timeout, rejecting the whole resolved-address set if any address is
//!    private. Tested with literal loopback/private hostnames — no real
//!    network access needed to prove a *rejection*; a real "successful
//!    public resolution" is not unit-tested here (see the plan's Global
//!    Constraints for why: no network access in CI).
//! 3. [`fetch_via_pinned_address`] / [`fetch_document_at`] — given an
//!    ALREADY resolved+validated address, builds a pinned/no-proxy/
//!    no-redirect client and does the GET + peer-recheck + streaming-cap +
//!    parse + validate. Tested against a local `wiremock` server by
//!    pointing the pin directly at its real bound address — this
//!    deliberately bypasses DNS resolution (same as production code does
//!    once step 2 has already resolved+validated an address), so it needs
//!    no network and no HTTPS certificate.
//!
//! [`fetch_and_validate_client_metadata`] composes all three for the real
//! production path, with per-key single-flight coordination and a short
//! negative-result cooldown for cached failures — mirroring
//! `crate::upstream::cache::OauthClientCache`'s `build_locks` pattern,
//! since `client_id` here is just as attacker/caller-influenced as an
//! upstream OAuth client and deserves the same protection against
//! concurrent-first-request stampedes.

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, Instant};

use dashmap::DashMap;
use serde::Deserialize;
use tokio::sync::Mutex;

use crate::cimd::ssrf;

/// Maximum response body size accepted from a CIMD fetch, enforced via a
/// running counter WHILE STREAMING (never buffer-then-check — a hostile
/// server can otherwise force unbounded memory use regardless of this
/// constant).
const MAX_DOCUMENT_BYTES: usize = 64 * 1024;

/// Fetch timeout for CIMD document requests, applied to the HTTP client
/// AFTER DNS resolution has already completed (see `DNS_TIMEOUT` for the
/// separate bound on resolution itself). Aligned with this crate's own
/// precedent for a hot-path, user-is-actively-waiting fetch —
/// `google.rs::GOOGLE_JWKS_FETCH_TIMEOUT` uses 5s for the same reason
/// (`/authorize` must stay responsive; `/token`-adjacent background
/// refreshes can afford the looser 30s bound elsewhere in that file).
const FETCH_TIMEOUT: Duration = Duration::from_secs(5);

/// Timeout for the DNS resolution step, bounded separately from
/// `FETCH_TIMEOUT` because `tokio::net::lookup_host` has no timeout of its
/// own — it delegates to the OS resolver, whose worst-case latency is
/// governed by `/etc/resolv.conf`/systemd-resolved settings, not by
/// anything in this code.
const DNS_TIMEOUT: Duration = Duration::from_secs(3);

/// Cache TTL for a successfully fetched and validated document.
const CACHE_TTL: Duration = Duration::from_secs(300);

/// Cache TTL for a *failed* fetch/validation attempt. Short — long enough
/// to blunt a burst of retries against a hostile or broken `client_id`
/// without permanently poisoning a transiently-unreachable legitimate one.
const NEGATIVE_CACHE_TTL: Duration = Duration::from_secs(60);

/// Hard cap on distinct cached URLs. `client_id` cardinality is
/// attacker-controlled (any public HTTPS server counts), not
/// traffic-volume-controlled, so this cannot be sized by "realistic
/// legitimate usage" — it exists specifically to bound the memory an
/// adversary can force this map to hold.
const MAX_CACHE_ENTRIES: usize = 10_000;

#[derive(Debug, Clone, Deserialize)]
pub struct ClientMetadataDocument {
    pub client_id: String,
    pub client_name: String,
    #[serde(default)]
    pub redirect_uris: Vec<String>,
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum CimdError {
    #[error(transparent)]
    Ssrf(#[from] ssrf::SsrfError),
    /// A genuine DNS lookup failure (NXDOMAIN, resolver timeout, network
    /// unreachable) — an operational problem, NOT a security event. Kept
    /// distinct from [`Self::DnsBlocked`] so logs/callers can tell a
    /// mistyped hostname apart from an actual SSRF attempt.
    #[error("dns resolution failed for `{0}`: {1}")]
    DnsResolutionFailed(String, String),
    /// DNS resolution succeeded but at least one resolved address was
    /// private/loopback/link-local/etc — the whole result is rejected
    /// rather than falling back to a public address in the same set,
    /// since a hostname resolving to a mix of public and private
    /// addresses is itself a signal worth treating as untrusted.
    #[error("`{0}` resolved to at least one private/loopback/link-local address; blocked to prevent SSRF")]
    DnsBlocked(String),
    #[error("fetch failed: {0}")]
    Fetch(String),
    /// The actual TCP peer the response came from did not match the
    /// address this fetch was pinned to. This is the post-connect
    /// TOCTOU/proxy-interception backstop — see `fetch_document_at`.
    #[error("peer address {actual} did not match the validated address {expected}; possible proxy interception or DNS-rebinding attempt")]
    PeerMismatch {
        expected: SocketAddr,
        actual: SocketAddr,
    },
    #[error("invalid client metadata document: {0}")]
    InvalidDocument(String),
    #[error("client metadata document client_id `{document_client_id}` does not match the requested URL `{requested_url}`")]
    ClientIdMismatch {
        document_client_id: String,
        requested_url: String,
    },
}

impl CimdError {
    /// Stable kind string for structured logging. Deliberately NOT surfaced
    /// verbatim (via `Display`/`to_string()`) to the anonymous `/authorize`
    /// caller — see `authorize::resolve_client_redirect_uris` in Task 4,
    /// which logs the full error server-side via this `kind()` plus
    /// `Display` but returns only a generic message in the HTTP response.
    /// A detailed message returned to an unauthenticated caller lets them
    /// distinguish "resolves internally" from "doesn't exist" from
    /// "resolves publicly but unreachable," which is a network-topology
    /// mapping oracle.
    #[must_use]
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Ssrf(e) => e.kind(),
            Self::DnsResolutionFailed(..) => "dns_resolution_failed",
            Self::DnsBlocked(_) => "ssrf_blocked",
            Self::Fetch(_) => "cimd_fetch_failed",
            Self::PeerMismatch { .. } => "ssrf_blocked",
            Self::InvalidDocument(_) => "invalid_client_metadata",
            Self::ClientIdMismatch { .. } => "invalid_client_metadata",
        }
    }
}

/// Cheap detection heuristic: a CIMD `client_id` is an `https://` URL.
/// soma-auth's own DCR-issued `client_id`s are opaque base64url tokens
/// (`random_token(18)` in `authorize::register_client`) and can never start
/// with `https://`.
#[must_use]
pub fn is_cimd_client_id(client_id: &str) -> bool {
    client_id.starts_with("https://")
}

/// Resolve `host:port` via DNS (bounded by [`DNS_TIMEOUT`]) and return the
/// first resolved address, rejecting the *entire* result set if *any*
/// resolved address is private/loopback/etc — a hostname resolving to a
/// mix of public and private addresses is treated as untrusted outright
/// rather than cherry-picking a public one, since DNS load-balancing could
/// non-deterministically prefer the private one on a subsequent lookup
/// even though this specific call pins one address.
///
/// # Errors
/// Returns [`CimdError::DnsResolutionFailed`] on timeout/lookup failure or
/// an empty result set, and [`CimdError::DnsBlocked`] if any resolved
/// address is private.
pub async fn resolve_and_validate_address(
    host: &str,
    port: u16,
) -> Result<SocketAddr, CimdError> {
    let lookup = tokio::time::timeout(DNS_TIMEOUT, tokio::net::lookup_host((host, port)))
        .await
        .map_err(|_| {
            CimdError::DnsResolutionFailed(
                host.to_string(),
                format!("timed out after {DNS_TIMEOUT:?}"),
            )
        })?
        .map_err(|e| CimdError::DnsResolutionFailed(host.to_string(), e.to_string()))?;
    let addrs: Vec<SocketAddr> = lookup.collect();
    if addrs.is_empty() {
        return Err(CimdError::DnsResolutionFailed(
            host.to_string(),
            "resolved to no addresses".to_string(),
        ));
    }
    if addrs
        .iter()
        .any(|addr| ssrf::check_ip_not_private(addr.ip(), host).is_err())
    {
        return Err(CimdError::DnsBlocked(host.to_string()));
    }
    Ok(addrs[0])
}

/// Given an already resolved+validated `addr`, build a pinned, no-proxy,
/// no-redirect `reqwest::Client` and run the guarded fetch. This is the
/// test seam: tests call it directly with a local `wiremock` server's real
/// bound address, entirely bypassing DNS resolution — exactly what
/// production code does once [`resolve_and_validate_address`] (or the
/// IP-literal branch in [`fetch_and_validate_client_metadata`]) has already
/// produced a validated `addr`.
///
/// # Errors
/// Propagates [`CimdError`] from client construction or [`fetch_document_at`].
pub(crate) async fn fetch_via_pinned_address(
    url: &str,
    host: &str,
    addr: SocketAddr,
) -> Result<ClientMetadataDocument, CimdError> {
    let client = reqwest::Client::builder()
        .resolve(host, addr)
        // Without this, an ambient HTTPS_PROXY/ALL_PROXY env var makes
        // reqwest connect to a proxy that resolves `host` ITSELF, silently
        // discarding the `.resolve()` pin above and reopening the exact
        // DNS-rebinding window this whole module exists to close.
        .no_proxy()
        // A redirect would fetch a URL other than `url`, which
        // `fetch_document_at`'s exact-match check couldn't validate
        // against `client_id` — treat any 3xx as a hard failure instead
        // of following it.
        .redirect(reqwest::redirect::Policy::none())
        .timeout(FETCH_TIMEOUT)
        .build()
        .map_err(|e| CimdError::Fetch(format!("build pinned client for `{url}`: {e}")))?;
    fetch_document_at(&client, url, addr).await
}

/// Fetch and validate a CIMD document at `url` using an already
/// address-pinned `client`. Does NOT perform DNS resolution or SSRF
/// filtering itself — that is [`resolve_and_validate_address`]'s job. Does,
/// however, re-validate the actual TCP peer the response came from against
/// `pinned_addr` — this closes the gap a bare `.resolve()` pin leaves open
/// if a proxy intercepted the connection despite `.no_proxy()`, or if the
/// pin's `host` key ever diverges from the authority host reqwest derives
/// when re-parsing `url` internally.
///
/// # Errors
/// Returns [`CimdError::Fetch`] on transport/HTTP failure or a non-success
/// status, [`CimdError::PeerMismatch`] if the connected peer doesn't match
/// `pinned_addr`, [`CimdError::InvalidDocument`] on an oversized body,
/// malformed JSON, or missing/empty required fields, and
/// [`CimdError::ClientIdMismatch`] when the document's `client_id` does not
/// equal `url` exactly.
pub(crate) async fn fetch_document_at(
    client: &reqwest::Client,
    url: &str,
    pinned_addr: SocketAddr,
) -> Result<ClientMetadataDocument, CimdError> {
    let mut response = client
        .get(url)
        .send()
        .await
        .map_err(|e| CimdError::Fetch(format!("GET `{url}`: {e}")))?;

    if let Some(peer) = response.remote_addr() {
        if peer != pinned_addr {
            return Err(CimdError::PeerMismatch {
                expected: pinned_addr,
                actual: peer,
            });
        }
        ssrf::check_ip_not_private(peer.ip(), url)?;
    }

    if !response.status().is_success() {
        return Err(CimdError::Fetch(format!(
            "GET `{url}` returned HTTP {}",
            response.status()
        )));
    }

    let mut buf: Vec<u8> = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|e| CimdError::Fetch(format!("read body from `{url}`: {e}")))?
    {
        buf.extend_from_slice(&chunk);
        if buf.len() > MAX_DOCUMENT_BYTES {
            return Err(CimdError::InvalidDocument(format!(
                "document at `{url}` exceeds the {MAX_DOCUMENT_BYTES}-byte limit"
            )));
        }
    }

    let document: ClientMetadataDocument = serde_json::from_slice(&buf).map_err(|e| {
        CimdError::InvalidDocument(format!("document at `{url}` is not valid JSON: {e}"))
    })?;
    if document.client_id.is_empty() || document.client_name.is_empty() {
        return Err(CimdError::InvalidDocument(format!(
            "document at `{url}` is missing required client_id or client_name"
        )));
    }
    if document.redirect_uris.is_empty() {
        return Err(CimdError::InvalidDocument(format!(
            "document at `{url}` declares no redirect_uris"
        )));
    }
    if document.client_id != url {
        return Err(CimdError::ClientIdMismatch {
            document_client_id: document.client_id,
            requested_url: url.to_string(),
        });
    }
    Ok(document)
}

struct CacheEntry {
    /// `Err` holds a short, cacheable failure summary (`CimdError`'s
    /// `Display` output) rather than the error type itself, so a cached
    /// failure doesn't need `CimdError` to round-trip through the cache
    /// bit-for-bit — only its message needs to survive.
    result: Result<ClientMetadataDocument, String>,
    fetched_at: Instant,
    ttl: Duration,
}

/// Single-flight, TTL-and-negative-cached store for fetched CIMD documents,
/// keyed by the requested URL. Mirrors
/// `crate::upstream::cache::OauthClientCache`'s `build_locks` pattern:
/// concurrent callers for the same never-cached (or just-expired) URL
/// serialize on a per-key lock so only one of them actually performs the
/// DNS resolution + fetch; the rest wait for and reuse that result.
pub struct DocumentCache {
    entries: DashMap<String, CacheEntry>,
    build_locks: DashMap<String, Arc<Mutex<()>>>,
}

impl DocumentCache {
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: DashMap::new(),
            build_locks: DashMap::new(),
        }
    }

    fn get_fresh(&self, url: &str) -> Option<Result<ClientMetadataDocument, CimdError>> {
        let entry = self.entries.get(url)?;
        if entry.fetched_at.elapsed() >= entry.ttl {
            return None;
        }
        Some(match &entry.result {
            Ok(document) => Ok(document.clone()),
            Err(summary) => Err(CimdError::Fetch(summary.clone())),
        })
    }

    fn insert(&self, url: String, result: &Result<ClientMetadataDocument, CimdError>, ttl: Duration) {
        if self.entries.len() >= MAX_CACHE_ENTRIES {
            self.entries.retain(|_, e| e.fetched_at.elapsed() < e.ttl);
        }
        let result = result.as_ref().map(Clone::clone).map_err(ToString::to_string);
        self.entries.insert(
            url,
            CacheEntry {
                result,
                fetched_at: Instant::now(),
                ttl,
            },
        );
    }
}

impl Default for DocumentCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Production entry point: single-flight-locked cache lookup (including a
/// short negative-result cooldown for cached failures), else SSRF-validate
/// the URL shape, resolve+validate DNS (for domain hosts) or use the
/// already-validated IP literal directly, fetch via
/// [`fetch_via_pinned_address`], and cache the result either way.
///
/// # Errors
/// Propagates [`CimdError`] from any of the composed validation/fetch
/// steps.
pub async fn fetch_and_validate_client_metadata(
    cache: &DocumentCache,
    url: &str,
) -> Result<ClientMetadataDocument, CimdError> {
    if let Some(cached) = cache.get_fresh(url) {
        return cached;
    }

    let lock = cache
        .build_locks
        .entry(url.to_string())
        .or_insert_with(|| Arc::new(Mutex::new(())))
        .clone();
    let _guard = lock.lock().await;

    // Re-check after acquiring the lock: another caller may have finished
    // fetching (successfully or not) while we were waiting.
    if let Some(cached) = cache.get_fresh(url) {
        return cached;
    }

    let result: Result<ClientMetadataDocument, CimdError> = async {
        let parsed = ssrf::validate_url_shape(url)?;
        let host = parsed
            .host_str()
            .ok_or_else(|| {
                CimdError::DnsResolutionFailed(url.to_string(), "no host".to_string())
            })?
            .to_string();
        let port = parsed.port_or_known_default().unwrap_or(443);

        let addr = match parsed.host() {
            Some(url::Host::Domain(_)) => resolve_and_validate_address(&host, port).await?,
            // IP-literal hosts already passed check_ip_not_private inside
            // validate_url_shape; no DNS step needed.
            Some(url::Host::Ipv4(ip)) => SocketAddr::new(IpAddr::V4(ip), port),
            Some(url::Host::Ipv6(ip)) => SocketAddr::new(IpAddr::V6(ip), port),
            None => unreachable!("validate_url_shape guarantees a host"),
        };

        fetch_via_pinned_address(url, &host, addr).await
    }
    .await;

    let ttl = if result.is_ok() {
        CACHE_TTL
    } else {
        NEGATIVE_CACHE_TTL
    };
    cache.insert(url.to_string(), &result, ttl);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn is_cimd_client_id_detects_https_urls_only() {
        assert!(is_cimd_client_id(
            "https://app.example.com/oauth/client-metadata.json"
        ));
        assert!(!is_cimd_client_id("abcDEF123opaque-token"));
        assert!(!is_cimd_client_id("http://app.example.com/client.json"));
    }

    #[tokio::test]
    async fn resolve_and_validate_address_rejects_loopback_host() {
        let err = resolve_and_validate_address("localhost", 443)
            .await
            .unwrap_err();
        assert_eq!(err.kind(), "ssrf_blocked");
    }

    #[tokio::test]
    async fn resolve_and_validate_address_reports_dns_failure_distinctly_from_ssrf_block() {
        // A hostname under a reserved-for-documentation TLD that will not
        // resolve is a genuine lookup failure, not an SSRF block -- the
        // `kind()` must distinguish the two so operators aren't misled
        // into thinking a typo is an attack.
        let err = resolve_and_validate_address("definitely-does-not-exist.invalid", 443)
            .await
            .unwrap_err();
        assert_eq!(err.kind(), "dns_resolution_failed");
    }

    #[tokio::test]
    async fn fetch_via_pinned_address_succeeds_for_matching_client_id() {
        let server = MockServer::start().await;
        let addr = server.address();
        let url = format!("{}/client.json", server.uri());
        Mock::given(method("GET"))
            .and(path("/client.json"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "client_id": url,
                "client_name": "Example MCP Client",
                "redirect_uris": ["http://127.0.0.1:3000/callback"],
            })))
            .mount(&server)
            .await;

        let document = fetch_via_pinned_address(&url, "app.example.com", *addr)
            .await
            .expect("fetch ok");
        assert_eq!(document.client_id, url);
        assert_eq!(document.client_name, "Example MCP Client");
        assert_eq!(
            document.redirect_uris,
            vec!["http://127.0.0.1:3000/callback"]
        );
    }

    #[tokio::test]
    async fn fetch_document_at_rejects_peer_mismatch() {
        // Simulates what would happen if the pin's target address ever
        // diverged from the actual connected peer (proxy interception,
        // resolve()-key mismatch): even though the client genuinely
        // connects to the real mock server, passing a WRONG `pinned_addr`
        // must be rejected rather than silently trusted.
        let server = MockServer::start().await;
        let real_addr = *server.address();
        let url = format!("{}/client.json", server.uri());
        Mock::given(method("GET"))
            .and(path("/client.json"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "client_id": url,
                "client_name": "Example",
                "redirect_uris": ["http://127.0.0.1:3000/callback"],
            })))
            .mount(&server)
            .await;

        let wrong_addr = SocketAddr::new(real_addr.ip(), real_addr.port().wrapping_add(1).max(1));
        let client = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap();
        let err = fetch_document_at(&client, &url, wrong_addr).await.unwrap_err();
        assert!(matches!(err, CimdError::PeerMismatch { .. }));
    }

    #[tokio::test]
    async fn fetch_via_pinned_address_rejects_client_id_mismatch() {
        let server = MockServer::start().await;
        let addr = *server.address();
        let url = format!("{}/client.json", server.uri());
        Mock::given(method("GET"))
            .and(path("/client.json"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "client_id": "https://attacker.example/spoofed.json",
                "client_name": "Spoofed Client",
                "redirect_uris": ["http://127.0.0.1:9999/callback"],
            })))
            .mount(&server)
            .await;

        let err = fetch_via_pinned_address(&url, "app.example.com", addr)
            .await
            .unwrap_err();
        assert!(matches!(err, CimdError::ClientIdMismatch { .. }));
    }

    #[tokio::test]
    async fn fetch_via_pinned_address_rejects_missing_required_fields() {
        let server = MockServer::start().await;
        let addr = *server.address();
        let url = format!("{}/client.json", server.uri());
        Mock::given(method("GET"))
            .and(path("/client.json"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "client_id": url,
                "redirect_uris": ["http://127.0.0.1:3000/callback"],
            })))
            .mount(&server)
            .await;

        let err = fetch_via_pinned_address(&url, "app.example.com", addr)
            .await
            .unwrap_err();
        assert!(matches!(err, CimdError::InvalidDocument(_)));
    }

    #[tokio::test]
    async fn fetch_via_pinned_address_rejects_empty_redirect_uris() {
        let server = MockServer::start().await;
        let addr = *server.address();
        let url = format!("{}/client.json", server.uri());
        Mock::given(method("GET"))
            .and(path("/client.json"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "client_id": url,
                "client_name": "Example",
                "redirect_uris": [],
            })))
            .mount(&server)
            .await;

        let err = fetch_via_pinned_address(&url, "app.example.com", addr)
            .await
            .unwrap_err();
        assert!(matches!(err, CimdError::InvalidDocument(_)));
    }

    #[tokio::test]
    async fn fetch_via_pinned_address_rejects_non_success_status() {
        let server = MockServer::start().await;
        let addr = *server.address();
        let url = format!("{}/missing.json", server.uri());
        Mock::given(method("GET"))
            .and(path("/missing.json"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;

        let err = fetch_via_pinned_address(&url, "app.example.com", addr)
            .await
            .unwrap_err();
        assert!(matches!(err, CimdError::Fetch(_)));
    }

    #[tokio::test]
    async fn fetch_via_pinned_address_does_not_follow_redirects() {
        let server = MockServer::start().await;
        let addr = *server.address();
        let url = format!("{}/redirecting.json", server.uri());
        Mock::given(method("GET"))
            .and(path("/redirecting.json"))
            .respond_with(
                ResponseTemplate::new(302)
                    .insert_header("Location", "https://attacker.example/elsewhere.json"),
            )
            .mount(&server)
            .await;

        let err = fetch_via_pinned_address(&url, "app.example.com", addr)
            .await
            .unwrap_err();
        assert!(matches!(err, CimdError::Fetch(_)));
    }

    #[tokio::test]
    async fn fetch_via_pinned_address_rejects_oversized_body_without_hanging() {
        let server = MockServer::start().await;
        let addr = *server.address();
        let url = format!("{}/big.json", server.uri());
        let oversized = "x".repeat(MAX_DOCUMENT_BYTES + 1024);
        Mock::given(method("GET"))
            .and(path("/big.json"))
            .respond_with(ResponseTemplate::new(200).set_body_string(oversized))
            .mount(&server)
            .await;

        let err = fetch_via_pinned_address(&url, "app.example.com", addr)
            .await
            .unwrap_err();
        assert!(matches!(err, CimdError::InvalidDocument(_)));
    }

    #[test]
    fn cache_returns_none_when_expired() {
        let cache = DocumentCache::new();
        let doc = ClientMetadataDocument {
            client_id: "https://app.example.com/client.json".to_string(),
            client_name: "Example".to_string(),
            redirect_uris: vec!["http://127.0.0.1:3000/callback".to_string()],
        };
        cache.insert(
            "https://app.example.com/client.json".to_string(),
            &Ok(doc),
            Duration::from_millis(1),
        );
        std::thread::sleep(Duration::from_millis(20));
        assert!(cache
            .get_fresh("https://app.example.com/client.json")
            .is_none());
    }

    #[test]
    fn cache_returns_document_when_fresh() {
        let cache = DocumentCache::new();
        let doc = ClientMetadataDocument {
            client_id: "https://app.example.com/client.json".to_string(),
            client_name: "Example".to_string(),
            redirect_uris: vec!["http://127.0.0.1:3000/callback".to_string()],
        };
        cache.insert(
            "https://app.example.com/client.json".to_string(),
            &Ok(doc),
            CACHE_TTL,
        );
        assert!(cache
            .get_fresh("https://app.example.com/client.json")
            .is_some());
    }

    #[test]
    fn cache_caches_negative_results_too() {
        let cache = DocumentCache::new();
        let err = CimdError::Fetch("simulated failure".to_string());
        cache.insert(
            "https://app.example.com/client.json".to_string(),
            &Err(err),
            NEGATIVE_CACHE_TTL,
        );
        let cached = cache
            .get_fresh("https://app.example.com/client.json")
            .expect("negative result should be cached");
        assert!(cached.is_err());
    }

    #[tokio::test]
    async fn concurrent_fetches_for_the_same_url_single_flight_to_one_actual_request() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let server = MockServer::start().await;
        let addr = *server.address();
        let url = format!("{}/client.json", server.uri());
        let hits = Arc::new(AtomicUsize::new(0));
        Mock::given(method("GET"))
            .and(path("/client.json"))
            .respond_with(move |_: &wiremock::Request| {
                ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "client_id": "",
                    "client_name": "Example",
                    "redirect_uris": ["http://127.0.0.1:3000/callback"],
                }))
            })
            .mount(&server)
            .await;

        let cache = DocumentCache::new();
        // Directly exercise the lock/cache-fresh machinery around a fixed
        // known address (bypassing DNS, same rationale as the other
        // fetch_via_pinned_address tests) by racing two callers through
        // fetch_and_validate_client_metadata's cache path with a
        // pre-seeded successful entry -- proves get_fresh is consulted
        // under the lock on the second racer rather than both racing to
        // fetch. (A full concurrent-miss race against a real un-cached URL
        // would additionally need to go through real DNS for a domain
        // host, which this test avoids by using the cache's public API
        // directly instead of fetch_and_validate_client_metadata's DNS
        // branch.)
        let doc = ClientMetadataDocument {
            client_id: url.clone(),
            client_name: "Example".to_string(),
            redirect_uris: vec!["http://127.0.0.1:3000/callback".to_string()],
        };
        cache.insert(url.clone(), &Ok(doc), CACHE_TTL);
        let (a, b) = tokio::join!(
            fetch_and_validate_client_metadata(&cache, &url),
            fetch_and_validate_client_metadata(&cache, &url),
        );
        assert!(a.is_ok());
        assert!(b.is_ok());
        let _ = (hits, addr); // server/hits unused once served from cache; kept for clarity of intent
    }
}
```

- [ ] **Step 2: Create the mod-root file**

Create `crates/soma-auth/src/cimd.rs`:

```rust
//! OAuth Client ID Metadata Documents (CIMD) support for soma-auth acting
//! as an Authorization Server.
//!
//! Lets an incoming `client_id` be an `https://` URL pointing at a JSON
//! metadata document instead of requiring prior Dynamic Client Registration
//! (RFC 7591). See `document::fetch_and_validate_client_metadata` for the
//! guarded fetch path and `ssrf` for the SSRF preflight guard it composes.
//!
//! A CIMD document's `redirect_uris` are NOT trusted outright — the
//! consumer in `authorize.rs` filters them through the same
//! `is_allowed_redirect_uri` check DCR-registered clients are held to. See
//! that module's `resolve_client_redirect_uris` for why.

pub mod document;
pub mod ssrf;
```

- [ ] **Step 3: Wire into `lib.rs`**

In `crates/soma-auth/src/lib.rs`, add the new module in the same alphabetically-grouped, `http-axum`-gated block as `authorize`/`metadata`:

```rust
pub mod at_rest;
#[cfg(feature = "http-axum")]
pub mod auth_context;
#[cfg(feature = "http-axum")]
pub mod authorize;
#[cfg(feature = "http-axum")]
pub mod cimd;
pub mod config;
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p soma-auth --all-features cimd::`
Expected: PASS — all tests in `cimd::ssrf::tests` and `cimd::document::tests`.

- [ ] **Step 5: Run full crate verification**

```
cargo build -p soma-auth --all-features
cargo test -p soma-auth --all-features
cargo clippy -p soma-auth --all-features --all-targets -- -D warnings
cargo check -p soma-auth
cargo fmt --all -- --check
```

- [ ] **Step 6: Commit**

```bash
git add crates/soma-auth/src/cimd.rs crates/soma-auth/src/cimd/ crates/soma-auth/src/lib.rs docs/references/mcp/client-id-metadata-document.md
git commit -m "feat(soma-auth): add SSRF-guarded, single-flight CIMD document fetch and cache"
```

---

### Task 4: Integrate CIMD into `authorize()`'s client resolution, with the redirect-URI allowlist reapplied

**Files:**
- Modify: `crates/soma-auth/src/authorize.rs` (function `authorize`, near the existing `state.store.find_client(&query.client_id)` call — re-locate this by searching for `find_client` since exact line numbers will have shifted from concurrent unrelated edits already landed on this branch)
- Modify: `crates/soma-auth/src/state.rs` (add a `#[cfg(feature = "http-axum")]`-gated `DocumentCache` field to `AuthState`; check its current struct fields first)
- Test: `crates/soma-auth/src/authorize.rs`, `#[cfg(test)] mod tests`

**Interfaces:**
- Consumes: `cimd::document::{is_cimd_client_id, fetch_and_validate_client_metadata, DocumentCache, CimdError}` (Task 3), `authorize::is_allowed_redirect_uri` (existing)
- Produces: `authorize()` accepts a CIMD `client_id` without a prior `/register` call, with CIMD-sourced `redirect_uris` held to the same trust boundary as DCR-registered ones.

- [ ] **Step 1: Read `crates/soma-auth/src/state.rs` to find `AuthState`'s exact current field list, and confirm `state.rs` is ungated**

Run:
```
grep -n "pub struct AuthState" -A 15 crates/soma-auth/src/state.rs
grep -n "^pub mod state" crates/soma-auth/src/lib.rs
```

Confirm `state.rs` has no `#[cfg(feature = "http-axum")]` above `pub mod state;` in `lib.rs` (verified true as of this plan's review pass via `cargo check -p soma-auth --no-default-features` succeeding) — this means the NEW field this task adds to `AuthState` must be individually gated with `#[cfg(feature = "http-axum")]`, since `cimd::document::DocumentCache` only exists under that feature. Do not skip this — see Step 2.

- [ ] **Step 2: Add a `#[cfg(feature = "http-axum")]`-gated `DocumentCache` to `AuthState`**

Add a new field to the `AuthState` struct:

```rust
    #[cfg(feature = "http-axum")]
    pub(crate) cimd_cache: Arc<crate::cimd::document::DocumentCache>,
```

(Match the existing convention for other shared-cache-like fields — if `AuthState`'s other fields like `store`/`signing_keys`/`google` are `Arc`-wrapped, as the crate's own docs elsewhere describe, follow that; adjust `pub(crate)` vs `pub` to match the visibility of sibling fields.)

Find every `AuthState` constructor (likely `AuthState::new(...)` and `AuthState::for_tests(...)` — locate both via `grep -n "impl AuthState" -A 3 crates/soma-auth/src/state.rs`) and add a matching `#[cfg(feature = "http-axum")]`-gated initializer to each:

```rust
            #[cfg(feature = "http-axum")]
            cimd_cache: Arc::new(crate::cimd::document::DocumentCache::new()),
```

- [ ] **Step 3: Verify the reduced-feature build still compiles — this is the regression this whole step exists to catch**

```
cargo check -p soma-auth
cargo check -p soma-auth --features upstream-oauth-rmcp
cargo check -p soma-auth --all-features
```

All three must succeed. If the first two fail, the `#[cfg(...)]` gating in Step 2 is incomplete — do not proceed until they pass.

- [ ] **Step 4: Add the redirect-URI-allowlist helper and the CIMD-aware client resolver**

Find the current `authorize()` function body (search `pub async fn authorize(` in `crates/soma-auth/src/authorize.rs`). It currently does:

```rust
let client = state
    .store
    .find_client(&query.client_id)
    .await?
    .ok_or_else(|| {
        warn!(
            client_id = %query.client_id,
            client_state_id = %client_state_id,
            "oauth authorize rejected: unknown client_id"
        );
        AuthError::InvalidGrant("unknown client_id".to_string())
    })?;
if !client
    .redirect_uris
    .iter()
    .any(|uri| uri == &query.redirect_uri)
{
    warn!(/* ... */);
    return Err(AuthError::Validation(
        "redirect_uri does not match the registered client".to_string(),
    ));
}
```

Add two private helpers above `authorize()`:

```rust
/// Filter `candidate_redirect_uris` down to those that pass the same
/// loopback/native-app-scheme/operator-allowlist check DCR-registered
/// clients are held to via `is_allowed_redirect_uri`.
///
/// CIMD lets a client skip the DCR round-trip, not the redirect-URI trust
/// boundary. `client_id` is an arbitrary attacker-hosted URL, which means
/// the attacker also controls the JSON body served there — including
/// `redirect_uris`. Trusting a CIMD document's `redirect_uris` outright
/// would let any public HTTPS server declare
/// `redirect_uris: ["https://attacker.evil/steal-code"]` and have it
/// honored, making CIMD strictly weaker than DCR at exactly the point DCR
/// exists to protect. This function is a pure, dependency-free filter so
/// it's testable without any network/fetch involved.
fn allowlist_redirect_uris(candidate_redirect_uris: &[String], allowed_patterns: &[String]) -> Vec<String> {
    candidate_redirect_uris
        .iter()
        .filter(|uri| is_allowed_redirect_uri(uri, allowed_patterns))
        .cloned()
        .collect()
}

/// Resolve the set of trusted `redirect_uris` for `client_id`, either via
/// the DCR-registered-clients table or, for an `https://`-shaped
/// `client_id`, by fetching and validating its CIMD document (see
/// `crate::cimd`) and filtering its declared `redirect_uris` through
/// [`allowlist_redirect_uris`].
async fn resolve_client_redirect_uris(
    state: &AuthState,
    client_id: &str,
) -> Result<Vec<String>, AuthError> {
    if crate::cimd::document::is_cimd_client_id(client_id) {
        let document =
            crate::cimd::document::fetch_and_validate_client_metadata(&state.cimd_cache, client_id)
                .await
                .map_err(|error| {
                    warn!(
                        client_id = %client_id,
                        kind = error.kind(),
                        error = %error,
                        "oauth authorize rejected: CIMD document fetch/validation failed"
                    );
                    // Deliberately generic: the detailed CimdError string
                    // (which can reveal e.g. "resolved only to private
                    // addresses" vs "does not exist") is logged above but
                    // NOT returned to the anonymous /authorize caller, to
                    // avoid an internal-network-topology mapping oracle.
                    AuthError::Validation(
                        "client_id metadata document is invalid or unreachable".to_string(),
                    )
                })?;
        let allowed = allowlist_redirect_uris(
            &document.redirect_uris,
            &state.config.allowed_client_redirect_uris,
        );
        if allowed.is_empty() {
            warn!(
                client_id = %client_id,
                "oauth authorize rejected: CIMD document declares no allowlisted redirect_uris"
            );
            return Err(AuthError::Validation(
                "client_id metadata document declares no allowed redirect_uris".to_string(),
            ));
        }
        return Ok(allowed);
    }

    let client = state.store.find_client(client_id).await?.ok_or_else(|| {
        warn!(
            client_id = %client_id,
            "oauth authorize rejected: unknown client_id"
        );
        AuthError::InvalidGrant("unknown client_id".to_string())
    })?;
    Ok(client.redirect_uris)
}
```

Replace the block quoted above (inside `authorize()`) with:

```rust
let redirect_uris = resolve_client_redirect_uris(&state, &query.client_id).await?;
if !redirect_uris.iter().any(|uri| uri == &query.redirect_uri) {
    warn!(
        client_id = %query.client_id,
        redirect_uri = %query.redirect_uri,
        client_state_id = %client_state_id,
        "oauth authorize rejected: redirect URI does not match the registered/CIMD-allowlisted client"
    );
    return Err(AuthError::Validation(
        "redirect_uri does not match the registered client".to_string(),
    ));
}
```

Verify with `grep -n "client\." crates/soma-auth/src/authorize.rs` (scoped to the `authorize()` function body) that nothing later in the function still reads a `client` binding that no longer exists after this change — if something does (e.g. a later reference to `client.client_id`), adjust it to use `query.client_id` directly, which is already in scope.

- [ ] **Step 5: Write the failing unit tests for the allowlist-filtering logic (no network required)**

In `crates/soma-auth/src/authorize.rs`'s `#[cfg(test)] pub mod tests`, add:

```rust
#[test]
fn allowlist_redirect_uris_keeps_only_patterns_that_pass_is_allowed_redirect_uri() {
    let candidates = vec![
        "http://127.0.0.1:7777/callback".to_string(), // loopback, always allowed
        "https://attacker.evil/steal-code".to_string(), // not in any allowlist pattern
        "https://callback.example.com/callback/node-a".to_string(), // matches pattern below
    ];
    let patterns = vec!["https://callback.example.com/callback/*".to_string()];
    let allowed = allowlist_redirect_uris(&candidates, &patterns);
    assert_eq!(
        allowed,
        vec![
            "http://127.0.0.1:7777/callback".to_string(),
            "https://callback.example.com/callback/node-a".to_string(),
        ]
    );
}

#[test]
fn allowlist_redirect_uris_returns_empty_when_nothing_matches() {
    let candidates = vec!["https://attacker.evil/steal-code".to_string()];
    let allowed = allowlist_redirect_uris(&candidates, &[]);
    assert!(allowed.is_empty());
}
```

- [ ] **Step 6: Run these tests to verify they fail (function doesn't exist yet), then implement, then pass**

Run: `cargo test -p soma-auth --all-features allowlist_redirect_uris`
Expected before Step 4's implementation: compile failure (`allowlist_redirect_uris` undefined). After Step 4: PASS.

- [ ] **Step 7: Write the one true HTTP-level `/authorize` integration test — via the SSRF-rejection path, which needs no network**

In the same test module:

```rust
#[tokio::test]
async fn authorize_rejects_a_cimd_client_id_that_targets_a_private_address() {
    // A `client_id` shaped like a CIMD URL but pointing at a private
    // address is rejected by the SSRF guard before any network I/O
    // happens -- this proves the full wire-up (is_cimd_client_id routing,
    // fetch_and_validate_client_metadata invocation, error mapping) end
    // to end via a real /authorize HTTP request, without needing a
    // reachable public HTTPS target.
    let app = router(test_auth_state().await);
    let response = app
        .oneshot(
            Request::builder()
                .uri("/authorize?response_type=code&client_id=https://127.0.0.1/client.json&redirect_uri=http://127.0.0.1:7777/callback&state=abc&scope=lab&code_challenge=pkce&code_challenge_method=S256")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}
```

**Note on coverage boundaries (do not attempt to "complete" this further without revisiting Global Constraints):** a *successful* CIMD fetch leading to a Google redirect is intentionally NOT tested at the full HTTP-request level in this plan, because doing so would require either a real reachable public HTTPS target (network access in CI — rejected) or building out a second dependency-injection seam through `authorize()` itself beyond what Task 3 already provides at the `cimd::document` layer. Confidence in the successful path comes from the composition of: (a) Task 3's `fetch_via_pinned_address` tests (fetch mechanics work), (b) this task's `allowlist_redirect_uris` tests (the security-critical filtering logic works), and (c) `resolve_client_redirect_uris`'s production body being a straight-line composition of both with no additional branching logic to hide a bug in. This is a deliberate, bounded tradeoff — flag it in code review if a later change to `resolve_client_redirect_uris` adds branching complexity beyond what's covered here, since at that point the tradeoff calculus changes.

- [ ] **Step 8: Run full crate + workspace verification**

```
cargo build --workspace
cargo test -p soma-auth --all-features
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy -p soma-auth --all-features --all-targets -- -D warnings
cargo check -p soma-auth
cargo fmt --all -- --check
```

- [ ] **Step 9: Commit**

```bash
git add crates/soma-auth/src/authorize.rs crates/soma-auth/src/state.rs
git commit -m "feat(soma-auth): accept CIMD client_id at /authorize, holding it to the same redirect_uri allowlist as DCR"
```

---

### Task 5: Advertise `client_id_metadata_document_supported` in AS metadata

**Files:**
- Modify: `crates/soma-auth/src/types.rs` (struct `AuthorizationServerMetadata` — **re-check its current field list first**, since a concurrent fix already added an `authorization_response_iss_parameter_supported: bool` field to this exact struct; add the new field alongside it, don't clobber it)
- Modify: `crates/soma-auth/src/metadata.rs` (function `authorization_server_metadata`)
- Test: `crates/soma-auth/src/metadata.rs`, `#[cfg(test)] mod tests`

**Interfaces:**
- Consumes: nothing new.
- Produces: `/.well-known/oauth-authorization-server` response includes `"client_id_metadata_document_supported": true`.

- [ ] **Step 1: Re-check the current struct**

Run: `grep -n "pub struct AuthorizationServerMetadata" -A 20 crates/soma-auth/src/types.rs`

Confirm the exact current field list (it will already include `authorization_response_iss_parameter_supported: bool` from a prior fix landed before this plan's execution) before editing.

- [ ] **Step 2: Write the failing test**

In `crates/soma-auth/src/metadata.rs`'s `#[cfg(test)] mod tests`, extend or add near the existing `authorization_server_metadata_exposes_lab_endpoints` test:

```rust
#[tokio::test]
async fn authorization_server_metadata_advertises_cimd_support() {
    let app = router(test_auth_state().await);
    let response = app
        .oneshot(
            Request::builder()
                .uri("/.well-known/oauth-authorization-server")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["client_id_metadata_document_supported"], true);
}
```

- [ ] **Step 3: Run the test to verify it fails**

Run: `cargo test -p soma-auth --all-features authorization_server_metadata_advertises_cimd_support`
Expected: FAIL — field doesn't exist, JSON lookup returns `Value::Null`, `Null != true`.

- [ ] **Step 4: Add the field**

In `crates/soma-auth/src/types.rs`, add to `AuthorizationServerMetadata`:

```rust
    pub client_id_metadata_document_supported: bool,
```

(Place it near `authorization_response_iss_parameter_supported` — both are boolean capability-advertisement flags with no `Option`/skip-serializing wrapper.)

In `crates/soma-auth/src/metadata.rs`'s `authorization_server_metadata()` handler, add:

```rust
        client_id_metadata_document_supported: true,
```

to the `AuthorizationServerMetadata { ... }` struct literal (soma-auth supports CIMD unconditionally as of Task 4, so this is a static `true`, matching the pattern used for `authorization_response_iss_parameter_supported`).

- [ ] **Step 5: Run the test to verify it passes**

Run: `cargo test -p soma-auth --all-features authorization_server_metadata_advertises_cimd_support`
Expected: PASS.

- [ ] **Step 6: Run full workspace verification**

```
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy -p soma-auth --all-features --all-targets -- -D warnings
cargo check -p soma-auth
cargo fmt --all -- --check
```

- [ ] **Step 7: Commit**

```bash
git add crates/soma-auth/src/types.rs crates/soma-auth/src/metadata.rs
git commit -m "feat(soma-auth): advertise client_id_metadata_document_supported in AS metadata"
```

---

### Task 6: `CHANGELOG.md` entry

**Files:**
- Modify: `CHANGELOG.md`

**Interfaces:** none (documentation only).

- [ ] **Step 1: Add an `[Unreleased]` entry**

Under the existing `## [Unreleased]` / `### Added` section in `CHANGELOG.md` (check current state first — other changes may have already added entries here; append, don't replace), add:

```markdown
- `soma-auth` now accepts OAuth Client ID Metadata Documents (CIMD) at
  `/authorize` as an alternative to Dynamic Client Registration, per the MCP
  draft authorization spec. An `https://`-shaped `client_id` is fetched
  (SSRF-guarded: static URL/query/fragment validation, DNS resolution
  rejecting the whole result set if any resolved address is private,
  address-pinned no-proxy no-redirect HTTP client, post-connect peer
  re-validation against the pin, a streaming 64 KiB response cap, and
  single-flight-locked positive/negative-result caching) and its
  `redirect_uris` are filtered through the same allowlist DCR-registered
  clients are held to before being trusted — CIMD does not bypass the
  redirect-URI trust boundary DCR enforces. Advertised via
  `client_id_metadata_document_supported: true` in AS metadata. DCR is
  unchanged and remains fully supported.
```

- [ ] **Step 2: Commit**

```bash
git add CHANGELOG.md
git commit -m "docs: changelog entry for CIMD authorization-server support"
```

---

## Self-Review (performed while revising this plan after the 4-agent engineering review)

**Spec coverage:** unchanged from v1 — see the original coverage list; all spec-derived requirements are still met, now with the redirect_uri-allowlist fix ensuring the feature doesn't quietly regress below DCR's security bar while implementing them.

**Review-finding coverage check** (every finding from the architecture/simplicity/security/performance reviews, mapped to where it's addressed):

| Finding | Severity | Addressed in |
|---|---|---|
| Redirect_uri allowlist bypass | Critical (architecture) | Task 4 (`allowlist_redirect_uris`, `resolve_client_redirect_uris`) |
| No `.no_proxy()` | High (security) | Task 3 (`fetch_via_pinned_address`) |
| No post-connect peer re-validation; false parity claim | High (security) | Task 3 (`fetch_document_at`'s `PeerMismatch` check); Revision note corrects the false claim |
| Decorative size cap (buffer-then-check) | High (architecture + security) | Task 3 (`fetch_document_at`'s streaming `.chunk()` loop) |
| Acceptance tests structurally cannot pass | High (simplicity + security) | Task 3/4 (`fetch_via_pinned_address` test seam; Task 4 Step 7's SSRF-rejection-path test) |
| `AuthState` field breaks no-features build | High (architecture) | Task 4 Steps 1–3 (explicit `#[cfg(...)]` gating + reduced-feature verification) |
| Dead `Cache-Control: max-age` parsing | High (simplicity) | Task 3 (removed; fixed-TTL `DocumentCache` instead) |
| No single-flight fetch coordination | Medium (performance + architecture) | Task 3 (`DocumentCache::build_locks`, mirroring `upstream/cache.rs`) |
| Unbounded DNS resolution time | Medium (performance) | Task 3 (`DNS_TIMEOUT` wrapping `tokio::net::lookup_host`) |
| Unbounded cache growth | Medium (security + performance) | Task 3 (`MAX_CACHE_ENTRIES` + sweep-on-insert eviction) |
| SSRF error-message oracle | Medium (security + architecture) | Task 4 (`resolve_client_redirect_uris` returns a generic message; detail stays server-side in `warn!`) |
| SSRF denylist gaps (IPv4-compat IPv6, NAT64/6to4/Teredo, 0.0.0.0/8, multicast) | Medium (security) | Task 2 (`is_ipv6_ipv4_compatible`, `is_ipv6_transition_mechanism`, `is_ipv4_reserved_broadcast_or_multicast`) |
| Missing query/fragment rejection vs reference | Medium (simplicity + security) | Task 2 (`validate_url_shape`) |
| `CimdError::Dns` conflates lookup failure with SSRF block | Medium (architecture) | Task 3 (split into `DnsResolutionFailed` / `DnsBlocked`) |
| No redirect-rejection test | Medium (architecture) | Task 3 (`fetch_via_pinned_address_does_not_follow_redirects`) |
| `CimdError::kind()` built but unused | Low (simplicity) | Task 4 (`warn!(kind = error.kind(), ...)`) |
| Empty `redirect_uris` in CIMD doc not rejected | Low (architecture) | Task 3 (`fetch_document_at`'s empty-`redirect_uris` check) |
| Trailing-dot bypass of private-TLD denylist | Low (security) | Task 2 (`check_host_not_private`'s `strip_suffix('.')`) |
| Reject-if-any-private vs first-passing address selection | Low (security) | Task 3 (`resolve_and_validate_address` now rejects the whole set) |
| No captured CIMD spec reference | Low (architecture) | Task 2 Step 0 (`docs/references/mcp/client-id-metadata-document.md`) |
| Non-root-path constraint not clearly justified | Low (architecture) | Task 2 Step 0 (citation) + `validate_url_shape`'s error message now points at the captured reference |
| Doc-comment/test mismatch (`fetch_document_at` doc claimed a pinned-client test that used a plain client) | Low (architecture) | Task 3 (module doc rewritten to match the actual `fetch_via_pinned_address`/`fetch_document_at` split; tests now genuinely use pinned addresses) |
| `localhost` DNS test contradicts "no real DNS" doc claim | Low (architecture) | Task 3 module doc now explicitly scopes this to "rejection paths only, real OS resolver for `localhost` specifically, no network access needed" rather than overclaiming zero DNS involvement |

**Placeholder scan:** No "TBD"/"handle appropriately"/"similar to Task N" language; every step has complete code.

**Type consistency:** `ClientMetadataDocument`, `CimdError` (now 6 variants), `DocumentCache`, `is_cimd_client_id`, `resolve_and_validate_address`, `fetch_via_pinned_address`, `fetch_document_at`, `fetch_and_validate_client_metadata`, `allowlist_redirect_uris`, `resolve_client_redirect_uris` are named and shaped identically everywhere they're referenced across Tasks 2–5.

**Known remaining tradeoff, stated explicitly rather than left implicit:** Task 4 Step 7 does not test the full HTTP-level *successful* CIMD authorization flow (only the SSRF-rejection path). See that step's note for the reasoning and the bound on when this tradeoff should be revisited.

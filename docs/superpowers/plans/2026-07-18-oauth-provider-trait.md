# OAuth Provider Trait (Authelia + GitHub) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Generalize `crates/shared/auth` (crate `soma-auth`) from a Google-only OAuth/OIDC login flow into a multi-provider system with an `OAuthProvider` trait, add `AutheliaProvider` (OIDC) and `GitHubProvider` (plain OAuth2 + REST userinfo), and let a deployment run several providers simultaneously with a browser login picker.

**Architecture:** Introduce `OAuthProvider` (async trait, object-safe via `async-trait`) with `authorize_url` / `exchange_code` / `refresh` / `provider_id` / `callback_path`. `AuthState.google: Arc<GoogleProvider>` becomes `AuthState.providers: Arc<BTreeMap<String, Arc<dyn OAuthProvider>>>` + `default_provider: String`. Google and Authelia share a new `oidc.rs` JWKS/RS256 ID-token verifier (Authelia is a real OIDC Provider — same authorization-code + PKCE + signed ID-token shape as Google, just with a configurable issuer instead of a hardcoded one). GitHub has no ID token; it fetches `/user` + `/user/emails` after code exchange instead. All three providers share a `provider_http.rs` HTTP/tracing/error-mapping helper (extracted from the current `google.rs`). Multi-provider selection: `/authorize` and `/auth/login` gain an optional `?provider=` query param; when `/auth/login` is hit with no provider and more than one is configured, it renders a plain HTML picker instead of redirecting immediately. Each configured provider mounts its own static callback path (e.g. `/auth/google/callback`, `/auth/authelia/callback`, `/auth/github/callback`) — the callback handler is provider-agnostic; it resolves which provider to call purely from the `provider` column persisted on the row keyed by the OAuth `state` parameter, never from the request path, so path/row confusion isn't a trust boundary. Four SQLite tables (`authorization_requests`, `authorization_codes`, `refresh_tokens`, `browser_login_states`) gain a `provider TEXT NOT NULL DEFAULT 'google'` column so in-flight and long-lived rows remember which upstream IdP they belong to (existing pre-migration rows implicitly become `'google'`, which is correct — that's all that existed before this change). To avoid subject collisions once multiple IdPs share one SQLite DB, non-Google subjects are namespaced as `{provider_id}:{raw_subject}` (e.g. `github:9182310`); Google's subject format is left untouched (bare `sub`) for backward compatibility with already-issued sessions/refresh tokens.

**Why `dyn OAuthProvider` + `async-trait` instead of a closed 3-variant enum:** this crate's own package description is "Vendored OAuth 2.0 + JWT auth crate for soma **and derived servers**", and the root `CLAUDE.md` documents it as "the shared internal JWT/OAuth crate used across Rust MCP servers" (rustifi, cortex, rustarr, synapse2, etc.) — a real, already-stated design goal, not a hypothetical. A closed enum would be simpler and give compile-time exhaustiveness for the 3 known providers, but it could never be extended by a downstream vendored consumer without patching this crate directly. `Arc<dyn OAuthProvider>` in `AuthState.providers` is what makes that extensibility possible; the one Box-per-call allocation `async-trait` costs is irrelevant at OAuth login/refresh cadence (human-interaction-paced, not a request hot path). This is a deliberate trade-off, made explicit here rather than left implicit — engineering review confirmed dynamic dispatch isn't over-engineering given this crate's actual distribution model.

**Tech Stack:** Rust 2024, `async-trait` (new *direct* dependency of `crates/shared/auth` — already used elsewhere in the workspace, see Global Constraints), `axum` 0.8, `reqwest`, `jsonwebtoken`, `rusqlite`, `tokio`. No new *transitive* dependencies beyond what `async-trait` itself pulls in.

## Global Constraints

- `async-trait` version floor: `"0.1"`. Already a direct dependency of 6 other crates in this workspace (`apps/soma`, `crates/soma/test-support`, `crates/soma/mcp`, `crates/soma/application`, `crates/soma/service`, `crates/shared/provider-core`) — none of them via `[workspace.dependencies]` (the workspace root has no such entry for it), so add it directly to `crates/shared/auth/Cargo.toml` `[dependencies]`, matching the existing per-crate convention rather than introducing a new workspace-level dependency.
- Every new/changed public item needs `cargo doc`-quality doc comments only where the WHY is non-obvious (this repo's convention — see existing `google.rs` comments). Do not add narrative comments explaining WHAT the code does.
- `crates/shared/auth/src/tools.rs`-style "thin shim" rule does not apply to this crate (that rule is specific to `crates/soma/mcp/src/tools.rs` and `crates/soma/cli/src/lib.rs`); business logic belongs directly in these provider/state/route modules per this crate's existing layout.
- `mod_module_files = deny` — every new module is a sibling `foo.rs` file directly under `crates/shared/auth/src/`, never `foo/mod.rs`. This crate already uses subdirectories (`cimd/`, `upstream/`) only where a module has genuine internal multi-file structure; the new provider modules do NOT get their own subdirectory — they stay flat, matching `google.rs`'s existing placement.
- Every SQL schema change follows the existing `add_column_if_missing` idempotent pattern used for the `resource` column (unconditional, no `PRAGMA user_version` bump needed) — see `sqlite.rs:1256-1273`.
- `cargo test -p soma-auth` and `cargo clippy -p soma-auth -- -D warnings` must pass at the end of every task.
- Never construct HTML by string-concatenating unescaped user input. The only new user-influenced HTML in this plan is the login picker's `return_to` value — it MUST be percent-encoded via `url::form_urlencoded::byte_serialize` before interpolation. Its real output charset is `[A-Za-z0-9*\-._+%]` (WHATWG `application/x-www-form-urlencoded`: unreserved bytes are alphanumeric plus `*-._`, space becomes `+`, everything else — including `~` — is percent-encoded), which cannot break out of an HTML attribute. Provider id/label strings are additionally passed through a small `html_escape` helper as defense-in-depth even though they're always compile-time literals today (Task 11).
- This plan touches ONLY `crates/shared/auth/**`. Wiring Authelia/GitHub into the `soma` binary's own CLI/config/setup-wizard/doctor surface (`crates/soma/contracts`, `crates/soma/cli`, `apps/soma`) is a **separate, dependent plan**: `docs/superpowers/plans/2026-07-18-soma-oauth-provider-config.md`. Do not touch those directories from this plan.
- **Security posture, decided during engineering review — do not re-litigate mid-implementation:** the email allowlist (`admin_email` + `allowed_users` table) is a single flat list shared across every configured provider; being on it grants `<default_scope>:admin` regardless of which provider authenticated the user (pre-existing behavior, unchanged by this plan). Enabling 2+ providers simultaneously — this plan's headline feature — means the deployment's effective admin-gate strength becomes that of its *weakest* configured provider's identity-verification signal (GitHub's `primary && verified` email flag is a point-in-time, non-re-verified claim, weaker than Google/Authelia's live per-login ID-token `email_verified` claim). Full per-provider allowlist scoping (a schema change to `allowed_users`) was considered and rejected as disproportionate for this crate's actual deployment shape (single-operator homelab/small-fleet, not multi-tenant SaaS) — instead, `AuthState::new` logs a `tracing::warn!` at startup whenever 2+ providers are configured with a non-empty allowlist (Task 9 Step 2), so the risk is always visible in server logs, never silent. Document this trade-off in `docs/AUTH.md` (Task 13 Step 5A) — do not silently drop it if you disagree with the call; raise it, don't unilaterally implement the schema-scoped alternative instead.

---

## File Structure

| File | Status | Responsibility |
|---|---|---|
| `crates/shared/auth/Cargo.toml` | Modify | Add `async-trait` dependency |
| `crates/shared/auth/src/provider_http.rs` | **New** | Shared HTTP call + tracing + error-mapping helper (`RequestTrace`, `RequestErrors`, `read_json_response`), extracted from `google.rs` |
| `crates/shared/auth/src/oauth_provider.rs` | **New** | `OAuthProvider` trait, `ProviderExchange`, `AuthorizeUrlRequest` (moved from `google.rs`), `namespaced_subject` |
| `crates/shared/auth/src/oidc.rs` | **New** | Shared JWKS caching + RS256 ID-token verification (`OidcVerifier`), extracted from `google.rs` |
| `crates/shared/auth/src/google.rs` | Modify | `GoogleProvider` now built on `oidc.rs` + `provider_http.rs`; implements `OAuthProvider` |
| `crates/shared/auth/src/authelia.rs` | **New** | `AutheliaProvider` — OIDC-shaped, configurable issuer, fixed Authelia endpoint paths |
| `crates/shared/auth/src/github.rs` | **New** | `GitHubProvider` — plain OAuth2, REST `/user` + `/user/emails`, no ID token, no refresh |
| `crates/shared/auth/src/config.rs` | Modify | `AutheliaConfig`, `GitHubConfig`, `default_provider`, multi-provider validation |
| `crates/shared/auth/src/types.rs` | Modify | Add `provider: String` to `AuthorizationRequestRow`, `AuthorizationCodeRow`, `RefreshTokenRow`, `BrowserLoginStateRow` |
| `crates/shared/auth/src/sqlite.rs` | Modify | `provider` column migration (4 tables) + SQL/row-mapping updates |
| `crates/shared/auth/src/state.rs` | Modify | `AuthState.providers` map + `default_provider`, `provider()`/`provider_or_default()` helpers |
| `crates/shared/auth/src/routes.rs` | Modify | Mount one callback route per configured provider instead of the hardcoded `/auth/google/callback` |
| `crates/shared/auth/src/authorize.rs` | Modify | Provider selection in `browser_login`/`authorize`, HTML picker, provider-agnostic `callback` |
| `crates/shared/auth/src/token.rs` | Modify | Propagate `provider` through auth-code → refresh-token issuance and refresh-grant redemption |
| `crates/shared/auth/src/lib.rs` | Modify | Register new modules |

---

### Task 1: `provider_http.rs` — shared HTTP/tracing/error helper

**Files:**
- Create: `crates/shared/auth/src/provider_http.rs`
- Modify: `crates/shared/auth/src/lib.rs:1-15` (add `pub(crate) mod provider_http;`)
- Modify: `crates/shared/auth/Cargo.toml` (no change needed here — this task uses only existing deps: `reqwest`, `tracing`, `serde`)

**Interfaces:**
- Produces: `pub(crate) struct RequestTrace<'a>`, `pub(crate) struct RequestErrors`, `pub(crate) async fn read_json_response<T: DeserializeOwned>(trace: RequestTrace<'_>, request: reqwest::RequestBuilder, errors: RequestErrors) -> Result<T, AuthError>` — used by Task 3 (google.rs refactor), Task 5 (authelia.rs), Task 6 (github.rs).

This is a straight generalization of `google.rs`'s existing private `GoogleRequestTrace` / `GoogleRequestErrors` / `read_json_response` (see `google.rs:115-247` in the current file), parameterized by a `provider_id: &'static str` instead of the hardcoded literal `"google"` in every tracing field.

- [ ] **Step 1: Create `provider_http.rs` with the generalized helper**

```rust
use std::time::Instant;

use reqwest::Url;
use reqwest::header;
use serde::de::DeserializeOwned;
use tracing::{info, warn};

use crate::error::AuthError;

pub(crate) struct RequestTrace<'a> {
    provider_id: &'static str,
    operation: &'static str,
    method: &'static str,
    endpoint: &'a Url,
    started: Instant,
}

impl<'a> RequestTrace<'a> {
    pub(crate) fn start(
        provider_id: &'static str,
        operation: &'static str,
        method: &'static str,
        endpoint: &'a Url,
    ) -> Self {
        info!(
            provider = provider_id,
            operation,
            method,
            host = endpoint.host_str().unwrap_or_default(),
            path = endpoint.path(),
            "request.start"
        );
        Self {
            provider_id,
            operation,
            method,
            endpoint,
            started: Instant::now(),
        }
    }

    fn finish(&self, status: reqwest::StatusCode) {
        info!(
            provider = self.provider_id,
            operation = self.operation,
            method = self.method,
            host = self.endpoint.host_str().unwrap_or_default(),
            path = self.endpoint.path(),
            status = status.as_u16(),
            elapsed_ms = self.started.elapsed().as_millis(),
            "request.finish"
        );
    }

    fn error(&self, status: Option<reqwest::StatusCode>, error: &reqwest::Error) {
        if let Some(status) = status {
            warn!(
                provider = self.provider_id,
                operation = self.operation,
                method = self.method,
                host = self.endpoint.host_str().unwrap_or_default(),
                path = self.endpoint.path(),
                status = status.as_u16(),
                elapsed_ms = self.started.elapsed().as_millis(),
                error = %error,
                "request.error"
            );
        } else {
            warn!(
                provider = self.provider_id,
                operation = self.operation,
                method = self.method,
                host = self.endpoint.host_str().unwrap_or_default(),
                path = self.endpoint.path(),
                elapsed_ms = self.started.elapsed().as_millis(),
                error = %error,
                "request.error"
            );
        }
    }
}

pub(crate) struct RequestErrors {
    provider_id: &'static str,
    transport_context: &'static str,
    status_context: &'static str,
    decode_context: &'static str,
}

impl RequestErrors {
    pub(crate) fn new(
        provider_id: &'static str,
        transport_context: &'static str,
        status_context: &'static str,
        decode_context: &'static str,
    ) -> Self {
        Self {
            provider_id,
            transport_context,
            status_context,
            decode_context,
        }
    }
}

pub(crate) async fn read_json_response<T: DeserializeOwned>(
    trace: RequestTrace<'_>,
    request: reqwest::RequestBuilder,
    errors: RequestErrors,
) -> Result<T, AuthError> {
    let response = request.send().await.map_err(|error| {
        let auth_error = AuthError::Network(format!("{}: {error}", errors.transport_context));
        trace.error(None, &error);
        warn!(
            provider = errors.provider_id,
            error = %error,
            kind = auth_error.kind(),
            "{}",
            errors.transport_context
        );
        auth_error
    })?;
    let status = response.status();
    let retry_after_ms = response
        .headers()
        .get(header::RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .map(|seconds| seconds.saturating_mul(1_000));
    let response = response.error_for_status().map_err(|error| {
        let auth_error = if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            AuthError::RateLimited {
                message: format!("{}: {}", errors.status_context, status),
                retry_after_ms: retry_after_ms.unwrap_or(1_000),
            }
        } else if status.is_server_error() {
            AuthError::Server(format!("{}: {error}", errors.status_context))
        } else {
            AuthError::AuthFailed(format!("{}: {error}", errors.status_context))
        };
        trace.error(Some(status), &error);
        warn!(
            provider = errors.provider_id,
            error = %error,
            kind = auth_error.kind(),
            "{}",
            errors.status_context
        );
        auth_error
    })?;
    trace.finish(status);
    response.json::<T>().await.map_err(|error| {
        let auth_error = AuthError::Decode(format!("{}: {error}", errors.decode_context));
        warn!(
            provider = errors.provider_id,
            error = %error,
            kind = auth_error.kind(),
            "{}",
            errors.decode_context
        );
        auth_error
    })
}
```

- [ ] **Step 2: Register the module in `lib.rs`**

In `crates/shared/auth/src/lib.rs`, add this line among the other unconditional `pub mod` / `pub(crate) mod` declarations (near `pub mod google;`):

```rust
pub(crate) mod provider_http;
```

- [ ] **Step 3: Build check**

Run: `cargo check -p soma-auth`
Expected: compiles cleanly (nothing calls `provider_http` yet — dead-code warnings are expected and fine at this stage since every item is used starting in Task 3).

- [ ] **Step 4: Commit**

```bash
git add crates/shared/auth/src/provider_http.rs crates/shared/auth/src/lib.rs
git commit -m "feat(soma-auth): extract shared HTTP/tracing helper for OAuth providers"
```

---

### Task 2: `oauth_provider.rs` — the trait itself

**Files:**
- Create: `crates/shared/auth/src/oauth_provider.rs`
- Modify: `crates/shared/auth/src/lib.rs` (add `pub mod oauth_provider;`)
- Modify: `crates/shared/auth/Cargo.toml` — add `async-trait = "0.1"` to `[dependencies]`

**Interfaces:**
- Produces: `pub struct AuthorizeUrlRequest { state, scope, code_challenge, code_challenge_method, force_consent }`, `pub struct ProviderExchange { subject, email, email_verified, access_token, refresh_token, expires_in, id_token: Option<String> }`, `pub trait OAuthProvider: Send + Sync + std::fmt::Debug { fn provider_id(&self) -> &'static str; fn callback_path(&self) -> &str; fn authorize_url(&self, request: &AuthorizeUrlRequest) -> Result<Url, AuthError>; async fn exchange_code(&self, code: &str, code_verifier: &str) -> Result<ProviderExchange, AuthError>; async fn refresh(&self, refresh_token: &str) -> Result<ProviderExchange, AuthError>; }`, `pub(crate) fn namespaced_subject(provider_id: &str, raw_subject: &str) -> String`.
- Consumes: `crate::error::AuthError` (existing).

- [ ] **Step 1: Add the `async-trait` dependency**

In `crates/shared/auth/Cargo.toml`, add to `[dependencies]` (alphabetical position, right after `anyhow`):

```toml
async-trait = "0.1"
```

- [ ] **Step 2: Create `oauth_provider.rs`**

```rust
use async_trait::async_trait;
use reqwest::Url;
use serde::{Deserialize, Serialize};

use crate::error::AuthError;

/// Parameters for building an upstream provider's `/authorize`-equivalent
/// redirect URL. `AuthorizeUrlRequest` was originally Google-specific
/// (`google::AuthorizeUrlRequest`); it moved here unchanged when the
/// `OAuthProvider` trait was introduced, since every provider needs the same
/// shape.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthorizeUrlRequest {
    pub state: String,
    pub scope: String,
    pub code_challenge: String,
    pub code_challenge_method: String,
    /// Force the upstream's full consent screen even if the user already
    /// granted these scopes. Needed the first time (to guarantee a refresh
    /// token comes back for providers that support one), but forcing it on
    /// every retry adds a slow, interactive round trip that impatient MCP
    /// clients can time out on before the human finishes clicking through it.
    ///
    /// This field is honestly OIDC/Google-shaped, not provider-neutral, and
    /// that's surfaced here rather than hidden: Google's `prompt=consent` is
    /// documented, verified behavior for guaranteeing a refresh token on
    /// re-authorization. Authelia's need for the same treatment is plausible
    /// (same `prompt` parameter, same OIDC family) but unverified against a
    /// real Authelia instance by this plan — treat it as inherited-but-not-
    /// proven. GitHub has no documented `prompt` parameter and no consent-
    /// gated refresh-token semantics at all (OAuth Apps never issue refresh
    /// tokens, full stop); `GitHubProvider::authorize_url` still appends
    /// `prompt=consent` when this is `true` purely because GitHub silently
    /// ignores unrecognized query params — it's dead weight, not a bug, but
    /// don't read "GitHub honors force_consent" into that.
    pub force_consent: bool,
}

/// Normalized result of a successful upstream code exchange or refresh,
/// common to every [`OAuthProvider`] implementation.
///
/// `id_token` is `Some` for OIDC-shaped providers (Google, Authelia) and
/// `None` for plain-OAuth2 providers with no ID token (GitHub).
///
/// `access_token`/`refresh_token`/`id_token` are `#[serde(skip_serializing)]`
/// as defense-in-depth: nothing in this plan serializes a whole
/// `ProviderExchange` to a client response or log line today (every call
/// site destructures individual non-secret fields), but nothing about the
/// type's shape should make that mistake easy for a future edit to make
/// silently — nothing in this crate's existing secret-handling discipline
/// (`fingerprint()`-before-log everywhere else) relies on "don't accidentally
/// serialize the whole struct" being enforced only by convention.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderExchange {
    pub subject: String,
    pub email: Option<String>,
    pub email_verified: Option<bool>,
    #[serde(skip_serializing)]
    pub access_token: String,
    #[serde(skip_serializing)]
    pub refresh_token: Option<String>,
    pub expires_in: Option<u64>,
    #[serde(skip_serializing)]
    pub id_token: Option<String>,
}

/// An upstream identity provider soma-auth can redirect a user to for login.
///
/// Implementations: [`crate::google::GoogleProvider`],
/// [`crate::authelia::AutheliaProvider`], [`crate::github::GitHubProvider`].
/// `AuthState.providers` holds a `provider_id() -> Arc<dyn OAuthProvider>`
/// map so a deployment can enable more than one simultaneously.
#[async_trait]
pub trait OAuthProvider: Send + Sync + std::fmt::Debug {
    /// Stable identifier used as the `providers` map key, the `provider`
    /// column value persisted in SQLite, and the subject-namespace prefix.
    /// One of `"google"`, `"authelia"`, `"github"`.
    fn provider_id(&self) -> &'static str;

    /// The absolute path (no scheme/host) this provider's registered
    /// `redirect_uri` resolves to, e.g. `/auth/google/callback`. Used by
    /// `routes::router` to mount one callback route per configured provider.
    fn callback_path(&self) -> &str;

    fn authorize_url(&self, request: &AuthorizeUrlRequest) -> Result<Url, AuthError>;

    async fn exchange_code(
        &self,
        code: &str,
        code_verifier: &str,
    ) -> Result<ProviderExchange, AuthError>;

    async fn refresh(&self, refresh_token: &str) -> Result<ProviderExchange, AuthError>;
}

/// Namespace a raw upstream subject by provider so two different IdPs
/// sharing one SQLite DB cannot collide on the same `subject` value.
///
/// Google is deliberately exempted (returns `raw_subject` unchanged): its
/// subject format predates multi-provider support, and already-issued
/// sessions/refresh tokens in production DBs have the bare, unprefixed
/// format. Changing it would silently invalidate every existing Google
/// session on upgrade. Authelia and GitHub are new — there is no existing
/// data to break, so they get the safer namespaced form from day one.
pub(crate) fn namespaced_subject(provider_id: &str, raw_subject: &str) -> String {
    if provider_id == "google" {
        raw_subject.to_string()
    } else {
        format!("{provider_id}:{raw_subject}")
    }
}

#[cfg(test)]
mod tests {
    use super::namespaced_subject;

    #[test]
    fn google_subject_is_not_namespaced() {
        assert_eq!(namespaced_subject("google", "108123456"), "108123456");
    }

    #[test]
    fn non_google_subjects_are_namespaced() {
        assert_eq!(namespaced_subject("github", "9182310"), "github:9182310");
        assert_eq!(
            namespaced_subject("authelia", "alice"),
            "authelia:alice"
        );
    }
}
```

- [ ] **Step 3: Register the module in `lib.rs`**

Add near `pub mod google;`:

```rust
pub mod oauth_provider;
```

- [ ] **Step 4: Run the new unit tests**

Run: `cargo test -p soma-auth oauth_provider::tests -- --nocapture`
Expected: `google_subject_is_not_namespaced` and `non_google_subjects_are_namespaced` PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/shared/auth/Cargo.toml crates/shared/auth/Cargo.lock crates/shared/auth/src/oauth_provider.rs crates/shared/auth/src/lib.rs
git commit -m "feat(soma-auth): add OAuthProvider trait and ProviderExchange"
```

---

### Task 3: `oidc.rs` — shared JWKS / RS256 ID-token verifier

**Files:**
- Create: `crates/shared/auth/src/oidc.rs`
- Modify: `crates/shared/auth/src/lib.rs` (add `pub(crate) mod oidc;`)

**Interfaces:**
- Consumes: `crate::error::AuthError` (existing).
- Produces: `pub(crate) struct OidcVerifier` with `pub(crate) fn new(provider_id: &'static str, issuer: String, jwks_endpoint: Url, http: reqwest::Client) -> Self`, `pub(crate) async fn verify(&self, id_token: &str, audience: &str) -> Result<IdTokenClaims, AuthError>`, and (test-only) `pub(crate) fn with_jwks_endpoint(self, jwks_endpoint: Url) -> Self`. `pub(crate) struct IdTokenClaims { pub iss: String, pub sub: String, pub email: Option<String>, pub email_verified: Option<bool> }`. Used by Task 4 (google.rs) and Task 5 (authelia.rs).

This is a direct generalization of `google.rs`'s current `GoogleJwks` / `GoogleJwk` / `CachedGoogleJwks` / `GoogleIdTokenClaims` / `verify_id_token` / `find_jwk_for_kid` / `fetch_jwks` / `refresh_jwks` / `refresh_jwks_locked` / `cached_jwks` / `google_jwks_ttl` / `parse_max_age` / `validate_id_token_header` (see `google.rs:85-591` in the current file) — same logic, parameterized by `provider_id` (for log fields and error messages) and `issuer` (previously the hardcoded `GOOGLE_ISSUER` constant).

- [ ] **Step 1: Create `oidc.rs`**

```rust
use std::sync::Arc;
use std::time::{Duration, Instant};

use jsonwebtoken::{Algorithm, DecodingKey, Header, Validation, decode, decode_header};
use reqwest::Url;
use reqwest::header;
use serde::Deserialize;
use tokio::sync::RwLock;
use tracing::{debug, warn};

use crate::error::AuthError;

const DEFAULT_JWKS_TTL: Duration = Duration::from_secs(60 * 60);
/// Per-request timeout on the JWKS GET. Bound aggressively (5s) so a slow
/// upstream JWKS endpoint cannot starve a tokio worker holding the JWKS
/// write lock. Token exchange / refresh keep the provider's own looser
/// timeout because those can legitimately take longer.
const JWKS_FETCH_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Deserialize)]
pub(crate) struct IdTokenClaims {
    pub iss: String,
    pub sub: String,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub email_verified: Option<bool>,
}

#[derive(Clone, Debug, Deserialize)]
struct Jwks {
    keys: Vec<Jwk>,
}

#[derive(Clone, Debug, Deserialize)]
struct Jwk {
    kid: String,
    #[serde(default)]
    alg: Option<String>,
    n: String,
    e: String,
}

#[derive(Clone, Debug)]
struct CachedJwks {
    jwks: Jwks,
    expires_at: Instant,
}

/// Shared RS256 ID-token verifier for OIDC-shaped upstream providers
/// (Google, Authelia). Caches the provider's JWKS document and validates
/// signature, expiry, audience, and issuer on every [`Self::verify`] call.
pub(crate) struct OidcVerifier {
    provider_id: &'static str,
    issuer: String,
    jwks_endpoint: Url,
    http: reqwest::Client,
    jwks_cache: Arc<RwLock<Option<CachedJwks>>>,
}

impl OidcVerifier {
    pub(crate) fn new(
        provider_id: &'static str,
        issuer: String,
        jwks_endpoint: Url,
        http: reqwest::Client,
    ) -> Self {
        Self {
            provider_id,
            issuer,
            jwks_endpoint,
            http,
            jwks_cache: Arc::new(RwLock::new(None)),
        }
    }

    #[cfg(test)]
    #[must_use]
    pub(crate) fn with_jwks_endpoint(mut self, jwks_endpoint: Url) -> Self {
        self.jwks_endpoint = jwks_endpoint;
        self
    }

    pub(crate) async fn verify(
        &self,
        id_token: &str,
        audience: &str,
    ) -> Result<IdTokenClaims, AuthError> {
        let header = decode_header(id_token).map_err(|error| {
            AuthError::Storage(format!("verify {} id_token: {error}", self.provider_id))
        })?;
        validate_header_alg(self.provider_id, &header)?;
        let kid = header.kid.ok_or_else(|| {
            AuthError::Storage(format!(
                "{} id_token is missing a key id",
                self.provider_id
            ))
        })?;
        let key = self.find_jwk_for_kid(&kid).await?;
        if let Some(alg) = key.alg.as_deref()
            && alg != "RS256"
        {
            return Err(AuthError::Storage(format!(
                "{} JWKS key `{}` uses unsupported algorithm `{alg}`",
                self.provider_id, key.kid
            )));
        }

        let decoding_key = DecodingKey::from_rsa_components(&key.n, &key.e).map_err(|error| {
            AuthError::Storage(format!(
                "build {} id_token decoding key: {error}",
                self.provider_id
            ))
        })?;
        let mut validation = Validation::new(Algorithm::RS256);
        validation.validate_exp = true;
        validation.leeway = 0;
        validation.set_audience(&[audience]);

        let claims = decode::<IdTokenClaims>(id_token, &decoding_key, &validation)
            .map(|data| data.claims)
            .map_err(|error| {
                AuthError::Storage(format!("invalid {} id_token: {error}", self.provider_id))
            })?;

        if claims.iss != self.issuer {
            return Err(AuthError::Storage(format!(
                "invalid {} id_token issuer `{}`",
                self.provider_id, claims.iss
            )));
        }

        Ok(claims)
    }

    async fn find_jwk_for_kid(&self, kid: &str) -> Result<Jwk, AuthError> {
        let jwks = self.fetch_jwks().await?;
        if let Some(key) = jwks.keys.into_iter().find(|key| key.kid == kid) {
            return Ok(key);
        }

        debug!(
            provider = self.provider_id,
            kid, "jwks cache miss for token key id; refreshing"
        );
        self.refresh_jwks()
            .await?
            .keys
            .into_iter()
            .find(|key| key.kid == kid)
            .ok_or_else(|| {
                AuthError::Storage(format!(
                    "{} id_token key id was not found in JWKS",
                    self.provider_id
                ))
            })
    }

    async fn fetch_jwks(&self) -> Result<Jwks, AuthError> {
        if let Some(jwks) = self.cached_jwks().await {
            debug!(provider = self.provider_id, "jwks cache hit");
            return Ok(jwks);
        }

        let jwks = {
            let mut cache = self.jwks_cache.write().await;
            if let Some(cached) = cache
                .as_ref()
                .filter(|cached| cached.expires_at > Instant::now())
            {
                debug!(provider = self.provider_id, "jwks cache hit after refresh lock");
                cached.jwks.clone()
            } else {
                self.refresh_jwks_locked(&mut cache).await?
            }
        };
        Ok(jwks)
    }

    async fn refresh_jwks(&self) -> Result<Jwks, AuthError> {
        let mut cache = self.jwks_cache.write().await;
        self.refresh_jwks_locked(&mut cache).await
    }

    async fn refresh_jwks_locked(
        &self,
        cache: &mut Option<CachedJwks>,
    ) -> Result<Jwks, AuthError> {
        let response = self
            .http
            .get(self.jwks_endpoint.clone())
            .timeout(JWKS_FETCH_TIMEOUT)
            .send()
            .await
            .map_err(|error| {
                warn!(provider = self.provider_id, error = %error, "jwks request failed");
                AuthError::Storage(format!("fetch {} jwks: {error}", self.provider_id))
            })?;
        let status = response.status();
        let ttl = jwks_ttl(response.headers());
        let response = response.error_for_status().map_err(|error| {
            warn!(provider = self.provider_id, error = %error, "jwks request returned error status");
            AuthError::Storage(format!("{} jwks endpoint error: {error}", self.provider_id))
        })?;
        let _ = status;
        let jwks = response.json::<Jwks>().await.map_err(|error| {
            warn!(provider = self.provider_id, error = %error, "jwks payload unreadable");
            AuthError::Storage(format!("decode {} jwks response: {error}", self.provider_id))
        })?;

        *cache = Some(CachedJwks {
            jwks: jwks.clone(),
            expires_at: Instant::now() + ttl,
        });

        Ok(jwks)
    }

    async fn cached_jwks(&self) -> Option<Jwks> {
        let cache = self.jwks_cache.read().await;
        cache
            .as_ref()
            .filter(|cached| cached.expires_at > Instant::now())
            .map(|cached| cached.jwks.clone())
    }
}

fn jwks_ttl(headers: &header::HeaderMap) -> Duration {
    headers
        .get(header::CACHE_CONTROL)
        .and_then(|value| value.to_str().ok())
        .and_then(parse_max_age)
        .map_or(DEFAULT_JWKS_TTL, Duration::from_secs)
}

fn parse_max_age(cache_control: &str) -> Option<u64> {
    cache_control.split(',').find_map(|directive| {
        let directive = directive.trim();
        let value = directive.strip_prefix("max-age=")?;
        value.parse::<u64>().ok()
    })
}

fn validate_header_alg(provider_id: &str, header: &Header) -> Result<(), AuthError> {
    if header.alg != Algorithm::RS256 {
        return Err(AuthError::Storage(format!(
            "verify {provider_id} id_token: unsupported algorithm `{:?}`",
            header.alg
        )));
    }
    Ok(())
}
```

- [ ] **Step 2: Register the module in `lib.rs`**

Add near `pub mod google;`:

```rust
pub(crate) mod oidc;
```

- [ ] **Step 3: Build check**

Run: `cargo check -p soma-auth`
Expected: compiles (unused-item warnings expected until Task 4 wires `GoogleProvider` to use it).

- [ ] **Step 4: Commit**

```bash
git add crates/shared/auth/src/oidc.rs crates/shared/auth/src/lib.rs
git commit -m "feat(soma-auth): extract shared OIDC JWKS/ID-token verifier"
```

---

### Task 4: Refactor `google.rs` onto `oidc.rs` + `provider_http.rs`; implement `OAuthProvider`

**Files:**
- Modify: `crates/shared/auth/src/google.rs` (entire file — see exact replacement below)

**Interfaces:**
- Consumes: `crate::oauth_provider::{AuthorizeUrlRequest, OAuthProvider, ProviderExchange}`, `crate::oidc::OidcVerifier`, `crate::provider_http::{RequestTrace, RequestErrors, read_json_response}`.
- Produces: `GoogleProvider` unchanged in its **public field/method surface** (`client_id`, `client_secret`, `redirect_uri`, `scopes`, `http`, `new()`, `authorize_url()`, `exchange_code()`, `refresh()` inherent methods all keep their existing signatures — only the return type of `exchange_code`/`refresh` changes from the removed `GoogleExchange` to `ProviderExchange`), plus a new `impl OAuthProvider for GoogleProvider` and a new `callback_path()`. This keeps every existing test call site in `authorize.rs`/`token.rs` that calls `GoogleProvider::new(...).exchange_code(...)` etc. as an inherent method call compiling unchanged.

`GoogleExchange` is deleted (confirmed unused outside `google.rs` — grep `GoogleExchange` before starting to re-verify no drift since planning).

- [ ] **Step 1: Confirm `GoogleExchange` has no external users**

Run: `grep -rn "GoogleExchange" crates/shared/auth/src | grep -v "src/google.rs"`
Expected: no output. If output appears, STOP and read those call sites before proceeding — the plan assumes there are none.

- [ ] **Step 2: Replace `google.rs`'s header, struct, and constructor**

Replace lines 1–332 of `crates/shared/auth/src/google.rs` (from the top of the file through the end of `authorize_url`) with:

```rust
use std::time::Duration;

use async_trait::async_trait;
use reqwest::Url;
use serde::{Deserialize, Serialize};
use tracing::{debug, info};

use crate::error::AuthError;
use crate::oauth_provider::{AuthorizeUrlRequest, OAuthProvider, ProviderExchange};
use crate::oidc::OidcVerifier;
use crate::provider_http::{RequestErrors, RequestTrace, read_json_response};
use crate::util::fingerprint;

const GOOGLE_AUTHORIZE_ENDPOINT: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const GOOGLE_TOKEN_ENDPOINT: &str = "https://oauth2.googleapis.com/token";
const GOOGLE_JWKS_ENDPOINT: &str = "https://www.googleapis.com/oauth2/v3/certs";
const GOOGLE_ISSUER: &str = "https://accounts.google.com";
const GOOGLE_HTTP_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Clone)]
pub struct GoogleProvider {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: Url,
    pub scopes: Vec<String>,
    pub http: reqwest::Client,
    authorize_endpoint: Url,
    token_endpoint: Url,
    verifier: OidcVerifier,
}

impl std::fmt::Debug for GoogleProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GoogleProvider")
            .field("client_id", &self.client_id)
            .field("redirect_uri", &self.redirect_uri)
            .field("scopes", &self.scopes)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Deserialize)]
struct GoogleTokenResponse {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    expires_in: Option<u64>,
    id_token: String,
}

impl GoogleProvider {
    pub fn new(
        client_id: String,
        client_secret: String,
        redirect_uri: Url,
    ) -> Result<Self, AuthError> {
        // rmcp's HTTP transport (and, transitively, reqwest) requires a rustls
        // crypto provider to be installed before the first TLS-capable client
        // is built. The real binary installs one at startup; test binaries
        // never go through that path, so this call is also needed here.
        // Idempotent — an `Err` just means a provider is already installed,
        // safe to ignore.
        drop(rustls::crypto::ring::default_provider().install_default());
        let http = reqwest::Client::builder()
            .timeout(GOOGLE_HTTP_TIMEOUT)
            .build()
            .map_err(|error| {
                AuthError::Storage(format!("build google oauth http client: {error}"))
            })?;
        let authorize_endpoint = Url::parse(GOOGLE_AUTHORIZE_ENDPOINT).map_err(|error| {
            AuthError::Config(format!("parse google authorize endpoint: {error}"))
        })?;
        let token_endpoint = Url::parse(GOOGLE_TOKEN_ENDPOINT)
            .map_err(|error| AuthError::Config(format!("parse google token endpoint: {error}")))?;
        let jwks_endpoint = Url::parse(GOOGLE_JWKS_ENDPOINT)
            .map_err(|error| AuthError::Config(format!("parse google jwks endpoint: {error}")))?;
        let verifier = OidcVerifier::new(
            "google",
            GOOGLE_ISSUER.to_string(),
            jwks_endpoint,
            http.clone(),
        );

        Ok(Self {
            client_id,
            client_secret,
            redirect_uri,
            scopes: vec![
                "openid".to_string(),
                "email".to_string(),
                "profile".to_string(),
            ],
            http,
            authorize_endpoint,
            token_endpoint,
            verifier,
        })
    }

    #[cfg(test)]
    #[must_use]
    pub fn with_endpoints(mut self, authorize_endpoint: Url, token_endpoint: Url) -> Self {
        self.authorize_endpoint = authorize_endpoint;
        self.token_endpoint = token_endpoint;
        self
    }

    #[cfg(test)]
    #[must_use]
    pub fn with_jwks_endpoint(mut self, jwks_endpoint: Url) -> Self {
        self.verifier = self.verifier.with_jwks_endpoint(jwks_endpoint);
        self
    }

    pub fn authorize_url(&self, request: &AuthorizeUrlRequest) -> Result<Url, AuthError> {
        let mut url = self.authorize_endpoint.clone();
        let scope = self.scopes.join(" ");
        url.query_pairs_mut()
            .append_pair("client_id", &self.client_id)
            .append_pair("redirect_uri", self.redirect_uri.as_str())
            .append_pair("response_type", "code")
            .append_pair("scope", &scope)
            .append_pair("access_type", "offline")
            .append_pair("include_granted_scopes", "true")
            .append_pair("state", &request.state)
            .append_pair("code_challenge", &request.code_challenge)
            .append_pair("code_challenge_method", &request.code_challenge_method);
        if request.force_consent {
            url.query_pairs_mut().append_pair("prompt", "consent");
        }
        debug!(
            provider = "google",
            oauth_state_id = %fingerprint(&request.state),
            scope = %scope,
            redirect_uri = %self.redirect_uri,
            "oauth upstream authorize URL constructed"
        );
        Ok(url)
    }
```

- [ ] **Step 3: Replace `exchange_code`, `refresh`, `verify_id_token`, and all JWKS methods**

Replace the old `exchange_code` / `refresh` / `verify_id_token` / `find_jwk_for_kid` / `fetch_jwks` / `refresh_jwks` / `refresh_jwks_locked` / `cached_jwks` methods (previously lines 334–563 of the old file, ending right before the closing `}` of `impl GoogleProvider`) with:

```rust
    pub async fn exchange_code(
        &self,
        code: &str,
        code_verifier: &str,
    ) -> Result<ProviderExchange, AuthError> {
        let trace = RequestTrace::start("google", "code_exchange", "POST", &self.token_endpoint);
        info!(
            provider = "google",
            oauth_code_id = %fingerprint(code),
            redirect_uri = %self.redirect_uri,
            "oauth upstream code exchange started"
        );
        let payload: GoogleTokenResponse = read_json_response(
            trace,
            self.http.post(self.token_endpoint.clone()).form(&[
                ("grant_type", "authorization_code"),
                ("code", code),
                ("client_id", self.client_id.as_str()),
                ("client_secret", self.client_secret.as_str()),
                ("redirect_uri", self.redirect_uri.as_str()),
                ("code_verifier", code_verifier),
            ]),
            RequestErrors::new(
                "google",
                "exchange google auth code",
                "google token endpoint error",
                "decode google token response",
            ),
        )
        .await?;
        let claims = self.verifier.verify(&payload.id_token, &self.client_id).await?;
        info!(
            provider = "google",
            subject_id = %fingerprint(&claims.sub),
            has_refresh_token = payload.refresh_token.is_some(),
            expires_in_secs = payload.expires_in,
            "oauth upstream code exchange succeeded"
        );
        Ok(ProviderExchange {
            subject: claims.sub,
            email: claims.email,
            email_verified: claims.email_verified,
            access_token: payload.access_token,
            refresh_token: payload.refresh_token,
            expires_in: payload.expires_in,
            id_token: Some(payload.id_token),
        })
    }

    pub async fn refresh(&self, refresh_token: &str) -> Result<ProviderExchange, AuthError> {
        let trace = RequestTrace::start("google", "refresh", "POST", &self.token_endpoint);
        info!(
            provider = "google",
            refresh_token_id = %fingerprint(refresh_token),
            "oauth upstream refresh started"
        );
        let payload: GoogleTokenResponse = read_json_response(
            trace,
            self.http.post(self.token_endpoint.clone()).form(&[
                ("grant_type", "refresh_token"),
                ("refresh_token", refresh_token),
                ("client_id", self.client_id.as_str()),
                ("client_secret", self.client_secret.as_str()),
            ]),
            RequestErrors::new(
                "google",
                "refresh google token",
                "google refresh endpoint error",
                "decode google refresh response",
            ),
        )
        .await?;
        let claims = self.verifier.verify(&payload.id_token, &self.client_id).await?;
        info!(
            provider = "google",
            subject_id = %fingerprint(&claims.sub),
            has_refresh_token = payload.refresh_token.is_some(),
            expires_in_secs = payload.expires_in,
            "oauth upstream refresh succeeded"
        );
        Ok(ProviderExchange {
            subject: claims.sub,
            email: claims.email,
            email_verified: claims.email_verified,
            access_token: payload.access_token,
            refresh_token: payload.refresh_token,
            expires_in: payload.expires_in,
            id_token: Some(payload.id_token),
        })
    }
}

#[async_trait]
impl OAuthProvider for GoogleProvider {
    fn provider_id(&self) -> &'static str {
        "google"
    }

    fn callback_path(&self) -> &str {
        self.redirect_uri.path()
    }

    fn authorize_url(&self, request: &AuthorizeUrlRequest) -> Result<Url, AuthError> {
        Self::authorize_url(self, request)
    }

    async fn exchange_code(
        &self,
        code: &str,
        code_verifier: &str,
    ) -> Result<ProviderExchange, AuthError> {
        Self::exchange_code(self, code, code_verifier).await
    }

    async fn refresh(&self, refresh_token: &str) -> Result<ProviderExchange, AuthError> {
        Self::refresh(self, refresh_token).await
    }
}
```

- [ ] **Step 4: Delete the now-dead JWKS/claims types and helper functions**

Delete these items entirely from `google.rs` (all now live in `oidc.rs`): `GoogleIdTokenClaims`, `GoogleJwks`, `GoogleJwk`, `CachedGoogleJwks`, `GoogleRequestTrace`, `GoogleRequestErrors`, `read_json_response` (the free function), `google_jwks_ttl`, `parse_max_age`, `validate_id_token_header`, and the `GOOGLE_JWKS_FETCH_TIMEOUT` / `GOOGLE_DEFAULT_JWKS_TTL` constants. Also delete `GoogleExchange`.

- [ ] **Step 5: Update the test module**

The existing `#[cfg(test)] mod tests` block at the bottom of `google.rs` references `CachedGoogleJwks`, `GoogleJwk`, `GoogleJwks` directly in `google_exchange_refreshes_jwks_when_cached_kid_is_missing`'s use of `provider.jwks_cache.write()` and `wrong_test_jwks()`. Since `jwks_cache` moved into the private `OidcVerifier` (no longer a `GoogleProvider` field), that specific white-box test can no longer poke the cache directly. Replace:

```rust
        let provider = test_google_provider()
            .with_endpoints(
                server.uri().parse::<Url>().unwrap(),
                server.uri().parse::<Url>().unwrap().join("/token").unwrap(),
            )
            .with_jwks_endpoint(server.uri().parse::<Url>().unwrap().join("/certs").unwrap());
        *provider.jwks_cache.write().await = Some(CachedGoogleJwks {
            jwks: wrong_test_jwks(),
            expires_at: Instant::now() + Duration::from_secs(3600),
        });

        let exchange = provider.exchange_code("code", "verifier").await.unwrap();
        assert_eq!(exchange.subject, "google-subject-123");

        let requests = server.received_requests().await.unwrap();
        let jwks_requests = requests
            .iter()
            .filter(|request| request.url.path() == "/certs")
            .count();
        assert_eq!(jwks_requests, 1);
    }
```

with a black-box equivalent that proves the same behavior (a JWKS cache miss on `kid` triggers exactly one refresh, not a hard failure) by calling `exchange_code` twice against a server whose JWKS mock only serves the correct key — the original test's real intent was "stale cache doesn't break lookup", which is now already covered by `google_exchange_reuses_cached_jwks` plus the fact that a NEW `OidcVerifier` always starts with an empty cache. Replace the whole test with:

```rust
    #[tokio::test]
    async fn google_exchange_succeeds_on_first_jwks_fetch_with_no_pre_seeded_cache() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "access_token": "google-access-token",
                "refresh_token": "refresh-token",
                "expires_in": 3600,
                "id_token": signed_test_id_token("client-id", false, true),
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/certs"))
            .respond_with(ResponseTemplate::new(200).set_body_json(test_jwks()))
            .mount(&server)
            .await;

        let provider = test_google_provider()
            .with_endpoints(
                server.uri().parse::<Url>().unwrap(),
                server.uri().parse::<Url>().unwrap().join("/token").unwrap(),
            )
            .with_jwks_endpoint(server.uri().parse::<Url>().unwrap().join("/certs").unwrap());

        let exchange = provider.exchange_code("code", "verifier").await.unwrap();
        assert_eq!(exchange.subject, "google-subject-123");

        let requests = server.received_requests().await.unwrap();
        let jwks_requests = requests
            .iter()
            .filter(|request| request.url.path() == "/certs")
            .count();
        assert_eq!(jwks_requests, 1);
    }
```

Also delete the now-unused `wrong_test_jwks()` helper and its `GoogleJwks`/`GoogleJwk` import in the test module's `use super::{...}` line (change to `use super::{AuthorizeUrlRequest, GoogleProvider};`), and drop the `use std::time::{Duration, Instant};` import from the test module if nothing else in it still needs `Instant` (check with `cargo check` in Step 6 — `Duration` is still used by `unix_now`-adjacent test data, confirm before removing).

- [ ] **Step 6: Run the crate's test suite**

Run: `cargo test -p soma-auth google::`
Expected: all `google::tests::*` tests PASS, including the renamed `google_exchange_succeeds_on_first_jwks_fetch_with_no_pre_seeded_cache`.

Run: `cargo clippy -p soma-auth -- -D warnings`
Expected: no warnings (dead-code warnings from Tasks 1–3 should now be gone since `google.rs` is the first real consumer of `provider_http`/`oidc`).

- [ ] **Step 7: Commit**

```bash
git add crates/shared/auth/src/google.rs
git commit -m "refactor(soma-auth): rebuild GoogleProvider on oidc.rs + provider_http.rs, implement OAuthProvider"
```

---

### Task 5: `authelia.rs` — new `AutheliaProvider`

**Files:**
- Create: `crates/shared/auth/src/authelia.rs`
- Modify: `crates/shared/auth/src/lib.rs` (add `pub mod authelia;`)

**Interfaces:**
- Consumes: `crate::oauth_provider::{AuthorizeUrlRequest, OAuthProvider, ProviderExchange}`, `crate::oidc::OidcVerifier`, `crate::provider_http::{RequestTrace, RequestErrors, read_json_response}`.
- Produces: `pub struct AutheliaProvider` with `pub fn new(issuer: Url, client_id: String, client_secret: String, redirect_uri: Url) -> Result<Self, AuthError>`, public `scopes: Vec<String>` field (mutable, same pattern as `GoogleProvider.scopes`), and `impl OAuthProvider for AutheliaProvider`. Consumed by Task 9 (`state.rs`).

Authelia's OIDC Provider exposes fixed, non-configurable relative paths off its issuer: `api/oidc/authorization`, `api/oidc/token`, `api/oidc/jwks` (confirmed against Authelia's OpenID Connect 1.0 Provider documentation — these are not discoverable/configurable per-deployment, they are Authelia's hardcoded route table). The `iss` claim Authelia's ID tokens carry is the issuer URL itself, so unlike Google (fixed issuer constant) `AutheliaProvider` must rebuild its `OidcVerifier` if the issuer ever changes — this only happens in tests via `with_endpoints`.

- [ ] **Step 1: Create `authelia.rs`**

```rust
use std::time::Duration;

use async_trait::async_trait;
use reqwest::Url;
use serde::Deserialize;
use tracing::info;

use crate::error::AuthError;
use crate::oauth_provider::{AuthorizeUrlRequest, OAuthProvider, ProviderExchange};
use crate::oidc::OidcVerifier;
use crate::provider_http::{RequestErrors, RequestTrace, read_json_response};
use crate::util::fingerprint;

const AUTHELIA_HTTP_TIMEOUT: Duration = Duration::from_secs(30);
/// Authelia's OpenID Connect 1.0 Provider mounts its endpoints at these
/// fixed paths off the issuer — they are not configurable per-deployment.
const AUTHELIA_AUTHORIZE_PATH: &str = "api/oidc/authorization";
const AUTHELIA_TOKEN_PATH: &str = "api/oidc/token";
const AUTHELIA_JWKS_PATH: &str = "api/oidc/jwks";

#[derive(Clone)]
pub struct AutheliaProvider {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: Url,
    pub scopes: Vec<String>,
    pub http: reqwest::Client,
    issuer: Url,
    authorize_endpoint: Url,
    token_endpoint: Url,
    verifier: OidcVerifier,
}

impl std::fmt::Debug for AutheliaProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AutheliaProvider")
            .field("client_id", &self.client_id)
            .field("issuer", &self.issuer)
            .field("redirect_uri", &self.redirect_uri)
            .field("scopes", &self.scopes)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Deserialize)]
struct AutheliaTokenResponse {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    expires_in: Option<u64>,
    id_token: String,
}

impl AutheliaProvider {
    pub fn new(
        issuer: Url,
        client_id: String,
        client_secret: String,
        redirect_uri: Url,
    ) -> Result<Self, AuthError> {
        drop(rustls::crypto::ring::default_provider().install_default());
        let http = reqwest::Client::builder()
            .timeout(AUTHELIA_HTTP_TIMEOUT)
            .build()
            .map_err(|error| {
                AuthError::Storage(format!("build authelia oauth http client: {error}"))
            })?;
        let authorize_endpoint = issuer.join(AUTHELIA_AUTHORIZE_PATH).map_err(|error| {
            AuthError::Config(format!("build authelia authorize endpoint: {error}"))
        })?;
        let token_endpoint = issuer.join(AUTHELIA_TOKEN_PATH).map_err(|error| {
            AuthError::Config(format!("build authelia token endpoint: {error}"))
        })?;
        let jwks_endpoint = issuer.join(AUTHELIA_JWKS_PATH).map_err(|error| {
            AuthError::Config(format!("build authelia jwks endpoint: {error}"))
        })?;
        let verifier = OidcVerifier::new(
            "authelia",
            issuer.as_str().trim_end_matches('/').to_string(),
            jwks_endpoint,
            http.clone(),
        );

        Ok(Self {
            client_id,
            client_secret,
            redirect_uri,
            scopes: vec![
                "openid".to_string(),
                "email".to_string(),
                "profile".to_string(),
                "offline_access".to_string(),
            ],
            http,
            issuer,
            authorize_endpoint,
            token_endpoint,
            verifier,
        })
    }

    #[cfg(test)]
    #[must_use]
    pub fn with_endpoints(
        mut self,
        issuer: Url,
        authorize_endpoint: Url,
        token_endpoint: Url,
        jwks_endpoint: Url,
    ) -> Self {
        self.authorize_endpoint = authorize_endpoint;
        self.token_endpoint = token_endpoint;
        self.verifier = OidcVerifier::new(
            "authelia",
            issuer.as_str().trim_end_matches('/').to_string(),
            jwks_endpoint,
            self.http.clone(),
        );
        self.issuer = issuer;
        self
    }

    pub fn authorize_url(&self, request: &AuthorizeUrlRequest) -> Result<Url, AuthError> {
        let mut url = self.authorize_endpoint.clone();
        let scope = self.scopes.join(" ");
        url.query_pairs_mut()
            .append_pair("client_id", &self.client_id)
            .append_pair("redirect_uri", self.redirect_uri.as_str())
            .append_pair("response_type", "code")
            .append_pair("scope", &scope)
            .append_pair("state", &request.state)
            .append_pair("code_challenge", &request.code_challenge)
            .append_pair("code_challenge_method", &request.code_challenge_method);
        if request.force_consent {
            url.query_pairs_mut().append_pair("prompt", "consent");
        }
        Ok(url)
    }

    pub async fn exchange_code(
        &self,
        code: &str,
        code_verifier: &str,
    ) -> Result<ProviderExchange, AuthError> {
        let trace = RequestTrace::start("authelia", "code_exchange", "POST", &self.token_endpoint);
        info!(
            provider = "authelia",
            oauth_code_id = %fingerprint(code),
            redirect_uri = %self.redirect_uri,
            "oauth upstream code exchange started"
        );
        let payload: AutheliaTokenResponse = read_json_response(
            trace,
            self.http.post(self.token_endpoint.clone()).form(&[
                ("grant_type", "authorization_code"),
                ("code", code),
                ("client_id", self.client_id.as_str()),
                ("client_secret", self.client_secret.as_str()),
                ("redirect_uri", self.redirect_uri.as_str()),
                ("code_verifier", code_verifier),
            ]),
            RequestErrors::new(
                "authelia",
                "exchange authelia auth code",
                "authelia token endpoint error",
                "decode authelia token response",
            ),
        )
        .await?;
        let claims = self.verifier.verify(&payload.id_token, &self.client_id).await?;
        Ok(ProviderExchange {
            subject: claims.sub,
            email: claims.email,
            email_verified: claims.email_verified,
            access_token: payload.access_token,
            refresh_token: payload.refresh_token,
            expires_in: payload.expires_in,
            id_token: Some(payload.id_token),
        })
    }

    pub async fn refresh(&self, refresh_token: &str) -> Result<ProviderExchange, AuthError> {
        let trace = RequestTrace::start("authelia", "refresh", "POST", &self.token_endpoint);
        let payload: AutheliaTokenResponse = read_json_response(
            trace,
            self.http.post(self.token_endpoint.clone()).form(&[
                ("grant_type", "refresh_token"),
                ("refresh_token", refresh_token),
                ("client_id", self.client_id.as_str()),
                ("client_secret", self.client_secret.as_str()),
            ]),
            RequestErrors::new(
                "authelia",
                "refresh authelia token",
                "authelia refresh endpoint error",
                "decode authelia refresh response",
            ),
        )
        .await?;
        let claims = self.verifier.verify(&payload.id_token, &self.client_id).await?;
        Ok(ProviderExchange {
            subject: claims.sub,
            email: claims.email,
            email_verified: claims.email_verified,
            access_token: payload.access_token,
            refresh_token: payload.refresh_token,
            expires_in: payload.expires_in,
            id_token: Some(payload.id_token),
        })
    }
}

#[async_trait]
impl OAuthProvider for AutheliaProvider {
    fn provider_id(&self) -> &'static str {
        "authelia"
    }

    fn callback_path(&self) -> &str {
        self.redirect_uri.path()
    }

    fn authorize_url(&self, request: &AuthorizeUrlRequest) -> Result<Url, AuthError> {
        Self::authorize_url(self, request)
    }

    async fn exchange_code(
        &self,
        code: &str,
        code_verifier: &str,
    ) -> Result<ProviderExchange, AuthError> {
        Self::exchange_code(self, code, code_verifier).await
    }

    async fn refresh(&self, refresh_token: &str) -> Result<ProviderExchange, AuthError> {
        Self::refresh(self, refresh_token).await
    }
}

#[cfg(test)]
mod tests {
    use base64::Engine;
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use jsonwebtoken::{Algorithm, Header, encode};
    use rsa::RsaPrivateKey;
    use rsa::pkcs8::EncodePrivateKey;
    use rsa::rand_core::{TryCryptoRng, TryRng, UnwrapErr};
    use rsa::traits::PublicKeyParts;
    use serde_json::json;
    use std::sync::OnceLock;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::{AuthorizeUrlRequest, AutheliaProvider};

    #[test]
    fn authelia_authorize_url_requests_offline_access_via_scope_not_access_type() {
        let provider = test_authelia_provider();
        let request = AuthorizeUrlRequest {
            state: "state-123".to_string(),
            scope: "lab".to_string(),
            code_challenge: "challenge".to_string(),
            code_challenge_method: "S256".to_string(),
            force_consent: true,
        };
        let url = provider.authorize_url(&request).unwrap();
        assert!(url.as_str().contains("scope=openid+email+profile+offline_access"));
        assert!(!url.as_str().contains("access_type="));
        assert!(url.as_str().contains("prompt=consent"));
    }

    #[tokio::test]
    async fn authelia_exchange_parses_subject_from_id_token() {
        let server = MockServer::start().await;
        let issuer = Url::parse(&server.uri()).unwrap();
        Mock::given(method("POST"))
            .and(path("/api/oidc/token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "access_token": "authelia-access-token",
                "refresh_token": "authelia-refresh-token",
                "expires_in": 3600,
                "id_token": signed_test_id_token(&issuer, "client-id"),
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/api/oidc/jwks"))
            .respond_with(ResponseTemplate::new(200).set_body_json(test_jwks()))
            .mount(&server)
            .await;

        let provider = test_authelia_provider().with_endpoints(
            issuer.clone(),
            issuer.join("api/oidc/authorization").unwrap(),
            issuer.join("api/oidc/token").unwrap(),
            issuer.join("api/oidc/jwks").unwrap(),
        );

        let exchange = provider.exchange_code("code", "verifier").await.unwrap();
        assert_eq!(exchange.subject, "authelia-subject-123");
        assert_eq!(exchange.refresh_token.as_deref(), Some("authelia-refresh-token"));
    }

    use reqwest::Url;

    fn test_authelia_provider() -> AutheliaProvider {
        AutheliaProvider::new(
            Url::parse("https://auth.example.com").unwrap(),
            "client-id".to_string(),
            "client-secret".to_string(),
            Url::parse("https://lab.example.com/auth/authelia/callback").unwrap(),
        )
        .unwrap()
    }

    fn signed_test_id_token(issuer: &Url, client_id: &str) -> String {
        let claims = json!({
            "iss": issuer.as_str().trim_end_matches('/'),
            "aud": client_id,
            "sub": "authelia-subject-123",
            "email": "user@example.com",
            "email_verified": true,
            "iat": (unix_now() - 10) as usize,
            "exp": (unix_now() + 3600) as usize,
        });
        let mut header = Header::new(Algorithm::RS256);
        header.kid = Some("test-kid".to_string());
        encode(&header, &claims, &test_encoding_key()).unwrap()
    }

    fn test_jwks() -> serde_json::Value {
        let key = test_rsa_key();
        let public_key = key.to_public_key();
        json!({
            "keys": [{
                "kid": "test-kid",
                "alg": "RS256",
                "kty": "RSA",
                "use": "sig",
                "n": URL_SAFE_NO_PAD.encode(public_key.n_bytes()),
                "e": URL_SAFE_NO_PAD.encode(public_key.e_bytes()),
            }]
        })
    }

    fn test_rsa_key() -> &'static RsaPrivateKey {
        static TEST_RSA_KEY: OnceLock<RsaPrivateKey> = OnceLock::new();
        TEST_RSA_KEY.get_or_init(|| {
            let mut rng = UnwrapErr(TestRng);
            RsaPrivateKey::new(&mut rng, 2048).unwrap()
        })
    }

    fn test_encoding_key() -> jsonwebtoken::EncodingKey {
        let pem = test_rsa_key().to_pkcs8_pem(Default::default()).unwrap();
        jsonwebtoken::EncodingKey::from_rsa_pem(pem.as_bytes()).unwrap()
    }

    fn unix_now() -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64
    }

    struct TestRng;

    impl TryRng for TestRng {
        type Error = getrandom::Error;

        fn try_next_u32(&mut self) -> Result<u32, Self::Error> {
            let mut bytes = [0u8; 4];
            getrandom::fill(&mut bytes)?;
            Ok(u32::from_le_bytes(bytes))
        }

        fn try_next_u64(&mut self) -> Result<u64, Self::Error> {
            let mut bytes = [0u8; 8];
            getrandom::fill(&mut bytes)?;
            Ok(u64::from_le_bytes(bytes))
        }

        fn try_fill_bytes(&mut self, dst: &mut [u8]) -> Result<(), Self::Error> {
            getrandom::fill(dst)
        }
    }

    impl TryCryptoRng for TestRng {}
}
```

- [ ] **Step 2: Register the module in `lib.rs`**

Add near `pub mod google;`:

```rust
pub mod authelia;
```

- [ ] **Step 3: Run the new tests**

Run: `cargo test -p soma-auth authelia::`
Expected: `authelia_authorize_url_requests_offline_access_via_scope_not_access_type` and `authelia_exchange_parses_subject_from_id_token` PASS.

Run: `cargo clippy -p soma-auth -- -D warnings`
Expected: no warnings.

- [ ] **Step 4: Commit**

```bash
git add crates/shared/auth/src/authelia.rs crates/shared/auth/src/lib.rs
git commit -m "feat(soma-auth): add AutheliaProvider (OIDC)"
```

---

### Task 6: `github.rs` — new `GitHubProvider`

**Files:**
- Create: `crates/shared/auth/src/github.rs`
- Modify: `crates/shared/auth/src/lib.rs` (add `pub mod github;`)

**Interfaces:**
- Consumes: `crate::oauth_provider::{AuthorizeUrlRequest, OAuthProvider, ProviderExchange}`, `crate::provider_http::{RequestTrace, RequestErrors, read_json_response}`.
- Produces: `pub struct GitHubProvider` with `pub fn new(client_id: String, client_secret: String, redirect_uri: Url) -> Result<Self, AuthError>`, public `scopes: Vec<String>` field, `impl OAuthProvider for GitHubProvider`. Consumed by Task 9 (`state.rs`).

GitHub OAuth Apps have no ID token and no refresh token (access tokens from OAuth Apps — as opposed to GitHub Apps — do not expire and cannot be refreshed). `refresh()` therefore always returns `AuthError::Config`; `exchange_code()` fetches `GET /user` then `GET /user/emails` to find the primary verified email. `subject` is the numeric GitHub user `id` (stable across username renames), not `login`.

- [ ] **Step 1: Create `github.rs`**

```rust
use std::time::Duration;

use async_trait::async_trait;
use reqwest::Url;
use serde::Deserialize;
use tracing::info;

use crate::error::AuthError;
use crate::oauth_provider::{AuthorizeUrlRequest, OAuthProvider, ProviderExchange};
use crate::provider_http::{RequestErrors, RequestTrace, read_json_response};
use crate::util::fingerprint;

const GITHUB_AUTHORIZE_ENDPOINT: &str = "https://github.com/login/oauth/authorize";
const GITHUB_TOKEN_ENDPOINT: &str = "https://github.com/login/oauth/access_token";
const GITHUB_USER_ENDPOINT: &str = "https://api.github.com/user";
const GITHUB_USER_EMAILS_ENDPOINT: &str = "https://api.github.com/user/emails";
const GITHUB_HTTP_TIMEOUT: Duration = Duration::from_secs(30);
const GITHUB_USER_AGENT: &str = "soma-auth";

#[derive(Clone, Debug)]
pub struct GitHubProvider {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: Url,
    pub scopes: Vec<String>,
    pub http: reqwest::Client,
    authorize_endpoint: Url,
    token_endpoint: Url,
    user_endpoint: Url,
    user_emails_endpoint: Url,
}

#[derive(Debug, Deserialize)]
struct GitHubTokenResponse {
    access_token: String,
}

#[derive(Debug, Deserialize)]
struct GitHubUser {
    id: u64,
    #[serde(default)]
    email: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GitHubUserEmail {
    email: String,
    primary: bool,
    verified: bool,
}

impl GitHubProvider {
    pub fn new(
        client_id: String,
        client_secret: String,
        redirect_uri: Url,
    ) -> Result<Self, AuthError> {
        drop(rustls::crypto::ring::default_provider().install_default());
        let http = reqwest::Client::builder()
            .timeout(GITHUB_HTTP_TIMEOUT)
            .user_agent(GITHUB_USER_AGENT)
            .build()
            .map_err(|error| {
                AuthError::Storage(format!("build github oauth http client: {error}"))
            })?;
        Ok(Self {
            client_id,
            client_secret,
            redirect_uri,
            scopes: vec!["read:user".to_string(), "user:email".to_string()],
            http,
            authorize_endpoint: Url::parse(GITHUB_AUTHORIZE_ENDPOINT)
                .expect("valid github authorize url"),
            token_endpoint: Url::parse(GITHUB_TOKEN_ENDPOINT).expect("valid github token url"),
            user_endpoint: Url::parse(GITHUB_USER_ENDPOINT).expect("valid github user url"),
            user_emails_endpoint: Url::parse(GITHUB_USER_EMAILS_ENDPOINT)
                .expect("valid github user emails url"),
        })
    }

    #[cfg(test)]
    #[must_use]
    pub fn with_endpoints(
        mut self,
        authorize_endpoint: Url,
        token_endpoint: Url,
        user_endpoint: Url,
        user_emails_endpoint: Url,
    ) -> Self {
        self.authorize_endpoint = authorize_endpoint;
        self.token_endpoint = token_endpoint;
        self.user_endpoint = user_endpoint;
        self.user_emails_endpoint = user_emails_endpoint;
        self
    }

    pub fn authorize_url(&self, request: &AuthorizeUrlRequest) -> Result<Url, AuthError> {
        let mut url = self.authorize_endpoint.clone();
        let scope = self.scopes.join(" ");
        url.query_pairs_mut()
            .append_pair("client_id", &self.client_id)
            .append_pair("redirect_uri", self.redirect_uri.as_str())
            .append_pair("response_type", "code")
            .append_pair("scope", &scope)
            .append_pair("state", &request.state)
            .append_pair("code_challenge", &request.code_challenge)
            .append_pair("code_challenge_method", &request.code_challenge_method);
        if request.force_consent {
            url.query_pairs_mut().append_pair("prompt", "consent");
        }
        Ok(url)
    }

    pub async fn exchange_code(
        &self,
        code: &str,
        code_verifier: &str,
    ) -> Result<ProviderExchange, AuthError> {
        let trace = RequestTrace::start("github", "code_exchange", "POST", &self.token_endpoint);
        info!(
            provider = "github",
            oauth_code_id = %fingerprint(code),
            redirect_uri = %self.redirect_uri,
            "oauth upstream code exchange started"
        );
        let payload: GitHubTokenResponse = read_json_response(
            trace,
            self.http
                .post(self.token_endpoint.clone())
                .header(reqwest::header::ACCEPT, "application/json")
                .form(&[
                    ("grant_type", "authorization_code"),
                    ("code", code),
                    ("client_id", self.client_id.as_str()),
                    ("client_secret", self.client_secret.as_str()),
                    ("redirect_uri", self.redirect_uri.as_str()),
                    ("code_verifier", code_verifier),
                ]),
            RequestErrors::new(
                "github",
                "exchange github auth code",
                "github token endpoint error",
                "decode github token response",
            ),
        )
        .await?;
        self.fetch_exchange(payload).await
    }

    pub async fn refresh(&self, _refresh_token: &str) -> Result<ProviderExchange, AuthError> {
        Err(AuthError::Config(
            "github oauth apps do not support token refresh — access tokens do not expire; \
             the user must re-authenticate via github once their local soma-issued refresh \
             token expires"
                .to_string(),
        ))
    }

    /// Fetches `GET /user` and `GET /user/emails` **concurrently** via
    /// `tokio::try_join!` — they are independent, both authenticated with the
    /// same bearer token, and running them sequentially (as an earlier draft
    /// of this plan did) needlessly widens the worst-case timeout envelope: 3
    /// sequential hops each independently subject to `GITHUB_HTTP_TIMEOUT`
    /// (30s) can chain up to ~90s before failing, vs Google/Authelia's ~35s
    /// worst case (30s token exchange + 5s JWKS). Joining the two GETs caps
    /// GitHub's worst case at ~60s (30s token exchange + max(30s, 30s)).
    async fn fetch_exchange(&self, payload: GitHubTokenResponse) -> Result<ProviderExchange, AuthError> {
        let (user, verified_email) = tokio::try_join!(
            self.fetch_user(&payload.access_token),
            self.fetch_primary_verified_email(&payload.access_token),
        )?;

        let (email, email_verified) = match verified_email {
            Some(verified) => (Some(verified), Some(true)),
            None => (user.email, None),
        };

        info!(
            provider = "github",
            subject_id = %fingerprint(&user.id.to_string()),
            "oauth upstream code exchange succeeded"
        );

        let exchange = ProviderExchange {
            subject: user.id.to_string(),
            email,
            email_verified,
            access_token: payload.access_token,
            refresh_token: None,
            expires_in: None,
            id_token: None,
        };
        debug_assert!(
            exchange.refresh_token.is_none(),
            "GitHubProvider::exchange_code must never set refresh_token — GitHub OAuth Apps \
             don't issue one, and refresh_token_grant's routing to GitHubProvider::refresh \
             (which unconditionally errors) is only unreachable in practice because this \
             invariant holds. If this ever fires, GitHubProvider::refresh needs a real \
             implementation, not just an error."
        );
        Ok(exchange)
    }

    async fn fetch_user(&self, access_token: &str) -> Result<GitHubUser, AuthError> {
        let trace = RequestTrace::start("github", "fetch_user", "GET", &self.user_endpoint);
        read_json_response(
            trace,
            self.http
                .get(self.user_endpoint.clone())
                .bearer_auth(access_token)
                .header(reqwest::header::ACCEPT, "application/vnd.github+json"),
            RequestErrors::new(
                "github",
                "fetch github user",
                "github user endpoint error",
                "decode github user response",
            ),
        )
        .await
    }

    async fn fetch_primary_verified_email(
        &self,
        access_token: &str,
    ) -> Result<Option<String>, AuthError> {
        let trace = RequestTrace::start(
            "github",
            "fetch_user_emails",
            "GET",
            &self.user_emails_endpoint,
        );
        let emails: Vec<GitHubUserEmail> = read_json_response(
            trace,
            self.http
                .get(self.user_emails_endpoint.clone())
                .bearer_auth(access_token)
                .header(reqwest::header::ACCEPT, "application/vnd.github+json"),
            RequestErrors::new(
                "github",
                "fetch github user emails",
                "github user emails endpoint error",
                "decode github user emails response",
            ),
        )
        .await?;
        Ok(emails
            .into_iter()
            .find(|entry| entry.primary && entry.verified)
            .map(|entry| entry.email))
    }
}

#[async_trait]
impl OAuthProvider for GitHubProvider {
    fn provider_id(&self) -> &'static str {
        "github"
    }

    fn callback_path(&self) -> &str {
        self.redirect_uri.path()
    }

    fn authorize_url(&self, request: &AuthorizeUrlRequest) -> Result<Url, AuthError> {
        Self::authorize_url(self, request)
    }

    async fn exchange_code(
        &self,
        code: &str,
        code_verifier: &str,
    ) -> Result<ProviderExchange, AuthError> {
        Self::exchange_code(self, code, code_verifier).await
    }

    async fn refresh(&self, refresh_token: &str) -> Result<ProviderExchange, AuthError> {
        Self::refresh(self, refresh_token).await
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::{AuthorizeUrlRequest, GitHubProvider};

    #[tokio::test]
    async fn github_exchange_uses_numeric_id_as_subject_and_primary_verified_email() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/login/oauth/access_token"))
            .and(header("accept", "application/json"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "access_token": "gho_test-token",
                "scope": "read:user,user:email",
                "token_type": "bearer",
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/user"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": 9182310,
                "login": "octocat",
                "email": null,
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/user/emails"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([
                {"email": "secondary@example.com", "primary": false, "verified": true},
                {"email": "primary@example.com", "primary": true, "verified": true},
            ])))
            .mount(&server)
            .await;

        let base = url::Url::parse(&server.uri()).unwrap();
        let provider = test_github_provider().with_endpoints(
            base.join("login/oauth/authorize").unwrap(),
            base.join("login/oauth/access_token").unwrap(),
            base.join("user").unwrap(),
            base.join("user/emails").unwrap(),
        );

        let exchange = provider.exchange_code("code", "verifier").await.unwrap();
        assert_eq!(exchange.subject, "9182310");
        assert_eq!(exchange.email.as_deref(), Some("primary@example.com"));
        assert_eq!(exchange.email_verified, Some(true));
        assert!(exchange.id_token.is_none());
        assert!(exchange.refresh_token.is_none());
    }

    #[tokio::test]
    async fn github_refresh_always_errors() {
        let provider = test_github_provider();
        let error = provider.refresh("whatever").await.unwrap_err();
        assert!(error.to_string().contains("do not support token refresh"));
    }

    #[test]
    fn github_authorize_url_uses_read_user_and_user_email_scopes() {
        let provider = test_github_provider();
        let request = AuthorizeUrlRequest {
            state: "state-123".to_string(),
            scope: "lab".to_string(),
            code_challenge: "challenge".to_string(),
            code_challenge_method: "S256".to_string(),
            force_consent: false,
        };
        let url = provider.authorize_url(&request).unwrap();
        assert!(url.as_str().contains("scope=read%3Auser+user%3Aemail"));
    }

    fn test_github_provider() -> GitHubProvider {
        GitHubProvider::new(
            "client-id".to_string(),
            "client-secret".to_string(),
            url::Url::parse("https://lab.example.com/auth/github/callback").unwrap(),
        )
        .unwrap()
    }
}
```

- [ ] **Step 2: Register the module in `lib.rs`**

Add near `pub mod google;`:

```rust
pub mod github;
```

- [ ] **Step 3: Run the new tests**

Run: `cargo test -p soma-auth github::`
Expected: `github_exchange_uses_numeric_id_as_subject_and_primary_verified_email`, `github_refresh_always_errors`, `github_authorize_url_uses_read_user_and_user_email_scopes` PASS.

Run: `cargo clippy -p soma-auth -- -D warnings`
Expected: no warnings.

- [ ] **Step 4: Commit**

```bash
git add crates/shared/auth/src/github.rs crates/shared/auth/src/lib.rs
git commit -m "feat(soma-auth): add GitHubProvider (plain OAuth2, no ID token, no refresh)"
```

---

### Task 7: `config.rs` — `AutheliaConfig`, `GitHubConfig`, `default_provider`, multi-provider validation

**Files:**
- Modify: `crates/shared/auth/src/config.rs`

**Interfaces:**
- Produces: `pub struct AutheliaConfig { issuer_url: Option<Url>, client_id: String, client_secret: String, callback_path: String, scopes: Vec<String> }`, `pub struct GitHubConfig { client_id: String, client_secret: String, callback_path: String, scopes: Vec<String> }`, `AuthConfig.authelia: AutheliaConfig`, `AuthConfig.github: GitHubConfig`, `AuthConfig.default_provider: String`. Consumed by Task 9 (`state.rs`).

- [ ] **Step 1: Add the two new config structs**

In `crates/shared/auth/src/config.rs`, right after the existing `GoogleConfig` struct (currently ending at line 92, just before `pub struct AuthConfig`), insert:

```rust
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutheliaConfig {
    #[serde(default)]
    pub issuer_url: Option<Url>,
    #[serde(default)]
    pub client_id: String,
    #[serde(default)]
    pub client_secret: String,
    #[serde(default = "default_authelia_callback_path")]
    pub callback_path: String,
    #[serde(default = "default_authelia_scopes")]
    pub scopes: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitHubConfig {
    #[serde(default)]
    pub client_id: String,
    #[serde(default)]
    pub client_secret: String,
    #[serde(default = "default_github_callback_path")]
    pub callback_path: String,
    #[serde(default = "default_github_scopes")]
    pub scopes: Vec<String>,
}
```

- [ ] **Step 2: Add the fields to `AuthConfig`, its `Default` impl, and the two new callback-path/scopes default constants**

In the `AuthConfig` struct, right after `pub google: GoogleConfig,`, add:

```rust
    pub authelia: AutheliaConfig,
    pub github: GitHubConfig,
    /// Which configured provider `/authorize` and `/auth/login` use when the
    /// request omits `?provider=`. Must name a provider that is actually
    /// configured (validated in [`AuthConfig::validate`]). Resolved
    /// automatically when unset: `google` > `authelia` > `github`, in that
    /// priority order, picking the first one that has credentials — this is
    /// what makes every existing single-provider (Google-only) deployment
    /// keep working with zero config changes after upgrading.
    pub default_provider: String,
```

In `impl Default for AuthConfig`, right after `google: GoogleConfig::default(),`, add:

```rust
            authelia: AutheliaConfig::default(),
            github: GitHubConfig::default(),
            default_provider: String::new(),
```

Near the bottom of the file, right after `fn default_google_scopes()`, add:

```rust
fn default_authelia_callback_path() -> String {
    "/auth/authelia/callback".to_string()
}

fn default_authelia_scopes() -> Vec<String> {
    vec![
        "openid".to_string(),
        "email".to_string(),
        "profile".to_string(),
        "offline_access".to_string(),
    ]
}

fn default_github_callback_path() -> String {
    "/auth/github/callback".to_string()
}

fn default_github_scopes() -> Vec<String> {
    vec!["read:user".to_string(), "user:email".to_string()]
}
```

- [ ] **Step 3: Rewrite `AuthConfig::validate()`'s OAuth-mode block**

Replace the existing `if matches!(self.mode, AuthMode::OAuth) { ... }` block in `validate()` (currently checking `self.google.client_id`/`client_secret` unconditionally) with:

```rust
        if matches!(self.mode, AuthMode::OAuth) {
            if self.public_url.is_none() {
                return Err(AuthError::Config(format!(
                    "{prefix}_PUBLIC_URL is required when {prefix}_AUTH_MODE=oauth"
                )));
            }

            let google_configured = !self.google.client_id.is_empty();
            let authelia_configured = !self.authelia.client_id.is_empty();
            let github_configured = !self.github.client_id.is_empty();

            if google_configured && self.google.client_secret.is_empty() {
                return Err(AuthError::Config(format!(
                    "{prefix}_GOOGLE_CLIENT_SECRET is required when {prefix}_GOOGLE_CLIENT_ID is set"
                )));
            }
            if authelia_configured {
                if self.authelia.issuer_url.is_none() {
                    return Err(AuthError::Config(format!(
                        "{prefix}_AUTHELIA_ISSUER_URL is required when {prefix}_AUTHELIA_CLIENT_ID is set"
                    )));
                }
                if self.authelia.client_secret.is_empty() {
                    return Err(AuthError::Config(format!(
                        "{prefix}_AUTHELIA_CLIENT_SECRET is required when {prefix}_AUTHELIA_CLIENT_ID is set"
                    )));
                }
                // Google's authorize/token/JWKS endpoints are hardcoded `https://`
                // string constants — no config can downgrade them. Authelia's are
                // entirely operator-supplied, so unlike Google this crate must
                // enforce the scheme itself: a plaintext issuer would send
                // authorization codes, tokens, and `client_secret` (in the token
                // exchange POST body) over the wire unencrypted with no other
                // signal that anything is wrong.
                if let Some(issuer) = self.authelia.issuer_url.as_ref()
                    && issuer.scheme() != "https"
                {
                    return Err(AuthError::Config(format!(
                        "{prefix}_AUTHELIA_ISSUER_URL must use https, got `{}`",
                        issuer.scheme()
                    )));
                }
            }
            if github_configured && self.github.client_secret.is_empty() {
                return Err(AuthError::Config(format!(
                    "{prefix}_GITHUB_CLIENT_SECRET is required when {prefix}_GITHUB_CLIENT_ID is set"
                )));
            }
            // Two configured providers with the same (possibly operator-overridden)
            // callback_path would make routes.rs's per-provider route-mounting loop
            // (Task 10) hit axum's duplicate-route panic at startup instead of a
            // clean config-time error — check pairwise uniqueness among only the
            // providers that are actually configured.
            {
                let mut configured_paths: Vec<(&str, &str)> = Vec::new();
                if google_configured {
                    configured_paths.push(("google", self.google.callback_path.as_str()));
                }
                if authelia_configured {
                    configured_paths.push(("authelia", self.authelia.callback_path.as_str()));
                }
                if github_configured {
                    configured_paths.push(("github", self.github.callback_path.as_str()));
                }
                for i in 0..configured_paths.len() {
                    for j in (i + 1)..configured_paths.len() {
                        if configured_paths[i].1 == configured_paths[j].1 {
                            return Err(AuthError::Config(format!(
                                "{prefix}_{a}_CALLBACK_PATH and {prefix}_{b}_CALLBACK_PATH must not both be `{path}`",
                                a = configured_paths[i].0.to_ascii_uppercase(),
                                b = configured_paths[j].0.to_ascii_uppercase(),
                                path = configured_paths[i].1,
                            )));
                        }
                    }
                }
            }
            if !google_configured && !authelia_configured && !github_configured {
                return Err(AuthError::Config(format!(
                    "at least one OAuth provider must be configured when {prefix}_AUTH_MODE=oauth — \
                     set {prefix}_GOOGLE_CLIENT_ID, {prefix}_AUTHELIA_CLIENT_ID (+ {prefix}_AUTHELIA_ISSUER_URL), \
                     or {prefix}_GITHUB_CLIENT_ID (each paired with its matching _CLIENT_SECRET)"
                )));
            }
            match self.default_provider.as_str() {
                "google" if !google_configured => {
                    return Err(AuthError::Config(format!(
                        "{prefix}_AUTH_DEFAULT_PROVIDER=google but {prefix}_GOOGLE_CLIENT_ID is not set"
                    )));
                }
                "authelia" if !authelia_configured => {
                    return Err(AuthError::Config(format!(
                        "{prefix}_AUTH_DEFAULT_PROVIDER=authelia but {prefix}_AUTHELIA_CLIENT_ID is not set"
                    )));
                }
                "github" if !github_configured => {
                    return Err(AuthError::Config(format!(
                        "{prefix}_AUTH_DEFAULT_PROVIDER=github but {prefix}_GITHUB_CLIENT_ID is not set"
                    )));
                }
                "google" | "authelia" | "github" => {}
                other => {
                    return Err(AuthError::Config(format!(
                        "{prefix}_AUTH_DEFAULT_PROVIDER must be `google`, `authelia`, or `github`, got `{other}`"
                    )));
                }
            }
            if self.admin_email.is_empty() {
                return Err(AuthError::Config(format!(
                    "{prefix}_AUTH_ADMIN_EMAIL is required when {prefix}_AUTH_MODE=oauth — \
                     set the admin's email so no account can log in unless explicitly permitted"
                )));
            }
        }
```

- [ ] **Step 4: Wire env parsing in `AuthConfigBuilder::build_from_sources`**

Right after the existing `let key_g_scopes = env_key(&prefix, "GOOGLE_SCOPES");` line, add:

```rust
        let key_a_issuer = env_key(&prefix, "AUTHELIA_ISSUER_URL");
        let key_a_id = env_key(&prefix, "AUTHELIA_CLIENT_ID");
        let key_a_secret = env_key(&prefix, "AUTHELIA_CLIENT_SECRET");
        let key_a_callback = env_key(&prefix, "AUTHELIA_CALLBACK_PATH");
        let key_a_scopes = env_key(&prefix, "AUTHELIA_SCOPES");
        let key_gh_id = env_key(&prefix, "GITHUB_CLIENT_ID");
        let key_gh_secret = env_key(&prefix, "GITHUB_CLIENT_SECRET");
        let key_gh_callback = env_key(&prefix, "GITHUB_CALLBACK_PATH");
        let key_gh_scopes = env_key(&prefix, "GITHUB_SCOPES");
        let key_default_provider = env_key(&prefix, "AUTH_DEFAULT_PROVIDER");
```

Right after the existing `google: GoogleConfig { ... },` field in the `AuthConfig { ... }` literal, add:

```rust
            authelia: AutheliaConfig {
                issuer_url: read_url(&vars, &key_a_issuer)?,
                client_id: read_string(&vars, &key_a_id).unwrap_or_default(),
                client_secret: read_string(&vars, &key_a_secret).unwrap_or_default(),
                callback_path: read_string(&vars, &key_a_callback)
                    .unwrap_or_else(default_authelia_callback_path),
                scopes: read_csv(&vars, &key_a_scopes).unwrap_or_else(default_authelia_scopes),
            },
            github: GitHubConfig {
                client_id: read_string(&vars, &key_gh_id).unwrap_or_default(),
                client_secret: read_string(&vars, &key_gh_secret).unwrap_or_default(),
                callback_path: read_string(&vars, &key_gh_callback)
                    .unwrap_or_else(default_github_callback_path),
                scopes: read_csv(&vars, &key_gh_scopes).unwrap_or_else(default_github_scopes),
            },
```

And right after the `google:` field block currently reads `scopes: read_csv(&vars, &key_g_scopes).unwrap_or_else(default_google_scopes),` — note `google_client_id`/`google_client_secret` local reads are inline already; you need those raw strings again below, so capture them into locals BEFORE the struct literal instead of reading twice. Immediately before the `let config = AuthConfig { ... };` line, add:

```rust
        let google_client_id = read_string(&vars, &key_g_id).unwrap_or_default();
        let authelia_client_id = read_string(&vars, &key_a_id).unwrap_or_default();
        let github_client_id = read_string(&vars, &key_gh_id).unwrap_or_default();
        let default_provider = read_string(&vars, &key_default_provider)
            .map(|raw| raw.trim().to_ascii_lowercase())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| {
                if !google_client_id.is_empty() {
                    "google".to_string()
                } else if !authelia_client_id.is_empty() {
                    "authelia".to_string()
                } else if !github_client_id.is_empty() {
                    "github".to_string()
                } else {
                    "google".to_string()
                }
            });
```

Then change the existing `google: GoogleConfig { client_id: read_string(&vars, &key_g_id).unwrap_or_default(), ... }` line's `client_id:` to reuse the local: `client_id: google_client_id.clone(),`. And add `default_provider,` as a field in the `AuthConfig { ... }` struct literal, right after the `github: GitHubConfig { ... },` block you just added.

- [ ] **Step 5: Update the existing `oauth_mode_requires_public_url_and_google_credentials` test**

This test currently asserts Google is unconditionally required. Rename and rewrite it to assert the new "at least one provider" semantics:

```rust
    #[test]
    fn oauth_mode_requires_at_least_one_configured_provider() {
        let err = AuthConfig::from_sources(fake_env_with_many([
            ("LAB_AUTH_MODE", "oauth"),
            ("LAB_PUBLIC_URL", "https://lab.example.com"),
            ("LAB_AUTH_ADMIN_EMAIL", "admin@example.com"),
        ]))
        .unwrap_err();
        assert!(err.to_string().contains("at least one OAuth provider"));
    }

    #[test]
    fn oauth_mode_accepts_authelia_only_configuration() {
        let cfg = AuthConfig::from_sources(fake_env_with_many([
            ("LAB_AUTH_MODE", "oauth"),
            ("LAB_PUBLIC_URL", "https://lab.example.com"),
            ("LAB_AUTHELIA_ISSUER_URL", "https://auth.example.com"),
            ("LAB_AUTHELIA_CLIENT_ID", "id"),
            ("LAB_AUTHELIA_CLIENT_SECRET", "secret"),
            ("LAB_AUTH_ADMIN_EMAIL", "admin@example.com"),
        ]))
        .unwrap();
        assert_eq!(cfg.default_provider, "authelia");
    }

    #[test]
    fn oauth_mode_accepts_github_only_configuration() {
        let cfg = AuthConfig::from_sources(fake_env_with_many([
            ("LAB_AUTH_MODE", "oauth"),
            ("LAB_PUBLIC_URL", "https://lab.example.com"),
            ("LAB_GITHUB_CLIENT_ID", "id"),
            ("LAB_GITHUB_CLIENT_SECRET", "secret"),
            ("LAB_AUTH_ADMIN_EMAIL", "admin@example.com"),
        ]))
        .unwrap();
        assert_eq!(cfg.default_provider, "github");
    }

    #[test]
    fn oauth_mode_default_provider_prefers_google_when_multiple_are_configured() {
        let cfg = AuthConfig::from_sources(fake_env_with_many([
            ("LAB_AUTH_MODE", "oauth"),
            ("LAB_PUBLIC_URL", "https://lab.example.com"),
            ("LAB_GOOGLE_CLIENT_ID", "id"),
            ("LAB_GOOGLE_CLIENT_SECRET", "secret"),
            ("LAB_GITHUB_CLIENT_ID", "gh-id"),
            ("LAB_GITHUB_CLIENT_SECRET", "gh-secret"),
            ("LAB_AUTH_ADMIN_EMAIL", "admin@example.com"),
        ]))
        .unwrap();
        assert_eq!(cfg.default_provider, "google");
    }

    #[test]
    fn oauth_mode_rejects_default_provider_naming_an_unconfigured_provider() {
        let err = AuthConfig::from_sources(fake_env_with_many([
            ("LAB_AUTH_MODE", "oauth"),
            ("LAB_PUBLIC_URL", "https://lab.example.com"),
            ("LAB_GOOGLE_CLIENT_ID", "id"),
            ("LAB_GOOGLE_CLIENT_SECRET", "secret"),
            ("LAB_AUTH_ADMIN_EMAIL", "admin@example.com"),
            ("LAB_AUTH_DEFAULT_PROVIDER", "github"),
        ]))
        .unwrap_err();
        assert!(err.to_string().contains("LAB_AUTH_DEFAULT_PROVIDER=github"));
    }

    #[test]
    fn oauth_mode_rejects_a_non_https_authelia_issuer_url() {
        let err = AuthConfig::from_sources(fake_env_with_many([
            ("LAB_AUTH_MODE", "oauth"),
            ("LAB_PUBLIC_URL", "https://lab.example.com"),
            ("LAB_AUTHELIA_ISSUER_URL", "http://auth.internal"),
            ("LAB_AUTHELIA_CLIENT_ID", "id"),
            ("LAB_AUTHELIA_CLIENT_SECRET", "secret"),
            ("LAB_AUTH_ADMIN_EMAIL", "admin@example.com"),
        ]))
        .unwrap_err();
        assert!(err.to_string().contains("LAB_AUTHELIA_ISSUER_URL must use https"));
    }

    #[test]
    fn oauth_mode_rejects_two_configured_providers_sharing_a_callback_path() {
        let err = AuthConfig::from_sources(fake_env_with_many([
            ("LAB_AUTH_MODE", "oauth"),
            ("LAB_PUBLIC_URL", "https://lab.example.com"),
            ("LAB_GOOGLE_CLIENT_ID", "id"),
            ("LAB_GOOGLE_CLIENT_SECRET", "secret"),
            ("LAB_GITHUB_CLIENT_ID", "gh-id"),
            ("LAB_GITHUB_CLIENT_SECRET", "gh-secret"),
            ("LAB_GITHUB_CALLBACK_PATH", "/auth/google/callback"),
            ("LAB_AUTH_ADMIN_EMAIL", "admin@example.com"),
        ]))
        .unwrap_err();
        assert!(err.to_string().contains("must not both be `/auth/google/callback`"));
    }
```

Keep the OLD test's original name available as a red flag for reviewers: since the old test name `oauth_mode_requires_public_url_and_google_credentials` also implicitly tested `LAB_PUBLIC_URL` requirement, split that concern out too — the existing test already covers the public-url-missing case earlier in the file (`oauth_mode_requires_public_url_and_google_credentials` is literally being replaced, not duplicated — delete the old test body and its name entirely, replaced by the five tests above).

- [ ] **Step 6: Run config tests**

Run: `cargo test -p soma-auth config::`
Expected: all `config::tests::*` PASS, including the 5 new/rewritten tests. The pre-existing `default_config_preserves_lab_brand_for_backward_compat`, `oauth_mode_defaults_paths_and_callback`, `oauth_mode_requires_admin_email`, `admin_email_normalizes_case_and_trims_whitespace`, `oauth_mode_parses_allowed_client_redirect_uris`, `builder_env_prefix_resolves_consumer_env_vars`, `builder_lab_env_vars_ignored_when_prefix_is_overridden` must still PASS unmodified (they all configure Google, which is still fully supported).

Run: `cargo clippy -p soma-auth -- -D warnings`

- [ ] **Step 7: Commit**

```bash
git add crates/shared/auth/src/config.rs
git commit -m "feat(soma-auth): add AutheliaConfig/GitHubConfig and multi-provider validation"
```

---

### Task 8: `types.rs` + `sqlite.rs` — `provider` column on 4 tables

**Files:**
- Modify: `crates/shared/auth/src/types.rs`
- Modify: `crates/shared/auth/src/sqlite.rs`

**Interfaces:**
- Produces: `AuthorizationRequestRow.provider: String`, `AuthorizationCodeRow.provider: String`, `RefreshTokenRow.provider: String`, `BrowserLoginStateRow.provider: String`, `SqliteStore::has_any_refresh_token_for_provider(&self, provider: &str) -> Result<bool, AuthError>`. Consumed by Task 10 (`authorize.rs`) and Task 11 (`token.rs`).

Every existing struct-literal construction site of these four types (grep confirmed: 10 + 7 + 19 + 8 = 44 sites across `sqlite.rs`, `token.rs`, `types.rs`, `authorize.rs`) needs a `provider: "google".to_string()` (or a variable, in production code paths) field added — Rust's exhaustive struct-literal syntax will not compile otherwise. This task only covers `types.rs`/`sqlite.rs`'s own internal call sites (the row-mapping closures and the two test-fixture builders `sample_code()`/similar already living in `sqlite.rs`'s test module). Tasks 10/11 cover the call sites in `authorize.rs`/`token.rs`.

- [ ] **Step 1: Add `provider: String` to the four row types in `types.rs`**

```rust
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthorizationRequestRow {
    pub state: String,
    pub client_id: String,
    pub redirect_uri: String,
    pub client_state: String,
    pub resource: String,
    pub scope: String,
    pub provider: String,
    pub provider_code_verifier: String,
    pub code_challenge: String,
    pub code_challenge_method: String,
    pub created_at: i64,
    pub expires_at: i64,
}
```

```rust
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthorizationCodeRow {
    pub code: String,
    pub client_id: String,
    pub subject: String,
    pub redirect_uri: String,
    pub resource: String,
    pub scope: String,
    pub provider: String,
    pub code_challenge: String,
    pub code_challenge_method: String,
    pub provider_refresh_token: Option<String>,
    pub created_at: i64,
    pub expires_at: i64,
}
```

```rust
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RefreshTokenRow {
    pub refresh_token: String,
    pub client_id: String,
    pub subject: String,
    pub resource: String,
    pub scope: String,
    pub provider: String,
    pub provider_refresh_token: Option<String>,
    pub created_at: i64,
    pub expires_at: i64,
}
```

```rust
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrowserLoginStateRow {
    pub state: String,
    pub return_to: String,
    pub provider: String,
    pub provider_code_verifier: String,
    pub created_at: i64,
    pub expires_at: i64,
}
```

(Field placed right after `scope`/`resource`/`return_to` in each — exact position doesn't matter for a named-field struct, this placement just keeps it near the other provider-related fields for readability.)

- [ ] **Step 2: Add the `provider` column to all four `CREATE TABLE IF NOT EXISTS` bodies in `sqlite.rs`**

In `open_connection`'s big `execute_batch` SQL string, add `provider TEXT NOT NULL DEFAULT 'google',` as a new column (position: right after `scope`/`return_to`) to `authorization_requests`, `authorization_codes`, `refresh_tokens`, and `browser_login_states`. Example for `authorization_requests` (apply the identical pattern to the other three):

```sql
        CREATE TABLE IF NOT EXISTS authorization_requests (
            state TEXT PRIMARY KEY,
            client_id TEXT NOT NULL,
            redirect_uri TEXT NOT NULL,
            client_state TEXT NOT NULL,
            resource TEXT NOT NULL DEFAULT '',
            scope TEXT NOT NULL,
            provider TEXT NOT NULL DEFAULT 'google',
            provider_code_verifier TEXT NOT NULL,
            code_challenge TEXT NOT NULL,
            code_challenge_method TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            expires_at INTEGER NOT NULL
        );
```

- [ ] **Step 3: Add `add_column_if_missing` calls for pre-existing databases**

These calls are unconditional (not gated behind the `run_migrations`/`PRAGMA user_version` path used elsewhere in this file for the `refresh_token_hash` and `dynamic_client_id` migrations) because — like the pre-existing `resource` column — `provider` needs only a static `DEFAULT 'google'`, no Rust-side computed backfill. The version-gated path exists specifically for migrations that need to run Rust logic per-row (e.g. `refresh_token_hash`'s SHA-256 backfill); a static default doesn't need that machinery. `open_connection()` runs this at most 4 times total, once per pooled connection at process startup (`SQLITE_POOL_SIZE = 4`) — not per-request, not per connection-checkout — so the added `PRAGMA table_info` scans are one-time startup cost, not an ongoing one.

Right after the three existing `add_column_if_missing(&conn, "...", "resource", "TEXT NOT NULL DEFAULT ''")?;` calls (immediately following the big `execute_batch` call in `open_connection`), add:

```rust
    add_column_if_missing(
        &conn,
        "authorization_requests",
        "provider",
        "TEXT NOT NULL DEFAULT 'google'",
    )?;
    add_column_if_missing(
        &conn,
        "authorization_codes",
        "provider",
        "TEXT NOT NULL DEFAULT 'google'",
    )?;
    add_column_if_missing(
        &conn,
        "refresh_tokens",
        "provider",
        "TEXT NOT NULL DEFAULT 'google'",
    )?;
    add_column_if_missing(
        &conn,
        "browser_login_states",
        "provider",
        "TEXT NOT NULL DEFAULT 'google'",
    )?;
```

- [ ] **Step 4: Update every INSERT / SELECT / RETURNING statement and row-mapping closure that touches these four tables**

For each of the following methods, add `provider` to the column list, the `VALUES (...)` placeholder list (renumbering every placeholder after it), the `params![...]` list, and (for reads) the row-mapping closure's field list. Apply this exact pattern to each — the column position in SQL doesn't need to match the Rust struct field order, so append `provider` as the LAST column in every statement to minimize renumbering:

  - `insert_authorization_request` — add `, provider` to the column list and `, ?12` as the last placeholder (was `?1..?11`), add `request.provider,` as the last param.
  - `take_authorization_request` — add `, provider` to the `RETURNING` clause (last column), and in `row_to_authorization_request`, add `provider: row.get(11)?,` (columns 0–10 already used; `resource` is fetched from index 10 per the existing code at `types.rs`/`sqlite.rs`'s `row_to_authorization_request`, so `provider` becomes index 11 — the new RETURNING column added last).
  - `insert_auth_code` — same pattern: append `, provider` to the column list, `, ?12` placeholder, `code.provider,` param.
  - `redeem_auth_code` — append `, provider` to `RETURNING`; in `row_to_authorization_code`, add `provider: row.get(11)?,` (existing code's `resource` is index 10, so `provider` is index 11).
  - `upsert_refresh_token` — append `, provider` to the column list, `, ?9` placeholder (existing statement has 8 placeholders `?1..?8`), `token.provider,` param, AND add `provider = excluded.provider,` to the `ON CONFLICT ... DO UPDATE SET` clause.
  - `rotate_refresh_token` — the `INSERT INTO refresh_tokens` inside this method needs the same treatment: append `, provider` / `, ?9` / `new_token.provider,`.
  - `find_refresh_token` — append `provider,` to the `SELECT` column list (position doesn't matter, put it last) and add `provider: row.get(7)?,` to the closure (existing closure reads indices 0–6 for `client_id, subject, scope, provider_refresh_token, created_at, expires_at`, plus `resource` at index 6 via `.unwrap_or_default()` — re-verify exact indices against the live file with `cargo check` in Step 5 rather than trusting this description blindly, since off-by-one here fails silently at the SQL layer, not at compile time).
  - `insert_browser_login_state` — append `, provider` to the column list, `, ?6` placeholder (existing has `?1..?5`), `login.provider,` param.
  - `take_browser_login_state` — append `, provider` to `RETURNING`; in `row_to_browser_login_state`, add `provider: row.get(5)?,` (existing closure reads indices 0–4).
  - **Add a new method**, `has_any_refresh_token_for_provider`, right next to the existing `has_any_refresh_token`:

    ```rust
    /// Same as [`Self::has_any_refresh_token`], scoped to one provider.
    ///
    /// `authorize()` (Task 11) uses this instead of the unscoped version to
    /// decide whether to force the upstream consent screen — the unscoped
    /// version incorrectly treats "Google already has a refresh token on
    /// file" as a reason to skip forced consent on a user's very first
    /// Authelia or GitHub login, silently degrading that new provider's
    /// first session to no local refresh token.
    pub async fn has_any_refresh_token_for_provider(
        &self,
        provider: &str,
    ) -> Result<bool, AuthError> {
        let provider = provider.to_string();
        let now = now_unix();
        self.with_conn(move |conn| {
            conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM refresh_tokens WHERE provider = ?1 AND expires_at > ?2)",
                params![provider, now],
                |row| row.get::<_, i64>(0),
            )
            .map(|count| count != 0)
            .map_err(sqlite_error)
        })
        .await
    }
    ```

  Each of these edits is inherently mechanical-but-precise (SQL column-index bugs are silent, not compile errors). Given engineering review flagged this exact risk, work through the nine statement edits above **one table at a time, not as one 9-statement batch**: after finishing all edits for `authorization_requests` (the `insert_authorization_request`/`take_authorization_request` pair), run `cargo check -p soma-auth --lib` before moving to `authorization_codes`, then `refresh_tokens`, then `browser_login_states`. A compile error at this stage only proves the Rust side type-checks — it does NOT catch a column-index-off-by-one (see Task 11 Step 8's new round-trip tests, which are what actually prove the SQL wiring is correct end to end).

- [ ] **Step 5: Fix every remaining struct-literal construction of these four types inside `sqlite.rs`'s own test module**

Run: `cargo check -p soma-auth 2>&1 | grep "missing field \`provider\`"` and fix every reported call site in `sqlite.rs`'s `#[cfg(test)] mod tests` (helpers like `sample_code()` and any inline `RefreshTokenRow { ... }` / `BrowserLoginStateRow { ... }` literals used only within this file) by adding `provider: "google".to_string(),`. Re-run until the grep returns nothing for `sqlite.rs`-reported lines (lines reported in `authorize.rs`/`token.rs` are handled in Tasks 10/11 — do not fix those here, just confirm they still exist as expected compile errors at this point).

- [ ] **Step 6: Run sqlite tests**

Run: `cargo test -p soma-auth sqlite::`
Expected: PASS. (The crate as a whole will NOT yet compile — `authorize.rs`/`token.rs` still have unfixed `AuthorizationRequestRow`/etc. literals missing `provider` — that's expected and resolved in Tasks 10–11. `cargo test -p soma-auth sqlite::` alone will fail to build too since it's the same crate; instead confirm progress with `cargo check -p soma-auth --tests 2>&1 | grep -c "missing field \`provider\`"` and note the count drops to only `authorize.rs`/`token.rs` sites.)

- [ ] **Step 7: Commit**

```bash
git add crates/shared/auth/src/types.rs crates/shared/auth/src/sqlite.rs
git commit -m "feat(soma-auth): add provider column to in-flight and refresh-token tables"
```

---

### Task 9: `state.rs` — multi-provider `AuthState`

**Files:**
- Modify: `crates/shared/auth/src/state.rs`

**Interfaces:**
- Consumes: `crate::oauth_provider::OAuthProvider`, `crate::authelia::AutheliaProvider`, `crate::github::GitHubProvider`, `crate::google::GoogleProvider` (all existing/new from Tasks 4–6), `crate::config::{AuthConfig, AutheliaConfig, GitHubConfig, GoogleConfig}`.
- Produces: `AuthState.providers: Arc<BTreeMap<String, Arc<dyn OAuthProvider>>>`, `AuthState.default_provider: String`, `pub fn provider(&self, id: &str) -> Result<Arc<dyn OAuthProvider>, AuthError>`, `pub fn provider_or_default(&self, id: Option<&str>) -> Result<Arc<dyn OAuthProvider>, AuthError>`. Consumed by Task 10 (`routes.rs`), Task 11 (`authorize.rs`), Task 12 (`token.rs`).

- [ ] **Step 1: Replace the `google` field and imports**

Replace:

```rust
use crate::google::GoogleProvider;
```

with:

```rust
use std::collections::BTreeMap;

use crate::authelia::AutheliaProvider;
use crate::github::GitHubProvider;
use crate::google::GoogleProvider;
use crate::oauth_provider::OAuthProvider;
```

(`std::collections::BTreeMap` may already be imported for `allowed_resource_scopes` — check the existing `use std::collections::{BTreeMap, BTreeSet};` line at the top of the file and merge rather than duplicating the import.)

Also change the existing `use tracing::{debug, info};` line to `use tracing::{debug, info, warn};` — Step 2 below adds a `warn!` call.

Replace the field:

```rust
    pub google: Arc<GoogleProvider>,
```

with:

```rust
    pub providers: Arc<BTreeMap<String, Arc<dyn OAuthProvider>>>,
    pub default_provider: String,
```

- [ ] **Step 2: Replace provider construction in `AuthState::new`**

Replace:

```rust
        let redirect_uri = build_google_redirect_uri(&public_url, &config.google.callback_path);
        let store = SqliteStore::open(config.sqlite_path.clone()).await?;
        let signing_keys = SigningKeys::load_or_create(&config.key_path)?;
        let mut google = GoogleProvider::new(
            config.google.client_id.clone(),
            config.google.client_secret.clone(),
            redirect_uri,
        )?;
        google.scopes.clone_from(&config.google.scopes);
        info!(
            crate_name = "lab-auth",
            env_prefix = %config.env_prefix,
            auth_mode = "oauth",
            public_url = %public_url,
            google_redirect_uri = %google.redirect_uri,
            sqlite_path = %config.sqlite_path.display(),
            key_path = %config.key_path.display(),
            google_scopes = ?config.google.scopes,
            "auth state initialized"
        );

        let authorize_limiter = PerIpRateLimiter::new(config.authorize_requests_per_minute);
        let register_limiter = PerIpRateLimiter::new(config.register_requests_per_minute);
        Ok(Self {
            config: Arc::new(config),
            store,
            signing_keys: Arc::new(signing_keys),
            google: Arc::new(google),
            allowed_resource_scopes: Arc::new(RwLock::new(BTreeMap::new())),
            authorize_limiter,
            register_limiter,
            #[cfg(feature = "http-axum")]
            cimd_cache: Arc::new(crate::cimd::document::DocumentCache::new()),
        })
```

with:

```rust
        let store = SqliteStore::open(config.sqlite_path.clone()).await?;
        let signing_keys = SigningKeys::load_or_create(&config.key_path)?;
        let providers = build_providers(&public_url, &config)?;
        if !providers.contains_key(&config.default_provider) {
            return Err(AuthError::Config(format!(
                "{prefix}_AUTH_DEFAULT_PROVIDER `{provider}` is not a configured provider",
                prefix = config.env_prefix,
                provider = config.default_provider,
            )));
        }
        info!(
            crate_name = "soma-auth",
            env_prefix = %config.env_prefix,
            auth_mode = "oauth",
            public_url = %public_url,
            configured_providers = ?providers.keys().collect::<Vec<_>>(),
            default_provider = %config.default_provider,
            sqlite_path = %config.sqlite_path.display(),
            key_path = %config.key_path.display(),
            "auth state initialized"
        );
        // Security posture note (see this plan's Global Constraints): the
        // email allowlist is a single flat list shared across every
        // configured provider, and being on it grants full admin scope
        // regardless of which provider authenticated the user. With 2+
        // providers configured, the deployment's effective admin-gate
        // strength is that of its weakest provider's identity-verification
        // signal (GitHub's non-re-verified "primary && verified" email flag
        // is weaker than Google/Authelia's live per-login ID-token claim).
        // `admin_email` is always non-empty in OAuth mode (enforced by
        // `AuthConfig::validate`), so this warning fires on every startup
        // where it's relevant — never silently.
        if providers.len() > 1 {
            warn!(
                crate_name = "soma-auth",
                env_prefix = %config.env_prefix,
                configured_providers = ?providers.keys().collect::<Vec<_>>(),
                "multiple OAuth providers configured — the email allowlist is shared across all \
                 of them, so admin access is only as strong as the weakest configured provider's \
                 identity verification; see docs/AUTH.md"
            );
        }

        let authorize_limiter = PerIpRateLimiter::new(config.authorize_requests_per_minute);
        let register_limiter = PerIpRateLimiter::new(config.register_requests_per_minute);
        let default_provider = config.default_provider.clone();
        Ok(Self {
            config: Arc::new(config),
            store,
            signing_keys: Arc::new(signing_keys),
            providers: Arc::new(providers),
            default_provider,
            allowed_resource_scopes: Arc::new(RwLock::new(BTreeMap::new())),
            authorize_limiter,
            register_limiter,
            #[cfg(feature = "http-axum")]
            cimd_cache: Arc::new(crate::cimd::document::DocumentCache::new()),
        })
```

- [ ] **Step 3: Add `build_providers` and `build_provider_redirect_uri` (renamed from `build_google_redirect_uri`) plus the two accessor methods**

Replace the existing `fn build_google_redirect_uri(...)` free function with a renamed, provider-agnostic version, and add `build_providers` right above it:

```rust
fn build_providers(
    public_url: &Url,
    config: &AuthConfig,
) -> Result<BTreeMap<String, Arc<dyn OAuthProvider>>, AuthError> {
    let mut providers: BTreeMap<String, Arc<dyn OAuthProvider>> = BTreeMap::new();

    if !config.google.client_id.is_empty() {
        let redirect_uri = build_provider_redirect_uri(public_url, &config.google.callback_path);
        let mut google = GoogleProvider::new(
            config.google.client_id.clone(),
            config.google.client_secret.clone(),
            redirect_uri,
        )?;
        google.scopes.clone_from(&config.google.scopes);
        providers.insert("google".to_string(), Arc::new(google));
    }

    if !config.authelia.client_id.is_empty() {
        let issuer = config.authelia.issuer_url.clone().ok_or_else(|| {
            AuthError::Config(format!(
                "{}_AUTHELIA_ISSUER_URL is required when {}_AUTHELIA_CLIENT_ID is set",
                config.env_prefix, config.env_prefix
            ))
        })?;
        let redirect_uri = build_provider_redirect_uri(public_url, &config.authelia.callback_path);
        let mut authelia = AutheliaProvider::new(
            issuer,
            config.authelia.client_id.clone(),
            config.authelia.client_secret.clone(),
            redirect_uri,
        )?;
        authelia.scopes.clone_from(&config.authelia.scopes);
        providers.insert("authelia".to_string(), Arc::new(authelia));
    }

    if !config.github.client_id.is_empty() {
        let redirect_uri = build_provider_redirect_uri(public_url, &config.github.callback_path);
        let mut github = GitHubProvider::new(
            config.github.client_id.clone(),
            config.github.client_secret.clone(),
            redirect_uri,
        )?;
        github.scopes.clone_from(&config.github.scopes);
        providers.insert("github".to_string(), Arc::new(github));
    }

    if providers.is_empty() {
        return Err(AuthError::Config(format!(
            "at least one OAuth provider must be configured when {}_AUTH_MODE=oauth",
            config.env_prefix
        )));
    }

    Ok(providers)
}

fn build_provider_redirect_uri(public_url: &Url, callback_path: &str) -> Url {
    let mut redirect_uri = public_url.clone();
    let base_path = redirect_uri.path().trim_end_matches('/');
    let callback_path = callback_path.trim_start_matches('/');
    let next_path = if base_path.is_empty() {
        format!("/{callback_path}")
    } else {
        format!("{base_path}/{callback_path}")
    };

    redirect_uri.set_path(&next_path);
    redirect_uri.set_query(None);
    redirect_uri.set_fragment(None);
    redirect_uri
}
```

Add these two accessor methods to `impl AuthState` (anywhere among the other `pub fn`/`pub async fn` methods, e.g. right after `ensure_pending_oauth_state_capacity`):

```rust
    /// Look up a specific configured provider by id. Returns
    /// [`AuthError::Validation`] if `id` does not name a configured
    /// provider — this is a request-shaped error (bad `?provider=` query
    /// param, or a stale DB row naming a provider that has since been
    /// unconfigured), not a server fault.
    pub fn provider(&self, id: &str) -> Result<Arc<dyn OAuthProvider>, AuthError> {
        self.providers
            .get(id)
            .cloned()
            .ok_or_else(|| AuthError::Validation(format!("unknown oauth provider `{id}`")))
    }

    /// [`Self::provider`], falling back to [`Self::default_provider`] when
    /// `id` is `None`.
    pub fn provider_or_default(&self, id: Option<&str>) -> Result<Arc<dyn OAuthProvider>, AuthError> {
        self.provider(id.unwrap_or(self.default_provider.as_str()))
    }
```

- [ ] **Step 4: Replace `for_tests` and add the test-only `google_only_providers` helper**

Replace:

```rust
    #[cfg(test)]
    pub fn for_tests(
        config: AuthConfig,
        store: SqliteStore,
        signing_keys: SigningKeys,
        google: GoogleProvider,
    ) -> Self {
        let authorize_limiter = PerIpRateLimiter::new(config.authorize_requests_per_minute);
        let register_limiter = PerIpRateLimiter::new(config.register_requests_per_minute);
        Self {
            config: Arc::new(config),
            store,
            signing_keys: Arc::new(signing_keys),
            google: Arc::new(google),
            allowed_resource_scopes: Arc::new(RwLock::new(BTreeMap::new())),
            authorize_limiter,
            register_limiter,
            #[cfg(feature = "http-axum")]
            cimd_cache: Arc::new(crate::cimd::document::DocumentCache::new()),
        }
    }
```

with:

```rust
    #[cfg(test)]
    pub fn for_tests(
        config: AuthConfig,
        store: SqliteStore,
        signing_keys: SigningKeys,
        providers: BTreeMap<String, Arc<dyn OAuthProvider>>,
    ) -> Self {
        let authorize_limiter = PerIpRateLimiter::new(config.authorize_requests_per_minute);
        let register_limiter = PerIpRateLimiter::new(config.register_requests_per_minute);
        let default_provider = config.default_provider.clone();
        Self {
            config: Arc::new(config),
            store,
            signing_keys: Arc::new(signing_keys),
            providers: Arc::new(providers),
            default_provider,
            allowed_resource_scopes: Arc::new(RwLock::new(BTreeMap::new())),
            authorize_limiter,
            register_limiter,
            #[cfg(feature = "http-axum")]
            cimd_cache: Arc::new(crate::cimd::document::DocumentCache::new()),
        }
    }
```

Add a small helper right after `for_tests` so the 3 existing call sites in `authorize.rs`/`token.rs` (Tasks 10/11) only need to wrap their `google` value in one extra function call rather than hand-build a `BTreeMap`:

```rust
    #[cfg(test)]
    pub fn google_only_providers(google: GoogleProvider) -> BTreeMap<String, Arc<dyn OAuthProvider>> {
        let mut providers: BTreeMap<String, Arc<dyn OAuthProvider>> = BTreeMap::new();
        providers.insert("google".to_string(), Arc::new(google));
        providers
    }
```

- [ ] **Step 5: Fix `state.rs`'s own test module**

Its two tests (`resolve_state` and `auth_state_preserves_public_url_path_prefix_in_google_redirect_uri`) both build a full `AuthConfig` and call `AuthState::new(...).await` — they don't call `for_tests` directly, so they need no field-level changes EXCEPT `default_provider` will now resolve automatically to `"google"` (since only `config.google` is set) via `build_providers`/the config-layer default-resolution added in Task 7 Step 4 — but these two tests construct `AuthConfig { ... }` literals DIRECTLY (not through `AuthConfigBuilder::build_from_sources`), so `default_provider` won't be auto-resolved; it'll be whatever the literal sets. Add `default_provider: "google".to_string(),` and `authelia: AutheliaConfig::default(), github: GitHubConfig::default(),` to both `AuthConfig { ... }` literals in this test module (they currently rely on `..AuthConfig::default()` for most fields already — check whether that spread already covers `authelia`/`github`/`default_provider`; since `AuthConfig::default()` now sets `default_provider: String::new()` per Task 7 Step 2, an explicit override to `"google".to_string()` is required in both tests, but `authelia`/`github` don't need explicit overrides since their `Default` impls are empty/unconfigured and the spread already provides them).

Update the test module's import line `use crate::config::GoogleConfig;` to also import what's needed: `use crate::config::GoogleConfig;` is unaffected since `AutheliaConfig`/`GitHubConfig` are only needed if referenced by name — since you're relying on `..AuthConfig::default()` for those two, no new import is required, only the `default_provider: "google".to_string(),` field addition to each of the two `AuthConfig { ... }` literals.

- [ ] **Step 6: Build check**

Run: `cargo check -p soma-auth --lib`
Expected: `state.rs` and its own test module compile. `authorize.rs`/`token.rs` still fail (Tasks 10–11 fix them) — confirm the remaining errors are ONLY in those two files: `cargo check -p soma-auth --tests 2>&1 | grep "^error" | grep -v "authorize.rs\|token.rs"` should print nothing.

- [ ] **Step 7: Commit**

```bash
git add crates/shared/auth/src/state.rs
git commit -m "feat(soma-auth): replace AuthState.google with a multi-provider map"
```

---

### Task 10: `routes.rs` — mount one callback route per configured provider

**Files:**
- Modify: `crates/shared/auth/src/routes.rs`

**Interfaces:**
- Consumes: `AuthState.providers` (Task 9), `OAuthProvider::callback_path()` (Task 2).

- [ ] **Step 1: Replace the hardcoded `/auth/google/callback` route in `router()`**

Replace:

```rust
pub fn router(state: AuthState) -> Router {
    let enable_registration = state.config.enable_dynamic_registration;
    let mut app = Router::new()
        .route(
            "/.well-known/oauth-authorization-server",
            get(authorization_server_metadata),
        )
        .route(
            "/.well-known/oauth-authorization-server/{*route}",
            get(authorization_server_metadata),
        )
        .route(
            "/.well-known/oauth-protected-resource",
            get(protected_resource_metadata),
        )
        .route("/jwks", get(jwks))
        .route("/authorize", get(authorize))
        .route("/auth/login", get(browser_login))
        .route("/auth/google/callback", get(callback))
        .route("/native/callback", get(native_callback))
        .route("/native/poll", get(native_poll))
        .route("/token", post(token));
    if enable_registration {
        app = app.route("/register", post(register_client));
    }
    app.with_state(state)
        .layer(middleware::from_fn(auth_dispatch_observability))
}
```

with:

```rust
pub fn router(state: AuthState) -> Router {
    let enable_registration = state.config.enable_dynamic_registration;
    let mut app = Router::new()
        .route(
            "/.well-known/oauth-authorization-server",
            get(authorization_server_metadata),
        )
        .route(
            "/.well-known/oauth-authorization-server/{*route}",
            get(authorization_server_metadata),
        )
        .route(
            "/.well-known/oauth-protected-resource",
            get(protected_resource_metadata),
        )
        .route("/jwks", get(jwks))
        .route("/authorize", get(authorize))
        .route("/auth/login", get(browser_login))
        .route("/native/callback", get(native_callback))
        .route("/native/poll", get(native_poll))
        .route("/token", post(token));
    for callback_path in callback_paths(&state) {
        app = app.route(&callback_path, get(callback));
    }
    if enable_registration {
        app = app.route("/register", post(register_client));
    }
    app.with_state(state)
        .layer(middleware::from_fn(auth_dispatch_observability))
}

/// Every configured provider's callback path, e.g.
/// `["/auth/google/callback"]` for a Google-only deployment or
/// `["/auth/authelia/callback", "/auth/github/callback", "/auth/google/callback"]`
/// once all three are configured. The `callback` handler itself is
/// provider-agnostic — see `authorize::callback` doc comment — so mounting
/// it at N distinct static paths is purely about matching each upstream
/// OAuth app's registered `redirect_uri`.
fn callback_paths(state: &AuthState) -> Vec<String> {
    state
        .providers
        .values()
        .map(|provider| provider.callback_path().to_string())
        .collect()
}
```

- [ ] **Step 2: Apply the identical change to `bearer_only_router`**

Replace:

```rust
pub fn bearer_only_router(state: AuthState) -> Router {
    Router::new()
        .route(
            "/.well-known/oauth-authorization-server",
            get(authorization_server_metadata),
        )
        .route(
            "/.well-known/oauth-authorization-server/{*route}",
            get(authorization_server_metadata),
        )
        .route(
            "/.well-known/oauth-protected-resource",
            get(protected_resource_metadata),
        )
        .route("/jwks", get(jwks))
        .route("/authorize", get(authorize))
        .route("/auth/google/callback", get(callback))
        .route("/token", post(token))
        .with_state(state)
        .layer(middleware::from_fn(auth_dispatch_observability))
}
```

with:

```rust
pub fn bearer_only_router(state: AuthState) -> Router {
    let mut app = Router::new()
        .route(
            "/.well-known/oauth-authorization-server",
            get(authorization_server_metadata),
        )
        .route(
            "/.well-known/oauth-authorization-server/{*route}",
            get(authorization_server_metadata),
        )
        .route(
            "/.well-known/oauth-protected-resource",
            get(protected_resource_metadata),
        )
        .route("/jwks", get(jwks))
        .route("/authorize", get(authorize))
        .route("/token", post(token));
    for callback_path in callback_paths(&state) {
        app = app.route(&callback_path, get(callback));
    }
    app.with_state(state)
        .layer(middleware::from_fn(auth_dispatch_observability))
}
```

- [ ] **Step 3: Leave `BEARER_ONLY_ROUTER_PATHS` / `BEARER_ONLY_ROUTER_FORBIDDEN_PATHS` and `auth_dispatch_action` unchanged**

These are static tables checked by the pinned-snapshot test (`bearer_only_router_route_list_matches_pinned_snapshot`), which uses `test_auth_state()` — that fixture only configures Google (Task 7 Step 5's test file is unaffected; `authorize.rs`'s `test_auth_config()` still only sets `google`), so `/auth/google/callback` remains the only mounted callback path in that test's context, and the snapshot passes with no edits. `auth_dispatch_action`'s `"/auth/google/callback" => "oauth.callback"` match arm also needs no change — for a deployment with Authelia/GitHub also configured, requests to `/auth/authelia/callback` or `/auth/github/callback` will fall through this `match` to the `_ => "oauth.unknown"` arm, which only affects a log field label, not behavior. This is an acceptable, non-blocking gap for this plan (logging label freshness) — do not expand the match arm speculatively for paths that don't exist by default.

- [ ] **Step 4: Build and test**

Run: `cargo check -p soma-auth --lib`
Expected: `routes.rs` compiles (crate as a whole still blocked on Task 11's `authorize.rs`/`token.rs` fixes).

- [ ] **Step 5: Commit**

```bash
git add crates/shared/auth/src/routes.rs
git commit -m "feat(soma-auth): mount one callback route per configured provider"
```

---

### Task 11: `authorize.rs` — provider selection, login picker, provider-agnostic callback

**Files:**
- Modify: `crates/shared/auth/src/authorize.rs`

**Interfaces:**
- Consumes: `AuthState.provider()`/`provider_or_default()` (Task 9), `crate::oauth_provider::{AuthorizeUrlRequest, namespaced_subject}` (Task 2), `AuthorizationRequestRow.provider`/`BrowserLoginStateRow.provider`/`AuthorizationCodeRow.provider` (Task 8).

- [ ] **Step 1: Update imports**

Replace:

```rust
use crate::google::AuthorizeUrlRequest;
```

with:

```rust
use crate::oauth_provider::{AuthorizeUrlRequest, namespaced_subject};
```

- [ ] **Step 2: Add `provider: Option<String>` to `BrowserLoginQuery` and `AuthorizeQuery`**

These live in `types.rs`, not `authorize.rs` — add as part of this task since they're used exclusively by the handlers in this file:

```rust
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrowserLoginQuery {
    #[serde(default)]
    pub return_to: Option<String>,
    #[serde(default)]
    pub provider: Option<String>,
}
```

```rust
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthorizeQuery {
    #[serde(default)]
    pub response_type: String,
    pub client_id: String,
    pub redirect_uri: String,
    pub state: String,
    #[serde(default)]
    pub resource: Option<String>,
    #[serde(default)]
    pub scope: String,
    #[serde(default)]
    pub provider: Option<String>,
    pub code_challenge: String,
    pub code_challenge_method: String,
}
```

- [ ] **Step 3: Rewrite `browser_login` to pick a provider (or render a picker)**

Replace the whole `browser_login` function body with:

```rust
pub async fn browser_login(
    State(state): State<AuthState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Query(query): Query<BrowserLoginQuery>,
) -> Result<Response, AuthError> {
    state.check_authorize_rate_limit(remote_ip(addr)).await?;
    let return_to = sanitize_return_to(&state, query.return_to.as_deref());

    let provider = match query.provider.as_deref() {
        Some(id) => state.provider(id)?,
        None if state.providers.len() > 1 => {
            return Ok(render_provider_picker(&state, &return_to));
        }
        None => state.provider_or_default(None)?,
    };

    state.ensure_pending_oauth_state_capacity().await?;
    let provider_code_verifier = random_token(32)?;
    let provider_code_challenge =
        URL_SAFE_NO_PAD.encode(Sha256::digest(provider_code_verifier.as_bytes()));
    let request_state = random_token(24)?;
    let oauth_state_id = fingerprint(&request_state);

    state
        .store
        .insert_browser_login_state(BrowserLoginStateRow {
            state: request_state.clone(),
            return_to: return_to.clone(),
            provider: provider.provider_id().to_string(),
            provider_code_verifier,
            created_at: now_unix(),
            expires_at: now_unix() + AUTH_REQUEST_TTL_SECS,
        })
        .await?;

    let location = provider.authorize_url(&AuthorizeUrlRequest {
        state: request_state,
        scope: state.config.default_scope.clone(),
        code_challenge: provider_code_challenge,
        code_challenge_method: "S256".to_string(),
        force_consent: true,
    })?;
    info!(
        oauth_state_id = %oauth_state_id,
        return_to = %return_to,
        provider = provider.provider_id(),
        "browser login redirected to upstream provider"
    );

    Ok((
        StatusCode::FOUND,
        [(header::LOCATION, location.to_string())],
    )
        .into_response())
}

/// Plain HTML provider-choice page shown by `browser_login` when the
/// deployment has more than one provider configured and the request did
/// not already say which one to use.
///
/// `return_to` is percent-encoded via `form_urlencoded` before
/// interpolation. Verified against `url::form_urlencoded::byte_serialize`'s
/// actual WHATWG `application/x-www-form-urlencoded` behavior: unreserved
/// bytes are alphanumeric plus `*-._`, space becomes `+`, everything else
/// (including `~`, which is NOT in the unreserved set for this encoder) is
/// percent-encoded — so the real output charset is `[A-Za-z0-9*\-._+%]`.
/// None of those characters can break out of a double-quoted HTML attribute,
/// so no separate HTML-escaping step is needed for `return_to_encoded`.
///
/// `provider_id`/`provider_label(...)` are interpolated as plain text (not
/// inside a percent-encoded query value), so they go through `html_escape`
/// as defense-in-depth even though every value that reaches this function
/// today is a hardcoded `&'static str` from a closed match — nothing about
/// the escaping costs anything, and it removes "provider names are always
/// compile-time literals" as a load-bearing invariant for this function's
/// XSS-safety.
fn render_provider_picker(state: &AuthState, return_to: &str) -> Response {
    let return_to_encoded: String =
        url::form_urlencoded::byte_serialize(return_to.as_bytes()).collect();
    let login_path = &state.config.login_path;
    let links: String = state
        .providers
        .values()
        .map(|provider| {
            let id = html_escape(provider.provider_id());
            let label = html_escape(provider_label(provider.provider_id()));
            format!(
                r#"<li><a href="{login_path}?provider={id}&return_to={return_to_encoded}">Sign in with {label}</a></li>"#
            )
        })
        .collect();
    let body = format!(
        r#"<!doctype html><html><body style="font-family:sans-serif;background:#07131c;color:#e6f4fb;text-align:center;padding-top:4rem"><h2>Sign in</h2><ul style="list-style:none;padding:0;font-size:1.1rem;line-height:2.5">{links}</ul></body></html>"#
    );
    let mut response = axum::response::Html(body).into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

fn provider_label(provider_id: &str) -> &'static str {
    match provider_id {
        "google" => "Google",
        "authelia" => "Authelia",
        "github" => "GitHub",
        _ => "your identity provider",
    }
}

fn html_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
```

- [ ] **Step 4: Rewrite `authorize` to resolve and store a provider**

Replace the block from `let provider_code_verifier = random_token(32)?;` (roughly 20 lines before `let location = state.google.authorize_url(...)`) through the end of the `insert_authorization_request` call and the `state.google.authorize_url(...)` call with:

```rust
    let provider = state.provider_or_default(query.provider.as_deref())?;
    let provider_code_verifier = random_token(32)?;
    let provider_code_challenge =
        URL_SAFE_NO_PAD.encode(Sha256::digest(provider_code_verifier.as_bytes()));
    let request_state = random_token(24)?;
    let oauth_state_id = fingerprint(&request_state);

    state
        .store
        .insert_authorization_request(AuthorizationRequestRow {
            state: request_state.clone(),
            client_id: query.client_id.clone(),
            redirect_uri: query.redirect_uri.clone(),
            client_state: query.state.clone(),
            resource: resource.clone(),
            scope: scope.clone(),
            provider: provider.provider_id().to_string(),
            provider_code_verifier,
            code_challenge: query.code_challenge.clone(),
            code_challenge_method: query.code_challenge_method.clone(),
            created_at: now_unix(),
            expires_at: now_unix() + AUTH_REQUEST_TTL_SECS,
        })
        .await?;

    // We don't know which upstream subject is about to sign in until they
    // come back from the consent screen, so use "has this gateway ever
    // minted a refresh token for THIS provider before" as a single-tenant
    // proxy for "already granted." Scoped per-provider (not global) —
    // otherwise a deployment where Google already has refresh tokens on file
    // would skip forced consent on a user's very first Authelia or GitHub
    // login, silently degrading that new provider's first session to no
    // local refresh token (caught in engineering review; see Task 8's
    // `has_any_refresh_token_for_provider`). Forcing full re-consent on
    // every DCR client attempt (Raycast, Warp, etc.) adds an interactive
    // round trip long enough for impatient clients to time out and retry
    // before the human finishes clicking through it.
    let force_consent = !state
        .store
        .has_any_refresh_token_for_provider(provider.provider_id())
        .await?;
    let location = provider.authorize_url(&AuthorizeUrlRequest {
        state: request_state,
        scope: scope.clone(),
        code_challenge: provider_code_challenge,
        code_challenge_method: "S256".to_string(),
        force_consent,
    })?;
    info!(
        client_id = %query.client_id,
        redirect_uri = %query.redirect_uri,
        client_state_id = %client_state_id,
        oauth_state_id = %oauth_state_id,
        resource = %resource,
        scope = %scope,
        provider = provider.provider_id(),
        "oauth authorize request redirected to upstream provider"
    );
```

(The `ensure_pending_oauth_state_capacity` call, the `code_challenge_method != "S256"` validation, and the subsequent `debug!`/`Ok((StatusCode::FOUND, ...))` tail are unchanged — only the middle section that touched `state.google` changes.)

- [ ] **Step 5: Rewrite `callback` to be provider-agnostic**

Replace the two `let google = state.google.exchange_code(...)` blocks and their surrounding logic. First occurrence (browser-login branch):

```rust
    if let Some(login) = state.store.take_browser_login_state(&query.state).await? {
        let provider = state.provider(&login.provider)?;
        let exchange = provider
            .exchange_code(&query.code, &login.provider_code_verifier)
            .await?;
        let allowed = state.resolve_allowed_emails().await?;
        check_email_allowlist(exchange.email.as_deref(), exchange.email_verified, &allowed)?;
        let subject = namespaced_subject(provider.provider_id(), &exchange.subject);
        let session = create_browser_session(&state, subject, exchange.email).await?;
        let mut response = Redirect::to(&login.return_to).into_response();
        append_set_cookie(
            &mut response,
            &build_browser_session_cookie(&state, &session.session_id),
        );
        info!(
            oauth_state_id = %oauth_state_id,
            return_to = %login.return_to,
            subject_id = %fingerprint(&session.subject),
            provider = provider.provider_id(),
            "browser login callback issued session cookie"
        );
        return Ok(response);
    }
```

Second occurrence (MCP `/authorize` flow branch) — replace:

```rust
    let google = state
        .google
        .exchange_code(&query.code, &request.provider_code_verifier)
        .await?;
```

with:

```rust
    let provider = state.provider(&request.provider)?;
    let exchange = provider
        .exchange_code(&query.code, &request.provider_code_verifier)
        .await?;
```

Then every subsequent reference to `google.email`, `google.email_verified`, `google.subject`, `google.refresh_token` in the rest of `callback` (the `check_email_allowlist` call, the `subject_id` log field, the `insert_auth_code` call) becomes `exchange.email`, `exchange.email_verified`, a namespaced `subject` local, and `exchange.refresh_token` respectively. Replace:

```rust
    let allowed = state.resolve_allowed_emails().await?;
    if let Err(denial) =
        check_email_allowlist(google.email.as_deref(), google.email_verified, &allowed)
    {
```

with:

```rust
    let allowed = state.resolve_allowed_emails().await?;
    if let Err(denial) =
        check_email_allowlist(exchange.email.as_deref(), exchange.email_verified, &allowed)
    {
```

Replace:

```rust
    let subject_id = fingerprint(&google.subject);
    info!(
        client_id = %request.client_id,
        oauth_state_id = %oauth_state_id,
        subject_id = %subject_id,
        has_provider_refresh_token = google.refresh_token.is_some(),
        "oauth callback exchanged upstream code successfully"
    );
```

with:

```rust
    let subject = namespaced_subject(provider.provider_id(), &exchange.subject);
    let subject_id = fingerprint(&subject);
    info!(
        client_id = %request.client_id,
        oauth_state_id = %oauth_state_id,
        subject_id = %subject_id,
        provider = provider.provider_id(),
        has_provider_refresh_token = exchange.refresh_token.is_some(),
        "oauth callback exchanged upstream code successfully"
    );
```

Replace the `insert_auth_code` call's struct literal:

```rust
        .insert_auth_code(AuthorizationCodeRow {
            code: auth_code.clone(),
            client_id: request.client_id,
            subject: google.subject,
            redirect_uri: request.redirect_uri.clone(),
            resource: request.resource,
            scope: elevated_scope,
            code_challenge: request.code_challenge,
            code_challenge_method: request.code_challenge_method,
            provider_refresh_token: google.refresh_token,
            created_at: now_unix(),
            expires_at: expires_at(
                now_unix(),
                state.config.auth_code_ttl,
                &format!("{}_AUTH_CODE_TTL_SECS", state.config.env_prefix),
            )?,
        })
```

with:

```rust
        .insert_auth_code(AuthorizationCodeRow {
            code: auth_code.clone(),
            client_id: request.client_id,
            subject,
            redirect_uri: request.redirect_uri.clone(),
            resource: request.resource,
            scope: elevated_scope,
            provider: provider.provider_id().to_string(),
            code_challenge: request.code_challenge,
            code_challenge_method: request.code_challenge_method,
            provider_refresh_token: exchange.refresh_token,
            created_at: now_unix(),
            expires_at: expires_at(
                now_unix(),
                state.config.auth_code_ttl,
                &format!("{}_AUTH_CODE_TTL_SECS", state.config.env_prefix),
            )?,
        })
```

- [ ] **Step 6: Fix `check_email_allowlist`'s hardcoded "google" wording**

The function currently says `"google did not return a verified email address"` etc. Since this is now provider-agnostic, take a `provider_id: &str` parameter and interpolate it. Replace the function signature and its three `warn!`/error-message call sites:

```rust
fn check_email_allowlist(
    provider_id: &str,
    email: Option<&str>,
    email_verified: Option<bool>,
    allowed_emails: &[String],
) -> Result<(), AuthError> {
    if allowed_emails.is_empty() {
        return Ok(());
    }
    if email_verified != Some(true) {
        warn!(
            provider = provider_id,
            "oauth callback rejected: provider did not return a verified email address"
        );
        return Err(AuthError::AuthFailed(format!(
            "{provider_id} did not return a verified email address"
        )));
    }
    let Some(e) = email else {
        warn!(
            provider = provider_id,
            "oauth callback rejected: provider did not return an email address"
        );
        return Err(AuthError::AuthFailed(format!(
            "{provider_id} did not return an email address"
        )));
    };
    let trimmed = e.trim();
    if allowed_emails
        .iter()
        .any(|a| a.eq_ignore_ascii_case(trimmed))
    {
        return Ok(());
    }
    warn!(
        provider = provider_id,
        email_id = %fingerprint(trimmed),
        "oauth callback rejected: email not in allowed list"
    );
    Err(AuthError::AuthFailed(
        "account is not permitted to access this gateway".to_string(),
    ))
}
```

Update its 3 call sites (the one added in Step 5's browser-login branch, and the one in the MCP-authorize branch) to pass `provider.provider_id()` as the first argument.

- [ ] **Step 7: Fix the fixed test fixtures in `authorize.rs`'s own test module**

`test_auth_config()` needs `authelia: AutheliaConfig::default(), github: GitHubConfig::default(), default_provider: "google".to_string(),` added to its `AuthConfig { ... }` literal (it currently ends with `..AuthConfig::default()`, so only `default_provider` strictly needs an explicit override — same reasoning as Task 9 Step 5 — add the import `use crate::config::{AuthConfig, AuthMode, GoogleConfig};` stays as-is unless you reference `AutheliaConfig`/`GitHubConfig` by name, which you don't need to since the spread covers them).

For every `AuthorizationRequestRow { ... }` / `AuthorizationCodeRow { ... }` / `RefreshTokenRow { ... }` / `BrowserLoginStateRow { ... }` literal in this test module (the ones surfaced by `cargo check -p soma-auth --tests` in Task 8 Step 6), add `provider: "google".to_string(),`.

For the 2 `AuthState::for_tests(...)` call sites in this file (`test_auth_state_with_mock_google`, `test_auth_state_with_mock_google_native`), change the last argument from the bare `google` value to `AuthState::google_only_providers(google)`:

```rust
        AuthState::for_tests(
            (*state.config).clone(),
            state.store.clone(),
            (*state.signing_keys).clone(),
            AuthState::google_only_providers(google),
        )
```

- [ ] **Step 8: Add multi-provider coverage tests**

Append to `authorize.rs`'s test module:

```rust
    #[tokio::test]
    async fn browser_login_renders_a_picker_when_more_than_one_provider_is_configured() {
        let mut config = test_auth_config();
        config.github.client_id = "gh-client".to_string();
        config.github.client_secret = "gh-secret".to_string();
        let state = test_auth_state_with_config(config).await;
        let app = router(state);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/auth/login")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body = String::from_utf8_lossy(&body);
        assert!(body.contains("Sign in with Google"));
        assert!(body.contains("Sign in with GitHub"));
        assert!(body.contains("provider=google"));
        assert!(body.contains("provider=github"));
    }

    #[tokio::test]
    async fn browser_login_skips_the_picker_and_redirects_directly_when_provider_is_given() {
        let mut config = test_auth_config();
        config.github.client_id = "gh-client".to_string();
        config.github.client_secret = "gh-secret".to_string();
        let state = test_auth_state_with_config(config).await;
        let app = router(state);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/auth/login?provider=github")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FOUND);
        let location = response
            .headers()
            .get(header::LOCATION)
            .unwrap()
            .to_str()
            .unwrap();
        assert!(location.contains("github.com/login/oauth/authorize"));
    }

    #[tokio::test]
    async fn authorize_rejects_an_unknown_provider_query_param() {
        let app = router(test_auth_state_with_registered_client().await);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/authorize?response_type=code&client_id=client&redirect_uri=http://127.0.0.1:7777/callback&state=abc&scope=lab&code_challenge=pkce&code_challenge_method=S256&provider=okta")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    async fn authorize_scopes_force_consent_per_provider_not_globally() {
        // Regression test for the engineering-review finding: force_consent
        // must be scoped to the provider about to be used, not "has ANY
        // provider ever issued a refresh token." Seed a Google refresh
        // token, then confirm a fresh GitHub authorize() request still
        // forces consent even though a (Google) refresh token already
        // exists in the DB.
        let mut config = test_auth_config();
        config.github.client_id = "gh-client".to_string();
        config.github.client_secret = "gh-secret".to_string();
        let state = test_auth_state_with_config(config).await;
        state
            .store
            .register_client(RegisteredClient {
                client_id: "client".to_string(),
                redirect_uris: vec!["http://127.0.0.1:7777/callback".to_string()],
                created_at: now_unix(),
            })
            .await
            .unwrap();
        state
            .store
            .upsert_refresh_token(crate::types::RefreshTokenRow {
                refresh_token: "existing-google-refresh".to_string(),
                client_id: "client".to_string(),
                subject: "google-user".to_string(),
                resource: "https://lab.example.com/mcp".to_string(),
                scope: "lab".to_string(),
                provider: "google".to_string(),
                provider_refresh_token: None,
                created_at: now_unix(),
                expires_at: now_unix() + 3600,
            })
            .await
            .unwrap();
        let app = router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/authorize?response_type=code&client_id=client&redirect_uri=http://127.0.0.1:7777/callback&state=abc&scope=lab&code_challenge=pkce&code_challenge_method=S256&provider=github")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FOUND);
        let location = response
            .headers()
            .get(header::LOCATION)
            .unwrap()
            .to_str()
            .unwrap();
        assert!(location.contains("github.com"));
        assert!(
            location.contains("prompt=consent"),
            "GitHub's first-ever authorize call must still force consent even though Google \
             already has a refresh token on file: {location}"
        );
    }

    /// End-to-end round-trip proving Task 8's manually-renumbered SQL
    /// actually routes to the CORRECT provider's `exchange_code`, not just
    /// that the Rust side compiles. Wires two distinct mock upstream
    /// servers (one per provider) into the same `AuthState` and asserts
    /// only the provider named by `?provider=` at `/auth/login` time
    /// receives the token-exchange call.
    #[tokio::test]
    async fn browser_login_round_trip_calls_only_the_selected_providers_mock_server() {
        use crate::github::GitHubProvider;

        let google_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "access_token": "google-access-token",
                "refresh_token": "google-refresh-token",
                "expires_in": 3600,
                "id_token": signed_test_id_token(),
            })))
            .mount(&google_server)
            .await;
        Mock::given(method("GET"))
            .and(path("/certs"))
            .respond_with(ResponseTemplate::new(200).set_body_json(test_jwks()))
            .mount(&google_server)
            .await;

        let github_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/login/oauth/access_token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "access_token": "github-access-token",
                "scope": "read:user,user:email",
                "token_type": "bearer",
            })))
            .mount(&github_server)
            .await;
        Mock::given(method("GET"))
            .and(path("/user"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": 555, "login": "octocat", "email": "user@example.com",
            })))
            .mount(&github_server)
            .await;
        Mock::given(method("GET"))
            .and(path("/user/emails"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([
                {"email": "user@example.com", "primary": true, "verified": true},
            ])))
            .mount(&github_server)
            .await;

        let mut config = test_auth_config();
        config.github.client_id = "gh-client".to_string();
        config.github.client_secret = "gh-secret".to_string();
        let base_state = test_auth_state_with_config(config).await;

        let google = GoogleProvider::new(
            "client-id".to_string(),
            "client-secret".to_string(),
            Url::parse("https://lab.example.com/auth/google/callback").unwrap(),
        )
        .unwrap()
        .with_endpoints(
            google_server.uri().parse::<Url>().unwrap(),
            google_server.uri().parse::<Url>().unwrap().join("/token").unwrap(),
        )
        .with_jwks_endpoint(google_server.uri().parse::<Url>().unwrap().join("/certs").unwrap());
        let github = GitHubProvider::new(
            "gh-client".to_string(),
            "gh-secret".to_string(),
            Url::parse("https://lab.example.com/auth/github/callback").unwrap(),
        )
        .unwrap()
        .with_endpoints(
            github_server.uri().parse::<Url>().unwrap().join("login/oauth/authorize").unwrap(),
            github_server.uri().parse::<Url>().unwrap().join("login/oauth/access_token").unwrap(),
            github_server.uri().parse::<Url>().unwrap().join("user").unwrap(),
            github_server.uri().parse::<Url>().unwrap().join("user/emails").unwrap(),
        );
        let mut providers: std::collections::BTreeMap<String, std::sync::Arc<dyn crate::oauth_provider::OAuthProvider>> =
            std::collections::BTreeMap::new();
        providers.insert("google".to_string(), std::sync::Arc::new(google));
        providers.insert("github".to_string(), std::sync::Arc::new(github));
        let state = AuthState::for_tests(
            (*base_state.config).clone(),
            base_state.store.clone(),
            (*base_state.signing_keys).clone(),
            providers,
        );
        let app = router(state);

        // Start a browser login explicitly for GitHub.
        let login_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/auth/login?provider=github")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(login_response.status(), StatusCode::FOUND);
        let location = Url::parse(
            login_response
                .headers()
                .get(header::LOCATION)
                .unwrap()
                .to_str()
                .unwrap(),
        )
        .unwrap();
        let upstream_state = location
            .query_pairs()
            .find(|(key, _)| key == "state")
            .map(|(_, value)| value.into_owned())
            .unwrap();

        // Complete the callback with a fake upstream code.
        let callback_response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/auth/github/callback?state={upstream_state}&code=upstream-code"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(callback_response.status(), StatusCode::FOUND);

        let google_requests = google_server.received_requests().await.unwrap();
        let github_requests = github_server.received_requests().await.unwrap();
        assert!(
            google_requests.is_empty(),
            "selecting provider=github must never call Google's mock server: {google_requests:?}"
        );
        assert!(
            github_requests.iter().any(|r| r.url.path() == "/login/oauth/access_token"),
            "expected GitHub's token endpoint to be called: {github_requests:?}"
        );
    }
```

- [ ] **Step 9: Run tests**

Run: `cargo test -p soma-auth authorize::`
Expected: every existing test in the file still PASSES (they all implicitly exercise the `"google"` default-provider path, which is unchanged in behavior), plus the 3 new tests above PASS.

Run: `cargo clippy -p soma-auth -- -D warnings`

- [ ] **Step 10: Commit**

```bash
git add crates/shared/auth/src/authorize.rs crates/shared/auth/src/types.rs
git commit -m "feat(soma-auth): provider selection, HTML login picker, provider-agnostic callback"
```

---

### Task 12: `token.rs` — propagate `provider` through auth-code and refresh-token grants

**Files:**
- Modify: `crates/shared/auth/src/token.rs`

**Interfaces:**
- Consumes: `AuthState.provider()` (Task 9), `crate::oauth_provider::namespaced_subject` (Task 2), `AuthorizationCodeRow.provider`/`RefreshTokenRow.provider` (Task 8).

- [ ] **Step 1: Propagate `provider` in `authorization_code_grant`**

In the `if let Some(provider_refresh_token) = row.provider_refresh_token { ... }` block, add `provider: row.provider.clone(),` to the `RefreshTokenRow { ... }` literal:

```rust
    let refresh_token = if let Some(provider_refresh_token) = row.provider_refresh_token {
        let refresh_token = random_token(24)?;
        let created_at = now_unix();
        state
            .store
            .upsert_refresh_token(RefreshTokenRow {
                refresh_token: refresh_token.clone(),
                client_id: row.client_id.clone(),
                subject: row.subject.clone(),
                resource: row.resource.clone(),
                scope: row.scope.clone(),
                provider: row.provider.clone(),
                provider_refresh_token: Some(provider_refresh_token),
                created_at,
                expires_at: expires_at(
                    created_at,
                    state.config.refresh_token_ttl,
                    &format!("{}_AUTH_REFRESH_TOKEN_TTL_SECS", state.config.env_prefix),
                )?,
            })
            .await?;
```

(Everything else in this function — the `info!` logs, the `resource` fallback, the final `build_token_response` call — is unchanged.)

- [ ] **Step 2: Resolve the correct provider in `refresh_token_grant`**

Replace:

```rust
    // Refresh upstream before consuming the local token. If Google or id-token
    // verification fails, the client can retry the same local refresh token
    // instead of being stranded with an unreturned replacement.
    let google = state.google.refresh(&provider_refresh_token).await?;

    let refreshed_expires_at = expires_at(
        now_unix(),
        state.config.refresh_token_ttl,
        &format!("{}_AUTH_REFRESH_TOKEN_TTL_SECS", state.config.env_prefix),
    )?;
    let next_provider_refresh_token = google
        .refresh_token
        .clone()
        .unwrap_or_else(|| provider_refresh_token.clone());
    // Re-apply admin elevation in case this refresh token was originally
    // issued before elevation was wired in, or before the user's email was
    // on the allowlist.  elevate_scope_for_allowed_user is idempotent — if
    // the scope already contains the admin token it is left unchanged.
    let elevated_scope = crate::authorize::elevate_scope_for_allowed_user(
        &stored.scope,
        &state.config.default_scope,
    );

    state
        .store
        .upsert_refresh_token(RefreshTokenRow {
            refresh_token: refresh_token.clone(),
            client_id: stored.client_id.clone(),
            subject: google.subject.clone(),
            resource: stored_resource.clone(),
            scope: elevated_scope.clone(),
            provider_refresh_token: Some(next_provider_refresh_token),
            created_at: stored.created_at,
            expires_at: refreshed_expires_at,
        })
        .await?;

    info!(
        grant_type = "refresh_token",
        client_id = %stored.client_id,
        refresh_token_id = %refresh_token_id,
        subject_id = %fingerprint(&google.subject),
        resource = %stored_resource,
        scope = %elevated_scope,
        "oauth refresh_token grant refreshed stable local token and issued new access token"
    );

    build_token_response(
        &state,
        stored.client_id,
        google.subject,
        stored_resource,
        elevated_scope,
        Some(refresh_token),
    )
```

with:

```rust
    // Refresh upstream before consuming the local token. If the provider or
    // id-token verification fails, the client can retry the same local
    // refresh token instead of being stranded with an unreturned replacement.
    let provider = state.provider(&stored.provider)?;
    let exchange = provider.refresh(&provider_refresh_token).await?;

    let refreshed_expires_at = expires_at(
        now_unix(),
        state.config.refresh_token_ttl,
        &format!("{}_AUTH_REFRESH_TOKEN_TTL_SECS", state.config.env_prefix),
    )?;
    let subject = crate::oauth_provider::namespaced_subject(provider.provider_id(), &exchange.subject);
    let next_provider_refresh_token = exchange
        .refresh_token
        .clone()
        .unwrap_or_else(|| provider_refresh_token.clone());
    // Re-apply admin elevation in case this refresh token was originally
    // issued before elevation was wired in, or before the user's email was
    // on the allowlist.  elevate_scope_for_allowed_user is idempotent — if
    // the scope already contains the admin token it is left unchanged.
    let elevated_scope = crate::authorize::elevate_scope_for_allowed_user(
        &stored.scope,
        &state.config.default_scope,
    );

    state
        .store
        .upsert_refresh_token(RefreshTokenRow {
            refresh_token: refresh_token.clone(),
            client_id: stored.client_id.clone(),
            subject: subject.clone(),
            resource: stored_resource.clone(),
            scope: elevated_scope.clone(),
            provider: stored.provider.clone(),
            provider_refresh_token: Some(next_provider_refresh_token),
            created_at: stored.created_at,
            expires_at: refreshed_expires_at,
        })
        .await?;

    info!(
        grant_type = "refresh_token",
        client_id = %stored.client_id,
        refresh_token_id = %refresh_token_id,
        subject_id = %fingerprint(&subject),
        provider = provider.provider_id(),
        resource = %stored_resource,
        scope = %elevated_scope,
        "oauth refresh_token grant refreshed stable local token and issued new access token"
    );

    build_token_response(
        &state,
        stored.client_id,
        subject,
        stored_resource,
        elevated_scope,
        Some(refresh_token),
    )
```

Note: this changes behavior for GitHub-backed refresh tokens — since `GitHubProvider::refresh` always returns `AuthError::Config` (Task 6), a GitHub-authenticated session's refresh token grant will now fail loudly with a clear config-shaped error instead of silently working, which is correct: GitHub never issued a `provider_refresh_token` in the first place (Task 6's `exchange_code` returns `refresh_token: None`), so `authorization_code_grant`'s `if let Some(provider_refresh_token) = row.provider_refresh_token` branch is never taken for GitHub logins — no local refresh token is ever minted for a GitHub-authenticated flow, so `refresh_token_grant` calling `GitHubProvider::refresh` is actually unreachable in practice. Document this reasoning as a doc comment rather than special-casing it in code — the existing structure already makes it correct by construction.

- [ ] **Step 3: Fix `token.rs`'s own test module**

Update `test_auth_state_with_failing_google_refresh` to use the new `for_tests` signature (same pattern as Task 11 Step 7):

```rust
        AuthState::for_tests(
            (*state.config).clone(),
            state.store.clone(),
            (*state.signing_keys).clone(),
            AuthState::google_only_providers(google),
        )
```

For every `AuthorizationCodeRow { ... }` / `RefreshTokenRow { ... }` literal in this file's test module (surfaced by `cargo check -p soma-auth --tests`), add `provider: "google".to_string(),`.

- [ ] **Step 4: Add a GitHub-refresh-token-never-minted regression test**

```rust
    #[tokio::test]
    async fn authorization_code_grant_never_mints_a_refresh_token_for_a_github_login() {
        let state = test_auth_state_with_registered_client().await;
        state
            .store
            .insert_auth_code(AuthorizationCodeRow {
                code: "github-code".to_string(),
                client_id: "client".to_string(),
                subject: "github:9182310".to_string(),
                redirect_uri: "http://127.0.0.1:7777/callback".to_string(),
                resource: "https://lab.example.com/mcp".to_string(),
                scope: "lab".to_string(),
                provider: "github".to_string(),
                code_challenge: "pkce-challenge".to_string(),
                code_challenge_method: "S256".to_string(),
                provider_refresh_token: None,
                created_at: now_unix(),
                expires_at: now_unix() + 300,
            })
            .await
            .unwrap();
        let app = router(state);
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/token")
                    .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                    .body(Body::from("grant_type=authorization_code&code=github-code&client_id=client&redirect_uri=http://127.0.0.1:7777/callback&code_verifier=pkce-challenge-does-not-need-to-match-since-this-fixture-skips-pkce-validation-path"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(
            json.get("refresh_token").is_none(),
            "github logins must never receive a local refresh token: {json}"
        );
    }
```

Before trusting this test verbatim, check `validate_authorization_code_row`'s PKCE check (it likely validates `code_verifier` against `code_challenge` using SHA-256 — if so, replace the placeholder verifier string above with a real value whose SHA-256/base64url digest equals `"pkce-challenge"`, or simplify by using the same `code_challenge`/`code_verifier` pattern the file's existing `seed_authorization_code` helper already uses, reusing that helper instead of hand-rolling a new `AuthorizationCodeRow`).

- [ ] **Step 4B: Add a regression test for a refresh token whose provider was later removed from config**

Engineering review flagged this as a real, previously-untested edge case: a deployment upgrades (backfilling pre-existing rows to `provider='google'`), then an operator removes Google from config while an unexpired `refresh_tokens` row still names it. `state.provider("google")` correctly returns `AuthError::Validation` (fail-safe), but nothing proved that before now.

```rust
    #[tokio::test]
    async fn refresh_token_grant_fails_clearly_when_its_provider_is_no_longer_configured() {
        let state = test_auth_state_with_registered_client().await;
        state
            .store
            .upsert_refresh_token(RefreshTokenRow {
                refresh_token: "orphaned-refresh".to_string(),
                client_id: "client".to_string(),
                subject: "authelia:some-user".to_string(),
                resource: "https://lab.example.com/mcp".to_string(),
                scope: "lab".to_string(),
                provider: "authelia".to_string(),
                provider_refresh_token: Some("upstream-refresh".to_string()),
                created_at: now_unix(),
                expires_at: now_unix() + 3600,
            })
            .await
            .unwrap();
        // `test_auth_state_with_registered_client` only configures Google —
        // "authelia" is intentionally never configured here, simulating an
        // operator who removed it after this token was issued.
        let app = router(state);
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/token")
                    .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                    .body(Body::from("grant_type=refresh_token&client_id=client&refresh_token=orphaned-refresh"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["error"], "invalid_request");
    }
```

Cross-check the expected HTTP status/`error` value against how `TokenEndpointError::oauth_error()`/`status()` map `AuthError::Validation` (Task 12's own code doesn't add a new mapping for this — confirm the existing arms in `token.rs`'s `TokenEndpointError` already cover it; adjust the assertion to match whatever they actually produce rather than trusting this guess).

- [ ] **Step 5: Run tests**

Run: `cargo test -p soma-auth token::`
Expected: every existing test PASSES, plus the new GitHub-refresh-never-minted test PASSES.

Run: `cargo clippy -p soma-auth -- -D warnings`

- [ ] **Step 6: Commit**

```bash
git add crates/shared/auth/src/token.rs
git commit -m "feat(soma-auth): propagate provider through auth-code and refresh-token grants"
```

---

### Task 13: Full-crate verification pass

**Files:**
- None (verification only).

**Interfaces:**
- None.

- [ ] **Step 1: Full test suite**

Run: `cargo test -p soma-auth`
Expected: every test in the crate PASSES, including the `session.rs`, `metadata.rs`, `registration.rs`, `redirect_uri.rs`, `cimd/*`, `at_rest.rs`, `jwt.rs`, `sqlite.rs`, `error.rs`, `util.rs` modules — none of these were touched by this plan, so they should be entirely unaffected; a failure here means an earlier task's edit had an unanticipated ripple effect and needs investigation before moving on.

- [ ] **Step 2: Full lint pass**

Run: `cargo clippy -p soma-auth --all-targets -- -D warnings`
Expected: zero warnings.

- [ ] **Step 3: Format check**

Run: `cargo fmt -p soma-auth -- --check`
Expected: no diff. If there is one, run `cargo fmt -p soma-auth` and re-review the diff before committing (auto-formatting can occasionally reflow a hand-crafted multi-line SQL string in an unwanted way — check `sqlite.rs` specifically).

- [ ] **Step 4: Whole-workspace build check**

Run: `cargo check --workspace`
Expected: compiles. This confirms Task 9's `AuthState.google` field removal didn't break anything outside the crate — per this plan's research, nothing in `apps/soma` or `crates/soma/*` references `.google`, `GoogleConfig`, `GoogleProvider`, or `AuthState::for_tests` directly, but a workspace-wide check is the authoritative confirmation, not the earlier grep.

- [ ] **Step 4B: Document the multi-provider allowlist security posture in `docs/AUTH.md`**

Per this plan's Global Constraints, this trade-off must be documented, not left implicit. Read `/home/jmagar/workspace/soma/.claude/worktrees/oauth-provider-support-f427c9/docs/AUTH.md` first to match its existing structure/heading style, then add a section (heading level matching the file's convention) covering:

- The allowlist (`admin_email` + `allowed_users`) is one flat list shared across every configured provider; being on it grants full admin scope regardless of which provider authenticated the user (pre-existing, unchanged behavior).
- Enabling 2+ providers simultaneously means the deployment's effective admin-gate strength is that of its weakest configured provider's identity-verification signal — spell out concretely that GitHub's "primary && verified" email flag is a point-in-time claim that GitHub does not re-verify on each login, unlike Google/Authelia's live per-login ID-token `email_verified` claim.
- `AuthState::new` logs a `tracing::warn!` at startup whenever this condition holds (Task 9 Step 2) — point operators at that log line as the visible signal.
- Practical guidance: operators who want strict per-identity isolation should run separate deployments (separate `soma-auth` SQLite DBs) per provider rather than enabling several providers with a shared allowlist in one deployment.

- [ ] **Step 5: Update CHANGELOG.md**

Add under `## [Unreleased]` → `### Added` in `/home/jmagar/workspace/soma/.claude/worktrees/oauth-provider-support-f427c9/CHANGELOG.md`:

```markdown
- `soma-auth` gained a multi-provider OAuth login system:
  - **`OAuthProvider` trait** (`crates/shared/auth/src/oauth_provider.rs`) generalizes the
    previously Google-only login flow. `AuthState.google: Arc<GoogleProvider>` became
    `AuthState.providers: Arc<BTreeMap<String, Arc<dyn OAuthProvider>>>` plus
    `default_provider: String`.
  - **Authelia support** (`AutheliaProvider`) — a real OIDC Provider, same
    authorization-code + PKCE + RS256 ID-token shape as Google, configurable issuer via
    `{PREFIX}_AUTHELIA_ISSUER_URL`. Shares a new `oidc.rs` JWKS verifier with `GoogleProvider`.
  - **GitHub support** (`GitHubProvider`) — plain OAuth2, no ID token; fetches `GET /user` +
    `GET /user/emails` for identity. GitHub OAuth Apps issue non-expiring access tokens with
    no refresh token, so GitHub-authenticated sessions never receive a local refresh token —
    documented, not a bug.
  - A deployment can configure more than one provider simultaneously. `/auth/login` renders
    a plain HTML picker when more than one is configured and the request doesn't already say
    `?provider=`; `/authorize` accepts the same optional `?provider=` query param for headless
    MCP clients. Each configured provider mounts its own static callback path
    (`/auth/google/callback`, `/auth/authelia/callback`, `/auth/github/callback` by default,
    each independently overridable via `{PREFIX}_{PROVIDER}_CALLBACK_PATH`).
  - Four SQLite tables (`authorization_requests`, `authorization_codes`, `refresh_tokens`,
    `browser_login_states`) gained a `provider TEXT NOT NULL DEFAULT 'google'` column.
    Non-Google subjects are namespaced `{provider_id}:{raw_subject}` to avoid collisions
    across providers sharing one DB; Google's existing bare-`sub` subject format is
    unchanged for backward compatibility with already-issued sessions.
  - `force_consent` (used to guarantee a refresh token on first login) is now scoped
    per-provider instead of globally — a deployment with an existing Google refresh
    token on file no longer skips forced consent on a user's first Authelia/GitHub login.
  - The email allowlist remains a single list shared across all configured providers;
    see `docs/AUTH.md` for the resulting security trade-off when running 2+ providers
    simultaneously, and the startup warning log that surfaces it.
  - Authelia issuer URLs must be `https://`; callback paths across configured providers
    must be pairwise distinct (both enforced at config-validation time, not as an axum
    startup panic).
```

- [ ] **Step 6: Commit**

```bash
git add CHANGELOG.md
git commit -m "docs: changelog entry for soma-auth multi-provider OAuth support"
```

---

## Follow-up plan

Wiring this into the `soma` binary's own env vars, `soma providers`/CLI setup wizard, and doctor checks (`crates/soma/contracts`, `crates/soma/cli`, `apps/soma`) is `docs/superpowers/plans/2026-07-18-soma-oauth-provider-config.md` — it depends on every task in this plan being merged first (it imports `soma_auth::config::{AutheliaConfig, GitHubConfig}` and constructs `AuthConfigBuilder` calls that assume `default_provider` exists).

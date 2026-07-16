//! Posture 2 of 4: bearer-auth local service — token-protected, still local.
//!
//! Wraps [`codex_app_server_client::rest::trusted_bridge_router`] with
//! [`codex_app_server_client::rest::bearer_auth`] so every request must
//! present `Authorization: Bearer <token>`. The default bind address is
//! still loopback — the token is an *additional* layer, not a replacement
//! for keeping this off untrusted networks.
//!
//! ## What this protects
//!
//! Every route except (by default) `GET /health` and `GET /v1/health`
//! requires the exact configured bearer token, checked in constant time
//! (see `rest::auth`'s `constant_time_eq`). A caller without the token gets
//! `401 Unauthorized` and nothing else — no session data, no call results,
//! not even which routes exist beyond the 401 body itself.
//!
//! ## What this does NOT protect
//!
//! - **Transport confidentiality.** This crate speaks plain HTTP; it does
//!   not terminate TLS. A token sent over an unencrypted network can be
//!   captured by anything on the wire between the caller and this process.
//!   Put a TLS-terminating reverse proxy in front before trusting this
//!   posture beyond loopback.
//! - **Per-caller authorization.** The token is a single shared secret: any
//!   holder of it gets everything the mounted router exposes, with no
//!   distinction between callers, no scopes, no rate limiting per caller
//!   (only the process-wide limits in [`codex_app_server_client::rest::RestLimits`]).
//! - **The health routes**, by default: `GET /health` and `GET /v1/health`
//!   return `200` with no token at all, so a bare liveness probe doesn't
//!   need credentials wired through. `GET /v1/compatibility` is never
//!   exempt — it reveals the installed `codex --version`, which the health
//!   routes deliberately do not. Set
//!   `REST_BEARER_AUTH_EXAMPLE_REQUIRE_AUTH_FOR_HEALTH=1` to require the token
//!   on the health routes too, for deployments that want every request
//!   authenticated uniformly regardless of what it reveals.
//! - **Unsafe client options.** `command`/`extraArgs`/`config` overrides and
//!   `approvalPolicy: "allow_all"` remain rejected here, same as
//!   `rest_loopback_dev` — see `rest_admin_unsafe` for that separate opt-in.
//!
//! ## What an attacker who has the token can do
//!
//! The same full trusted-bridge access `rest_loopback_dev` grants to anyone
//! on loopback: session creation, turn execution, raw JSON-RPC calls,
//! server-request replies. The token is the entire authorization boundary —
//! treat it like any other production secret (env var / secret manager,
//! never committed, rotated if leaked).
//!
//! ## When this posture is (and isn't) appropriate
//!
//! Appropriate: a local service shared by a handful of trusted callers on
//! the same host or a private network segment, where a shared secret is an
//! acceptable authorization model and TLS is handled by a proxy in front.
//! Not appropriate as a substitute for real per-caller auth, and not a
//! reason to expose this directly to the public internet — combine it with
//! `rest_trusted_gateway`'s posture (a real gateway in front) for that.
//!
//! Run: `CODEX_APP_SERVER_REST_TOKEN=<token> cargo run -p codex-app-server-client --features rest --example rest_bearer_auth`
//!
//! Try it:
//! ```text
//! curl -i http://127.0.0.1:43230/health                                        # 200, no token needed
//! curl -i http://127.0.0.1:43230/v1/compatibility                              # 401, no token
//! curl -i http://127.0.0.1:43230/v1/compatibility -H "Authorization: Bearer $CODEX_APP_SERVER_REST_TOKEN"  # 200
//! ```

use std::net::SocketAddr;

use codex_app_server_client::rest;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let token = match std::env::var("CODEX_APP_SERVER_REST_TOKEN") {
        Ok(token) if !token.trim().is_empty() => token,
        Ok(_) => {
            return Err(
                "CODEX_APP_SERVER_REST_TOKEN is set but empty/whitespace-only; \
                 refusing to start with a blank bearer token"
                    .into(),
            );
        }
        Err(std::env::VarError::NotPresent) => {
            return Err(
                "CODEX_APP_SERVER_REST_TOKEN is not set. This example never falls \
                 back to a hardcoded token — set one explicitly, e.g.:\n\n  \
                 export CODEX_APP_SERVER_REST_TOKEN=$(openssl rand -hex 32)\n"
                    .into(),
            );
        }
        Err(std::env::VarError::NotUnicode(_)) => {
            return Err("CODEX_APP_SERVER_REST_TOKEN is set but is not valid UTF-8".into());
        }
    };

    // Deliberately outside the `CODEX_APP_SERVER_REST_*` prefix, unlike
    // `CODEX_APP_SERVER_REST_ADDR`/`_TOKEN` below. Every name under that
    // prefix is a real knob the shipped library or binary reads, enumerated in
    // `RestLimits`'s doc table; this one is understood only by this example
    // file. An operator grepping the tree for `CODEX_APP_SERVER_REST_` to find
    // what they can tune should not find a lookalike that does nothing outside
    // this example.
    let require_auth_for_health = env_flag("REST_BEARER_AUTH_EXAMPLE_REQUIRE_AUTH_FOR_HEALTH");

    let addr = std::env::var("CODEX_APP_SERVER_REST_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:43230".to_owned())
        .parse::<SocketAddr>()?;
    let listener = tokio::net::TcpListener::bind(addr).await?;

    let auth_layer =
        rest::bearer_auth(token).allow_unauthenticated_health(!require_auth_for_health);
    let app = rest::trusted_bridge_router().layer(auth_layer);

    println!("codex-app-server-client: rest_bearer_auth");
    println!("listening on http://{addr}");
    println!("posture: bearer-token-protected, full trusted bridge — see module docs");
    println!("health routes require token: {}", require_auth_for_health);
    println!();
    println!("  curl -i http://{addr}/health");
    println!("  curl -i http://{addr}/v1/compatibility");
    println!(
        "  curl -i http://{addr}/v1/compatibility -H \"Authorization: Bearer $CODEX_APP_SERVER_REST_TOKEN\""
    );
    println!();

    axum::serve(listener, app).await?;
    Ok(())
}

/// Treats any non-empty value other than `0`/`false`/`no` (case-insensitive)
/// as "on"; an unset variable is "off". Kept local rather than pulled from a
/// shared crate — this example has exactly one boolean flag to parse.
fn env_flag(var: &str) -> bool {
    match std::env::var(var) {
        Ok(value) => !matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "" | "0" | "false" | "no"
        ),
        Err(_) => false,
    }
}

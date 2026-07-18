//! Posture 3 of 4: trusted internal gateway — full bridge, zero auth of its
//! own, embedded behind *someone else's* authz boundary.
//!
//! This is the realistic "production" shape for the trusted bridge: a
//! platform team mounts [`codex_app_server_client::rest::trusted_bridge_router`]
//! as an internal service and relies entirely on a gateway in front of it
//! (an API gateway, a service mesh sidecar, an internal load balancer with
//! mTLS, an authenticating reverse proxy) to decide who may connect at all.
//! This process trusts every request it receives.
//!
//! ## What this protects
//!
//! Nothing, by itself. This example adds **no auth layer**. Every
//! protection comes from whatever the operator puts in front of it: network
//! policy that only lets the gateway reach this port, mTLS between the
//! gateway and this service, the gateway's own authentication and
//! authorization decisions before it ever proxies a request here.
//!
//! What this example *does* configure is operational limits via
//! [`codex_app_server_client::rest::RestLimits::from_env`] — bounded session
//! counts, call concurrency, poll timeouts, and turn durations, all tunable
//! per-deployment through `CODEX_APP_SERVER_REST_*` environment variables
//! (see the [`RestLimits`](codex_app_server_client::rest::RestLimits) doc
//! table) without a code change. Limits are a resource-abuse backstop, not
//! an authorization mechanism — they bound how much damage a connection can
//! do, they do not decide whether a connection should be allowed at all.
//!
//! ## What this does NOT protect
//!
//! Reachability. If anything other than the gateway can open a TCP
//! connection to this port — a network misconfiguration, an overly broad
//! security group, a sidecar listening on a pod IP instead of loopback, a
//! debug port left open — that caller gets full, unauthenticated trusted-
//! bridge access, identical to what the gateway itself has. This process
//! cannot tell the difference between "the gateway, having already
//! authenticated and authorized this request" and "anyone who reached the
//! port directly." **Reaching this port at all is equivalent to being fully
//! authorized** for every route it serves.
//!
//! Unsafe client options (`command`/`extraArgs`/`config` overrides,
//! `approvalPolicy: "allow_all"`) remain rejected here — see
//! `rest_admin_unsafe` for that separate, admin-only opt-in.
//!
//! ## What an attacker who reaches this port can do
//!
//! Everything: create and delete sessions, drive turns, call any app-server
//! JSON-RPC method, observe and answer server-originated requests — as the
//! OS user running this process, only rate-limited by the configured
//! [`RestLimits`](codex_app_server_client::rest::RestLimits).
//!
//! ## When this posture is (and isn't) appropriate
//!
//! Appropriate: internal-only deployments where network topology and a real
//! gateway genuinely make this port unreachable except from that gateway —
//! verified, not assumed. Not appropriate as an internet-facing service, not
//! appropriate if "the gateway" is aspirational rather than currently
//! enforced, and not a substitute for `rest_bearer_auth`'s token check when
//! there is no separate gateway component at all.
//!
//! Run: `cargo run -p codex-app-server-client --features rest --example rest_trusted_gateway`
//!
//! Try it (from the gateway's position — or from anywhere, since this
//! example itself enforces nothing; that is the point being demonstrated):
//! ```text
//! curl http://127.0.0.1:43240/health
//! curl http://127.0.0.1:43240/v1/compatibility
//! ```

use std::net::SocketAddr;

use codex_app_server_client::rest::{self, RestLimits, RestRouterOptions};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = std::env::var("CODEX_APP_SERVER_REST_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:43240".to_owned())
        .parse::<SocketAddr>()?;
    let listener = tokio::net::TcpListener::bind(addr).await?;

    // `RestLimits::from_env()` reads every `CODEX_APP_SERVER_REST_*` limit
    // variable (falling back to hardcoded defaults for anything unset) and
    // panics loudly on a malformed override rather than silently keeping
    // the default — see `RestLimits::try_from_env` if a non-panicking
    // startup path is preferred.
    let limits = RestLimits::from_env();
    let options = RestRouterOptions::trusted_bridge().with_limits(limits.clone());
    let app = rest::router_with_options(options);

    println!("codex-app-server-client: rest_trusted_gateway");
    println!("listening on http://{addr}");
    println!("posture: NO AUTH OF ITS OWN — full trusted bridge for embedding behind a gateway");
    println!("this process trusts every request it receives; see module docs");
    println!();
    println!("effective limits (CODEX_APP_SERVER_REST_* overridable):");
    println!(
        "  max_sessions                              = {}",
        limits.max_sessions
    );
    println!(
        "  max_one_shot_concurrency                  = {}",
        limits.max_one_shot_concurrency
    );
    println!(
        "  max_session_call_concurrency               = {}",
        limits.max_session_call_concurrency
    );
    println!(
        "  max_session_call_concurrency_per_session   = {}",
        limits.max_session_call_concurrency_per_session
    );
    println!(
        "  max_poll_timeout                          = {:?}",
        limits.max_poll_timeout
    );
    println!(
        "  max_text_turn_duration                    = {:?}",
        limits.max_text_turn_duration
    );
    println!(
        "  max_text_turn_output_bytes                 = {}",
        limits.max_text_turn_output_bytes
    );
    println!(
        "  idle_session_ttl                          = {:?}",
        limits.idle_session_ttl
    );
    println!();
    println!("  curl http://{addr}/health");
    println!("  curl http://{addr}/v1/compatibility");
    println!();

    axum::serve(listener, app).await?;
    Ok(())
}

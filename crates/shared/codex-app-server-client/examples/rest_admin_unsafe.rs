//! Posture 4 of 4 — the loudest one: admin-only unsafe client options.
//!
//! Enables `RestRouterOptions::trusted_bridge().with_unsafe_client_options(true)`.
//! This is the single most dangerous knob the REST adapter exposes, so this
//! example is deliberately harder to run than the other three: it refuses to
//! bind a non-loopback address unless an explicit, scarily-named environment
//! variable opts in, it still requires a bearer token, and it prints a loud
//! warning on every startup regardless of bind address.
//!
//! ## What `allow_unsafe_client_options` actually permits
//!
//! Verified against this crate's own request validation
//! (`validate_client_options` / `validate_text_turn_request` in
//! `src/rest/routes.rs`) rather than summarized from memory. With this flag
//! **off** (every other example in this set), a request body's `client`
//! object is rejected if it sets `command`, `extraArgs`, or `config`, and
//! `approvalPolicy: "allow_all"` is rejected outright. With this flag **on**,
//! all four are accepted from the request body, meaning an authenticated
//! caller of this process can, per request:
//!
//! - Set `client.command` to **any executable path readable and executable
//!   by this process's OS user** — not necessarily `codex` at all. Whatever
//!   is named there is what this process spawns.
//! - Append arbitrary `client.extraArgs` to that spawned command's argv.
//! - Set arbitrary `client.config` key/value overrides passed to the
//!   spawned process's config layer.
//! - Set `approvalPolicy: "allow_all"` on a text-turn request, which removes
//!   the approval gate that would otherwise stop a turn from taking
//!   consequential actions (file writes, shell commands, network calls —
//!   whatever the spawned command's own capabilities allow) without a human
//!   in the loop.
//!
//! Combined, an authenticated caller of this posture has **the same command-
//! execution power as this process's OS user** — not "control of Codex", but
//! control of what process gets spawned and how, with the turn's own
//! approval gate also removable on request. Treat holding valid credentials
//! against this endpoint as equivalent to holding a shell on this host as
//! this process's user.
//!
//! ## What this protects
//!
//! - **A bearer token is mandatory**, sourced from
//!   `CODEX_APP_SERVER_REST_TOKEN` (same rule as `rest_bearer_auth`: never a
//!   hardcoded literal, this process refuses to start without it).
//! - **Binding to a non-loopback address is refused by default.** Overriding
//!   `CODEX_APP_SERVER_REST_ADDR` to anything but loopback aborts startup
//!   unless `CODEX_APP_SERVER_REST_ADMIN_UNSAFE_BIND_ANYWHERE` is set to the
//!   exact sentinel value below — a name and value chosen so nobody sets it
//!   by accident.
//! - A prominent warning is printed on every run, regardless of bind
//!   address, because "it's on loopback" does not make arbitrary command
//!   execution safe on a shared or multi-tenant machine.
//!
//! ## What this does NOT protect
//!
//! Everything downstream of a valid token. There is no per-caller scoping —
//! any holder of the token gets the full unsafe surface described above.
//! There is no allowlist of permitted `command` values, no sandboxing of the
//! spawned process beyond whatever the OS user account itself is confined
//! to, and no audit trail beyond this crate's ordinary tracing output.
//!
//! ## When this posture is (and isn't) appropriate
//!
//! Appropriate: an operator/admin tool, run by the same trusted person who
//! already has a shell on this host, for a short-lived, deliberate session
//! (e.g. driving Codex through a non-default binary during development).
//! Not appropriate as a long-running service, not appropriate for any caller
//! who does not already have equivalent access to this host, and never
//! appropriate exposed beyond loopback without deliberately re-deriving
//! (not copy-pasting) a real authorization boundary in front of it.
//!
//! Run:
//! ```text
//! CODEX_APP_SERVER_REST_TOKEN=<token> \
//!   cargo run -p codex-app-server-client --features rest --example rest_admin_unsafe
//! ```
//!
//! Try it:
//! ```text
//! curl -i http://127.0.0.1:43250/v1/compatibility \
//!   -H "Authorization: Bearer $CODEX_APP_SERVER_REST_TOKEN"
//! ```

use std::net::SocketAddr;

use codex_app_server_client::rest::{self, RestRouterOptions};

/// Env var name deliberately spells out exactly what it does — this is not a
/// generic `_UNSAFE=1` flag that could be flipped absentmindedly while
/// copying an env block between deployments.
const BIND_ANYWHERE_VAR: &str = "CODEX_APP_SERVER_REST_ADMIN_UNSAFE_BIND_ANYWHERE";
const BIND_ANYWHERE_VALUE: &str = "i-accept-arbitrary-code-execution-on-this-host";

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    print_unsafe_warning();

    let token = require_token()?;

    let addr = std::env::var("CODEX_APP_SERVER_REST_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:43250".to_owned())
        .parse::<SocketAddr>()?;
    require_loopback_or_explicit_opt_in(addr)?;

    let listener = tokio::net::TcpListener::bind(addr).await?;

    let options = RestRouterOptions::trusted_bridge().with_unsafe_client_options(true);
    let app = rest::router_with_options(options).layer(rest::bearer_auth(token));

    println!("codex-app-server-client: rest_admin_unsafe");
    println!("listening on http://{addr}");
    println!("posture: bearer-token-protected, UNSAFE CLIENT OPTIONS ENABLED — see module docs");
    println!();
    println!(
        "  curl -i http://{addr}/v1/compatibility -H \"Authorization: Bearer $CODEX_APP_SERVER_REST_TOKEN\""
    );
    println!();

    axum::serve(listener, app).await?;
    Ok(())
}

fn print_unsafe_warning() {
    eprintln!("################################################################");
    eprintln!("# rest_admin_unsafe: allow_unsafe_client_options = true         #");
    eprintln!("#                                                               #");
    eprintln!("# Any authenticated caller can set client.command to ANY host   #");
    eprintln!("# executable, append arbitrary extraArgs, override config, and  #");
    eprintln!("# set approvalPolicy: allow_all to bypass the turn approval     #");
    eprintln!("# gate. This is equivalent to shell access as this process's    #");
    eprintln!("# OS user for anyone holding the bearer token. See this file's  #");
    eprintln!("# module doc comment for the full breakdown before running.     #");
    eprintln!("################################################################");
}

fn require_token() -> Result<String, Box<dyn std::error::Error>> {
    match std::env::var("CODEX_APP_SERVER_REST_TOKEN") {
        Ok(token) if !token.trim().is_empty() => Ok(token),
        Ok(_) => Err(
            "CODEX_APP_SERVER_REST_TOKEN is set but empty/whitespace-only; \
             refusing to start with a blank bearer token"
                .into(),
        ),
        Err(std::env::VarError::NotPresent) => Err(
            "CODEX_APP_SERVER_REST_TOKEN is not set. This unsafe posture still \
             requires a bearer token — set one explicitly, e.g.:\n\n  \
             export CODEX_APP_SERVER_REST_TOKEN=$(openssl rand -hex 32)\n"
                .into(),
        ),
        Err(std::env::VarError::NotUnicode(_)) => {
            Err("CODEX_APP_SERVER_REST_TOKEN is set but is not valid UTF-8".into())
        }
    }
}

/// Refuses to proceed with a non-loopback bind address unless
/// `BIND_ANYWHERE_VAR` is set to exactly `BIND_ANYWHERE_VALUE`. Loopback
/// addresses always pass, since that is this example's safe default.
fn require_loopback_or_explicit_opt_in(addr: SocketAddr) -> Result<(), Box<dyn std::error::Error>> {
    if addr.ip().is_loopback() {
        return Ok(());
    }
    let opted_in = std::env::var(BIND_ANYWHERE_VAR)
        .map(|value| value == BIND_ANYWHERE_VALUE)
        .unwrap_or(false);
    if opted_in {
        eprintln!(
            "WARNING: binding rest_admin_unsafe to non-loopback address {addr} because \
             {BIND_ANYWHERE_VAR} is set. Arbitrary command execution is now reachable \
             from anything that can reach this address and the bearer token."
        );
        return Ok(());
    }
    Err(format!(
        "refusing to bind rest_admin_unsafe to non-loopback address {addr}.\n\n\
         This posture allows an authenticated caller to run arbitrary host \
         executables (see module docs). Binding it beyond loopback is refused \
         by default. To override, set:\n\n  \
         {BIND_ANYWHERE_VAR}={BIND_ANYWHERE_VALUE}\n\n\
         ...and only after re-deriving a real authorization boundary for this \
         deployment — do not copy-paste this override.\n"
    )
    .into())
}

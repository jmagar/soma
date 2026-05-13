//! Binary entry point — mode dispatch only.
//!
//! Modes:
//!   `example [serve]`        Start MCP HTTP server (default if no args)
//!   `example mcp`            Start MCP stdio transport
//!   `example greet ...`      CLI greet command
//!   `example echo ...`       CLI echo command
//!   `example status`         CLI status command
//!   `example --help`         Print usage
//!   `example --version`      Print version
//!
//! **Template**: add your binary name in Cargo.toml `[[bin]] name = "..."`.
//! Extend `run_cli` if you add more CLI subcommands.

use anyhow::Result;
use std::sync::Arc;

use rmcp::{transport::stdio, ServiceExt};
use rmcp_template::{
    app::ExampleService,
    config::{AuthMode, Config},
    example::ExampleClient,
    mcp::{self, AppState, AuthPolicy},
};
use tracing::info;
use tracing_subscriber::{fmt, EnvFilter};

mod cli;

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();

    // Handle meta-flags before initialising logging (they print and exit)
    match args.as_slice() {
        [f] if matches!(f.as_str(), "--help" | "-h" | "help") => {
            print_usage();
            return Ok(());
        }
        [f] if matches!(f.as_str(), "--version" | "-V" | "version") => {
            println!("example {}", env!("CARGO_PKG_VERSION"));
            return Ok(());
        }
        _ => {}
    }

    // Suppress logs in stdio/CLI mode — MCP clients communicate over stdio
    // and cannot tolerate log lines mixed into the JSON stream.
    let stdio_mode = matches!(args.as_slice(), [c] if c == "mcp");
    let serve_mode = args.is_empty()
        || matches!(args.as_slice(), [c] if c == "serve")
        || matches!(args.as_slice(), [a, b] if a == "serve" && b == "mcp");

    let log_level = if stdio_mode || !serve_mode {
        "warn"
    } else {
        "info"
    };
    fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(log_level)),
        )
        .with_writer(std::io::stderr)
        .with_target(true)
        .init();

    if serve_mode {
        serve_mcp().await
    } else if stdio_mode {
        serve_stdio_mcp().await
    } else {
        run_cli().await
    }
}

// ── modes ─────────────────────────────────────────────────────────────────────

/// Start the MCP HTTP server (Streamable HTTP transport).
async fn serve_mcp() -> Result<()> {
    let config = Config::load()?;
    // Pattern §27: Refuse to bind to a non-loopback address without auth unless
    // the operator explicitly opts out via EXAMPLE_NOAUTH=true.
    validate_bind_security(&config)?;
    let state = build_state(config).await?;

    info!(
        bind = %state.config.bind_addr(),
        server_name = %state.config.server_name,
        auth = ?state.auth_policy,
        "example-mcp starting"
    );

    let bind = state.config.bind_addr();
    let app = mcp::router(state).layer(tower_http::trace::TraceLayer::new_for_http());
    let listener = tokio::net::TcpListener::bind(&bind).await?;
    info!(bind = %bind, "MCP HTTP server listening");

    axum::serve(listener, app.into_make_service())
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

/// Start the MCP stdio transport (for local/subprocess MCP clients).
///
/// Stdio is always LoopbackDev — it's a local trusted pipe between parent and
/// child process. HTTP auth middleware doesn't apply; forcing Mounted here
/// breaks all stdio clients with "forbidden: missing http context".
async fn serve_stdio_mcp() -> Result<()> {
    let config = Config::load()?;
    let service = ExampleService::new(ExampleClient::new(&config.example)?);
    let state = AppState {
        config: config.mcp,
        auth_policy: AuthPolicy::LoopbackDev, // stdio = trusted local transport
        service,
    };
    let svc = mcp::rmcp_server(state).serve(stdio()).await?;
    svc.waiting().await?;
    Ok(())
}

/// Dispatch CLI subcommands.
async fn run_cli() -> Result<()> {
    let config = Config::load()?;
    match cli::parse_args() {
        Some(cli::Command::Doctor { json }) => {
            // Doctor needs the full Config (not just ExampleConfig) to check
            // MCP port, auth mode, etc. — intercept here before service construction.
            cli::doctor::run_doctor(&config, json).await
        }
        Some(cmd) => cli::run(cmd, &config.example).await,
        None => {
            eprintln!("Unknown command. Run `example --help` for usage.");
            std::process::exit(1);
        }
    }
}

// ── security ──────────────────────────────────────────────────────────────────

/// Refuse to bind to a non-loopback address without authentication.
///
/// Pattern §27: Binding MCP on 0.0.0.0 with no auth exposes the server to
/// anyone on the network. This guard prevents accidental exposure.
///
/// Three ways to satisfy the guard:
///   1. Bind to loopback:  set EXAMPLE_MCP_HOST=127.0.0.1  (or ::1)
///   2. Enable auth:       set EXAMPLE_MCP_TOKEN=<token>   (bearer mode)
///                         or set EXAMPLE_MCP_AUTH_MODE=oauth
///   3. Explicit override: set EXAMPLE_NOAUTH=true          (upstream gateway handles auth)
///
/// TEMPLATE: The env var name for the override is EXAMPLE_NOAUTH (not
///           EXAMPLE_MCP_NO_AUTH). The latter disables auth for the server's
///           own middleware. EXAMPLE_NOAUTH tells THIS check that an upstream
///           reverse proxy or gateway handles auth instead.
///
/// Note: EXAMPLE_MCP_NO_AUTH (config.mcp.no_auth) disables the auth middleware.
///       EXAMPLE_NOAUTH is a separate acknowledgement that the operator knows
///       auth is absent and accepts responsibility.
fn validate_bind_security(config: &Config) -> Result<()> {
    let is_loopback = config.mcp.host.starts_with("127.") || config.mcp.host == "::1";
    // has_auth is true when:
    //   a) auth middleware is active (no_auth is false) AND
    //   b) at least one auth mechanism is configured (token OR OAuth)
    // no_auth=true means the server itself disables auth — NOT a safe state for 0.0.0.0.
    let has_auth = !config.mcp.no_auth
        && (config.mcp.api_token.is_some() || config.mcp.auth.mode == AuthMode::OAuth);

    // TEMPLATE: The env var name is EXAMPLE_NOAUTH — update if you change the prefix.
    let noauth_override = std::env::var("EXAMPLE_NOAUTH")
        .map(|v| matches!(v.to_lowercase().as_str(), "true" | "1" | "yes"))
        .unwrap_or(false);

    if !is_loopback && !has_auth && !noauth_override {
        anyhow::bail!(
            "Refusing to bind MCP server to {} without authentication.\n\
             \n\
             Choose one of:\n\
             1. Bind to loopback:    EXAMPLE_MCP_HOST=127.0.0.1\n\
             2. Set a bearer token:  EXAMPLE_MCP_TOKEN=$(openssl rand -hex 32)\n\
             3. Enable OAuth:        EXAMPLE_MCP_AUTH_MODE=oauth (+ OAuth credentials)\n\
             4. Upstream gateway:    EXAMPLE_NOAUTH=true  (if a proxy handles auth)\n\
             \n\
             For local dev, run:  just dev   (sets EXAMPLE_MCP_NO_AUTH=true on 0.0.0.0)\n\
             \n\
             TEMPLATE: Replace EXAMPLE_ prefix with your service's prefix throughout.",
            config.mcp.host
        );
    }
    Ok(())
}

// ── helpers ───────────────────────────────────────────────────────────────────

async fn build_state(config: Config) -> Result<AppState> {
    let auth_policy = build_auth_policy(&config).await?;
    let service = ExampleService::new(ExampleClient::new(&config.example)?);
    Ok(AppState {
        config: config.mcp,
        auth_policy,
        service,
    })
}

async fn build_auth_policy(config: &Config) -> Result<AuthPolicy> {
    if config.mcp.no_auth || config.mcp.host.starts_with("127.") {
        return Ok(AuthPolicy::LoopbackDev);
    }
    if config.mcp.auth.mode == AuthMode::OAuth {
        let auth_cfg = lab_auth::config::AuthConfigBuilder::new()
            .env_prefix("EXAMPLE_MCP")
            .session_cookie_name("example_mcp_session")
            .scopes_supported(vec!["example:read".into(), "example:write".into()])
            .default_scope("example:read")
            .resource_path("/mcp")
            .enable_dynamic_registration(true)
            .build_from_sources(vec![])
            .map_err(|e| anyhow::anyhow!("OAuth config error: {e}"))?;
        let auth_state = lab_auth::state::AuthState::new(auth_cfg)
            .await
            .map_err(|e| anyhow::anyhow!("OAuth state init error: {e}"))?;
        Ok(AuthPolicy::Mounted {
            auth_state: Some(Arc::new(auth_state)),
        })
    } else {
        Ok(AuthPolicy::Mounted { auth_state: None })
    }
}

fn print_usage() {
    eprintln!(
        "Usage:
  example [serve]          Start MCP HTTP server (default)
  example mcp              Start MCP stdio transport

  example greet [--name NAME]       Greet NAME (or the world)
  example echo --message MSG        Echo MSG back
  example status                    Show server status

  example --help                    Show this help
  example --version                 Show version

Environment:
  EXAMPLE_API_URL          Upstream service URL
  EXAMPLE_API_KEY          Upstream service API key
  EXAMPLE_MCP_HOST         Bind host (default 0.0.0.0)
  EXAMPLE_MCP_PORT         Bind port (default 40060)
  EXAMPLE_MCP_NO_AUTH      Disable auth (loopback only)
  EXAMPLE_MCP_TOKEN        Static bearer token
  RUST_LOG                 Log filter (e.g. info,rmcp=warn)"
    );
}

async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(e) = tokio::signal::ctrl_c().await {
            tracing::error!(error = %e, "CTRL+C handler failed");
            std::future::pending::<()>().await;
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut s) => {
                s.recv().await;
            }
            Err(e) => {
                tracing::error!(error = %e, "SIGTERM handler failed");
                std::future::pending::<()>().await;
            }
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! { _ = ctrl_c => {}, _ = terminate => {} }
    tracing::info!("Shutdown signal received");
}

//! Full server binary entry point.
//!
//! Modes:
//!   `soma-server [serve]`  Start Streamable HTTP MCP + REST + web server
//!   `soma-server mcp`      Start stdio MCP transport
//!   `soma-server <cmd>`    Run CLI command

use anyhow::Result;
use soma::{cli, runtime};

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();

    match args.as_slice() {
        [f] if matches!(f.as_str(), "--help" | "-h") => {
            eprintln!("{}", cli::usage());
            return Ok(());
        }
        [f] if matches!(f.as_str(), "--version" | "-V" | "version") => {
            println!("soma-server {}", env!("CARGO_PKG_VERSION"));
            return Ok(());
        }
        _ => {}
    }

    let stdio_mode = matches!(args.as_slice(), [c] if c == "mcp");
    let serve_mode = args.is_empty()
        || matches!(args.as_slice(), [c] if c == "serve")
        || matches!(args.as_slice(), [a, b] if a == "serve" && b == "mcp");

    // Load ~/.<service>/.env (or /data/.env in a container) before any config
    // load so the binary works on bare metal without a process manager injecting
    // env. Non-overriding: explicit process env still wins.
    soma::config::load_dotenv();

    runtime::init_logging(stdio_mode, serve_mode);

    if serve_mode {
        runtime::serve_http_mcp().await
    } else if stdio_mode {
        runtime::serve_stdio_mcp().await
    } else {
        runtime::run_cli().await
    }
}

//! CLI — thin shim that parses args, calls `ExampleService`, formats output.
//!
//! The CLI uses the same service layer as the MCP server. No business logic lives here.
//!
//! **Template**: add subcommands to match your service's operations.
//!
//! # Usage
//!
//! ```
//! example greet --name Alice
//! example echo --message "Hello!"
//! example status
//! example doctor [--json]
//! ```

use anyhow::Result;
use rmcp_template::{app::ExampleService, config::ExampleConfig, example::ExampleClient};

// TEMPLATE: The doctor module is the §48 reference implementation.
//           Import it from here and wire into run() below.
pub mod doctor;

#[derive(Debug)]
pub enum Command {
    Greet {
        name: Option<String>,
    },
    Echo {
        message: String,
    },
    Status,
    /// Pre-flight environment validation (§48).
    ///
    /// TEMPLATE: Always keep this command. It is the operator's first stop
    /// when setting up or debugging the service.
    Doctor {
        /// Output JSON instead of human-readable text.
        json: bool,
    },
}

/// Parse CLI arguments from `std::env::args()`.
///
/// Returns `None` if the first argument is not a known subcommand.
/// **Template**: extend this to use clap or another arg parser for a real CLI.
/// This is intentionally minimal so the template compiles without extra deps.
///
/// # TEMPLATE: Adding a new subcommand
///
/// 1. Add a variant to `Command` above.
/// 2. Add a match arm here to construct it from args.
/// 3. Add a dispatch arm in `run()` below.
/// 4. Update `print_usage()` in main.rs.
pub fn parse_args() -> Option<Command> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [] => None,
        [subcommand, rest @ ..] => match subcommand.as_str() {
            "greet" => {
                let name = flag_value(rest, "--name");
                Some(Command::Greet { name })
            }
            "echo" => {
                let message = flag_value(rest, "--message")
                    .unwrap_or_else(|| "(no message provided)".to_string());
                Some(Command::Echo { message })
            }
            "status" => Some(Command::Status),
            // §48: doctor is always parsed here, dispatched via run_cli in main.rs.
            // TEMPLATE: Keep this arm. It routes to doctor::run_doctor() which needs
            //           the full Config (not just ExampleConfig), so main.rs handles it.
            "doctor" => {
                let json = rest.iter().any(|a| a == "--json");
                Some(Command::Doctor { json })
            }
            _ => None,
        },
    }
}

/// Run a CLI command, print the result, and exit.
///
/// # TEMPLATE
/// - `Doctor` is handled specially in `main.rs::run_cli` (needs full `Config`).
/// - All other commands get only `ExampleConfig`; keep it that way.
/// - Add `--json` support to each new command by forwarding a `json` flag.
pub async fn run(cmd: Command, cfg: &ExampleConfig) -> Result<()> {
    let client = ExampleClient::new(cfg)?;
    let service = ExampleService::new(client);

    let result = match &cmd {
        Command::Greet { name } => service.greet(name.as_deref()).await?,
        Command::Echo { message } => service.echo(message).await?,
        Command::Status => service.status().await?,
        // Doctor is never dispatched via this function — main.rs calls
        // doctor::run_doctor directly so it can pass the full Config.
        // TEMPLATE: Do not add Doctor here. It needs config.mcp fields.
        Command::Doctor { .. } => {
            unreachable!("Doctor is dispatched directly in main.rs::run_cli")
        }
    };

    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}

// ── arg parsing helpers ───────────────────────────────────────────────────────

fn flag_value(args: &[String], flag: &str) -> Option<String> {
    let pos = args.iter().position(|a| a == flag)?;
    args.get(pos + 1).cloned()
}

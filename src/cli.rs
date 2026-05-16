//! CLI — thin shim that parses args, calls `ExampleService`, formats output.
//!
//! The CLI uses the same service layer as the MCP server. No business logic lives here.
//!
//! **Template**: add subcommands to match your service's operations.
//!
//! # Usage
//!
//! ```text
//! example greet --name Alice
//! example echo --message "Hello!"
//! example status
//! example doctor [--json]
//! ```

use crate::{app::ExampleService, config::ExampleConfig, example::ExampleClient};
use anyhow::{anyhow, Result};

// TEMPLATE: The doctor module is the §48 reference implementation.
//           Import it from here and wire into run() below.
pub mod doctor;
pub mod setup;
pub mod watch;

pub use setup::{run_setup, SetupCommand};

#[derive(Debug, PartialEq, Eq)]
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
    /// Poll the MCP server health endpoint and emit a line on every state change.
    ///
    /// Designed to be run as a plugin monitor — stdout is the event stream,
    /// stderr is debug output. Exits only on CTRL+C.
    Watch {
        /// Base URL of the MCP server (default: http://localhost:{EXAMPLE_MCP_PORT}).
        url: Option<String>,
        /// Poll interval in seconds (default: 10).
        interval: u64,
    },
    Setup(SetupCommand),
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
pub fn parse_args() -> Result<Option<Command>> {
    parse_args_from(std::env::args().skip(1))
}

pub fn parse_args_from<I, S>(args: I) -> Result<Option<Command>>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let args: Vec<String> = args.into_iter().map(Into::into).collect();
    let command = match args.as_slice() {
        [] => None,
        [subcommand, rest @ ..] => match subcommand.as_str() {
            "greet" => {
                let name = flag_value(rest, "--name");
                Some(Command::Greet { name })
            }
            "echo" => {
                let message = flag_value(rest, "--message")
                    .filter(|m| !m.is_empty())
                    .ok_or_else(|| anyhow!("echo requires non-empty --message"))?;
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
            "watch" => {
                let url = flag_value(rest, "--url");
                let interval = match flag_value(rest, "--interval") {
                    Some(v) => v.parse().map_err(|_| {
                        anyhow!("watch --interval must be a positive integer number of seconds")
                    })?,
                    None => 10,
                };
                Some(Command::Watch { url, interval })
            }
            "setup" => match rest {
                [action] if action == "check" => Some(Command::Setup(SetupCommand::Check)),
                [action] if action == "repair" => Some(Command::Setup(SetupCommand::Repair)),
                [action, flags @ ..] if action == "plugin-hook" => {
                    Some(Command::Setup(SetupCommand::PluginHook {
                        no_repair: flags.iter().any(|f| f == "--no-repair"),
                    }))
                }
                _ => None,
            },
            _ => None,
        },
    };
    Ok(command)
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
        // Doctor, Watch, and Setup are never dispatched via this function — main.rs
        // handles them directly because they need config.mcp fields.
        Command::Doctor { .. } | Command::Watch { .. } | Command::Setup(_) => {
            unreachable!("dispatched directly in main.rs::run_cli")
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

#[cfg(test)]
#[path = "cli_tests.rs"]
mod tests;

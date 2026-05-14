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

use std::net::TcpListener;
use std::path::{Path, PathBuf};

use anyhow::Result;
use rmcp_template::{
    app::ExampleService,
    config::{default_data_dir, AuthMode, Config, ExampleConfig},
    example::ExampleClient,
};

// TEMPLATE: The doctor module is the §48 reference implementation.
//           Import it from here and wire into run() below.
pub mod doctor;
pub mod watch;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetupCommand {
    Check,
    Repair,
    PluginHook { no_repair: bool },
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
            "watch" => {
                let url = flag_value(rest, "--url");
                let interval = flag_value(rest, "--interval")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(10);
                Some(Command::Watch { url, interval })
            }
            "setup" => match rest {
                [action] if action == "check" => Some(Command::Setup(SetupCommand::Check)),
                [action] if action == "repair" => Some(Command::Setup(SetupCommand::Repair)),
                [action, flags @ ..] if action == "plugin-hook" => {
                    Some(Command::Setup(SetupCommand::PluginHook {
                        no_repair: flags.iter().any(|flag| flag == "--no-repair"),
                    }))
                }
                _ => None,
            },
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
        // Doctor and Watch are never dispatched via this function — main.rs
        // handles them directly because they need config.mcp fields.
        Command::Doctor { .. } => {
            unreachable!("Doctor is dispatched directly in main.rs::run_cli")
        }
        Command::Watch { .. } | Command::Setup(_) => {
            unreachable!("Watch is dispatched directly in main.rs::run_cli")
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

// ── setup command ────────────────────────────────────────────────────────────

#[derive(Debug, serde::Serialize)]
struct SetupFailure {
    code: &'static str,
    message: String,
}

#[derive(Debug, serde::Serialize)]
struct SetupReport {
    exit_policy: &'static str,
    ran_repair: bool,
    no_repair: bool,
    blocking_failures: Vec<SetupFailure>,
    advisory_failures: Vec<SetupFailure>,
}

impl SetupReport {
    fn new(no_repair: bool) -> Self {
        Self {
            exit_policy: "success",
            ran_repair: false,
            no_repair,
            blocking_failures: Vec::new(),
            advisory_failures: Vec::new(),
        }
    }

    fn finish(mut self) -> Self {
        self.exit_policy = if !self.blocking_failures.is_empty() {
            "blocking_failure"
        } else if !self.advisory_failures.is_empty() {
            "advisory_failure"
        } else {
            "success"
        };
        self
    }
}

pub async fn run_setup(config: &Config, command: SetupCommand) -> Result<()> {
    let report = match command {
        SetupCommand::Check => setup_check(config, true),
        SetupCommand::Repair => setup_repair(config)?,
        SetupCommand::PluginHook { no_repair } => setup_plugin_hook(config, no_repair)?,
    };

    println!("{}", serde_json::to_string_pretty(&report)?);
    if !report.blocking_failures.is_empty() {
        std::process::exit(1);
    }
    Ok(())
}

fn setup_plugin_hook(config: &Config, no_repair: bool) -> Result<SetupReport> {
    let initial = setup_check(config, no_repair);
    if initial.blocking_failures.is_empty() || no_repair {
        return Ok(initial);
    }
    setup_repair(config)
}

fn setup_check(config: &Config, no_repair: bool) -> SetupReport {
    let mut report = SetupReport::new(no_repair);
    let data_dir = setup_data_dir();

    if !data_dir.is_dir() {
        report.blocking_failures.push(SetupFailure {
            code: "appdata_missing",
            message: format!("appdata directory does not exist: {}", data_dir.display()),
        });
    }
    if config.example.api_url.is_empty() {
        report.blocking_failures.push(SetupFailure {
            code: "missing_example_api_url",
            message: "EXAMPLE_API_URL is required".into(),
        });
    }
    if config.example.api_key.is_empty() {
        report.blocking_failures.push(SetupFailure {
            code: "missing_example_api_key",
            message: "EXAMPLE_API_KEY is required".into(),
        });
    }

    validate_setup_auth(config, &mut report);
    check_setup_port(config.mcp.port, &mut report);

    report.finish()
}

fn setup_repair(config: &Config) -> Result<SetupReport> {
    let data_dir = setup_data_dir();
    std::fs::create_dir_all(&data_dir)?;

    let mut report = setup_check(config, false);
    report.ran_repair = true;
    if report
        .blocking_failures
        .iter()
        .any(|failure| failure.code == "appdata_missing")
    {
        report = setup_check(config, false);
        report.ran_repair = true;
    }

    write_setup_env(&data_dir, config)?;
    Ok(report.finish())
}

fn validate_setup_auth(config: &Config, report: &mut SetupReport) {
    if config.mcp.no_auth {
        return;
    }

    if config.mcp.auth.mode == AuthMode::OAuth {
        if config
            .mcp
            .auth
            .public_url
            .as_deref()
            .unwrap_or("")
            .is_empty()
        {
            report.blocking_failures.push(SetupFailure {
                code: "missing_oauth_public_url",
                message: "EXAMPLE_MCP_PUBLIC_URL is required for OAuth mode".into(),
            });
        }
        if config
            .mcp
            .auth
            .google_client_id
            .as_deref()
            .unwrap_or("")
            .is_empty()
        {
            report.blocking_failures.push(SetupFailure {
                code: "missing_oauth_client_id",
                message: "EXAMPLE_MCP_GOOGLE_CLIENT_ID is required for OAuth mode".into(),
            });
        }
        if config
            .mcp
            .auth
            .google_client_secret
            .as_deref()
            .unwrap_or("")
            .is_empty()
        {
            report.blocking_failures.push(SetupFailure {
                code: "missing_oauth_client_secret",
                message: "EXAMPLE_MCP_GOOGLE_CLIENT_SECRET is required for OAuth mode".into(),
            });
        }
        if config.mcp.auth.admin_email.is_empty() {
            report.blocking_failures.push(SetupFailure {
                code: "missing_oauth_admin_email",
                message: "EXAMPLE_MCP_AUTH_ADMIN_EMAIL is required for OAuth mode".into(),
            });
        }
    } else if config.mcp.api_token.as_deref().unwrap_or("").is_empty() {
        report.blocking_failures.push(SetupFailure {
            code: "missing_mcp_token",
            message: "EXAMPLE_MCP_TOKEN is required unless no_auth or OAuth mode is enabled".into(),
        });
    }
}

fn check_setup_port(port: u16, report: &mut SetupReport) {
    if TcpListener::bind(("127.0.0.1", port)).is_err() {
        report.advisory_failures.push(SetupFailure {
            code: "mcp_port_in_use",
            message: format!("MCP port {port} is already in use"),
        });
    }
}

fn setup_data_dir() -> PathBuf {
    std::env::var_os("CLAUDE_PLUGIN_DATA")
        .or_else(|| std::env::var_os("EXAMPLE_HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(default_data_dir)
}

fn write_setup_env(data_dir: &Path, config: &Config) -> Result<()> {
    let mut lines = vec![
        format!("EXAMPLE_API_URL={}", config.example.api_url),
        format!("EXAMPLE_API_KEY={}", config.example.api_key),
        format!("EXAMPLE_MCP_HOST={}", config.mcp.host),
        format!("EXAMPLE_MCP_PORT={}", config.mcp.port),
        format!("EXAMPLE_MCP_NO_AUTH={}", config.mcp.no_auth),
    ];

    if let Some(token) = config
        .mcp
        .api_token
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        lines.push(format!("EXAMPLE_MCP_TOKEN={token}"));
    }
    if config.mcp.auth.mode == AuthMode::OAuth {
        lines.push("EXAMPLE_MCP_AUTH_MODE=oauth".into());
        if let Some(value) = &config.mcp.auth.public_url {
            lines.push(format!("EXAMPLE_MCP_PUBLIC_URL={value}"));
        }
        if let Some(value) = &config.mcp.auth.google_client_id {
            lines.push(format!("EXAMPLE_MCP_GOOGLE_CLIENT_ID={value}"));
        }
        if let Some(value) = &config.mcp.auth.google_client_secret {
            lines.push(format!("EXAMPLE_MCP_GOOGLE_CLIENT_SECRET={value}"));
        }
        if !config.mcp.auth.admin_email.is_empty() {
            lines.push(format!(
                "EXAMPLE_MCP_AUTH_ADMIN_EMAIL={}",
                config.mcp.auth.admin_email
            ));
        }
    }

    std::fs::write(data_dir.join(".env"), format!("{}\n", lines.join("\n")))?;
    Ok(())
}

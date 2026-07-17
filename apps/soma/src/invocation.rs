//! Converts the top-level CLI invocation into an execution mode.
//!
//! Mode selection stays separate from what each mode does: this module only
//! classifies `argv` into a [`Mode`]. `local.rs` runs CLI commands, `http.rs`
//! runs the HTTP server, and `stdio.rs` runs the stdio MCP transport (plan
//! section 3.1).

/// How the `soma` binary was invoked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mode {
    /// `--help` / `-h`: print usage and exit.
    Help,
    /// `--version` / `-V` / `version`: print the binary version and exit.
    Version,
    /// `serve` / `serve mcp`: run the HTTP server.
    Serve,
    /// `mcp`: run the stdio MCP transport.
    Stdio,
    /// Any other subcommand: dispatch through the CLI.
    Cli,
}

impl Mode {
    /// Default `RUST_LOG` level for this mode when the environment does not
    /// set one explicitly. Stdio and CLI modes stay quiet (`warn`) so JSON-RPC
    /// framing and terminal output are never corrupted by log lines; the HTTP
    /// server defaults to `info`.
    pub(crate) fn default_log_level(self) -> &'static str {
        match self {
            Mode::Serve => "info",
            Mode::Help | Mode::Version | Mode::Stdio | Mode::Cli => "warn",
        }
    }
}

/// Classify `argv` (excluding `argv[0]`) into an execution [`Mode`].
pub(crate) fn resolve(args: &[String]) -> Mode {
    if matches!(args, [f] if matches!(f.as_str(), "--help" | "-h")) {
        return Mode::Help;
    }
    if matches!(args, [f] if matches!(f.as_str(), "--version" | "-V" | "version")) {
        return Mode::Version;
    }
    if is_http_server_request(args) {
        return Mode::Serve;
    }
    if matches!(args, [c] if c == "mcp") {
        return Mode::Stdio;
    }
    Mode::Cli
}

fn is_http_server_request(args: &[String]) -> bool {
    matches!(args, [c] if c == "serve") || matches!(args, [a, b] if a == "serve" && b == "mcp")
}

#[cfg(test)]
#[path = "invocation_tests.rs"]
mod tests;

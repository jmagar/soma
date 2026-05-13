//! doctor — pre-flight environment validation command.
//!
//! Pattern §48: Every server binary MUST implement a `doctor` subcommand that
//! validates the environment and reports what's missing before the user tries
//! to start the server.
//!
//! # Usage
//!
//! ```text
//! example doctor           # human-readable coloured output; exit 0/1
//! example doctor --json    # machine-readable JSON; exit 0/1
//! ```
//!
//! # TEMPLATE
//!
//! This is the reference implementation for the rmcp-template family. When you
//! clone the template for a real service, the things you MUST change are:
//!
//! 1. Replace `EXAMPLE_API_URL` / `EXAMPLE_API_KEY` with your service's env vars.
//! 2. Replace `"example"` binary name with your binary name in `check_binary_in_path`.
//! 3. Replace `~/.example/` data dir with your service's data dir (see `config::default_data_dir`).
//! 4. Add any service-specific checks (e.g. database connectivity, auth token format).
//! 5. Update the `print_doctor_report` section headings and hint text to match your service.
//!
//! Nothing else here needs changing for a basic deployment. Business logic for the
//! checks belongs in the individual `check_*` functions — never in `run_doctor`.

use std::net::TcpListener;
use std::path::Path;
use std::time::Instant;

use anyhow::Result;
use serde::Serialize;

use rmcp_template::config::{default_data_dir, AuthMode, Config};

// ── Public entry point ────────────────────────────────────────────────────────

/// Run the doctor command.
///
/// Executes all pre-flight checks in order and prints a summary. Exits with
/// code 1 if any check fails; 0 if all pass.
///
/// # TEMPLATE
/// This function is the canonical §48 implementation. Add calls to new
/// `check_*` functions below to extend the set of checks for your service.
pub async fn run_doctor(config: &Config, json: bool) -> Result<()> {
    let mut checks: Vec<DoctorCheck> = Vec::new();

    // ── 1. Config and filesystem ──────────────────────────────────────────────
    //
    // TEMPLATE: The data dir is resolved via `config::default_data_dir()`.
    //           In Docker it resolves to /data; bare-metal to ~/.example/.
    //           Replace ".example" with your service name in config.rs.
    let data_dir = default_data_dir();

    checks.push(check_config_file(&data_dir));
    checks.push(check_dir_writable("Data directory", &data_dir));
    checks.push(check_dir_writable("Log directory", &data_dir.join("logs")));

    // ── 2. Binary in PATH ─────────────────────────────────────────────────────
    //
    // TEMPLATE: Replace "example" with your binary name (Cargo.toml [[bin]] name).
    checks.push(check_binary_in_path("example"));

    // ── 3. Required environment variables / config ────────────────────────────
    //
    // TEMPLATE: Replace these with your service's required vars. Mark vars that
    //           have safe defaults as optional (they will warn, not fail).
    //
    // Required vars fail with ✗.  Optional vars warn with ⚠.
    checks.push(check_required_var(
        "EXAMPLE_API_URL",
        &config.example.api_url,
    ));
    checks.push(check_required_var(
        "EXAMPLE_API_KEY",
        &config.example.api_key,
    ));

    // ── 4. Upstream connectivity ──────────────────────────────────────────────
    //
    // TEMPLATE: Adjust the health path for your upstream service.
    //           If the URL is empty we skip the check — the required-var check
    //           above already flagged it.
    if !config.example.api_url.is_empty() {
        // TEMPLATE: Replace "/health" with your upstream's health or ping endpoint.
        //           If your upstream has no health endpoint, do a simple HEAD / request.
        checks.push(check_upstream(&config.example.api_url).await);
    }

    // ── 5. MCP server port ────────────────────────────────────────────────────
    //
    // TEMPLATE: config.mcp.port defaults to 3000 for the template.
    //           Your service's port is set in config.toml [mcp] port.
    checks.push(check_port_available(config.mcp.port));

    // ── 6. Auth configuration ─────────────────────────────────────────────────
    //
    // TEMPLATE: The auth check inspects the combination of host / auth settings
    //           and reports which auth mode is active, or warns if 0.0.0.0 has
    //           no auth configured.
    checks.push(check_auth_config(config));

    // ── Render output ─────────────────────────────────────────────────────────

    let issues = checks.iter().filter(|c| !c.ok).count();

    if json {
        println!("{}", serde_json::to_string_pretty(&checks)?);
    } else {
        print_doctor_report(&checks);
    }

    // Exit code 1 when any check fails.
    if issues > 0 {
        std::process::exit(1);
    }
    Ok(())
}

// ── DoctorCheck struct ────────────────────────────────────────────────────────

/// A single pre-flight check result.
///
/// `ok = true`  → the check passed; `value` shows what was found.
/// `ok = false` → the check failed; `hint` explains how to fix it.
///
/// # TEMPLATE
/// Serialises directly to the `--json` output. Add fields here if you need
/// additional metadata (e.g. `severity: "warning" | "error"`, `doc_url`).
#[derive(Debug, Serialize)]
pub struct DoctorCheck {
    /// Logical category for grouping in human output and JSON filtering.
    ///
    /// TEMPLATE: Defined by each `check_*` function. Categories in the template:
    ///   "config" | "credentials" | "connectivity" | "server" | "auth"
    pub category: &'static str,

    /// Short human-readable name for the check (shown in the left column).
    pub name: String,

    /// `true` = passed (✓), `false` = failed (✗).
    pub ok: bool,

    /// What was found — shown in the right column when ok=true.
    /// For failed checks, the hint is more useful.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,

    /// How to fix the problem — only present when `ok = false`.
    ///
    /// TEMPLATE: Make hints actionable — tell the user exactly what to type.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,

    /// Round-trip latency in milliseconds — only for connectivity checks.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latency_ms: Option<u64>,
}

impl DoctorCheck {
    fn pass(category: &'static str, name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            category,
            name: name.into(),
            ok: true,
            value: Some(value.into()),
            hint: None,
            latency_ms: None,
        }
    }

    fn fail(category: &'static str, name: impl Into<String>, hint: impl Into<String>) -> Self {
        Self {
            category,
            name: name.into(),
            ok: false,
            value: None,
            hint: Some(hint.into()),
            latency_ms: None,
        }
    }

    fn pass_with_latency(
        category: &'static str,
        name: impl Into<String>,
        value: impl Into<String>,
        latency_ms: u64,
    ) -> Self {
        Self {
            category,
            name: name.into(),
            ok: true,
            value: Some(value.into()),
            hint: None,
            latency_ms: Some(latency_ms),
        }
    }

    fn fail_with_latency(
        category: &'static str,
        name: impl Into<String>,
        hint: impl Into<String>,
        latency_ms: u64,
    ) -> Self {
        Self {
            category,
            name: name.into(),
            ok: false,
            value: None,
            hint: Some(hint.into()),
            latency_ms: Some(latency_ms),
        }
    }
}

// ── Individual check functions ────────────────────────────────────────────────

/// Check that the config file exists in the data directory.
///
/// The template looks for `<data_dir>/config.toml` (e.g. `~/.example/config.toml`).
/// A missing config file is non-fatal — the binary works with env vars alone —
/// but the check warns so operators know where to place one if needed.
///
/// # TEMPLATE
/// If your service requires config.toml to be present, change `pass` to `fail`
/// when the file is not found.
pub fn check_config_file(data_dir: &Path) -> DoctorCheck {
    let config_path = data_dir.join("config.toml");

    if config_path.exists() {
        DoctorCheck::pass("config", "Config file", config_path.display().to_string())
    } else {
        // Non-fatal: env vars can supply all config.
        // TEMPLATE: Change `pass` → `fail` if config.toml is mandatory.
        DoctorCheck {
            category: "config",
            name: "Config file".into(),
            ok: true, // warning-level: missing is OK, env vars cover it
            value: Some(format!(
                "{} (not found — using env vars / defaults)",
                config_path.display()
            )),
            hint: None,
            latency_ms: None,
        }
    }
}

/// Check that a directory exists and is writable by the current process.
///
/// For missing directories the check returns a failure — many operations
/// (logging, auth DB, etc.) require a writable data dir.
///
/// # TEMPLATE
/// `label` is shown in the left column ("Data directory", "Log directory", …).
/// Add this check for every directory your service writes to.
pub fn check_dir_writable(label: &str, dir: &Path) -> DoctorCheck {
    let name = format!("{label}: {}", dir.display());

    // Attempt to create the directory if missing (idempotent).
    if let Err(e) = std::fs::create_dir_all(dir) {
        return DoctorCheck::fail(
            "config",
            name,
            format!(
                "Cannot create {}: {e}\n    → Check parent directory permissions.",
                dir.display()
            ),
        );
    }

    // Test writability by creating and removing a temp file.
    let test_file = dir.join(".doctor_write_test");
    match std::fs::write(&test_file, b"") {
        Ok(_) => {
            let _ = std::fs::remove_file(&test_file);

            // Report size if it's the log dir.
            let size_label = dir_size_label(dir);
            DoctorCheck::pass("config", name, format!("writable{size_label}"))
        }
        Err(e) => DoctorCheck::fail(
            "config",
            name,
            format!("Not writable: {e}\n    → Run: chmod u+w {}", dir.display()),
        ),
    }
}

/// Return a human-readable size label for a directory, or empty string on error.
fn dir_size_label(dir: &Path) -> String {
    fn du(dir: &Path) -> Option<u64> {
        let mut total = 0u64;
        let entries = std::fs::read_dir(dir).ok()?;
        for entry in entries.flatten() {
            let meta = entry.metadata().ok()?;
            if meta.is_file() {
                total += meta.len();
            } else if meta.is_dir() {
                total += du(&entry.path()).unwrap_or(0);
            }
        }
        Some(total)
    }

    match du(dir) {
        Some(bytes) if bytes > 0 => {
            if bytes < 1024 {
                format!(", {} B", bytes)
            } else if bytes < 1024 * 1024 {
                format!(", {:.1} KB", bytes as f64 / 1024.0)
            } else {
                format!(", {:.1} MB", bytes as f64 / (1024.0 * 1024.0))
            }
        }
        _ => String::new(),
    }
}

/// Check that the binary is on `$PATH`.
///
/// Claude Code stdio config (`~/.claude/settings.json`) resolves the binary by
/// name. If it is not in PATH the stdio transport will silently fail.
///
/// # TEMPLATE
/// Replace `"example"` with your binary name (matches Cargo.toml `[[bin]] name`).
pub fn check_binary_in_path(binary: &str) -> DoctorCheck {
    // `which`-style resolution: walk PATH entries looking for the binary.
    let path_var = std::env::var("PATH").unwrap_or_default();
    for dir in path_var.split(':') {
        let candidate = std::path::Path::new(dir).join(binary);
        if candidate.is_file() {
            return DoctorCheck::pass(
                "config",
                format!("Binary in PATH: {binary}"),
                candidate.display().to_string(),
            );
        }
    }

    DoctorCheck::fail(
        "config",
        format!("Binary in PATH: {binary}"),
        format!(
            "`{binary}` not found in $PATH.\n    \
             → Run: install.sh   (installs to ~/.local/bin)\n    \
             → Or:  cargo install --path .  (builds from source)\n    \
             → Then add ~/.local/bin to your PATH."
        ),
    )
}

/// Check that a required environment variable / config value is non-empty.
///
/// `var_name` is the env var name (for display and the hint message).
/// `value` is the resolved value from the loaded `Config` (which merges env +
/// config.toml, so a non-empty value here means it is actually configured).
///
/// # TEMPLATE
/// Call this once per required variable. Add entries for every var that must be
/// set before `example serve` will work.
pub fn check_required_var(var_name: &str, value: &str) -> DoctorCheck {
    if !value.is_empty() {
        // Redact the value — it may be a secret (API key, token).
        let display = redact(value);
        DoctorCheck::pass(
            "credentials",
            var_name.to_string(),
            format!("{display} (set)"),
        )
    } else {
        DoctorCheck::fail(
            "credentials",
            var_name.to_string(),
            format!(
                "Not set.\n    \
                 → Add to ~/.example/.env:  {var_name}=<your_value>\n    \
                 → Or export in your shell: export {var_name}=<your_value>\n    \
                 TEMPLATE: Replace ~/.example/ with your service data dir."
            ),
        )
    }
}

/// Redact a secret value for display — show the first 4 chars and mask the rest.
fn redact(s: &str) -> String {
    if s.len() <= 4 {
        return "*".repeat(s.len());
    }
    format!("{}****", &s[..4])
}

/// Check that the upstream service is reachable via HTTP GET.
///
/// Attempts `GET <url>/health` with a 5-second timeout. Records round-trip
/// latency. This check is non-fatal (ok=true on timeout) — a misconfigured
/// upstream should not block the doctor report entirely.
///
/// # TEMPLATE
/// Replace `/health` with your upstream's actual health endpoint.
/// If your upstream is not HTTP (e.g. GraphQL, gRPC), adapt this check.
/// If the upstream requires auth, add the API key header:
///   `.header("x-api-key", api_key)`
pub async fn check_upstream(base_url: &str) -> DoctorCheck {
    // TEMPLATE: Change "/health" to your upstream's actual probe path.
    //           If the upstream has no health endpoint, try GET / or HEAD /.
    let health_url = format!("{}/health", base_url.trim_end_matches('/'));

    let client = match reqwest::ClientBuilder::new()
        .timeout(std::time::Duration::from_secs(5))
        .danger_accept_invalid_certs(true) // tolerate self-signed certs in doctor
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            return DoctorCheck::fail(
                "connectivity",
                "Upstream reachable",
                format!("Could not build HTTP client: {e}"),
            )
        }
    };

    let start = Instant::now();
    match client.get(&health_url).send().await {
        Ok(resp) => {
            let elapsed = start.elapsed().as_millis() as u64;
            let status = resp.status();
            if status.is_success() {
                DoctorCheck::pass_with_latency(
                    "connectivity",
                    "Upstream reachable",
                    format!("{health_url} → {status} ({elapsed} ms)"),
                    elapsed,
                )
            } else {
                DoctorCheck::fail_with_latency(
                    "connectivity",
                    "Upstream reachable",
                    format!(
                        "HTTP {status} from {health_url}\n    \
                         → Check that the upstream service is healthy.\n    \
                         TEMPLATE: Verify the correct health endpoint path."
                    ),
                    elapsed,
                )
            }
        }
        Err(e) => {
            let elapsed = start.elapsed().as_millis() as u64;
            DoctorCheck::fail_with_latency(
                "connectivity",
                "Upstream reachable",
                format!(
                    "Could not reach {health_url}: {e}\n    \
                     → Check EXAMPLE_API_URL is correct and the service is running.\n    \
                     TEMPLATE: Replace EXAMPLE_API_URL with your service's env var."
                ),
                elapsed,
            )
        }
    }
}

/// Check that the configured MCP port is available (not already in use).
///
/// Binding on a port that is already taken causes `example serve` to fail at
/// startup. This check catches that problem before the server starts.
///
/// # TEMPLATE
/// Port 3000 is the template default. Your service's port is in config.toml
/// `[mcp] port` (e.g. 6970 for unrust, 9158 for rustify).
pub fn check_port_available(port: u16) -> DoctorCheck {
    match TcpListener::bind(("127.0.0.1", port)) {
        Ok(_) => DoctorCheck::pass("server", format!("MCP port {port}"), "available"),
        Err(e) => DoctorCheck::fail(
            "server",
            format!("MCP port {port}"),
            format!(
                "Port {port} is already in use: {e}\n    \
                 → Set EXAMPLE_MCP_PORT to a different port.\n    \
                 → Or stop the process using port {port}: ss -tlnp | grep :{port}\n    \
                 TEMPLATE: Replace EXAMPLE_MCP_PORT with your service prefix."
            ),
        ),
    }
}

/// Check that the auth configuration is consistent and safe.
///
/// Validates:
/// - Binding 0.0.0.0 without auth is rejected (§27).
/// - Reports which auth mode is active.
/// - Warns if no auth is configured.
///
/// # TEMPLATE
/// This check mirrors `validate_bind_security()` in main.rs but produces a
/// friendly report instead of aborting. No logic changes needed unless you
/// add a new auth mode.
pub fn check_auth_config(config: &Config) -> DoctorCheck {
    let is_loopback = config.mcp.host.starts_with("127.") || config.mcp.host == "::1";
    let has_token = config.mcp.api_token.is_some();
    let is_oauth = config.mcp.auth.mode == AuthMode::OAuth;
    let no_auth = config.mcp.no_auth;
    let noauth_override = std::env::var("EXAMPLE_NOAUTH")
        .map(|v| matches!(v.to_lowercase().as_str(), "true" | "1" | "yes"))
        .unwrap_or(false);

    // TEMPLATE: Replace "EXAMPLE_NOAUTH" with your service prefix.

    if is_loopback || no_auth {
        DoctorCheck::pass(
            "auth",
            "Auth mode",
            format!(
                "no-auth ({})",
                if is_loopback {
                    "loopback bind"
                } else {
                    "EXAMPLE_MCP_NO_AUTH=true"
                }
            ),
        )
    } else if is_oauth {
        DoctorCheck::pass("auth", "Auth mode", "OAuth (Google)")
    } else if has_token {
        DoctorCheck::pass("auth", "Auth mode", "bearer token (set)")
    } else if noauth_override {
        DoctorCheck::pass(
            "auth",
            "Auth mode",
            "no-auth (EXAMPLE_NOAUTH=true — upstream gateway handles auth)",
        )
    } else {
        // Binding 0.0.0.0 with no auth — §27 violation.
        DoctorCheck::fail(
            "auth",
            "Auth mode",
            format!(
                "Binding to {} with no authentication configured.\n    \
                 This violates §27 (pattern: No 0.0.0.0 Without Auth).\n    \
                 Fix ONE of:\n    \
                 1. Bind to loopback:    EXAMPLE_MCP_HOST=127.0.0.1\n    \
                 2. Set a bearer token:  EXAMPLE_MCP_TOKEN=$(openssl rand -hex 32)\n    \
                 3. Enable OAuth:        EXAMPLE_MCP_AUTH_MODE=oauth\n    \
                 4. Upstream gateway:    EXAMPLE_NOAUTH=true\n    \
                 TEMPLATE: Replace EXAMPLE_ prefix with your service prefix.",
                config.mcp.host
            ),
        )
    }
}

// ── Human-readable report ─────────────────────────────────────────────────────

/// Print the doctor report in human-readable coloured format.
///
/// Output follows the §48 layout:
///
/// ```text
/// example-mcp v0.1.0 — environment check
///
///   Config
///   ────────────────────────────────────────────
///   ✓ Config file:  ~/.example/config.toml
///   ✗ Data dir:     not writable
///     → Fix: chmod u+w ~/.example
///   ...
/// ```
///
/// # TEMPLATE
/// Section headings and the version string are the main things to customise.
/// Add new sections if you add new check categories beyond the five defaults.
fn print_doctor_report(checks: &[DoctorCheck]) {
    use std::io::IsTerminal;
    let color = std::io::stderr().is_terminal() && std::env::var_os("NO_COLOR").is_none();

    // ── ANSI helpers ──────────────────────────────────────────────────────────
    macro_rules! green {
        ($s:expr) => {
            if color {
                format!("\x1b[32m{}\x1b[0m", $s)
            } else {
                $s.to_string()
            }
        };
    }
    macro_rules! red {
        ($s:expr) => {
            if color {
                format!("\x1b[31m{}\x1b[0m", $s)
            } else {
                $s.to_string()
            }
        };
    }
    macro_rules! yellow {
        ($s:expr) => {
            if color {
                format!("\x1b[33m{}\x1b[0m", $s)
            } else {
                $s.to_string()
            }
        };
    }
    macro_rules! bold {
        ($s:expr) => {
            if color {
                format!("\x1b[1m{}\x1b[0m", $s)
            } else {
                $s.to_string()
            }
        };
    }
    macro_rules! dim {
        ($s:expr) => {
            if color {
                format!("\x1b[2m{}\x1b[0m", $s)
            } else {
                $s.to_string()
            }
        };
    }

    // TEMPLATE: Replace "example-mcp" with your service name and binary name.
    println!();
    println!(
        "{}",
        bold!(format!(
            "example-mcp v{} — environment check",
            env!("CARGO_PKG_VERSION")
        ))
    );
    println!();

    // Group checks by category and print in order.
    // TEMPLATE: Reorder categories or add new ones to match your service.
    let categories: &[(&str, &str)] = &[
        ("config", "Config"),
        ("credentials", "Service credentials"),
        ("connectivity", "Connectivity"),
        ("server", "MCP server"),
        ("auth", "Authentication"),
    ];

    for (cat_key, cat_label) in categories {
        let cat_checks: Vec<&DoctorCheck> =
            checks.iter().filter(|c| c.category == *cat_key).collect();
        if cat_checks.is_empty() {
            continue;
        }

        println!("  {}", bold!(cat_label));
        println!("  {}", dim!("─".repeat(44)));

        for check in &cat_checks {
            if check.ok {
                let value = check.value.as_deref().unwrap_or("");
                let latency = check
                    .latency_ms
                    .map(|ms| format!(" ({ms} ms)"))
                    .unwrap_or_default();
                println!(
                    "  {}  {:<28}  {}{}",
                    green!("✓"),
                    check.name,
                    value,
                    latency
                );
            } else {
                println!("  {}  {}", red!("✗"), check.name);
                if let Some(hint) = &check.hint {
                    for line in hint.lines() {
                        println!("    {}", yellow!(line));
                    }
                }
            }
        }

        println!();
    }

    // ── Summary line ──────────────────────────────────────────────────────────
    let issues = checks.iter().filter(|c| !c.ok).count();
    println!("  {}", dim!("━".repeat(44)));

    if issues == 0 {
        println!(
            "  {}  All checks passed. Run: {}",
            green!("✓"),
            bold!("example serve")
        );
    } else {
        // TEMPLATE: Replace "example serve" with your binary name.
        let noun = if issues == 1 { "issue" } else { "issues" };
        println!(
            "  {}  {} {noun} found. Fix before running: {}",
            red!("✗"),
            red!(issues.to_string()),
            bold!("example serve")
        );
    }
    println!();
}

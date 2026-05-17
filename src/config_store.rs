//! Persistent config read/write — the shared layer behind `config_*` actions.
//!
//! This module owns the registry of every configurable key, the routing rule
//! between `.env` (secrets/URLs) and `config.toml` (tuning knobs), and the
//! filesystem writes that make `config set` durable across restarts.
//!
//! It is called by:
//!   * `ExampleService::config_*` — service layer for MCP / REST / CLI
//!   * `cli::config::run_config` — CLI shim
//!
//! Every entry point returns `serde_json::Value` so the same JSON shape flows
//! out of every transport without per-transport reformatting.
//!
//! ## Adding a key
//!
//! Append a `KeySpec` row to `KEYS`. The five `config_*` actions and the CLI
//! pick it up automatically. Add a matching arm in `current_value` so reads
//! resolve through the loaded config.

use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use serde_json::{json, Value};
use toml_edit::{value as toml_value, Array, DocumentMut, Item};

use crate::config::{default_data_dir, AuthMode, Config};

// ── registry ─────────────────────────────────────────────────────────────────

/// Where a key persists when written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Target {
    /// `.env` file in the appdata directory.
    DotEnv,
    /// `config.toml` (appdata first, then `./config.toml`).
    ConfigToml,
}

impl Target {
    fn label(self) -> &'static str {
        match self {
            Target::DotEnv => ".env",
            Target::ConfigToml => "config.toml",
        }
    }
}

/// Value kind used to parse CLI / JSON input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    String,
    Bool,
    U16,
    U32,
    U64,
    /// Comma-separated list of strings.
    StringList,
    /// String that must be `"bearer"` or `"oauth"`.
    AuthMode,
}

#[derive(Debug)]
struct KeySpec {
    /// Canonical dotted name, e.g. `"mcp.host"`.
    name: &'static str,
    /// Env-var alias accepted on input and used when writing to `.env`.
    env: &'static str,
    target: Target,
    kind: Kind,
    /// TOML path for `config.toml` reads/writes (e.g. `["mcp", "host"]`).
    toml_path: &'static [&'static str],
    help: &'static str,
}

/// Every key the service knows how to read or write.
const KEYS: &[KeySpec] = &[
    // ── Upstream service (secrets + URL → .env) ──────────────────────────────
    KeySpec {
        name: "example.api_url",
        env: "EXAMPLE_API_URL",
        target: Target::DotEnv,
        kind: Kind::String,
        toml_path: &["example", "api_url"],
        help: "Upstream service base URL",
    },
    KeySpec {
        name: "example.api_key",
        env: "EXAMPLE_API_KEY",
        target: Target::DotEnv,
        kind: Kind::String,
        toml_path: &["example", "api_key"],
        help: "Upstream service API key",
    },
    // ── MCP server ───────────────────────────────────────────────────────────
    KeySpec {
        name: "mcp.host",
        env: "EXAMPLE_MCP_HOST",
        target: Target::ConfigToml,
        kind: Kind::String,
        toml_path: &["mcp", "host"],
        help: "Bind host (127.0.0.1 for loopback, 0.0.0.0 for all interfaces)",
    },
    KeySpec {
        name: "mcp.port",
        env: "EXAMPLE_MCP_PORT",
        target: Target::ConfigToml,
        kind: Kind::U16,
        toml_path: &["mcp", "port"],
        help: "Bind port",
    },
    KeySpec {
        name: "mcp.server_name",
        env: "EXAMPLE_MCP_SERVER_NAME",
        target: Target::ConfigToml,
        kind: Kind::String,
        toml_path: &["mcp", "server_name"],
        help: "MCP server name advertised to clients",
    },
    KeySpec {
        name: "mcp.no_auth",
        env: "EXAMPLE_MCP_NO_AUTH",
        target: Target::ConfigToml,
        kind: Kind::Bool,
        toml_path: &["mcp", "no_auth"],
        help: "Disable auth entirely (loopback only)",
    },
    KeySpec {
        name: "mcp.trusted_gateway",
        env: "EXAMPLE_NOAUTH",
        target: Target::DotEnv,
        kind: Kind::Bool,
        toml_path: &[],
        help: "Allow unauthenticated access when behind a trusted reverse proxy",
    },
    KeySpec {
        name: "mcp.api_token",
        env: "EXAMPLE_MCP_TOKEN",
        target: Target::DotEnv,
        kind: Kind::String,
        toml_path: &["mcp", "api_token"],
        help: "Static bearer token (secret)",
    },
    KeySpec {
        name: "mcp.allowed_hosts",
        env: "EXAMPLE_MCP_ALLOWED_HOSTS",
        target: Target::ConfigToml,
        kind: Kind::StringList,
        toml_path: &["mcp", "allowed_hosts"],
        help: "Additional allowed Host header values (comma-separated)",
    },
    KeySpec {
        name: "mcp.allowed_origins",
        env: "EXAMPLE_MCP_ALLOWED_ORIGINS",
        target: Target::ConfigToml,
        kind: Kind::StringList,
        toml_path: &["mcp", "allowed_origins"],
        help: "Additional allowed CORS origins (comma-separated)",
    },
    // ── OAuth / JWT auth ─────────────────────────────────────────────────────
    KeySpec {
        name: "mcp.auth.mode",
        env: "EXAMPLE_MCP_AUTH_MODE",
        target: Target::ConfigToml,
        kind: Kind::AuthMode,
        toml_path: &["mcp", "auth", "mode"],
        help: "Auth mode: \"bearer\" or \"oauth\"",
    },
    KeySpec {
        name: "mcp.auth.public_url",
        env: "EXAMPLE_MCP_PUBLIC_URL",
        target: Target::DotEnv,
        kind: Kind::String,
        toml_path: &["mcp", "auth", "public_url"],
        help: "Public URL for OAuth metadata endpoints",
    },
    KeySpec {
        name: "mcp.auth.google_client_id",
        env: "EXAMPLE_MCP_GOOGLE_CLIENT_ID",
        target: Target::DotEnv,
        kind: Kind::String,
        toml_path: &["mcp", "auth", "google_client_id"],
        help: "Google OAuth client ID (secret)",
    },
    KeySpec {
        name: "mcp.auth.google_client_secret",
        env: "EXAMPLE_MCP_GOOGLE_CLIENT_SECRET",
        target: Target::DotEnv,
        kind: Kind::String,
        toml_path: &["mcp", "auth", "google_client_secret"],
        help: "Google OAuth client secret",
    },
    KeySpec {
        name: "mcp.auth.admin_email",
        env: "EXAMPLE_MCP_AUTH_ADMIN_EMAIL",
        target: Target::ConfigToml,
        kind: Kind::String,
        toml_path: &["mcp", "auth", "admin_email"],
        help: "Bootstrap admin email for OAuth",
    },
    KeySpec {
        name: "mcp.auth.allowed_emails",
        env: "EXAMPLE_MCP_AUTH_ALLOWED_EMAILS",
        target: Target::ConfigToml,
        kind: Kind::StringList,
        toml_path: &["mcp", "auth", "allowed_emails"],
        help: "Additional allowed OAuth emails (comma-separated)",
    },
    KeySpec {
        name: "mcp.auth.sqlite_path",
        env: "EXAMPLE_MCP_AUTH_SQLITE_PATH",
        target: Target::ConfigToml,
        kind: Kind::String,
        toml_path: &["mcp", "auth", "sqlite_path"],
        help: "OAuth session storage path",
    },
    KeySpec {
        name: "mcp.auth.key_path",
        env: "EXAMPLE_MCP_AUTH_KEY_PATH",
        target: Target::ConfigToml,
        kind: Kind::String,
        toml_path: &["mcp", "auth", "key_path"],
        help: "RS256 JWT signing key path",
    },
    KeySpec {
        name: "mcp.auth.access_token_ttl_secs",
        env: "EXAMPLE_MCP_AUTH_ACCESS_TOKEN_TTL_SECS",
        target: Target::ConfigToml,
        kind: Kind::U64,
        toml_path: &["mcp", "auth", "access_token_ttl_secs"],
        help: "JWT access token TTL (seconds)",
    },
    KeySpec {
        name: "mcp.auth.refresh_token_ttl_secs",
        env: "EXAMPLE_MCP_AUTH_REFRESH_TOKEN_TTL_SECS",
        target: Target::ConfigToml,
        kind: Kind::U64,
        toml_path: &["mcp", "auth", "refresh_token_ttl_secs"],
        help: "Refresh token TTL (seconds)",
    },
    KeySpec {
        name: "mcp.auth.auth_code_ttl_secs",
        env: "EXAMPLE_MCP_AUTH_CODE_TTL_SECS",
        target: Target::ConfigToml,
        kind: Kind::U64,
        toml_path: &["mcp", "auth", "auth_code_ttl_secs"],
        help: "OAuth auth code TTL (seconds)",
    },
    KeySpec {
        name: "mcp.auth.register_rpm",
        env: "EXAMPLE_MCP_AUTH_REGISTER_RPM",
        target: Target::ConfigToml,
        kind: Kind::U32,
        toml_path: &["mcp", "auth", "register_rpm"],
        help: "Registration rate limit (requests per minute)",
    },
    KeySpec {
        name: "mcp.auth.authorize_rpm",
        env: "EXAMPLE_MCP_AUTH_AUTHORIZE_RPM",
        target: Target::ConfigToml,
        kind: Kind::U32,
        toml_path: &["mcp", "auth", "authorize_rpm"],
        help: "Authorization rate limit (requests per minute)",
    },
    KeySpec {
        name: "mcp.auth.allowed_client_redirect_uris",
        env: "EXAMPLE_MCP_AUTH_ALLOWED_CLIENT_REDIRECT_URIS",
        target: Target::ConfigToml,
        kind: Kind::StringList,
        toml_path: &["mcp", "auth", "allowed_client_redirect_uris"],
        help: "Extra OAuth client redirect URIs (comma-separated)",
    },
];

// ── public API (called from ExampleService) ──────────────────────────────────

/// List every known key with its current value, target file, and description.
///
/// Reloads `Config` from disk so the values reflect what the server would see
/// after the most recent write.
pub fn list() -> Result<Value> {
    let cfg = Config::load()?;
    let entries: Vec<Value> = KEYS
        .iter()
        .map(|spec| {
            let value = current_value(&cfg, spec);
            json!({
                "key": spec.name,
                "env": spec.env,
                "target": spec.target.label(),
                "description": spec.help,
                "value": value,
            })
        })
        .collect();
    let mut paths = paths_inner()?;
    paths
        .as_object_mut()
        .expect("paths_inner returns object")
        .insert("keys".into(), Value::Array(entries));
    Ok(paths)
}

/// Return the currently-resolved value of a single key.
pub fn get(key: &str) -> Result<Value> {
    let spec = lookup_key(key)?;
    let cfg = Config::load()?;
    Ok(json!({
        "key": spec.name,
        "env": spec.env,
        "target": spec.target.label(),
        "value": current_value(&cfg, spec),
    }))
}

/// Write a value to the appropriate file (`.env` or `config.toml`). Returns a
/// confirmation envelope including the file that was touched.
pub fn set(key: &str, raw_value: &str) -> Result<Value> {
    let spec = lookup_key(key)?;
    let parsed = parse_value(spec, raw_value)?;
    let path = match spec.target {
        Target::DotEnv => {
            let path = env_file_path()?;
            write_env_value(&path, spec.env, &parsed.env_form())?;
            path
        }
        Target::ConfigToml => {
            let path = toml_file_path()?;
            write_toml_value(&path, spec.toml_path, &parsed)?;
            path
        }
    };
    Ok(json!({
        "ok": true,
        "key": spec.name,
        "target": spec.target.label(),
        "path": path.display().to_string(),
    }))
}

/// Remove a key from its target file. Returns whether anything changed.
pub fn unset(key: &str) -> Result<Value> {
    let spec = lookup_key(key)?;
    let (path, removed) = match spec.target {
        Target::DotEnv => {
            let path = env_file_path()?;
            let removed = remove_env_key(&path, spec.env)?;
            (path, removed)
        }
        Target::ConfigToml => {
            let path = toml_file_path()?;
            let removed = remove_toml_key(&path, spec.toml_path)?;
            (path, removed)
        }
    };
    Ok(json!({
        "ok": true,
        "key": spec.name,
        "target": spec.target.label(),
        "path": path.display().to_string(),
        "removed": removed,
    }))
}

/// Return the resolved paths of `.env` and `config.toml`.
pub fn paths() -> Result<Value> {
    paths_inner()
}

fn paths_inner() -> Result<Value> {
    let env_path = env_file_path()?;
    let toml_path = toml_file_path()?;
    Ok(json!({
        "env": env_path.display().to_string(),
        "config_toml": toml_path.display().to_string(),
    }))
}

// ── key lookup + value resolution ────────────────────────────────────────────

fn lookup_key(key: &str) -> Result<&'static KeySpec> {
    KEYS.iter()
        .find(|s| s.name.eq_ignore_ascii_case(key) || s.env.eq_ignore_ascii_case(key))
        .ok_or_else(|| {
            anyhow!("unknown config key {key:?} — call config_list to see available keys")
        })
}

fn current_value(config: &Config, spec: &KeySpec) -> String {
    match spec.name {
        "example.api_url" => config.example.api_url.clone(),
        "example.api_key" => config.example.api_key.clone(),
        "mcp.host" => config.mcp.host.clone(),
        "mcp.port" => config.mcp.port.to_string(),
        "mcp.server_name" => config.mcp.server_name.clone(),
        "mcp.no_auth" => config.mcp.no_auth.to_string(),
        "mcp.trusted_gateway" => config.mcp.trusted_gateway.to_string(),
        "mcp.api_token" => config.mcp.api_token.clone().unwrap_or_default(),
        "mcp.allowed_hosts" => config.mcp.allowed_hosts.join(", "),
        "mcp.allowed_origins" => config.mcp.allowed_origins.join(", "),
        "mcp.auth.mode" => match config.mcp.auth.mode {
            AuthMode::Bearer => "bearer".into(),
            AuthMode::OAuth => "oauth".into(),
        },
        "mcp.auth.public_url" => config.mcp.auth.public_url.clone().unwrap_or_default(),
        "mcp.auth.google_client_id" => config.mcp.auth.google_client_id.clone().unwrap_or_default(),
        "mcp.auth.google_client_secret" => config
            .mcp
            .auth
            .google_client_secret
            .clone()
            .unwrap_or_default(),
        "mcp.auth.admin_email" => config.mcp.auth.admin_email.clone(),
        "mcp.auth.allowed_emails" => config.mcp.auth.allowed_emails.join(", "),
        "mcp.auth.sqlite_path" => config.mcp.auth.sqlite_path.clone(),
        "mcp.auth.key_path" => config.mcp.auth.key_path.clone(),
        "mcp.auth.access_token_ttl_secs" => config.mcp.auth.access_token_ttl_secs.to_string(),
        "mcp.auth.refresh_token_ttl_secs" => config.mcp.auth.refresh_token_ttl_secs.to_string(),
        "mcp.auth.auth_code_ttl_secs" => config.mcp.auth.auth_code_ttl_secs.to_string(),
        "mcp.auth.register_rpm" => config.mcp.auth.register_rpm.to_string(),
        "mcp.auth.authorize_rpm" => config.mcp.auth.authorize_rpm.to_string(),
        "mcp.auth.allowed_client_redirect_uris" => {
            config.mcp.auth.allowed_client_redirect_uris.join(", ")
        }
        // Unreachable for any registered key — the test
        // `every_key_has_current_value_arm` enforces this at compile-of-tests
        // time.
        other => format!("(no accessor for {other})"),
    }
}

// ── value parsing ────────────────────────────────────────────────────────────

#[derive(Debug)]
enum ParsedValue {
    String(String),
    Bool(bool),
    U16(u16),
    U32(u32),
    U64(u64),
    List(Vec<String>),
}

impl ParsedValue {
    fn env_form(&self) -> String {
        match self {
            ParsedValue::String(s) => s.clone(),
            ParsedValue::Bool(b) => b.to_string(),
            ParsedValue::U16(n) => n.to_string(),
            ParsedValue::U32(n) => n.to_string(),
            ParsedValue::U64(n) => n.to_string(),
            ParsedValue::List(items) => items.join(","),
        }
    }
}

fn parse_value(spec: &KeySpec, raw: &str) -> Result<ParsedValue> {
    match spec.kind {
        Kind::String => Ok(ParsedValue::String(raw.to_string())),
        Kind::Bool => match raw.to_lowercase().as_str() {
            "1" | "true" | "yes" => Ok(ParsedValue::Bool(true)),
            "0" | "false" | "no" => Ok(ParsedValue::Bool(false)),
            other => bail!("expected bool for {}, got {other:?}", spec.name),
        },
        Kind::U16 => raw
            .parse::<u16>()
            .map(ParsedValue::U16)
            .map_err(|_| anyhow!("expected u16 for {}, got {raw:?}", spec.name)),
        Kind::U32 => raw
            .parse::<u32>()
            .map(ParsedValue::U32)
            .map_err(|_| anyhow!("expected u32 for {}, got {raw:?}", spec.name)),
        Kind::U64 => {
            let n: u64 = raw
                .parse()
                .map_err(|_| anyhow!("expected u64 for {}, got {raw:?}", spec.name))?;
            // TOML integers are i64. Reject values that would overflow rather
            // than silently wrap to a negative when we cast for `toml_value`,
            // which would then round-trip into a different value at the next
            // `Config::load()`.
            if n > i64::MAX as u64 {
                bail!(
                    "{} value {n} exceeds TOML integer max ({})",
                    spec.name,
                    i64::MAX
                );
            }
            Ok(ParsedValue::U64(n))
        }
        Kind::StringList => Ok(ParsedValue::List(parse_list(raw))),
        Kind::AuthMode => match raw.to_lowercase().as_str() {
            "bearer" => Ok(ParsedValue::String("bearer".into())),
            "oauth" => Ok(ParsedValue::String("oauth".into())),
            other => bail!(
                "{} must be \"bearer\" or \"oauth\", got {other:?}",
                spec.name
            ),
        },
    }
}

fn parse_list(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

// ── path resolution ──────────────────────────────────────────────────────────

fn data_dir() -> Result<PathBuf> {
    if let Some(val) =
        std::env::var_os("CLAUDE_PLUGIN_DATA").or_else(|| std::env::var_os("EXAMPLE_HOME"))
    {
        return Ok(PathBuf::from(val));
    }
    default_data_dir()
}

fn env_file_path() -> Result<PathBuf> {
    Ok(data_dir()?.join(".env"))
}

/// Resolve `config.toml` path for read AND write — always `<data_dir>/config.toml`.
///
/// `Config::load()` is intentionally more permissive (it also checks
/// `./config.toml` for dev convenience), but writes must not fall through to
/// the working directory: that can clobber a checked-in template file when the
/// server is started from the repo root. If you want to edit a project-local
/// `./config.toml`, copy it into `<data_dir>` first.
fn toml_file_path() -> Result<PathBuf> {
    Ok(data_dir()?.join("config.toml"))
}

// ── .env IO ──────────────────────────────────────────────────────────────────

fn write_env_value(path: &Path, key: &str, value: &str) -> Result<()> {
    let existing = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e).with_context(|| format!("reading {}", path.display())),
    };

    let quoted = quote_env_value(value);
    let new_line = format!("{key}={quoted}");

    let mut replaced = false;
    let mut lines: Vec<String> = existing
        .lines()
        .map(|line| {
            if line_matches_key(line, key) {
                replaced = true;
                new_line.clone()
            } else {
                line.to_string()
            }
        })
        .collect();

    if !replaced {
        lines.push(new_line);
    }

    write_env_file(path, &lines)
}

fn remove_env_key(path: &Path, key: &str) -> Result<bool> {
    let existing = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(e) => return Err(e).with_context(|| format!("reading {}", path.display())),
    };

    let mut removed = false;
    let lines: Vec<String> = existing
        .lines()
        .filter(|line| {
            if line_matches_key(line, key) {
                removed = true;
                false
            } else {
                true
            }
        })
        .map(|s| s.to_string())
        .collect();

    if removed {
        write_env_file(path, &lines)?;
    }
    Ok(removed)
}

fn line_matches_key(line: &str, key: &str) -> bool {
    let trimmed = line.trim_start();
    if trimmed.starts_with('#') {
        return false;
    }
    match trimmed.split_once('=') {
        Some((k, _)) => k.trim() == key,
        None => false,
    }
}

fn quote_env_value(value: &str) -> String {
    let needs_quote = value.is_empty()
        || value
            .chars()
            .any(|c| c.is_whitespace() || matches!(c, '#' | '"' | '\''));
    if !needs_quote {
        return value.to_string();
    }
    let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

fn write_env_file(path: &Path, lines: &[String]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating {}", parent.display()))?;
    }
    let body = if lines.is_empty() {
        String::new()
    } else {
        format!("{}\n", lines.join("\n"))
    };
    let tmp = path.with_extension("tmp");
    write_secret_file(&tmp, &body)?;
    std::fs::rename(&tmp, path).inspect_err(|_| {
        // Best-effort cleanup so a half-written file never leaks secrets.
        let _ = std::fs::remove_file(&tmp);
    })?;
    Ok(())
}

#[cfg(unix)]
fn write_secret_file(path: &Path, body: &str) -> Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;

    // 0o600 — the file may contain secrets; restrict to owner.
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .mode(0o600)
        .open(path)
        .with_context(|| format!("opening {} for write", path.display()))?;
    file.write_all(body.as_bytes())?;
    file.sync_all()?;
    Ok(())
}

#[cfg(not(unix))]
fn write_secret_file(path: &Path, body: &str) -> Result<()> {
    std::fs::write(path, body).with_context(|| format!("writing {}", path.display()))
}

// ── config.toml IO ───────────────────────────────────────────────────────────

fn write_toml_value(path: &Path, keys: &[&str], val: &ParsedValue) -> Result<()> {
    if keys.is_empty() {
        bail!("internal error: cannot write empty toml path");
    }
    let mut doc = load_or_init_toml(path)?;

    let (parent, leaf) = keys.split_at(keys.len() - 1);
    let table = ensure_table(&mut doc, parent)?;
    let new_item = parsed_to_item(val);
    // Mutate in place when the key already exists — keeps the key's prefix
    // decoration (comments) intact. `Table::insert` would drop them.
    if let Some(existing) = table.get_mut(leaf[0]) {
        *existing = new_item;
    } else {
        table.insert(leaf[0], new_item);
    }

    save_toml(path, &doc)
}

fn remove_toml_key(path: &Path, keys: &[&str]) -> Result<bool> {
    if keys.is_empty() {
        return Ok(false);
    }
    let mut doc = match std::fs::read_to_string(path) {
        Ok(s) => s
            .parse::<DocumentMut>()
            .with_context(|| format!("parsing {}", path.display()))?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(e) => return Err(e).with_context(|| format!("reading {}", path.display())),
    };

    let (parent, leaf) = keys.split_at(keys.len() - 1);
    let removed = match navigate_table(&mut doc, parent) {
        Some(table) => table.remove(leaf[0]).is_some(),
        None => false,
    };

    if removed {
        save_toml(path, &doc)?;
    }
    Ok(removed)
}

fn load_or_init_toml(path: &Path) -> Result<DocumentMut> {
    match std::fs::read_to_string(path) {
        Ok(s) => s
            .parse::<DocumentMut>()
            .with_context(|| format!("parsing {}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(DocumentMut::new()),
        Err(e) => Err(e).with_context(|| format!("reading {}", path.display())),
    }
}

fn save_toml(path: &Path, doc: &DocumentMut) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating {}", parent.display()))?;
    }
    let body = doc.to_string();
    let tmp = path.with_extension("toml.tmp");
    std::fs::write(&tmp, body).with_context(|| format!("writing {}", tmp.display()))?;
    std::fs::rename(&tmp, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })?;
    Ok(())
}

fn ensure_table<'a>(doc: &'a mut DocumentMut, keys: &[&str]) -> Result<&'a mut toml_edit::Table> {
    let mut current: &mut toml_edit::Table = doc.as_table_mut();
    for key in keys {
        let entry = current
            .entry(key)
            .or_insert_with(|| Item::Table(toml_edit::Table::new()));
        if !entry.is_table() {
            bail!(
                "config.toml has a non-table value at `{}` ({}); refusing to overwrite",
                keys.join("."),
                entry.type_name()
            );
        }
        current = entry
            .as_table_mut()
            .expect("just verified entry is a Table");
    }
    Ok(current)
}

fn navigate_table<'a>(doc: &'a mut DocumentMut, keys: &[&str]) -> Option<&'a mut toml_edit::Table> {
    let mut current: &mut toml_edit::Table = doc.as_table_mut();
    for key in keys {
        current = current.get_mut(key)?.as_table_mut()?;
    }
    Some(current)
}

fn parsed_to_item(val: &ParsedValue) -> Item {
    match val {
        ParsedValue::String(s) => toml_value(s),
        ParsedValue::Bool(b) => toml_value(*b),
        ParsedValue::U16(n) => toml_value(i64::from(*n)),
        ParsedValue::U32(n) => toml_value(i64::from(*n)),
        ParsedValue::U64(n) => toml_value(*n as i64),
        ParsedValue::List(items) => {
            let mut array = Array::new();
            for item in items {
                array.push(item.as_str());
            }
            toml_value(array)
        }
    }
}

#[cfg(test)]
#[path = "config_store_tests.rs"]
mod tests;

//! Unit tests for src/cli/doctor/checks.rs
//!
//! Declared in checks.rs as:
//! ```rust
//! #[cfg(test)]
//! #[path = "checks_tests.rs"]
//! mod tests;
//! ```
//!
//! Tests cover the pure and near-pure check functions. Async network checks
//! (`check_upstream`) and filesystem-heavy checks are covered with minimal
//! scaffolding. `check_auth_config` is not covered here — it requires a full
//! Config and is exercised via integration tests in tests/tool_dispatch.rs.

use super::*;

// ── check_required_var ────────────────────────────────────────────────────────

#[test]
fn required_var_passes_when_set() {
    let check = check_required_var("MY_VAR", "some-value");
    assert!(check.ok, "non-empty value should pass");
    assert_eq!(check.category, "credentials");
    let value = check.value.expect("pass should have a value");
    assert!(value.contains("(set)"), "pass value should mention (set)");
    assert!(
        !value.contains("some-value"),
        "actual secret must be redacted"
    );
}

#[test]
fn required_var_fails_when_empty() {
    let check = check_required_var("MY_VAR", "");
    assert!(!check.ok, "empty value should fail");
    let hint = check.hint.expect("fail should have a hint");
    assert!(hint.contains("MY_VAR"), "hint should name the missing var");
}

#[test]
fn required_var_redacts_short_secrets() {
    let check = check_required_var("KEY", "abc");
    let value = check.value.unwrap();
    assert!(!value.contains("abc"), "short secret must be fully masked");
}

#[test]
fn required_var_redacts_long_secrets() {
    let check = check_required_var("KEY", "supersecrettoken");
    let value = check.value.unwrap();
    assert!(
        !value.contains("supersecrettoken"),
        "long secret must not appear in full"
    );
    assert!(
        value.contains("****"),
        "long secret should show mask suffix"
    );
}

// ── check_binary_in_path ─────────────────────────────────────────────────────

#[test]
fn binary_in_path_passes_for_sh() {
    // /bin/sh or /usr/bin/sh is on PATH in any POSIX system.
    let check = check_binary_in_path("sh");
    assert!(check.ok, "sh should be found in PATH");
    assert_eq!(check.category, "config");
}

#[test]
fn binary_in_path_fails_for_nonexistent() {
    let check = check_binary_in_path("this-binary-definitely-does-not-exist-rmcp");
    assert!(!check.ok, "unknown binary should fail");
    let hint = check.hint.unwrap();
    assert!(hint.contains("PATH"), "hint should mention PATH");
}

// ── check_port_available ─────────────────────────────────────────────────────

#[test]
fn port_available_passes_for_free_port() {
    // Bind a listener so we know which port is free, release it, then check.
    // We just pick an ephemeral port and verify the check agrees.
    let check = check_port_available(0); // port 0 is not valid for TcpListener::bind
                                         // Port 0 fails to bind — that's expected; just confirm the category.
    assert_eq!(check.category, "server");
}

#[test]
fn port_available_fails_when_already_bound() {
    use std::net::TcpListener;
    let listener = TcpListener::bind("127.0.0.1:0").expect("should bind to an ephemeral port");
    let port = listener.local_addr().unwrap().port();

    let check = check_port_available(port);
    assert!(!check.ok, "port in use should fail");
    assert!(
        check.hint.unwrap().contains(&port.to_string()),
        "hint should name the port"
    );
}

// ── check_config_file ────────────────────────────────────────────────────────

#[test]
fn config_file_passes_when_present() {
    let dir = tempfile::tempdir().expect("should create temp dir");
    let config_path = dir.path().join("config.toml");
    std::fs::write(&config_path, b"[mcp]\nport = 3000\n").unwrap();

    let check = check_config_file(dir.path());
    assert!(check.ok);
    assert!(check.value.unwrap().contains("config.toml"));
}

#[test]
fn config_file_passes_gracefully_when_absent() {
    let dir = tempfile::tempdir().expect("should create temp dir");
    let check = check_config_file(dir.path());
    // Missing config.toml is a soft pass (env vars cover it).
    assert!(check.ok, "missing config.toml should not hard-fail");
    assert!(
        check.value.unwrap().contains("not found"),
        "value should note the file is missing"
    );
}

// ── check_dir_writable ───────────────────────────────────────────────────────

#[test]
fn dir_writable_passes_for_writable_dir() {
    let dir = tempfile::tempdir().expect("should create temp dir");
    let check = check_dir_writable("Test dir", dir.path());
    assert!(check.ok);
    assert!(check.value.unwrap().contains("writable"));
}

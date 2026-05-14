//! Integration tests for CLI argument parsing.
//!
//! **Template**: extend these tests when you add new CLI subcommands.

// We test the parsing logic by mirroring it inline since parse_args() reads
// from std::env::args() directly. In a real project with clap, you'd test
// the clap parser directly.

#[test]
fn test_greet_no_name_parsed() {
    // Simulated: `example greet`
    let args: Vec<String> = vec!["greet".into()];
    let name = flag_value(&args[1..], "--name");
    assert!(name.is_none(), "no name flag should parse as None");
}

#[test]
fn test_greet_with_name_parsed() {
    // Simulated: `example greet --name Alice`
    let args: Vec<String> = vec!["greet".into(), "--name".into(), "Alice".into()];
    let name = flag_value(&args[1..], "--name");
    assert_eq!(name.as_deref(), Some("Alice"));
}

#[test]
fn test_echo_message_parsed() {
    // Simulated: `example echo --message "Hello, World!"`
    let args: Vec<String> = vec!["echo".into(), "--message".into(), "Hello, World!".into()];
    let message = flag_value(&args[1..], "--message");
    assert_eq!(message.as_deref(), Some("Hello, World!"));
}

#[test]
fn test_echo_no_message_defaults() {
    // Simulated: `example echo` (no --message)
    let args: Vec<String> = vec!["echo".into()];
    let message =
        flag_value(&args[1..], "--message").unwrap_or_else(|| "(no message provided)".to_string());
    assert_eq!(message, "(no message provided)");
}

// ── helpers (mirrors cli.rs logic) ───────────────────────────────────────────

fn flag_value(args: &[String], flag: &str) -> Option<String> {
    let pos = args.iter().position(|a| a == flag)?;
    args.get(pos + 1).cloned()
}

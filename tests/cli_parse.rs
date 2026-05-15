//! Integration tests for CLI argument parsing.
//!
//! **Template**: extend these tests when you add new CLI subcommands.

use rmcp_template::cli::{parse_args_from, Command, SetupCommand};

#[test]
fn test_greet_no_name_parsed() {
    assert_eq!(
        parse_args_from(["greet"]).unwrap(),
        Some(Command::Greet { name: None })
    );
}

#[test]
fn test_greet_with_name_parsed() {
    assert_eq!(
        parse_args_from(["greet", "--name", "Alice"]).unwrap(),
        Some(Command::Greet {
            name: Some("Alice".into())
        })
    );
}

#[test]
fn test_echo_message_parsed() {
    assert_eq!(
        parse_args_from(["echo", "--message", "Hello, World!"]).unwrap(),
        Some(Command::Echo {
            message: "Hello, World!".into()
        })
    );
}

#[test]
fn test_echo_no_message_is_rejected() {
    let error = parse_args_from(["echo"]).unwrap_err();
    assert!(error.to_string().contains("requires non-empty --message"));
}

#[test]
fn test_watch_bad_interval_is_rejected() {
    let error = parse_args_from(["watch", "--interval", "nope"]).unwrap_err();
    assert!(error.to_string().contains("--interval"));
}

#[test]
fn test_setup_plugin_hook_no_repair_parsed() {
    assert_eq!(
        parse_args_from(["setup", "plugin-hook", "--no-repair"]).unwrap(),
        Some(Command::Setup(SetupCommand::PluginHook { no_repair: true }))
    );
}

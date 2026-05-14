//! Integration tests for CLI argument parsing.
//!
//! **Template**: extend these tests when you add new CLI subcommands.

use rmcp_template::cli::{parse_args_from, Command, SetupCommand};

#[test]
fn test_greet_no_name_parsed() {
    assert_eq!(
        parse_args_from(["greet"]),
        Some(Command::Greet { name: None })
    );
}

#[test]
fn test_greet_with_name_parsed() {
    assert_eq!(
        parse_args_from(["greet", "--name", "Alice"]),
        Some(Command::Greet {
            name: Some("Alice".into())
        })
    );
}

#[test]
fn test_echo_message_parsed() {
    assert_eq!(
        parse_args_from(["echo", "--message", "Hello, World!"]),
        Some(Command::Echo {
            message: "Hello, World!".into()
        })
    );
}

#[test]
fn test_echo_no_message_defaults() {
    assert_eq!(
        parse_args_from(["echo"]),
        Some(Command::Echo {
            message: "(no message provided)".into()
        })
    );
}

#[test]
fn test_setup_plugin_hook_no_repair_parsed() {
    assert_eq!(
        parse_args_from(["setup", "plugin-hook", "--no-repair"]),
        Some(Command::Setup(SetupCommand::PluginHook { no_repair: true }))
    );
}

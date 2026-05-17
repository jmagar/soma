use super::{parse_args_from, Command, ConfigCommand, SetupCommand};

#[test]
fn empty_args_returns_none() {
    let result = parse_args_from::<_, String>([]).unwrap();
    assert!(result.is_none());
}

#[test]
fn unknown_subcommand_returns_none() {
    let result = parse_args_from(["unknown-command"]).unwrap();
    assert!(result.is_none());
}

#[test]
fn greet_no_name() {
    let cmd = parse_args_from(["greet"]).unwrap().unwrap();
    assert_eq!(cmd, Command::Greet { name: None });
}

#[test]
fn greet_with_name_flag() {
    let cmd = parse_args_from(["greet", "--name", "Alice"])
        .unwrap()
        .unwrap();
    assert_eq!(
        cmd,
        Command::Greet {
            name: Some("Alice".into())
        }
    );
}

#[test]
fn echo_with_message_flag() {
    let cmd = parse_args_from(["echo", "--message", "hello"])
        .unwrap()
        .unwrap();
    assert_eq!(
        cmd,
        Command::Echo {
            message: "hello".into()
        }
    );
}

#[test]
fn echo_missing_message_is_error() {
    let err = parse_args_from(["echo"]).unwrap_err();
    assert!(err.to_string().contains("--message"));
}

#[test]
fn status_subcommand() {
    let cmd = parse_args_from(["status"]).unwrap().unwrap();
    assert_eq!(cmd, Command::Status);
}

#[test]
fn doctor_no_flags() {
    let cmd = parse_args_from(["doctor"]).unwrap().unwrap();
    assert_eq!(cmd, Command::Doctor { json: false });
}

#[test]
fn doctor_json_flag() {
    let cmd = parse_args_from(["doctor", "--json"]).unwrap().unwrap();
    assert_eq!(cmd, Command::Doctor { json: true });
}

#[test]
fn watch_defaults() {
    let cmd = parse_args_from(["watch"]).unwrap().unwrap();
    assert_eq!(
        cmd,
        Command::Watch {
            url: None,
            interval: 10
        }
    );
}

#[test]
fn watch_with_url_and_interval() {
    let cmd = parse_args_from([
        "watch",
        "--url",
        "http://localhost:40060",
        "--interval",
        "5",
    ])
    .unwrap()
    .unwrap();
    assert_eq!(
        cmd,
        Command::Watch {
            url: Some("http://localhost:40060".into()),
            interval: 5
        }
    );
}

#[test]
fn setup_check() {
    let cmd = parse_args_from(["setup", "check"]).unwrap().unwrap();
    assert_eq!(cmd, Command::Setup(SetupCommand::Check));
}

#[test]
fn setup_repair() {
    let cmd = parse_args_from(["setup", "repair"]).unwrap().unwrap();
    assert_eq!(cmd, Command::Setup(SetupCommand::Repair));
}

#[test]
fn setup_plugin_hook() {
    let cmd = parse_args_from(["setup", "plugin-hook"]).unwrap().unwrap();
    assert_eq!(
        cmd,
        Command::Setup(SetupCommand::PluginHook { no_repair: false })
    );
}

#[test]
fn setup_plugin_hook_no_repair_flag() {
    let cmd = parse_args_from(["setup", "plugin-hook", "--no-repair"])
        .unwrap()
        .unwrap();
    assert_eq!(
        cmd,
        Command::Setup(SetupCommand::PluginHook { no_repair: true })
    );
}

#[test]
fn config_no_args_lists() {
    let cmd = parse_args_from(["config"]).unwrap().unwrap();
    assert_eq!(cmd, Command::Config(ConfigCommand::List));
}

#[test]
fn config_list_subcommand() {
    let cmd = parse_args_from(["config", "list"]).unwrap().unwrap();
    assert_eq!(cmd, Command::Config(ConfigCommand::List));
}

#[test]
fn config_path_subcommand() {
    let cmd = parse_args_from(["config", "path"]).unwrap().unwrap();
    assert_eq!(cmd, Command::Config(ConfigCommand::Path));
}

#[test]
fn config_get_with_key() {
    let cmd = parse_args_from(["config", "get", "mcp.host"])
        .unwrap()
        .unwrap();
    assert_eq!(
        cmd,
        Command::Config(ConfigCommand::Get {
            key: "mcp.host".into()
        })
    );
}

#[test]
fn config_get_without_key_is_error() {
    let err = parse_args_from(["config", "get"]).unwrap_err();
    assert!(err.to_string().contains("<key>"));
}

#[test]
fn config_set_with_key_and_value() {
    let cmd = parse_args_from(["config", "set", "mcp.host", "0.0.0.0"])
        .unwrap()
        .unwrap();
    assert_eq!(
        cmd,
        Command::Config(ConfigCommand::Set {
            key: "mcp.host".into(),
            value: "0.0.0.0".into()
        })
    );
}

#[test]
fn config_set_missing_value_is_error() {
    let err = parse_args_from(["config", "set", "mcp.host"]).unwrap_err();
    assert!(err.to_string().contains("<value>") || err.to_string().contains("arguments"));
}

#[test]
fn config_unset_with_key() {
    let cmd = parse_args_from(["config", "unset", "mcp.host"])
        .unwrap()
        .unwrap();
    assert_eq!(
        cmd,
        Command::Config(ConfigCommand::Unset {
            key: "mcp.host".into()
        })
    );
}

use super::*;

fn args(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

#[test]
fn explicit_serve_requests_enter_server_mode() {
    assert_eq!(resolve(&args(&[])), Mode::Cli);
    assert_eq!(resolve(&args(&["serve"])), Mode::Serve);
    assert_eq!(resolve(&args(&["serve", "mcp"])), Mode::Serve);
}

#[test]
fn local_cli_and_stdio_requests_stay_in_local_binary() {
    assert_eq!(resolve(&args(&["doctor"])), Mode::Cli);
    assert_eq!(resolve(&args(&["mcp"])), Mode::Stdio);
    assert_eq!(resolve(&args(&["setup", "plugin-hook"])), Mode::Cli);
}

#[test]
fn help_and_version_flags_are_recognized() {
    assert_eq!(resolve(&args(&["--help"])), Mode::Help);
    assert_eq!(resolve(&args(&["-h"])), Mode::Help);
    assert_eq!(resolve(&args(&["--version"])), Mode::Version);
    assert_eq!(resolve(&args(&["-V"])), Mode::Version);
    assert_eq!(resolve(&args(&["version"])), Mode::Version);
}

#[test]
fn local_and_stdio_modes_default_to_quiet_logging() {
    assert_eq!(Mode::Cli.default_log_level(), "warn");
    assert_eq!(Mode::Stdio.default_log_level(), "warn");
    assert_eq!(Mode::Help.default_log_level(), "warn");
    assert_eq!(Mode::Version.default_log_level(), "warn");
}

#[test]
fn http_server_mode_defaults_to_info_logging() {
    assert_eq!(Mode::Serve.default_log_level(), "info");
}

use serde_json::Value;
use std::fs;
use std::process::Command;

use tempfile::tempdir;

fn read(path: &str) -> String {
    fs::read_to_string(path).unwrap_or_else(|err| panic!("failed to read {path}: {err}"))
}

fn json(path: &str) -> Value {
    serde_json::from_str(&read(path)).unwrap_or_else(|err| panic!("failed to parse {path}: {err}"))
}

#[test]
fn plugin_manifests_exist_for_all_supported_hosts() {
    for path in [
        "plugins/example/.claude-plugin/plugin.json",
        "plugins/example/.codex-plugin/plugin.json",
        "plugins/example/gemini-extension.json",
        "plugins/example/.mcp.json",
        "plugins/example/hooks/hooks.json",
        "plugins/example/hooks/plugin-setup.sh",
        "plugins/example/skills/example/SKILL.md",
    ] {
        assert!(std::path::Path::new(path).exists(), "{path} should exist");
    }
}

#[test]
fn plugin_manifests_share_identity_and_connection_settings() {
    let claude = json("plugins/example/.claude-plugin/plugin.json");
    let codex = json("plugins/example/.codex-plugin/plugin.json");
    let gemini = json("plugins/example/gemini-extension.json");
    let mcp = json("plugins/example/.mcp.json");

    assert_eq!(claude["name"], "example");
    assert_eq!(codex["name"], "example-mcp");
    assert_eq!(gemini["name"], "example-mcp");

    assert!(claude["repository"]
        .as_str()
        .unwrap()
        .ends_with("example-mcp"));
    assert!(codex["repository"]
        .as_str()
        .unwrap()
        .ends_with("example-mcp"));
    assert!(gemini["repository"]
        .as_str()
        .unwrap()
        .ends_with("example-mcp"));

    let user_config = claude["userConfig"].as_object().unwrap();
    for key in [
        "server_url",
        "api_token",
        "example_api_url",
        "example_api_key",
    ] {
        assert!(
            user_config.contains_key(key),
            "Claude userConfig missing {key}"
        );
    }

    let gemini_settings: Vec<&str> = gemini["settings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|setting| setting["name"].as_str().unwrap())
        .collect();
    for key in [
        "server_url",
        "api_token",
        "example_api_url",
        "example_api_key",
    ] {
        assert!(
            gemini_settings.contains(&key),
            "Gemini settings missing {key}"
        );
    }

    assert_eq!(
        mcp["mcpServers"]["example"]["url"],
        "${user_config.server_url}/mcp"
    );
    assert_eq!(
        mcp["mcpServers"]["example"]["headers"]["Authorization"],
        "Bearer ${user_config.api_token}"
    );
    assert_eq!(
        gemini["mcpServers"]["example"]["url"],
        "${settings.server_url}/mcp"
    );
    assert_eq!(
        gemini["mcpServers"]["example"]["headers"]["Authorization"],
        "Bearer ${settings.api_token}"
    );
}

#[test]
fn claude_hooks_delegate_to_plugin_setup_script() {
    let hooks = json("plugins/example/hooks/hooks.json");
    for hook_name in ["SessionStart", "ConfigChange"] {
        let command = hooks["hooks"][hook_name][0]["hooks"][0]["command"]
            .as_str()
            .unwrap();
        assert_eq!(command, "${CLAUDE_PLUGIN_ROOT}/hooks/plugin-setup.sh");
    }
}

#[test]
fn plugin_setup_delegates_to_binary_owned_hook_command() {
    let setup = read("plugins/example/hooks/plugin-setup.sh");
    assert!(
        setup.contains("example setup plugin-hook"),
        "plugin setup should delegate to the binary-owned hook command"
    );
    assert!(
        !setup.contains("systemctl --user"),
        "plugin setup should not own systemd orchestration"
    );
    assert!(
        !setup.contains("docker compose"),
        "plugin setup should not own Docker orchestration"
    );
}

#[test]
fn plugin_hook_standard_is_documented() {
    let plugins = read("docs/PLUGINS.md");
    let patterns = read("docs/PATTERNS.md");
    for doc in [plugins, patterns] {
        assert!(doc.contains("<binary> setup plugin-hook"));
        assert!(doc.contains("<binary> setup plugin-hook --no-repair"));
        assert!(doc.contains("exit_policy"));
        assert!(doc.contains("blocking_failures"));
        assert!(doc.contains("advisory_failures"));
        assert!(doc.contains("ran_repair"));
    }
}

fn example_bin() -> &'static str {
    env!("CARGO_BIN_EXE_example")
}

fn setup_command(data_dir: &std::path::Path) -> Command {
    let mut cmd = Command::new(example_bin());
    cmd.env_clear()
        .env("HOME", data_dir)
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .env("CLAUDE_PLUGIN_DATA", data_dir)
        .env("EXAMPLE_API_URL", "https://api.example.test")
        .env("EXAMPLE_API_KEY", "example-secret")
        .env("EXAMPLE_MCP_TOKEN", "mcp-secret");
    cmd
}

#[test]
fn setup_plugin_hook_no_repair_emits_json_contract() {
    let dir = tempdir().unwrap();
    let mut cmd = setup_command(dir.path());
    let output = cmd
        .args(["setup", "plugin-hook", "--no-repair"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["exit_policy"], "success");
    assert_eq!(json["ran_repair"], false);
    assert_eq!(json["no_repair"], true);
    assert!(json["blocking_failures"].as_array().unwrap().is_empty());
    assert!(json["advisory_failures"].is_array());
    assert!(!dir.path().join(".env").exists());
}

#[test]
fn setup_repair_creates_env_file_without_upstream_contact() {
    let dir = tempdir().unwrap();
    let missing = dir.path().join("appdata");
    let mut cmd = setup_command(&missing);
    let output = cmd.args(["setup", "repair"]).output().unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["exit_policy"], "success");
    assert_eq!(json["ran_repair"], true);
    assert_eq!(json["no_repair"], false);

    let env_file = std::fs::read_to_string(missing.join(".env")).unwrap();
    assert!(env_file.contains("EXAMPLE_API_URL=https://api.example.test"));
    assert!(env_file.contains("EXAMPLE_API_KEY=example-secret"));
    assert!(env_file.contains("EXAMPLE_MCP_TOKEN=mcp-secret"));
}

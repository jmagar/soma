use super::*;
use tempfile::TempDir;

// ── registry sanity ──────────────────────────────────────────────────────────

#[test]
fn every_key_has_unique_name_and_env_alias() {
    let mut names: Vec<&str> = KEYS.iter().map(|k| k.name).collect();
    let mut envs: Vec<&str> = KEYS.iter().map(|k| k.env).collect();
    names.sort_unstable();
    envs.sort_unstable();
    let dup_name = names.windows(2).find(|w| w[0] == w[1]);
    let dup_env = envs.windows(2).find(|w| w[0] == w[1]);
    assert!(dup_name.is_none(), "duplicate key name: {dup_name:?}");
    assert!(dup_env.is_none(), "duplicate env alias: {dup_env:?}");
}

#[test]
fn every_key_has_current_value_arm() {
    // `current_value` should never fall through to its `(no accessor for…)`
    // placeholder for any registered key. Catches forgotten arms when a
    // new key is added to KEYS.
    let cfg = Config::default();
    for spec in KEYS {
        let v = current_value(&cfg, spec);
        assert!(
            !v.starts_with("(no accessor for"),
            "missing current_value arm for {}",
            spec.name
        );
    }
}

#[test]
fn lookup_key_accepts_dotted_name_and_env_alias() {
    assert_eq!(lookup_key("mcp.host").unwrap().name, "mcp.host");
    assert_eq!(lookup_key("EXAMPLE_MCP_HOST").unwrap().name, "mcp.host");
    assert_eq!(lookup_key("example_mcp_host").unwrap().name, "mcp.host");
}

#[test]
fn lookup_key_rejects_unknown() {
    let err = lookup_key("not.a.real.key").unwrap_err();
    assert!(err.to_string().contains("unknown config key"));
}

// ── value parsing ────────────────────────────────────────────────────────────

fn spec_for(name: &str) -> &'static KeySpec {
    lookup_key(name).unwrap()
}

#[test]
fn parse_value_bool_accepts_common_forms() {
    let s = spec_for("mcp.no_auth");
    assert!(matches!(
        parse_value(s, "true"),
        Ok(ParsedValue::Bool(true))
    ));
    assert!(matches!(
        parse_value(s, "FALSE"),
        Ok(ParsedValue::Bool(false))
    ));
    assert!(matches!(parse_value(s, "1"), Ok(ParsedValue::Bool(true))));
    assert!(matches!(parse_value(s, "no"), Ok(ParsedValue::Bool(false))));
}

#[test]
fn parse_value_bool_rejects_garbage() {
    let err = parse_value(spec_for("mcp.no_auth"), "maybe").unwrap_err();
    assert!(err.to_string().contains("expected bool"));
}

#[test]
fn parse_value_u16_rejects_overflow() {
    let err = parse_value(spec_for("mcp.port"), "999999").unwrap_err();
    assert!(err.to_string().contains("expected u16"));
}

#[test]
fn parse_value_auth_mode_normalises_case() {
    match parse_value(spec_for("mcp.auth.mode"), "OAUTH").unwrap() {
        ParsedValue::String(v) => assert_eq!(v, "oauth"),
        _ => panic!("expected String"),
    }
    let err = parse_value(spec_for("mcp.auth.mode"), "googleauth").unwrap_err();
    assert!(err.to_string().contains("bearer"));
}

#[test]
fn parse_value_list_trims_and_drops_empties() {
    match parse_value(spec_for("mcp.allowed_hosts"), "a , b ,, c").unwrap() {
        ParsedValue::List(items) => assert_eq!(items, vec!["a", "b", "c"]),
        _ => panic!("expected List"),
    }
}

// ── .env IO ──────────────────────────────────────────────────────────────────

#[test]
fn write_env_value_creates_and_then_updates() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join(".env");

    write_env_value(&path, "EXAMPLE_API_URL", "https://api.test/v1").unwrap();
    let body = std::fs::read_to_string(&path).unwrap();
    assert!(body.contains("EXAMPLE_API_URL=https://api.test/v1"));

    write_env_value(&path, "EXAMPLE_API_URL", "https://api.test/v2").unwrap();
    let body = std::fs::read_to_string(&path).unwrap();
    assert_eq!(body.matches("EXAMPLE_API_URL=").count(), 1);
    assert!(body.contains("EXAMPLE_API_URL=https://api.test/v2"));
}

#[test]
fn write_env_value_quotes_when_needed() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join(".env");

    write_env_value(&path, "EXAMPLE_API_KEY", "abc def#hash\"q").unwrap();
    let body = std::fs::read_to_string(&path).unwrap();
    assert!(body.contains("EXAMPLE_API_KEY=\"abc def#hash\\\"q\""));
}

#[test]
fn write_env_value_skips_commented_examples() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join(".env");
    std::fs::write(&path, "# EXAMPLE_MCP_TOKEN=placeholder\nKEEP=me\n").unwrap();

    write_env_value(&path, "EXAMPLE_MCP_TOKEN", "real-token").unwrap();
    let body = std::fs::read_to_string(&path).unwrap();
    assert!(body.contains("# EXAMPLE_MCP_TOKEN=placeholder"));
    assert!(body.contains("EXAMPLE_MCP_TOKEN=real-token"));
    assert!(body.contains("KEEP=me"));
}

#[test]
fn remove_env_key_removes_only_matching_line() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join(".env");
    std::fs::write(&path, "KEEP=me\nDROP=this\nALSO_KEEP=me\n").unwrap();

    assert!(remove_env_key(&path, "DROP").unwrap());
    let body = std::fs::read_to_string(&path).unwrap();
    assert!(body.contains("KEEP=me"));
    assert!(body.contains("ALSO_KEEP=me"));
    assert!(!body.contains("DROP="));
}

#[test]
fn remove_env_key_missing_file_is_noop() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join(".env");
    assert!(!remove_env_key(&path, "NOTHING").unwrap());
}

// ── config.toml IO ───────────────────────────────────────────────────────────

#[test]
fn write_toml_value_preserves_comments() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("config.toml");
    std::fs::write(
        &path,
        "# top comment\n[mcp]\n# host comment\nhost = \"127.0.0.1\"\nport = 40060\n",
    )
    .unwrap();

    write_toml_value(
        &path,
        &["mcp", "host"],
        &ParsedValue::String("0.0.0.0".into()),
    )
    .unwrap();
    let body = std::fs::read_to_string(&path).unwrap();
    assert!(body.contains("# top comment"));
    assert!(body.contains("# host comment"));
    assert!(body.contains("host = \"0.0.0.0\""));
    assert!(body.contains("port = 40060"));
}

#[test]
fn write_toml_value_creates_intermediate_tables() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("config.toml");
    write_toml_value(
        &path,
        &["mcp", "auth", "mode"],
        &ParsedValue::String("oauth".into()),
    )
    .unwrap();
    let body = std::fs::read_to_string(&path).unwrap();
    assert!(body.contains("[mcp.auth]"));
    assert!(body.contains("mode = \"oauth\""));
}

#[test]
fn write_toml_value_writes_list_as_array() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("config.toml");
    write_toml_value(
        &path,
        &["mcp", "allowed_hosts"],
        &ParsedValue::List(vec!["a.example.com".into(), "b.example.com".into()]),
    )
    .unwrap();
    let body = std::fs::read_to_string(&path).unwrap();
    assert!(body.contains("allowed_hosts = [\"a.example.com\", \"b.example.com\"]"));
}

#[test]
fn write_toml_value_writes_ints_correctly() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("config.toml");
    write_toml_value(&path, &["mcp", "port"], &ParsedValue::U16(8080)).unwrap();
    let body = std::fs::read_to_string(&path).unwrap();
    assert!(body.contains("port = 8080"));
}

#[test]
fn remove_toml_key_returns_false_when_missing() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("config.toml");
    std::fs::write(&path, "[mcp]\nport = 40060\n").unwrap();
    assert!(!remove_toml_key(&path, &["mcp", "host"]).unwrap());
}

#[test]
fn remove_toml_key_removes_existing_value() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("config.toml");
    std::fs::write(&path, "[mcp]\nport = 40060\nhost = \"0.0.0.0\"\n").unwrap();
    assert!(remove_toml_key(&path, &["mcp", "host"]).unwrap());
    let body = std::fs::read_to_string(&path).unwrap();
    assert!(!body.contains("host"));
    assert!(body.contains("port = 40060"));
}

#[test]
fn write_toml_refuses_to_clobber_non_table() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("config.toml");
    std::fs::write(&path, "mcp = \"this is a string\"\n").unwrap();
    let err =
        write_toml_value(&path, &["mcp", "host"], &ParsedValue::String("x".into())).unwrap_err();
    assert!(err.to_string().contains("non-table value"));
}

// ── quote_env_value ──────────────────────────────────────────────────────────

#[test]
fn quote_env_value_leaves_simple_values_bare() {
    assert_eq!(quote_env_value("plain"), "plain");
    assert_eq!(quote_env_value("a.b-c_d"), "a.b-c_d");
}

#[test]
fn quote_env_value_quotes_empty_and_whitespace() {
    assert_eq!(quote_env_value(""), "\"\"");
    assert_eq!(quote_env_value("has space"), "\"has space\"");
    assert_eq!(quote_env_value("has\ttab"), "\"has\ttab\"");
}

#[test]
fn quote_env_value_escapes_quotes_and_backslashes() {
    assert_eq!(quote_env_value(r#"a"b\c"#), r#""a\"b\\c""#);
}

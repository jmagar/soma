use super::*;

#[test]
fn paths_use_soma_names() {
    let root = std::env::temp_dir().join("phase3-defaults").join(".soma");
    let paths = GatewayPaths::new(root.clone()).unwrap();
    assert_eq!(paths.home(), root.as_path());
    assert_eq!(paths.config_path(), root.join("config.toml"));
    assert_eq!(paths.env_path(), root.join(".env"));
}

#[test]
fn env_home_accepts_parent_or_soma_dir() {
    let parent = std::env::temp_dir().join("phase3-env-home");
    assert_eq!(
        normalize_env_soma_home(parent.clone()),
        parent.join(".soma")
    );

    let soma_home = parent.join(".soma");
    assert_eq!(normalize_env_soma_home(soma_home.clone()), soma_home);
}

#[test]
fn rejects_non_soma_and_relative_homes() {
    assert!(GatewayPaths::new(PathBuf::from("relative/.soma")).is_err());
    assert!(GatewayPaths::new(std::env::temp_dir().join(".labby")).is_err());
}

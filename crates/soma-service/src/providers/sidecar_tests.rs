use std::{ffi::OsString, fs};

use super::resolve_sidecar_command_with_env;

#[test]
fn resolves_bare_command_from_parent_path_for_env_cleared_spawn() {
    let dir = std::env::temp_dir().join(format!(
        "soma-sidecar-command-resolution-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("test command dir");

    let command_path = dir.join("fake-python.EXE");

    fs::write(&command_path, b"").expect("fake command");
    let path_env = std::env::join_paths([&dir]).expect("PATH value");
    let pathext_env = Some(OsString::from(".EXE;.CMD"));

    let resolved = resolve_sidecar_command_with_env("fake-python", Some(path_env), pathext_env);

    assert_eq!(resolved, command_path);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn leaves_explicit_command_paths_unresolved() {
    let resolved = resolve_sidecar_command_with_env(
        "./fake-python",
        Some(OsString::from("/tmp")),
        Some(OsString::from(".EXE")),
    );

    assert_eq!(resolved, std::path::PathBuf::from("./fake-python"));
}

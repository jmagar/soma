use std::{fs, path::Path};

#[test]
fn plugin_manifests_remain_versionless_when_present() {
    let root = workspace_root();
    for path in [
        ".claude-plugin/plugin.json",
        ".codex-plugin/plugin.json",
        "gemini-extension.json",
    ] {
        let path = root.join(path);
        if !path.exists() {
            continue;
        }
        let value: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).expect("manifest should read"))
                .expect("manifest JSON");
        assert!(
            value.get("version").is_none(),
            "{} must stay versionless",
            path.display()
        );
    }
}

#[test]
fn generated_docs_do_not_elevate_provider_descriptions_to_instructions() {
    let root = workspace_root();
    let manifest = root.join("docs/generated/palette-manifest.json");
    let text = fs::read_to_string(manifest).expect("palette manifest should exist");
    assert!(!text.contains("ignore previous instructions"));
    assert!(!text.contains("developer message"));
}

#[test]
fn generated_marketplaces_point_at_template_plugin() {
    let root = workspace_root();
    let codex: serde_json::Value = serde_json::from_slice(
        &fs::read(root.join(".agents/plugins/marketplace.json")).expect("codex marketplace"),
    )
    .expect("codex marketplace JSON");
    assert_eq!(codex["plugins"][0]["source"]["path"], "./plugins/rtemplate");
    assert!(codex["plugins"][0].get("version").is_none());

    let claude: serde_json::Value = serde_json::from_slice(
        &fs::read(root.join(".claude-plugin/marketplace.json")).expect("claude marketplace"),
    )
    .expect("claude marketplace JSON");
    assert_eq!(claude["plugins"][0]["source"], "./plugins/rtemplate");
    assert!(claude["plugins"][0].get("version").is_none());
}

#[test]
fn node_package_exposes_npx_launcher() {
    let root = workspace_root();
    let package: serde_json::Value = serde_json::from_slice(
        &fs::read(root.join("packages/rtemplate-mcp/package.json")).expect("package json"),
    )
    .expect("package JSON");
    assert_eq!(package["bin"]["rtemplate-mcp"], "./bin/rtemplate-mcp.js");
    assert_eq!(package["bin"]["rtemplate"], "./bin/rtemplate-mcp.js");

    let launcher = root.join("packages/rtemplate-mcp/bin/rtemplate-mcp.js");
    let mode = fs::metadata(&launcher)
        .expect("launcher metadata")
        .permissions();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_ne!(mode.mode() & 0o111, 0, "launcher should be executable");
    }
}

fn workspace_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
}

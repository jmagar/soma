use std::fs;

use serde_json::json;
use tempfile::tempdir;

use super::{load_catalog, FileProviderSource};

#[test]
fn wasm_sidecar_manifest_is_loaded_as_the_wasm_provider_manifest() {
    let temp = tempdir().expect("tempdir");
    let wasm_path = temp.path().join("edge.wasm");
    let sidecar_path = temp.path().join("edge.wasm.json");
    fs::write(&wasm_path, b"not actually wasm").expect("write wasm placeholder");
    fs::write(&sidecar_path, manifest_bytes("edge")).expect("write sidecar manifest");

    let catalog = load_catalog(&wasm_path).expect("sidecar manifest");

    assert_eq!(catalog.provider.name, "edge");
    assert_eq!(catalog.provider.kind.as_str(), "wasm");
    assert_eq!(catalog.tools[0].name, "edge-run");
}

#[test]
fn wasm_sidecar_manifest_is_not_loaded_as_a_second_provider() {
    let temp = tempdir().expect("tempdir");
    fs::write(temp.path().join("edge.wasm"), b"not actually wasm").expect("write wasm");
    fs::write(temp.path().join("edge.wasm.json"), manifest_bytes("edge"))
        .expect("write sidecar manifest");

    let source = FileProviderSource::new(temp.path());
    let providers = source.load().expect("providers");

    assert_eq!(providers.len(), 1);
    assert_eq!(providers[0].catalog().provider.name, "edge");
}

#[test]
fn fingerprint_changes_when_wasm_sidecar_manifest_changes() {
    let temp = tempdir().expect("tempdir");
    fs::write(temp.path().join("edge.wasm"), b"not actually wasm").expect("write wasm");
    let sidecar_path = temp.path().join("edge.wasm.json");
    fs::write(&sidecar_path, manifest_bytes("edge")).expect("write sidecar manifest");
    let source = FileProviderSource::new(temp.path());

    let first = source.fingerprint().expect("first fingerprint");
    fs::write(&sidecar_path, manifest_bytes("edge-next")).expect("update sidecar manifest");
    let second = source.fingerprint().expect("second fingerprint");

    assert_ne!(first, second);
}

fn manifest_bytes(name: &str) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "schema_version": 1,
        "provider": {
            "name": name,
            "kind": "wasm",
            "enabled": true
        },
        "tools": [
            {
                "name": "edge-run",
                "description": "Run an edge provider.",
                "input_schema": {
                    "type": "object",
                    "additionalProperties": false,
                    "properties": {}
                }
            }
        ]
    }))
    .expect("manifest json")
}

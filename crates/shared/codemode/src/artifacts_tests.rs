use super::artifacts::ArtifactStore;
use serial_test::serial;

#[test]
#[serial(code_mode_soma_home)]
fn artifact_store_root_uses_run_id() {
    assert!(ArtifactStore::new("abc").unwrap().root().ends_with("abc"));
}

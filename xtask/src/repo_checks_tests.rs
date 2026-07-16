use super::*;

#[test]
fn nested_shared_mcp_roots_are_checked_for_test_siblings() {
    assert!(crate_src_roots()
        .iter()
        .any(|path| path == &PathBuf::from("crates/shared/mcp/gateway/src")));
}

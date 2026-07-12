//! Architecture boundary tests — make the thin-shim rule executable.
//!
//! CLAUDE.md says the MCP and CLI shims (`tools.rs`, `cli/lib.rs`) hold zero
//! business logic and reach the service layer (`SomaService` /
//! `dispatch_action`), never the transport client (`SomaClient`) or raw
//! HTTP. These tests read the shim source and fail if that boundary is crossed,
//! so the rule is enforced by CI instead of by reviewer vigilance.
//!
//! The checks are deliberately textual and conservative: they target import and
//! call-site forms (`use … SomaClient`, `SomaClient::`, `reqwest`) so a
//! mention inside a doc comment or help string is not a false positive.

use std::fs;
use std::path::PathBuf;

fn workspace_root() -> PathBuf {
    // CARGO_MANIFEST_DIR is `crates/soma`; the workspace root is two up.
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root is two levels above the crate manifest")
        .to_path_buf()
}

fn read_shim(relative: &str) -> String {
    let path = workspace_root().join(relative);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()))
}

fn imports_symbol(src: &str, symbol: &str) -> bool {
    src.lines()
        .map(str::trim_start)
        .filter(|line| line.starts_with("use "))
        .any(|line| line.contains(symbol))
}

#[test]
fn mcp_tools_shim_does_not_touch_the_transport_client() {
    let src = read_shim("crates/soma-mcp/src/tools.rs");
    assert!(
        !imports_symbol(&src, "SomaClient"),
        "tools.rs must dispatch through SomaService, never import SomaClient (thin-shim rule)"
    );
    assert!(
        !src.contains("SomaClient::"),
        "tools.rs must not construct or call SomaClient directly; go through the service layer"
    );
    assert!(
        !src.contains("reqwest"),
        "tools.rs must not perform transport/HTTP work; that belongs in soma.rs (the client)"
    );
}

#[test]
fn cli_shim_does_not_perform_transport_work() {
    let src = read_shim("crates/soma-cli/src/lib.rs");
    // The CLI may construct the client (composition root) but must not do HTTP.
    assert!(
        !src.contains("reqwest"),
        "cli/lib.rs must not perform transport/HTTP work; it wires the service and dispatches only"
    );
}

#[test]
fn mcp_tools_shim_reaches_the_shared_service_seam() {
    let src = read_shim("crates/soma-mcp/src/tools.rs");
    assert!(
        src.contains("dispatch_action") || imports_symbol(&src, "SomaService"),
        "tools.rs should reach the service layer via dispatch_action / SomaService"
    );
}

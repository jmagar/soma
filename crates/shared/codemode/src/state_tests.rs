use super::state::path::state_root;
use serial_test::serial;

#[test]
#[serial(code_mode_soma_home)]
fn state_root_uses_soma_home() {
    assert!(state_root().ends_with(".soma-state"));
}

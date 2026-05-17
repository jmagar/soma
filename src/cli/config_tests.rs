use super::*;

// The shim has no logic worth unit-testing on its own — argument parsing is
// covered in `cli_tests.rs::config_*` and the file IO + registry live in
// `config_store_tests.rs`. This stub keeps `xtask check-test-siblings` happy
// (every `<name>.rs` source file must have a `<name>_tests.rs` neighbour).

#[test]
fn config_command_variants_are_distinct() {
    let a = ConfigCommand::List;
    let b = ConfigCommand::Path;
    let c = ConfigCommand::Get { key: "k".into() };
    assert_ne!(a, b);
    assert_ne!(a, c);
    assert_ne!(b, c);
}

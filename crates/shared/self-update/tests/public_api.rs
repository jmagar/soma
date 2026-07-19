use soma_self_update::{
    ArtifactTransportPolicy, MigrationOutcome, RecoveryAction, UpdateDirective, UpdateLayout,
    UpdatePolicy, Updater,
};

#[test]
fn public_contract_is_constructible_without_product_types() {
    let executable = std::path::Path::new("opt/example/bin/example");
    let state_file = std::path::Path::new("opt/example/state/update.json");
    let construction_dir = std::env::current_dir().unwrap();
    let layout = UpdateLayout::new(executable, state_file);
    let updater = Updater::new(layout, UpdatePolicy::default());
    let directive = UpdateDirective::new(
        "1.2.3",
        "/v1/agent/binary",
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
    )
    .unwrap();
    assert_eq!(directive.version(), "1.2.3");
    assert_eq!(
        updater.policy().transport(),
        ArtifactTransportPolicy::HttpsOnly
    );
    assert!(matches!(
        RecoveryAction::NoPendingUpdate,
        RecoveryAction::NoPendingUpdate
    ));
    let migrated = MigrationOutcome::MigratedIndeterminate {
        updater,
        diagnostic: "directory sync must be retried".into(),
    };
    assert_eq!(
        migrated.updater().layout().state_file(),
        construction_dir.join(state_file)
    );
    assert_eq!(
        migrated.into_updater().layout().executable(),
        construction_dir.join(executable)
    );
}

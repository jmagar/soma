use tempfile::tempdir;

use super::*;
use crate::transaction::TestFailpoint;
use crate::{UpdateLayout, UpdatePolicy};

#[test]
fn retry_after_authority_rename_before_directory_sync_is_idempotent() {
    let temp = tempdir().unwrap();
    let executable = temp.path().join("agent");
    let old_state = temp.path().join("old.json");
    let new_state = temp.path().join("new.json");
    std::fs::write(&executable, b"old").unwrap();
    let updater = Updater::new(
        UpdateLayout::new(&executable, &old_state),
        UpdatePolicy::default(),
    );
    let paths = updater.validated_layout().unwrap();
    drop(updater.transaction_locks(&paths).unwrap());
    updater.set_test_failpoint(TestFailpoint::AuthorityBeforeDirectorySync);

    let outcome = updater.migrate_state_file_sync(new_state.clone()).unwrap();
    let migrated = match outcome {
        MigrationOutcome::MigratedIndeterminate {
            updater,
            diagnostic,
        } => {
            assert!(!diagnostic.is_empty());
            updater
        }
        MigrationOutcome::Migrated { .. } => panic!("expected indeterminate migration"),
    };
    updater.set_test_failpoint(TestFailpoint::None);

    let migrated_paths = migrated.validated_layout().unwrap();
    drop(migrated.transaction_locks(&migrated_paths).unwrap());
    assert!(matches!(
        updater.migrate_state_file_sync(new_state).unwrap(),
        MigrationOutcome::Migrated { .. }
    ));
}

#[test]
fn old_marker_collision_with_new_lock_is_rejected_before_side_effects() {
    migration_collision_is_side_effect_free("destination.json.lock", "destination.json");
}

#[test]
fn new_marker_collision_with_old_lock_is_rejected_before_side_effects() {
    migration_collision_is_side_effect_free("current.json", "current.json.lock");
}

fn migration_collision_is_side_effect_free(old_name: &str, new_name: &str) {
    let temp = tempdir().unwrap();
    let executable = temp.path().join("agent");
    let old_state = temp.path().join(old_name);
    let new_state = temp.path().join(new_name);
    std::fs::write(&executable, b"old").unwrap();
    let updater = Updater::new(
        UpdateLayout::new(&executable, &old_state),
        UpdatePolicy::default(),
    );

    assert!(matches!(
        updater.migrate_state_file_sync(new_state),
        Err(UpdateError::InvalidLayout { .. })
    ));
    assert_eq!(std::fs::read_dir(temp.path()).unwrap().count(), 1);
}

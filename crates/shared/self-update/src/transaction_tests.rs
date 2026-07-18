use super::*;
use crate::{UpdateDirective, UpdateLayout, UpdatePolicy};
use tempfile::tempdir;

async fn updater_and_artifact(
    max_restarts: u32,
) -> (
    tempfile::TempDir,
    Updater,
    ValidatedArtifact,
    Vec<u8>,
    Vec<u8>,
) {
    use std::os::unix::fs::PermissionsExt;

    let temp = tempdir().unwrap();
    let executable = temp.path().join("agent");
    let state = temp.path().join("update.json");
    let old = b"#!/bin/sh\necho 'agent 1.0.0'\n".to_vec();
    let new = b"#!/bin/sh\necho 'agent 2.0.0'\n".to_vec();
    std::fs::write(&executable, &old).unwrap();
    std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700)).unwrap();
    let updater = Updater::new(
        UpdateLayout::new(&executable, &state),
        UpdatePolicy::default()
            .with_max_unconfirmed_restarts(max_restarts)
            .unwrap(),
    );
    let directive = UpdateDirective::new("2.0.0", "/agent", hash_bytes(&new)).unwrap();
    let staged = updater.stage(&new[..], &directive).await.unwrap();
    let validated = updater.validate(staged).await.unwrap();
    (temp, updater, validated, old, new)
}

fn hash_bytes(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[tokio::test(flavor = "current_thread")]
async fn failpoints_after_marker_and_swap_recover_idempotently() {
    for (failpoint, running, expected) in [
        (
            TestFailpoint::AfterMarkerSync,
            "1.0.0",
            RecoveryAction::NoPendingUpdate,
        ),
        (
            TestFailpoint::AfterSwap,
            "2.0.0",
            RecoveryAction::PendingUpdate {
                target: "2.0.0".into(),
                attempts: 1,
                max_attempts: 1,
            },
        ),
    ] {
        let (_temp, updater, artifact, _old, _new) = updater_and_artifact(1).await;
        updater.set_test_failpoint(failpoint);
        assert!(updater.install(artifact, "1.0.0").await.is_err());
        updater.set_test_failpoint(TestFailpoint::None);
        assert_eq!(updater.recover_on_startup(running).await.unwrap(), expected);
    }
}

#[tokio::test(flavor = "current_thread")]
async fn failpoint_after_rollback_rename_recovers_idempotently() {
    let (_temp, updater, artifact, old, _new) = updater_and_artifact(1).await;
    updater.install(artifact, "1.0.0").await.unwrap();
    updater.recover_on_startup("2.0.0").await.unwrap();
    updater.set_test_failpoint(TestFailpoint::AfterRollbackRename);
    assert!(updater.recover_on_startup("2.0.0").await.is_err());
    updater.set_test_failpoint(TestFailpoint::None);
    assert!(matches!(
        updater.recover_on_startup("1.0.0").await.unwrap(),
        RecoveryAction::RollbackInstalled { .. }
    ));
    assert_eq!(std::fs::read(updater.layout().executable()).unwrap(), old);
}

#[tokio::test(flavor = "current_thread")]
async fn failed_rename_cleanup_is_authoritative_state_first() {
    for (failpoint, expected_backups) in [
        (TestFailpoint::FailedRenameAfterMarkerCleanup, 1),
        (TestFailpoint::FailedRenameAfterBackupCleanup, 0),
    ] {
        let (_temp, updater, artifact, old, _new) = updater_and_artifact(1).await;
        updater.set_test_failpoint(failpoint);
        assert!(updater.install(artifact, "1.0.0").await.is_err());
        updater.set_test_failpoint(TestFailpoint::None);
        assert!(!updater.layout().state_file().exists());
        assert_eq!(std::fs::read(updater.layout().executable()).unwrap(), old);
        let backup_count = std::fs::read_dir(updater.layout().executable().parent().unwrap())
            .unwrap()
            .filter_map(std::result::Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().contains(".rollback-"))
            .count();
        assert_eq!(backup_count, expected_backups);
        assert_eq!(
            updater.recover_on_startup("1.0.0").await.unwrap(),
            RecoveryAction::NoPendingUpdate
        );
    }
}

#[tokio::test]
async fn failpoint_on_one_updater_does_not_affect_another_updater() {
    let (_first_temp, first, first_artifact, _old, _new) = updater_and_artifact(1).await;
    let (_second_temp, second, second_artifact, _old, _new) = updater_and_artifact(1).await;
    first.set_test_failpoint(TestFailpoint::AfterMarkerSync);

    assert!(second.install(second_artifact, "1.0.0").await.is_ok());
    assert!(first.install(first_artifact, "1.0.0").await.is_err());
}

#[tokio::test]
async fn marker_temp_crash_is_reclaimed_under_the_state_lock() {
    let (_temp, updater, artifact, old, _new) = updater_and_artifact(1).await;
    let marker_temp = suffix_path(updater.layout().state_file(), ".tmp");
    updater.set_test_failpoint(TestFailpoint::AfterMarkerTempSync);

    assert!(updater.install(artifact, "1.0.0").await.is_err());
    assert!(marker_temp.is_file());
    assert_eq!(std::fs::read(updater.layout().executable()).unwrap(), old);

    updater.set_test_failpoint(TestFailpoint::None);
    assert_eq!(
        updater.recover_on_startup("1.0.0").await.unwrap(),
        RecoveryAction::NoPendingUpdate
    );
    assert!(!marker_temp.exists());
}

#[test]
fn marker_temp_owner_matches_service_even_when_directory_owner_differs() {
    let root_owned_directory = 0;
    let service_effective_uid = 1000;
    let service_created_temp = service_effective_uid;

    assert_ne!(root_owned_directory, service_effective_uid);
    assert!(marker_temp_owner_is_valid(
        service_created_temp,
        service_effective_uid
    ));
    assert!(!marker_temp_owner_is_valid(
        root_owned_directory,
        service_effective_uid
    ));
}

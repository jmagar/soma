use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use fs2::FileExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::validation::ArtifactIdentity;
use crate::{
    BackupStrategy, RecoveryAction, Result, UpdateError, Updater, ValidatedArtifact,
    reject_executable_leaf_symlink,
};

#[path = "transaction_artifacts.rs"]
mod artifacts;
use artifacts::{
    cleanup_owned_artifacts, exact_artifact_name, validate_marker_backup_metadata,
    validate_marker_staged_metadata, validate_rollback_backup,
};

static TRANSACTION_COUNTER: AtomicU64 = AtomicU64::new(0);
const MAX_MARKER_BYTES: u64 = 64 * 1024;

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Clone, Copy)]
#[repr(u8)]
enum TestFailpoint {
    None,
    AfterMarkerTempSync,
    AfterMarkerSync,
    AfterSwap,
    AfterRollbackRename,
    FailedRenameAfterMarkerCleanup,
    FailedRenameAfterBackupCleanup,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InstallOutcome {
    RestartRequired {
        executable: PathBuf,
        from: String,
        to: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConfirmationOutcome {
    NoPendingUpdate,
    Confirmed { version: String },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum MarkerPhase {
    Prepared,
    Installed,
    RollingBack,
    RolledBack,
}

#[derive(Debug, Deserialize, Serialize)]
struct Marker {
    schema_version: u32,
    phase: MarkerPhase,
    target: String,
    previous: String,
    executable: PathBuf,
    backup: PathBuf,
    staged: PathBuf,
    attempts: u32,
    sha256: String,
    previous_sha256: String,
}

struct TransactionLock {
    _file: File,
}

struct LayoutPaths {
    executable: PathBuf,
    state: PathBuf,
    lock: PathBuf,
}

impl Updater {
    #[cfg(test)]
    fn set_test_failpoint(&self, failpoint: TestFailpoint) {
        self.test_failpoint.store(failpoint as u8, Ordering::SeqCst);
    }

    #[cfg(test)]
    fn failpoint_active(&self, failpoint: TestFailpoint) -> bool {
        self.test_failpoint.load(Ordering::SeqCst) == failpoint as u8
    }

    #[cfg(not(test))]
    fn failpoint_active(&self, _failpoint: TestFailpoint) -> bool {
        false
    }

    fn maybe_fail(&self, failpoint: TestFailpoint, path: &Path) -> Result<()> {
        if self.failpoint_active(failpoint) {
            return Err(UpdateError::io(
                path,
                std::io::Error::other("injected transaction crash boundary"),
            ));
        }
        Ok(())
    }

    pub async fn install(
        &self,
        validated: ValidatedArtifact,
        previous_version: impl Into<String>,
    ) -> Result<InstallOutcome> {
        let paths = self.validated_layout()?;
        let _lock = self.transaction_lock(&paths.lock)?;
        let executable = paths.executable;
        let state = paths.state;
        cleanup_marker_temp(&state)?;
        let validated_path = absolute(validated.path())?;
        let staged_metadata = std::fs::symlink_metadata(&validated_path)
            .map_err(|error| UpdateError::io(&validated_path, error))?;
        if !staged_metadata.file_type().is_file() {
            return Err(UpdateError::InvalidStagedArtifact {
                path: validated_path,
            });
        }
        if let Some(marker) = read_marker(&state, &executable)? {
            return Err(UpdateError::PendingUpdateExists {
                path: state,
                target: marker.target,
            });
        }
        cleanup_owned_artifacts(&executable, None, Some(&validated_path))?;
        let actual_digest = hash_stable_validated_artifact(&validated, &validated_path)?;
        if actual_digest != validated.sha256() {
            return Err(UpdateError::DigestMismatch {
                expected: validated.sha256().to_owned(),
                actual: actual_digest,
            });
        }
        let previous = previous_version.into();
        let target = validated.target_version().to_owned();
        let backup = unique_backup(&executable);
        create_backup(&executable, &backup, self.policy().backup_strategy())?;
        let previous_sha256 = hash_file(&backup)?;
        let mut marker = Marker {
            schema_version: 2,
            phase: MarkerPhase::Prepared,
            target: target.clone(),
            previous: previous.clone(),
            executable: executable.clone(),
            backup: backup.clone(),
            staged: validated_path.clone(),
            attempts: 0,
            sha256: validated.sha256().to_owned(),
            previous_sha256,
        };
        if let Err(error) = write_marker(self, &state, &marker) {
            remove_file(&backup)?;
            return Err(error);
        }
        self.maybe_fail(TestFailpoint::AfterMarkerSync, &state)?;
        let final_digest = hash_stable_validated_artifact(&validated, &validated_path)?;
        if final_digest != validated.sha256() {
            return Err(UpdateError::DigestMismatch {
                expected: validated.sha256().to_owned(),
                actual: final_digest,
            });
        }
        let forced_rename_failure = self
            .failpoint_active(TestFailpoint::FailedRenameAfterMarkerCleanup)
            || self.failpoint_active(TestFailpoint::FailedRenameAfterBackupCleanup);
        let rename_result = if forced_rename_failure {
            Err(std::io::Error::other("injected final rename failure"))
        } else {
            std::fs::rename(&validated_path, &executable)
        };
        if let Err(source) = rename_result {
            remove_and_sync(&state)?;
            self.maybe_fail(TestFailpoint::FailedRenameAfterMarkerCleanup, &state)?;
            remove_and_sync(&backup)?;
            self.maybe_fail(TestFailpoint::FailedRenameAfterBackupCleanup, &backup)?;
            return Err(UpdateError::io(&executable, source));
        }
        sync_parent(&executable)?;
        self.maybe_fail(TestFailpoint::AfterSwap, &executable)?;
        marker.phase = MarkerPhase::Installed;
        write_marker(self, &state, &marker)?;
        Ok(InstallOutcome::RestartRequired {
            executable,
            from: previous,
            to: target,
        })
    }

    pub async fn recover_on_startup(&self, running_version: &str) -> Result<RecoveryAction> {
        let paths = self.validated_layout()?;
        let _lock = self.transaction_lock(&paths.lock)?;
        let state = paths.state;
        cleanup_marker_temp(&state)?;
        let marker = read_marker(&state, &paths.executable)?;
        cleanup_owned_artifacts(
            &paths.executable,
            marker.as_ref().map(|marker| marker.backup.as_path()),
            None,
        )?;
        let Some(mut marker) = marker else {
            return Ok(RecoveryAction::NoPendingUpdate);
        };
        match marker.phase {
            MarkerPhase::Prepared => {
                let executable_digest = hash_file(&marker.executable)?;
                if running_version == marker.previous && executable_digest == marker.previous_sha256
                {
                    abort_prepared(&state, &marker)?;
                    return Ok(RecoveryAction::NoPendingUpdate);
                }
                if running_version == marker.target && executable_digest == marker.sha256 {
                    marker.phase = MarkerPhase::Installed;
                    write_marker(self, &state, &marker)?;
                } else {
                    return Err(version_mismatch(running_version, &marker));
                }
            }
            MarkerPhase::Installed => {
                if marker.target != running_version {
                    return Err(version_mismatch(running_version, &marker));
                }
            }
            MarkerPhase::RollingBack => {
                return resume_rollback(self, &state, marker, running_version);
            }
            MarkerPhase::RolledBack => {
                return finish_rollback(&state, marker, running_version);
            }
        }
        marker.attempts = marker.attempts.saturating_add(1);
        if marker.attempts <= self.policy().max_unconfirmed_restarts() {
            write_marker(self, &state, &marker)?;
            return Ok(RecoveryAction::PendingUpdate {
                target: marker.target,
                attempts: marker.attempts,
                max_attempts: self.policy().max_unconfirmed_restarts(),
            });
        }
        validate_rollback_backup(&state, &marker)?;
        marker.phase = MarkerPhase::RollingBack;
        write_marker(self, &state, &marker)?;
        std::fs::rename(&marker.backup, &marker.executable)
            .map_err(|error| UpdateError::io(&marker.executable, error))?;
        sync_parent(&marker.executable)?;
        self.maybe_fail(TestFailpoint::AfterRollbackRename, &marker.executable)?;
        marker.phase = MarkerPhase::RolledBack;
        write_marker(self, &state, &marker)?;
        finalize_rollback(&state, marker)
    }

    pub async fn confirm_success(&self, running_version: &str) -> Result<ConfirmationOutcome> {
        let paths = self.validated_layout()?;
        let _lock = self.transaction_lock(&paths.lock)?;
        let state = paths.state;
        cleanup_marker_temp(&state)?;
        let marker = read_marker(&state, &paths.executable)?;
        cleanup_owned_artifacts(
            &paths.executable,
            marker.as_ref().map(|marker| marker.backup.as_path()),
            None,
        )?;
        let Some(mut marker) = marker else {
            return Ok(ConfirmationOutcome::NoPendingUpdate);
        };
        if marker.phase == MarkerPhase::Prepared
            && marker.target == running_version
            && hash_file(&marker.executable)? == marker.sha256
        {
            marker.phase = MarkerPhase::Installed;
            write_marker(self, &state, &marker)?;
        }
        if marker.phase != MarkerPhase::Installed {
            return Err(version_mismatch(running_version, &marker));
        }
        if marker.target != running_version {
            return Err(UpdateError::RunningVersionMismatch {
                running: running_version.to_owned(),
                target: marker.target,
            });
        }
        validate_rollback_backup(&state, &marker)?;
        remove_file(&state)?;
        sync_parent(&state)?;
        remove_file(&marker.backup)?;
        sync_parent(&marker.backup)?;
        Ok(ConfirmationOutcome::Confirmed {
            version: running_version.to_owned(),
        })
    }

    fn transaction_lock(&self, lock_path: &Path) -> Result<TransactionLock> {
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(lock_path)
            .map_err(|error| UpdateError::io(lock_path, error))?;
        file.try_lock_exclusive().map_err(|error| {
            if error.kind() == std::io::ErrorKind::WouldBlock {
                UpdateError::UpdateInProgress {
                    path: lock_path.to_path_buf(),
                }
            } else {
                UpdateError::io(lock_path, error)
            }
        })?;
        Ok(TransactionLock { _file: file })
    }

    fn validated_layout(&self) -> Result<LayoutPaths> {
        reject_executable_leaf_symlink(self.layout().executable())?;
        let executable = path_identity(self.layout().executable())?;
        let state = path_identity(self.layout().state_file())?;
        let lock = suffix_path(&state, ".lock");
        for (first, second) in [(&executable, &state), (&executable, &lock), (&state, &lock)] {
            if first == second {
                return Err(UpdateError::InvalidLayout {
                    first: first.clone(),
                    second: second.clone(),
                });
            }
        }
        Ok(LayoutPaths {
            executable,
            state,
            lock,
        })
    }
}

fn absolute(path: &Path) -> Result<PathBuf> {
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }
    std::env::current_dir()
        .map(|directory| directory.join(path))
        .map_err(|error| UpdateError::io(path, error))
}

fn path_identity(path: &Path) -> Result<PathBuf> {
    path_identity_inner(path, 0)
}

fn path_identity_inner(path: &Path, depth: usize) -> Result<PathBuf> {
    if depth > 8 {
        return Err(UpdateError::InvalidPolicy(
            "transaction path has too many symlink indirections",
        ));
    }
    let absolute = absolute(path)?;
    match std::fs::symlink_metadata(&absolute) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            let target =
                std::fs::read_link(&absolute).map_err(|error| UpdateError::io(&absolute, error))?;
            let target = if target.is_absolute() {
                target
            } else {
                absolute
                    .parent()
                    .ok_or(UpdateError::InvalidPolicy(
                        "transaction path must have a parent",
                    ))?
                    .join(target)
            };
            return path_identity_inner(&target, depth + 1);
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(UpdateError::io(&absolute, error)),
    }
    match std::fs::canonicalize(&absolute) {
        Ok(canonical) => Ok(canonical),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let parent = absolute.parent().ok_or(UpdateError::InvalidPolicy(
                "transaction path must have a parent",
            ))?;
            let canonical_parent = std::fs::canonicalize(parent)
                .map_err(|parent_error| UpdateError::io(parent, parent_error))?;
            Ok(
                canonical_parent.join(absolute.file_name().ok_or(UpdateError::InvalidPolicy(
                    "transaction path must have a file name",
                ))?),
            )
        }
        Err(error) => Err(UpdateError::io(&absolute, error)),
    }
}

fn suffix_path(path: &Path, suffix: &str) -> PathBuf {
    let mut value = path.as_os_str().to_os_string();
    value.push(suffix);
    PathBuf::from(value)
}

fn unique_backup(executable: &Path) -> PathBuf {
    let name = executable
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("executable");
    executable.with_file_name(format!(
        ".{name}.rollback-{}-{}",
        std::process::id(),
        TRANSACTION_COUNTER.fetch_add(1, Ordering::Relaxed)
    ))
}

fn create_backup(executable: &Path, backup: &Path, strategy: BackupStrategy) -> Result<()> {
    let hard_linked = strategy == BackupStrategy::HardLinkOrCopy
        && std::fs::hard_link(executable, backup).is_ok();
    if !hard_linked {
        let mut source =
            File::open(executable).map_err(|error| UpdateError::io(executable, error))?;
        let source_permissions = source
            .metadata()
            .map_err(|error| UpdateError::io(executable, error))?
            .permissions();
        let mut destination = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(backup)
            .map_err(|error| UpdateError::io(backup, error))?;
        std::io::copy(&mut source, &mut destination)
            .map_err(|error| UpdateError::io(backup, error))?;
        destination
            .set_permissions(source_permissions)
            .map_err(|error| UpdateError::io(backup, error))?;
        destination
            .sync_all()
            .map_err(|error| UpdateError::io(backup, error))?;
    }
    let synced = File::open(backup)
        .and_then(|file| file.sync_all())
        .map_err(|error| UpdateError::io(backup, error))
        .and_then(|()| sync_parent(backup));
    if let Err(error) = synced {
        std::fs::remove_file(backup).map_err(|cleanup| UpdateError::io(backup, cleanup))?;
        return Err(error);
    }
    Ok(())
}

fn hash_file(path: &Path) -> Result<String> {
    let mut file = File::open(path).map_err(|error| UpdateError::io(path, error))?;
    hash_reader(&mut file, path)
}

fn hash_stable_validated_artifact(validated: &ValidatedArtifact, path: &Path) -> Result<String> {
    let path_metadata =
        std::fs::symlink_metadata(path).map_err(|error| UpdateError::io(path, error))?;
    if !path_metadata.file_type().is_file()
        || ArtifactIdentity::from_metadata(&path_metadata) != validated.identity
    {
        return Err(UpdateError::ArtifactIdentityChanged {
            path: path.to_path_buf(),
        });
    }
    let mut file = File::open(path).map_err(|error| UpdateError::io(path, error))?;
    let opened_identity = ArtifactIdentity::from_metadata(
        &file
            .metadata()
            .map_err(|error| UpdateError::io(path, error))?,
    );
    if opened_identity != validated.identity {
        return Err(UpdateError::ArtifactIdentityChanged {
            path: path.to_path_buf(),
        });
    }
    let digest = hash_reader(&mut file, path)?;
    let after_read_identity = ArtifactIdentity::from_metadata(
        &file
            .metadata()
            .map_err(|error| UpdateError::io(path, error))?,
    );
    let final_path_metadata =
        std::fs::symlink_metadata(path).map_err(|error| UpdateError::io(path, error))?;
    if !final_path_metadata.file_type().is_file()
        || after_read_identity != validated.identity
        || ArtifactIdentity::from_metadata(&final_path_metadata) != validated.identity
    {
        return Err(UpdateError::ArtifactIdentityChanged {
            path: path.to_path_buf(),
        });
    }
    Ok(digest)
}

fn hash_reader(file: &mut File, path: &Path) -> Result<String> {
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        use std::io::Read;
        let read = file
            .read(&mut buffer)
            .map_err(|error| UpdateError::io(path, error))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

fn write_marker(updater: &Updater, path: &Path, marker: &Marker) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(marker).map_err(|error| UpdateError::InvalidMarker {
        path: path.to_path_buf(),
        message: error.to_string(),
    })?;
    // Every marker write holds the state lock, so one deterministic sibling is
    // sufficient and can be recovered without trusting a recycled PID.
    let temporary = suffix_path(path, ".tmp");
    let result = (|| {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)
            .map_err(|error| UpdateError::io(&temporary, error))?;
        use std::io::Write;
        file.write_all(&bytes)
            .map_err(|error| UpdateError::io(&temporary, error))?;
        file.sync_all()
            .map_err(|error| UpdateError::io(&temporary, error))?;
        updater.maybe_fail(TestFailpoint::AfterMarkerTempSync, &temporary)?;
        std::fs::rename(&temporary, path).map_err(|error| UpdateError::io(path, error))?;
        sync_parent(path)
    })();
    if result.is_err()
        && temporary.exists()
        && !updater.failpoint_active(TestFailpoint::AfterMarkerTempSync)
    {
        std::fs::remove_file(&temporary).map_err(|error| UpdateError::io(&temporary, error))?;
    }
    result
}

fn cleanup_marker_temp(state: &Path) -> Result<()> {
    use std::os::unix::fs::MetadataExt;

    let temporary = suffix_path(state, ".tmp");
    let metadata = match std::fs::symlink_metadata(&temporary) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(UpdateError::io(&temporary, error)),
    };
    let effective_uid = nix::unistd::geteuid().as_raw();
    if !metadata.file_type().is_file() || !marker_temp_owner_is_valid(metadata.uid(), effective_uid)
    {
        return Err(UpdateError::InvalidMarker {
            path: state.to_path_buf(),
            message: "marker temporary must be an owned non-symlink regular file".into(),
        });
    }
    std::fs::remove_file(&temporary).map_err(|error| UpdateError::io(&temporary, error))?;
    sync_parent(&temporary)
}

fn marker_temp_owner_is_valid(owner_uid: u32, effective_uid: u32) -> bool {
    owner_uid == effective_uid
}

fn read_marker(path: &Path, expected_executable: &Path) -> Result<Option<Marker>> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(UpdateError::io(path, error)),
    };
    if file
        .metadata()
        .map_err(|error| UpdateError::io(path, error))?
        .len()
        > MAX_MARKER_BYTES
    {
        return Err(UpdateError::InvalidMarker {
            path: path.to_path_buf(),
            message: format!("marker exceeds {MAX_MARKER_BYTES} byte limit"),
        });
    }
    use std::io::Read;
    let mut bytes = Vec::with_capacity(MAX_MARKER_BYTES as usize);
    file.take(MAX_MARKER_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| UpdateError::io(path, error))?;
    if bytes.len() as u64 > MAX_MARKER_BYTES {
        return Err(UpdateError::InvalidMarker {
            path: path.to_path_buf(),
            message: format!("marker exceeds {MAX_MARKER_BYTES} byte limit"),
        });
    }
    let marker: Marker =
        serde_json::from_slice(&bytes).map_err(|error| UpdateError::InvalidMarker {
            path: path.to_path_buf(),
            message: error.to_string(),
        })?;
    let executable = absolute(expected_executable)?;
    let valid_backup = marker.backup.is_absolute()
        && marker.backup.parent() == executable.parent()
        && exact_artifact_name(&executable, &marker.backup, "rollback", false).is_some();
    let valid_staged = marker.staged.is_absolute()
        && marker.staged.parent() == executable.parent()
        && exact_artifact_name(&executable, &marker.staged, "update", true).is_some();
    if marker.schema_version != 2
        || marker.executable != executable
        || !valid_backup
        || !valid_staged
    {
        return Err(UpdateError::InvalidMarker {
            path: path.to_path_buf(),
            message: "unsupported schema or unsafe recovery path".into(),
        });
    }
    validate_marker_backup_metadata(path, &marker)?;
    validate_marker_staged_metadata(path, &marker)?;
    Ok(Some(marker))
}

fn version_mismatch(running_version: &str, marker: &Marker) -> UpdateError {
    UpdateError::RunningVersionMismatch {
        running: running_version.to_owned(),
        target: marker.target.clone(),
    }
}

fn abort_prepared(state: &Path, marker: &Marker) -> Result<()> {
    remove_and_sync(state)?;
    remove_if_present_and_sync(&marker.backup)?;
    remove_if_present_and_sync(&marker.staged)
}

fn resume_rollback(
    updater: &Updater,
    state: &Path,
    mut marker: Marker,
    running_version: &str,
) -> Result<RecoveryAction> {
    let executable_digest = hash_file(&marker.executable)?;
    if running_version == marker.previous && executable_digest == marker.previous_sha256 {
        marker.phase = MarkerPhase::RolledBack;
        write_marker(updater, state, &marker)?;
        return finalize_rollback(state, marker);
    }
    if running_version != marker.target || executable_digest != marker.sha256 {
        return Err(version_mismatch(running_version, &marker));
    }
    validate_rollback_backup(state, &marker)?;
    std::fs::rename(&marker.backup, &marker.executable)
        .map_err(|error| UpdateError::io(&marker.executable, error))?;
    sync_parent(&marker.executable)?;
    updater.maybe_fail(TestFailpoint::AfterRollbackRename, &marker.executable)?;
    marker.phase = MarkerPhase::RolledBack;
    write_marker(updater, state, &marker)?;
    finalize_rollback(state, marker)
}

fn finish_rollback(state: &Path, marker: Marker, running_version: &str) -> Result<RecoveryAction> {
    if running_version != marker.previous
        || hash_file(&marker.executable)? != marker.previous_sha256
    {
        return Err(version_mismatch(running_version, &marker));
    }
    finalize_rollback(state, marker)
}

fn finalize_rollback(state: &Path, marker: Marker) -> Result<RecoveryAction> {
    remove_and_sync(state)?;
    Ok(RecoveryAction::RollbackInstalled {
        executable: marker.executable,
        restored_version: marker.previous,
    })
}

fn remove_if_present_and_sync(path: &Path) -> Result<()> {
    match std::fs::remove_file(path) {
        Ok(()) => sync_parent(path),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(UpdateError::io(path, error)),
    }
}

fn remove_and_sync(path: &Path) -> Result<()> {
    remove_file(path)?;
    sync_parent(path)
}

fn remove_file(path: &Path) -> Result<()> {
    std::fs::remove_file(path).map_err(|error| UpdateError::io(path, error))
}

fn sync_parent(path: &Path) -> Result<()> {
    let parent = path.parent().ok_or(UpdateError::InvalidPolicy(
        "transaction path must have a parent",
    ))?;
    File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| UpdateError::io(parent, error))
}

#[cfg(test)]
#[path = "transaction_tests.rs"]
mod tests;

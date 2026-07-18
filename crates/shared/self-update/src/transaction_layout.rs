use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};

use fs2::FileExt;

use super::transaction_io::{path_identity, suffix_path};
use crate::{Result, UpdateError, Updater, reject_executable_leaf_symlink};

pub(super) struct TransactionLock {
    file: File,
}

impl Drop for TransactionLock {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.file);
    }
}

pub(super) struct LayoutPaths {
    pub(super) executable: PathBuf,
    pub(super) state: PathBuf,
    pub(super) lock: PathBuf,
}

impl Updater {
    pub(super) fn transaction_lock(&self, lock_path: &Path) -> Result<TransactionLock> {
        use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};

        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .mode(0o600)
            .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_NONBLOCK)
            .open(lock_path)
            .map_err(|error| UpdateError::io(lock_path, error))?;
        let metadata = file
            .metadata()
            .map_err(|error| UpdateError::io(lock_path, error))?;
        if !metadata.file_type().is_file() || metadata.uid() != nix::unistd::geteuid().as_raw() {
            return Err(UpdateError::InvalidMarker {
                path: lock_path.to_path_buf(),
                message: "transaction lock must be a service-owned non-symlink regular file".into(),
            });
        }
        if metadata.mode() & 0o777 != 0o600 {
            file.set_permissions(std::fs::Permissions::from_mode(0o600))
                .map_err(|error| UpdateError::io(lock_path, error))?;
            file.sync_all()
                .map_err(|error| UpdateError::io(lock_path, error))?;
            let repaired = file
                .metadata()
                .map_err(|error| UpdateError::io(lock_path, error))?;
            if repaired.mode() & 0o777 != 0o600 {
                return Err(UpdateError::InvalidMarker {
                    path: lock_path.to_path_buf(),
                    message: "transaction lock permissions must be 0600".into(),
                });
            }
        }
        file.try_lock_exclusive().map_err(|error| {
            if error.kind() == std::io::ErrorKind::WouldBlock {
                UpdateError::UpdateInProgress {
                    path: lock_path.to_path_buf(),
                }
            } else {
                UpdateError::io(lock_path, error)
            }
        })?;
        Ok(TransactionLock { file })
    }

    pub(super) fn validated_layout(&self) -> Result<LayoutPaths> {
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

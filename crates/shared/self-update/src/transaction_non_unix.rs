use std::path::PathBuf;

use crate::{RecoveryAction, Result, UpdateError, Updater, ValidatedArtifact};

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

impl Updater {
    pub async fn install(
        &self,
        _validated: ValidatedArtifact,
        _previous_version: impl Into<String>,
    ) -> Result<InstallOutcome> {
        Err(UpdateError::UnsupportedPlatform)
    }

    pub async fn recover_on_startup(&self, _running_version: &str) -> Result<RecoveryAction> {
        Err(UpdateError::UnsupportedPlatform)
    }

    pub async fn confirm_success(&self, _running_version: &str) -> Result<ConfirmationOutcome> {
        Err(UpdateError::UnsupportedPlatform)
    }
}

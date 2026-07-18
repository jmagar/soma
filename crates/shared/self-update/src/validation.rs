use std::process::Stdio;

use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::Command;

use crate::{Result, StagedArtifact, UpdateError, Updater};

const OUTPUT_LIMIT: usize = 16 * 1024;

/// An artifact that executed successfully and reported its exact target version.
#[derive(Debug)]
pub struct ValidatedArtifact {
    pub(crate) staged: StagedArtifact,
}

impl ValidatedArtifact {
    pub fn path(&self) -> &std::path::Path {
        self.staged.path()
    }
    pub fn target_version(&self) -> &str {
        self.staged.target_version()
    }
    pub fn sha256(&self) -> &str {
        self.staged.sha256()
    }
}

impl Updater {
    /// Executes `--version` and consumes the staged artifact on success.
    pub async fn validate(&self, staged: StagedArtifact) -> Result<ValidatedArtifact> {
        let path = staged.path().to_path_buf();
        let timeout = self.policy().validation_timeout();
        let deadline = tokio::time::Instant::now() + timeout;
        let mut child = match tokio::time::timeout_at(deadline, spawn_validator(&path)).await {
            Ok(result) => result?,
            Err(_) => return Err(UpdateError::ValidationTimedOut { timeout }),
        };
        #[cfg(unix)]
        let process_group = child.id().map(|id| id as i32);
        let stdout = child.stdout.take().expect("piped stdout is configured");
        let stderr = child.stderr.take().expect("piped stderr is configured");
        let completed = tokio::time::timeout_at(deadline, async {
            let (status, stdout, stderr) = tokio::join!(
                child.wait(),
                read_bounded(stdout),
                read_bounded(stderr)
            );
            (status, stdout, stderr)
        })
        .await;
        let (status, stdout, stderr) = match completed {
            Ok((status, stdout, stderr)) => (
                status.map_err(|error| UpdateError::io(&path, error))?,
                stdout.map_err(|error| UpdateError::io(&path, error))?,
                stderr.map_err(|error| UpdateError::io(&path, error))?,
            ),
            Err(_) => {
                #[cfg(unix)]
                if let Some(process_group) = process_group {
                    use nix::sys::signal::{Signal, killpg};
                    use nix::unistd::Pid;
                    let _ = killpg(Pid::from_raw(process_group), Signal::SIGKILL);
                }
                let _ = child.kill().await;
                let _ = child.wait().await;
                return Err(UpdateError::ValidationTimedOut { timeout });
            }
        };
        if stdout.overflowed {
            return Err(UpdateError::ValidationOutputTooLarge {
                stream: "stdout",
                limit: OUTPUT_LIMIT,
            });
        }
        if stderr.overflowed {
            return Err(UpdateError::ValidationOutputTooLarge {
                stream: "stderr",
                limit: OUTPUT_LIMIT,
            });
        }
        let stderr_text = String::from_utf8_lossy(&stderr.bytes).into_owned();
        if !status.success() {
            return Err(UpdateError::ValidationFailed {
                code: status.code(),
                stderr: stderr_text,
            });
        }
        let output =
            String::from_utf8(stdout.bytes).map_err(|_| UpdateError::InvalidVersionOutput)?;
        let expected = staged.target_version();
        let matches = output.split_ascii_whitespace().any(|token| {
            token.trim_matches(|character: char| character.is_ascii_punctuation()) == expected
        });
        if !matches {
            return Err(UpdateError::VersionMismatch {
                expected: expected.to_owned(),
                output: output.trim().to_owned(),
            });
        }
        Ok(ValidatedArtifact { staged })
    }
}

async fn spawn_validator(path: &std::path::Path) -> Result<tokio::process::Child> {
    // Tokio's asynchronous file close can briefly race exec on Linux and
    // surface ETXTBSY even after the staged writer has been flushed and
    // converted back to a closed std file. Retry only that transient kernel
    // condition; every other spawn error remains immediate and typed.
    for _ in 0..10 {
        let result = validator_command(path).spawn();
        match result {
            Ok(child) => return Ok(child),
            Err(error) if error.raw_os_error() == Some(26) => {
                tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            }
            Err(error) => return Err(UpdateError::io(path, error)),
        }
    }
    validator_command(path)
        .spawn()
        .map_err(|error| UpdateError::io(path, error))
}

fn validator_command(path: &std::path::Path) -> Command {
    let mut command = Command::new(path);
    command
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(unix)]
    command.process_group(0);
    command
}

struct BoundedOutput {
    bytes: Vec<u8>,
    overflowed: bool,
}

async fn read_bounded(mut reader: impl AsyncRead + Unpin) -> std::io::Result<BoundedOutput> {
    let mut bytes = Vec::with_capacity(OUTPUT_LIMIT.min(4096));
    let mut buffer = [0_u8; 4096];
    let mut overflowed = false;
    loop {
        let read = reader.read(&mut buffer).await?;
        if read == 0 {
            break;
        }
        let remaining = OUTPUT_LIMIT.saturating_sub(bytes.len());
        bytes.extend_from_slice(&buffer[..read.min(remaining)]);
        overflowed |= read > remaining;
    }
    Ok(BoundedOutput { bytes, overflowed })
}

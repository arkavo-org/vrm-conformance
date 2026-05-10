//! Subprocess wrapper around the mrxz/vrm-validator native CLI shim
//! (installed by `scripts/install-validator.sh` to `.tools/vrm-validator-cli`).
//!
//! The shim's exit codes:
//!
//! - 0 = no validation errors; JSON report on stdout
//! - 1 = validation errors found; JSON report on stdout
//! - 2 = bad usage (e.g. missing input path)
//! - 3 = internal error (e.g. input not detectable as glTF/VRM)
//!
//! Exit code 0 or 1 → return Ok(report). Anything else → return Err.

use camino::{Utf8Path, Utf8PathBuf};
use serde::Deserialize;
use std::process::Command;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ValidatorError {
    #[error("validator shim not found at {0}; run scripts/install-validator.sh")]
    NotInstalled(Utf8PathBuf),

    #[error("input path does not exist: {0}")]
    InputMissing(Utf8PathBuf),

    #[error("validator process failed: {0}")]
    Process(#[from] std::io::Error),

    #[error("validator usage error (exit 2): {stderr}")]
    BadUsage { stderr: String },

    #[error("validator internal error (exit 3): {stderr}")]
    Internal { stderr: String },

    #[error("validator exited with unexpected status {status}: {stderr}")]
    UnexpectedExit { status: i32, stderr: String },

    #[error("validator emitted unparseable JSON: {0}")]
    BadJson(#[from] serde_json::Error),
}

#[derive(Debug, Clone)]
pub struct ValidatorConfig {
    pub binary: Utf8PathBuf,
}

impl ValidatorConfig {
    /// Resolve from the `VRM_VALIDATOR_BIN` env var or fall back to
    /// `./.tools/vrm-validator-cli` relative to the current working directory.
    pub fn from_env() -> Result<Self, ValidatorError> {
        let candidate = std::env::var("VRM_VALIDATOR_BIN")
            .map(Utf8PathBuf::from)
            .unwrap_or_else(|_| Utf8PathBuf::from(".tools/vrm-validator-cli"));

        if !candidate.exists() {
            return Err(ValidatorError::NotInstalled(candidate));
        }
        Ok(Self { binary: candidate })
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ValidatorReport {
    #[serde(default)]
    pub uri: Option<String>,

    #[serde(default, rename = "mimeType")]
    pub mime_type: Option<String>,

    #[serde(default, rename = "validatorVersion")]
    pub validator_version: Option<String>,

    #[serde(default)]
    pub issues: Issues,

    #[serde(default)]
    pub info: serde_json::Value,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Issues {
    #[serde(default, rename = "numErrors")]
    pub num_errors: u32,

    #[serde(default, rename = "numWarnings")]
    pub num_warnings: u32,

    #[serde(default, rename = "numInfos")]
    pub num_infos: u32,

    #[serde(default, rename = "numHints")]
    pub num_hints: u32,

    #[serde(default)]
    pub messages: Vec<IssueMessage>,

    #[serde(default)]
    pub truncated: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct IssueMessage {
    pub code: String,
    pub message: String,
    #[serde(default)]
    pub severity: u32,
    #[serde(default)]
    pub pointer: Option<String>,
}

pub fn validate(
    config: &ValidatorConfig,
    input: &Utf8Path,
) -> Result<ValidatorReport, ValidatorError> {
    if !input.exists() {
        return Err(ValidatorError::InputMissing(input.to_owned()));
    }

    tracing::debug!(binary = %config.binary, input = %input, "running validator");

    let output = Command::new(config.binary.as_std_path())
        .arg(input.as_str())
        .output()?;

    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let status = output.status.code().unwrap_or(-1);

    match status {
        0 | 1 => {
            // Both clean and errors-found cases return the JSON report.
            let report: ValidatorReport = serde_json::from_slice(&output.stdout)?;
            Ok(report)
        }
        2 => Err(ValidatorError::BadUsage { stderr }),
        3 => Err(ValidatorError::Internal { stderr }),
        s => Err(ValidatorError::UnexpectedExit { status: s, stderr }),
    }
}

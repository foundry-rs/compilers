use super::super::CompilerError;
use semver::Version;

#[derive(Debug, thiserror::Error)]
pub enum SolcError {
    #[error(transparent)]
    Svm(#[from] svm::SvmError),
    #[error("solc exited with {1}\n{2}")]
    SolcError(Option<Version>, std::process::ExitStatus, String),
    #[error("invalid UTF-8 in solc output")]
    InvalidUtf8,
    #[error("no svm home dir")]
    NoSvmHomeDir,
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Serde(#[from] serde_json::Error),
    #[error("solc version {0} not installed")]
    VersionNotInstalled(Version),
    #[error("{0}")]
    Message(String),
    #[error(transparent)]
    SemVer(#[from] semver::Error),
}

impl SolcError {
    /// Create an error from the Solc executable's output.
    pub(crate) fn solc_output(version: Option<Version>, output: &std::process::Output) -> Self {
        let mut msg = String::from_utf8_lossy(&output.stderr);
        let mut trimmed = msg.trim();
        if trimmed.is_empty() {
            msg = String::from_utf8_lossy(&output.stdout);
            trimmed = msg.trim();
            if trimmed.is_empty() {
                trimmed = "<empty output>";
            }
        }
        SolcError::SolcError(version, output.status, trimmed.into())
    }

    pub(crate) fn msg(msg: impl std::fmt::Display) -> Self {
        SolcError::Message(msg.to_string())
    }
}

impl CompilerError for SolcError {
    fn compiler_version(&self) -> Option<&Version> {
        match self {
            SolcError::VersionNotInstalled(v) => Some(v),
            _ => None,
        }
    }
}

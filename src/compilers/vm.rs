use core::fmt;
use std::fmt::Debug;

use super::Compiler;
use auto_impl::auto_impl;
use semver::Version;
use serde::{Deserialize, Serialize};

/// A compiler version is either installed (available locally) or can be downloaded, from the remote
/// endpoint
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CompilerVersion {
    Installed(Version),
    Remote(Version),
}

impl CompilerVersion {
    pub fn is_installed(&self) -> bool {
        matches!(self, CompilerVersion::Installed(_))
    }
}

impl AsRef<Version> for CompilerVersion {
    fn as_ref(&self) -> &Version {
        match self {
            CompilerVersion::Installed(v) | CompilerVersion::Remote(v) => v,
        }
    }
}

impl From<CompilerVersion> for Version {
    fn from(s: CompilerVersion) -> Version {
        match s {
            CompilerVersion::Installed(v) | CompilerVersion::Remote(v) => v,
        }
    }
}

impl fmt::Display for CompilerVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_ref())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum VersionManagerError {
    #[error("compiler version {0} not installed")]
    VersionNotInstalled(Version),

    #[error("{0}")]
    Message(String),

    #[error("installation failed: {0}")]
    IntallationFailed(Box<dyn std::error::Error + Send + Sync>),
}

impl VersionManagerError {
    pub fn msg(msg: impl std::fmt::Display) -> Self {
        VersionManagerError::Message(msg.to_string())
    }
}

/// Abstraction over a compiler version manager. Currently main implementation is
/// [`SolcVersionManager`]. Acts as a factory of [Compiler]s.
///
/// [`SolcVersionManager`]: crate::compilers::solc::SolcVersionManager
#[auto_impl(&, Box, Arc)]
pub trait CompilerVersionManager: Debug {
    type Compiler: Compiler;

    /// Returns all versions available locally and remotely.
    fn all_versions(&self) -> Vec<CompilerVersion>;
    /// Returns all versions available locally.
    fn installed_versions(&self) -> Vec<CompilerVersion>;

    /// Installs a compiler version and returns the compiler instance.
    fn install(&self, version: &Version) -> Result<Self::Compiler, VersionManagerError>;
    /// Returns the compiler instance for the given version if it is installed. If not, returns an
    /// error.
    fn get_installed(&self, version: &Version) -> Result<Self::Compiler, VersionManagerError>;

    /// Returns the compiler instance for the given version if it is installed. If not, installs it.
    fn get_or_install(&self, version: &Version) -> Result<Self::Compiler, VersionManagerError> {
        self.get_installed(version).or_else(|_| self.install(version))
    }
}

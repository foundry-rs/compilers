use crate::{
    artifacts::{
        output_selection::OutputSelection, Contract, FileToContractsMap, SourceFile, Sources,
    },
    error::Result,
    remappings::Remapping,
    ProjectPathsConfig,
};
use auto_impl::auto_impl;
use semver::{Version, VersionReq};
use serde::{de::DeserializeOwned, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Debug,
    path::{Path, PathBuf},
};

pub mod solc;
mod vm;
pub mod vyper;
pub use vm::{CompilerVersion, CompilerVersionManager, VersionManagerError};

pub trait CompilerSettings:
    Default + Serialize + DeserializeOwned + Clone + Debug + Send + Sync
{
    fn output_selection_mut(&mut self) -> &mut OutputSelection;

    /// Returns true if artifacts compiled with given `other` config are compatible with this
    /// config and if compilation can be skipped.
    ///
    /// Ensures that all settings fields are equal except for `output_selection` which is required
    /// to be a subset of `cached.output_selection`.
    fn can_use_cached(&self, other: &Self) -> bool;
}

pub trait CompilerInput: Serialize + Send + Sized {
    type Settings;

    fn build(sources: Sources, settings: Self::Settings, version: &Version) -> Vec<Self>;
    fn sources(&self) -> &Sources;
    fn with_remappings(self, _remappings: Vec<Remapping>) -> Self {
        self
    }
}

pub trait ParsedSource: Debug + Sized + Send {
    fn parse(content: &str, file: &Path) -> Self;
    fn version_req(&self) -> Option<&VersionReq>;
    fn resolve_imports(&self, paths: &ProjectPathsConfig) -> Vec<PathBuf>;
}

pub trait CompilerError: std::error::Error + Send + Sync {
    fn compiler_version(&self) -> Option<&Version>;
}

pub struct CompilerOutput<E> {
    pub errors: Vec<E>,
    pub contracts: FileToContractsMap<Contract>,
    pub sources: BTreeMap<PathBuf, SourceFile>,
}

/// Error returned by compiler. Might also represent a warning or informational message.
pub trait CompilationError: DeserializeOwned + Send + Debug {
    fn is_warning(&self) -> bool;
    fn is_error(&self) -> bool;
    fn source_location(&self) -> Option<crate::artifacts::error::SourceLocation>;
    fn severity(&self) -> crate::artifacts::error::Severity;
    fn error_code(&self) -> Option<u64>;
}

pub trait Compiler: Send + Sync + Clone {
    type Input: CompilerInput<Settings = Self::Settings>;
    type CompilationError: CompilationError;
    type ParsedSource: ParsedSource;
    type Settings: CompilerSettings;

    fn compile(
        &self,
        input: Self::Input,
    ) -> Result<(Self::Input, CompilerOutput<Self::CompilationError>)>;

    fn version(&self) -> &Version;

    fn with_base_path(self, _base_path: PathBuf) -> Self {
        self
    }
    fn with_allowed_paths(self, _allowed_paths: BTreeSet<PathBuf>) -> Self {
        self
    }
    fn with_include_paths(self, _include_paths: BTreeSet<PathBuf>) -> Self {
        self
    }
}

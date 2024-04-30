use crate::{
    artifacts::{
        output_selection::OutputSelection, Contract, FileToContractsMap, SourceFile, Sources,
    },
    error::Result,
    remappings::Remapping,
    ProjectPathsConfig,
};
use semver::{Version, VersionReq};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    fmt::{Debug, Display},
    path::{Path, PathBuf},
};

pub mod solc;
mod vm;
pub mod vyper;
pub use vm::{CompilerVersion, CompilerVersionManager, VersionManagerError};

/// Compilation settings including evm_version, output_selection, etc.
pub trait CompilerSettings:
    Default + Serialize + DeserializeOwned + Clone + Debug + Send + Sync
{
    /// Returns mutable reference to configured [OutputSelection].
    fn output_selection_mut(&mut self) -> &mut OutputSelection;

    /// Returns true if artifacts compiled with given `other` config are compatible with this
    /// config and if compilation can be skipped.
    ///
    /// Ensures that all settings fields are equal except for `output_selection` which is required
    /// to be a subset of `cached.output_selection`.
    fn can_use_cached(&self, other: &Self) -> bool;
}

/// Input of a compiler, including sources and settings used for their compilation.
pub trait CompilerInput: Serialize + Send + Sized {
    type Settings;

    /// Constructs one or multiple inputs from given sources set. Might return multiple inputs in
    /// cases when sources need to be divided into sets per language (Yul + Solidity for example).
    fn build(sources: Sources, settings: Self::Settings, version: &Version) -> Vec<Self>;

    /// Returns reference to sources included into this input.
    fn sources(&self) -> &Sources;

    /// Method which might be invoked to add remappings to the input.
    fn with_remappings(self, _remappings: Vec<Remapping>) -> Self {
        self
    }

    /// Returns compiler name used by reporters to display output during compilation.
    fn compiler_name(&self) -> String;
}

/// Parser of the source files which is used to identify imports and version requirements of the
/// given source. Used by path resolver to resolve imports or determine compiler versions needed to
/// compiler given sources.
pub trait ParsedSource: Debug + Sized + Send {
    fn parse(content: &str, file: &Path) -> Self;
    fn version_req(&self) -> Option<&VersionReq>;
    fn resolve_imports(&self, paths: &ProjectPathsConfig) -> Vec<PathBuf>;
}

/// Error returned by compiler. Might also represent a warning or informational message.
pub trait CompilationError: Serialize + DeserializeOwned + Send + Display + Debug {
    fn is_warning(&self) -> bool;
    fn is_error(&self) -> bool;
    fn source_location(&self) -> Option<crate::artifacts::error::SourceLocation>;
    fn severity(&self) -> crate::artifacts::error::Severity;
    fn error_code(&self) -> Option<u64>;
}

/// Output of the compiler, including contracts, sources and errors. Currently only generic over the
/// error but might be extended in the future.
#[derive(Debug, Serialize, Deserialize)]
pub struct CompilerOutput<E> {
    #[serde(default = "Vec::new", skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<E>,
    #[serde(default)]
    pub contracts: FileToContractsMap<Contract>,
    #[serde(default)]
    pub sources: BTreeMap<PathBuf, SourceFile>,
}

impl<E> CompilerOutput<E> {
    /// Retains only those files the given iterator yields
    ///
    /// In other words, removes all contracts for files not included in the iterator
    pub fn retain_files<'a, I>(&mut self, files: I)
    where
        I: IntoIterator<Item = &'a Path>,
    {
        // Note: use `to_lowercase` here because solc not necessarily emits the exact file name,
        // e.g. `src/utils/upgradeProxy.sol` is emitted as `src/utils/UpgradeProxy.sol`
        let files: HashSet<_> =
            files.into_iter().map(|s| s.to_string_lossy().to_lowercase()).collect();
        self.contracts.retain(|f, _| files.contains(&f.to_string_lossy().to_lowercase()));
        self.sources.retain(|f, _| files.contains(&f.to_string_lossy().to_lowercase()));
    }

    pub fn merge(&mut self, other: CompilerOutput<E>) {
        self.errors.extend(other.errors);
        self.contracts.extend(other.contracts);
        self.sources.extend(other.sources);
    }
}

impl<E> Default for CompilerOutput<E> {
    fn default() -> Self {
        Self { errors: Vec::new(), contracts: BTreeMap::new(), sources: BTreeMap::new() }
    }
}

/// The main compiler abstraction trait. Currently mostly represents a wrapper around compiler
/// binary aware of the version and able to compile given input into [CompilerOutput] including
/// artifacts and errors.
pub trait Compiler: Send + Sync + Clone {
    type Input: CompilerInput<Settings = Self::Settings>;
    type CompilationError: CompilationError;
    type ParsedSource: ParsedSource;
    type Settings: CompilerSettings;

    /// Main entrypoint for the compiler. Compiles given input into [CompilerOutput]. Takes
    /// ownership over the input and returns back version with potential modifications made to it.
    /// Returned input is always the one which was seen by the binary.
    fn compile(
        &self,
        input: Self::Input,
    ) -> Result<(Self::Input, CompilerOutput<Self::CompilationError>)>;

    /// Returns the version of the compiler.
    fn version(&self) -> &Version;

    /// Builder method to set the base path for the compiler. Primarily used by solc implementation
    /// to se --base-path.
    fn with_base_path(self, _base_path: PathBuf) -> Self {
        self
    }

    /// Builder method to set the allowed paths for the compiler. Primarily used by solc
    /// implementation to set --allow-paths.
    fn with_allowed_paths(self, _allowed_paths: BTreeSet<PathBuf>) -> Self {
        self
    }

    /// Builder method to set the include paths for the compiler. Primarily used by solc
    /// implementation to set --include-paths.
    fn with_include_paths(self, _include_paths: BTreeSet<PathBuf>) -> Self {
        self
    }
}

#[cfg(feature = "svm-solc")]
mod version_manager;
#[cfg(feature = "svm-solc")]
pub use version_manager::SolcVersionManager;

use itertools::Itertools;

use super::{
    version_manager::CompilerVersion, CompilationError, Compiler, CompilerInput, CompilerOutput,
    CompilerSettings, Language, ParsedSource,
};
use crate::{
    artifacts::{
        output_selection::OutputSelection, Error, Settings as SolcSettings, SolcInput, Sources,
    },
    error::{Result, SolcError},
    remappings::Remapping,
    resolver::parse::SolData,
    utils::RuntimeOrHandle,
    Solc, SOLC_EXTENSIONS,
};
use semver::Version;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum SolcLanguages {
    Solidity,
    Yul,
}

impl Language for SolcLanguages {
    const FILE_EXTENSIONS: &'static [&'static str] = SOLC_EXTENSIONS;
}

impl Compiler for Solc {
    type Input = SolcVerionedInput;
    type CompilationError = crate::artifacts::Error;
    type ParsedSource = SolData;
    type Settings = SolcSettings;
    type Language = SolcLanguages;

    fn compile(&self, input: &Self::Input) -> Result<CompilerOutput<Self::CompilationError>> {
        let solc =
            if let Some(solc) = Solc::find_svm_installed_version(input.version().to_string())? {
                solc
            } else {
                #[cfg(test)]
                crate::take_solc_installer_lock!(_lock);

                let version = if !input.version.pre.is_empty() || !input.version.build.is_empty() {
                    Version::new(input.version.major, input.version.minor, input.version.patch)
                } else {
                    input.version.clone()
                };

                trace!("blocking installing solc version \"{}\"", version);
                crate::report::solc_installation_start(&version);
                // The async version `svm::install` is used instead of `svm::blocking_intsall`
                // because the underlying `reqwest::blocking::Client` does not behave well
                // inside of a Tokio runtime. See: https://github.com/seanmonstar/reqwest/issues/1017
                match RuntimeOrHandle::new().block_on(svm::install(&version)) {
                    Ok(path) => {
                        crate::report::solc_installation_success(&version);
                        Ok(Solc::new_with_version(path, version))
                    }
                    Err(err) => {
                        crate::report::solc_installation_error(&version, &err.to_string());
                        Err(SolcError::msg(format!("failed to install {}", version)))
                    }
                }?
            };
        let solc_output = solc.compile(&input.input)?;

        let output = CompilerOutput {
            errors: solc_output.errors,
            contracts: solc_output.contracts,
            sources: solc_output.sources,
        };

        Ok(output)
    }

    fn available_versions(&self, _language: &Self::Language) -> Vec<CompilerVersion> {
        Solc::installed_versions().into_iter().map(CompilerVersion::Installed).collect()
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SolcVerionedInput {
    #[serde(skip)]
    pub version: Version,
    #[serde(flatten)]
    pub input: SolcInput,
    #[serde(skip)]
    pub allowed_paths: BTreeSet<PathBuf>,
    #[serde(skip)]
    pub base_path: PathBuf,
    #[serde(skip)]
    pub include_paths: BTreeSet<PathBuf>,
}

impl CompilerInput for SolcVerionedInput {
    type Settings = SolcSettings;
    type Language = SolcLanguages;

    /// Creates a new [CompilerInput]s with default settings and the given sources
    ///
    /// A [CompilerInput] expects a language setting, supported by solc are solidity or yul.
    /// In case the `sources` is a mix of solidity and yul files, 2 CompilerInputs are returned
    fn build(
        sources: Sources,
        settings: Self::Settings,
        language: Self::Language,
        version: &Version,
    ) -> Self {
        let input = SolcInput::new(language, sources, settings).sanitized(version);

        Self {
            version: version.clone(),
            input,
            allowed_paths: BTreeSet::new(),
            base_path: PathBuf::new(),
            include_paths: BTreeSet::new(),
        }
    }

    fn sources(&self) -> &Sources {
        &self.input.sources
    }

    fn language(&self) -> Self::Language {
        self.input.language.clone()
    }

    fn version(&self) -> &Version {
        &self.version
    }

    fn with_remappings(mut self, remappings: Vec<Remapping>) -> Self {
        self.input = self.input.with_remappings(remappings);

        self
    }

    fn compiler_name(&self) -> String {
        "Solc".to_string()
    }

    fn strip_prefix(&mut self, base: &Path) {
        self.input.strip_prefix(base);
    }

    fn with_allowed_paths(mut self, allowed_paths: BTreeSet<PathBuf>) -> Self {
        self.allowed_paths = allowed_paths;
        self
    }

    fn with_base_path(mut self, base_path: PathBuf) -> Self {
        self.base_path = base_path;
        self
    }

    fn with_include_paths(mut self, include_paths: BTreeSet<PathBuf>) -> Self {
        self.include_paths = include_paths;
        self
    }
}

impl CompilerSettings for SolcSettings {
    fn output_selection_mut(&mut self) -> &mut OutputSelection {
        &mut self.output_selection
    }

    fn can_use_cached(&self, other: &Self) -> bool {
        let SolcSettings {
            stop_after,
            remappings,
            optimizer,
            model_checker,
            metadata,
            output_selection,
            evm_version,
            via_ir,
            debug,
            libraries,
        } = self;

        *stop_after == other.stop_after
            && *remappings == other.remappings
            && *optimizer == other.optimizer
            && *model_checker == other.model_checker
            && *metadata == other.metadata
            && *evm_version == other.evm_version
            && *via_ir == other.via_ir
            && *debug == other.debug
            && *libraries == other.libraries
            && output_selection.is_subset_of(&other.output_selection)
    }
}

impl ParsedSource for SolData {
    type Language = SolcLanguages;

    fn parse(content: &str, file: &std::path::Path) -> Self {
        SolData::parse(content, file)
    }

    fn version_req(&self) -> Option<&semver::VersionReq> {
        self.version_req.as_ref()
    }

    fn resolve_imports<C>(&self, _paths: &crate::ProjectPathsConfig<C>) -> Result<Vec<PathBuf>> {
        return Ok(self.imports.iter().map(|i| i.data().path().to_path_buf()).collect_vec());
    }

    fn language(&self) -> Self::Language {
        if self.is_yul {
            SolcLanguages::Yul
        } else {
            SolcLanguages::Solidity
        }
    }
}

impl CompilationError for Error {
    fn is_warning(&self) -> bool {
        self.severity.is_warning()
    }
    fn is_error(&self) -> bool {
        self.severity.is_error()
    }

    fn source_location(&self) -> Option<crate::artifacts::error::SourceLocation> {
        self.source_location.clone()
    }

    fn severity(&self) -> crate::artifacts::error::Severity {
        self.severity
    }

    fn error_code(&self) -> Option<u64> {
        self.error_code
    }
}

use super::{
    CompilationError, Compiler, CompilerInput, CompilerOutput, CompilerSettings, ParsedSource,
};
use crate::{
    artifacts::{
        output_selection::OutputSelection, Error, Settings as SolcSettings, SolcInput, Sources,
        SOLIDITY, YUL,
    },
    error::Result,
    remappings::Remapping,
    resolver::parse::SolData,
    Solc, SOLC_EXTENSIONS,
};
use itertools::Itertools;
use semver::Version;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

#[cfg(feature = "svm-solc")]
mod version_manager;
#[cfg(feature = "svm-solc")]
pub use version_manager::SolcVersionManager;

impl Compiler for Solc {
    const FILE_EXTENSIONS: &'static [&'static str] = SOLC_EXTENSIONS;

    type Input = SolcInput;
    type CompilationError = crate::artifacts::Error;
    type ParsedSource = SolData;
    type Settings = SolcSettings;

    fn compile(&self, input: &Self::Input) -> Result<CompilerOutput<Self::CompilationError>> {
        let solc_output = self.compile(&input)?;

        let output = CompilerOutput {
            errors: solc_output.errors,
            contracts: solc_output.contracts,
            sources: solc_output.sources,
        };

        Ok(output)
    }

    fn version(&self) -> &Version {
        &self.version
    }

    fn with_allowed_paths(mut self, allowed_paths: BTreeSet<PathBuf>) -> Self {
        self.allow_paths = allowed_paths;
        self
    }

    fn with_base_path(mut self, base_path: PathBuf) -> Self {
        self.base_path = Some(base_path);
        self
    }

    fn with_include_paths(mut self, include_paths: BTreeSet<PathBuf>) -> Self {
        self.include_paths = include_paths;
        self
    }
}

impl CompilerInput for SolcInput {
    type Settings = SolcSettings;

    /// Creates a new [CompilerInput]s with default settings and the given sources
    ///
    /// A [CompilerInput] expects a language setting, supported by solc are solidity or yul.
    /// In case the `sources` is a mix of solidity and yul files, 2 CompilerInputs are returned
    fn build(sources: Sources, mut settings: Self::Settings, version: &Version) -> Vec<Self> {
        settings.sanitize(version);
        if let Some(ref mut evm_version) = settings.evm_version {
            settings.evm_version = evm_version.normalize_version_solc(version);
        }

        let mut solidity_sources = BTreeMap::new();
        let mut yul_sources = BTreeMap::new();
        for (path, source) in sources {
            if path.extension() == Some(std::ffi::OsStr::new("yul")) {
                yul_sources.insert(path, source);
            } else {
                solidity_sources.insert(path, source);
            }
        }
        let mut res = Vec::new();
        if !solidity_sources.is_empty() {
            res.push(Self {
                language: SOLIDITY.to_string(),
                sources: solidity_sources,
                settings: settings.clone(),
            });
        }
        if !yul_sources.is_empty() {
            if !settings.remappings.is_empty() {
                warn!("omitting remappings supplied for the yul sources");
                settings.remappings = vec![];
            }

            if let Some(debug) = settings.debug.as_mut() {
                if debug.revert_strings.is_some() {
                    warn!("omitting revertStrings supplied for the yul sources");
                    debug.revert_strings = None;
                }
            }
            res.push(Self { language: YUL.to_string(), sources: yul_sources, settings });
        }
        res
    }

    fn sources(&self) -> &Sources {
        &self.sources
    }

    fn with_remappings(mut self, remappings: Vec<Remapping>) -> Self {
        if self.language == YUL {
            if !remappings.is_empty() {
                warn!("omitting remappings supplied for the yul sources");
            }
        } else {
            self.settings.remappings = remappings;
        }
        self
    }

    fn compiler_name(&self) -> String {
        "Solc".to_string()
    }

    fn strip_prefix(&mut self, base: &Path) {
        self.strip_prefix(base)
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
    fn parse(content: &str, file: &std::path::Path) -> Self {
        SolData::parse(content, file)
    }

    fn version_req(&self) -> Option<&semver::VersionReq> {
        self.version_req.as_ref()
    }

    fn resolve_imports<C>(&self, _paths: &crate::ProjectPathsConfig<C>) -> Result<Vec<PathBuf>> {
        return Ok(self.imports.iter().map(|i| i.data().path().to_path_buf()).collect_vec());
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

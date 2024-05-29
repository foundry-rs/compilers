use itertools::Itertools;

use super::{
    CompilationError, Compiler, CompilerInput, CompilerOutput, CompilerSettings, CompilerVersion,
    Language, ParsedSource,
};
use crate::{
    artifacts::{
        output_selection::OutputSelection, Error, Settings as SolcSettings, SolcInput, Sources,
    },
    error::Result,
    remappings::Remapping,
    resolver::parse::SolData,
    Solc, SOLC_EXTENSIONS,
};
use semver::Version;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    fmt,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone)]
#[cfg_attr(feature = "svm-solc", derive(Default))]
pub enum SolcCompiler {
    #[default]
    #[cfg(feature = "svm-solc")]
    AutoDetect,

    Specific(Solc),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum SolcLanguage {
    Solidity,
    Yul,
}

impl Language for SolcLanguage {
    const FILE_EXTENSIONS: &'static [&'static str] = SOLC_EXTENSIONS;
}

impl fmt::Display for SolcLanguage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Solidity => write!(f, "Solidity"),
            Self::Yul => write!(f, "Yul"),
        }
    }
}

impl Compiler for SolcCompiler {
    type Input = SolcVersionedInput;
    type CompilationError = crate::artifacts::Error;
    type ParsedSource = SolData;
    type Settings = SolcSettings;
    type Language = SolcLanguage;

    fn compile(&self, input: &Self::Input) -> Result<CompilerOutput<Self::CompilationError>> {
        let mut solc = match self {
            Self::Specific(solc) => solc.clone(),

            #[cfg(feature = "svm-solc")]
            Self::AutoDetect => Solc::find_or_install(&input.version)?,
        };
        solc.base_path = input.base_path.clone();
        solc.allow_paths = input.allow_paths.clone();
        solc.include_paths = input.include_paths.clone();

        let solc_output = solc.compile(&input.input)?;

        let output = CompilerOutput {
            errors: solc_output.errors,
            contracts: solc_output.contracts,
            sources: solc_output.sources,
        };

        Ok(output)
    }

    fn available_versions(&self, _language: &Self::Language) -> Vec<CompilerVersion> {
        match self {
            Self::Specific(solc) => vec![CompilerVersion::Installed(Version::new(
                solc.version.major,
                solc.version.minor,
                solc.version.patch,
            ))],

            #[cfg(feature = "svm-solc")]
            Self::AutoDetect => {
                let mut all_versions = Solc::installed_versions()
                    .into_iter()
                    .map(CompilerVersion::Installed)
                    .collect::<Vec<_>>();
                let mut uniques = all_versions
                    .iter()
                    .map(|v| {
                        let v = v.as_ref();
                        (v.major, v.minor, v.patch)
                    })
                    .collect::<std::collections::HashSet<_>>();
                all_versions.extend(
                    Solc::released_versions()
                        .into_iter()
                        .filter(|v| uniques.insert((v.major, v.minor, v.patch)))
                        .map(CompilerVersion::Remote),
                );
                all_versions.sort_unstable();
                all_versions
            }
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SolcVersionedInput {
    #[serde(skip)]
    pub version: Version,
    #[serde(flatten)]
    pub input: SolcInput,
    #[serde(skip)]
    pub allow_paths: BTreeSet<PathBuf>,
    #[serde(skip)]
    pub base_path: Option<PathBuf>,
    #[serde(skip)]
    pub include_paths: BTreeSet<PathBuf>,
}

impl CompilerInput for SolcVersionedInput {
    type Settings = SolcSettings;
    type Language = SolcLanguage;

    /// Creates a new [CompilerInput]s with default settings and the given sources
    ///
    /// A [CompilerInput] expects a language setting, supported by solc are solidity or yul.
    /// In case the `sources` is a mix of solidity and yul files, 2 CompilerInputs are returned
    fn build(
        sources: Sources,
        settings: Self::Settings,
        language: Self::Language,
        version: Version,
    ) -> Self {
        let input = SolcInput::new(language, sources, settings).sanitized(&version);

        Self {
            version,
            input,
            base_path: None,
            include_paths: Default::default(),
            allow_paths: Default::default(),
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

    fn with_allow_paths(mut self, allowed_paths: BTreeSet<PathBuf>) -> Self {
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

impl CompilerSettings for SolcSettings {
    fn update_output_selection(&mut self, f: impl FnOnce(&mut OutputSelection) + Copy) {
        f(&mut self.output_selection)
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
    type Language = SolcLanguage;

    fn parse(content: &str, file: &std::path::Path) -> Result<Self> {
        Ok(SolData::parse(content, file))
    }

    fn version_req(&self) -> Option<&semver::VersionReq> {
        self.version_req.as_ref()
    }

    fn resolve_imports<C>(&self, _paths: &crate::ProjectPathsConfig<C>) -> Result<Vec<PathBuf>> {
        return Ok(self.imports.iter().map(|i| i.data().path().to_path_buf()).collect_vec());
    }

    fn language(&self) -> Self::Language {
        if self.is_yul {
            SolcLanguage::Yul
        } else {
            SolcLanguage::Solidity
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

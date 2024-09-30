use super::{
    CompilationError, Compiler, CompilerInput, CompilerOutput, CompilerSettings, CompilerVersion,
    Language, ParsedSource,
};
use crate::resolver::parse::SolData;
pub use foundry_compilers_artifacts::SolcLanguage;
use foundry_compilers_artifacts::{
    error::SourceLocation,
    output_selection::OutputSelection,
    remappings::Remapping,
    sources::{Source, Sources},
    Error, Settings, Severity, SolcInput,
};
use foundry_compilers_core::error::Result;
use itertools::Itertools;
use semver::Version;
use serde::{Deserialize, Serialize};
use std::{
    borrow::Cow,
    collections::BTreeSet,
    ops::{Deref, DerefMut},
    path::{Path, PathBuf},
};
mod compiler;
pub use compiler::{Solc, SOLC_EXTENSIONS};

#[derive(Clone, Debug)]
#[cfg_attr(feature = "svm-solc", derive(Default))]
pub enum SolcCompiler {
    #[default]
    #[cfg(feature = "svm-solc")]
    AutoDetect,

    Specific(Solc),
}

impl Language for SolcLanguage {
    const FILE_EXTENSIONS: &'static [&'static str] = SOLC_EXTENSIONS;
}

impl Compiler for SolcCompiler {
    type Input = SolcVersionedInput;
    type CompilationError = Error;
    type ParsedSource = SolData;
    type Settings = SolcSettings;
    type Language = SolcLanguage;

    fn compile(&self, input: &Self::Input) -> Result<CompilerOutput<Self::CompilationError>> {
        let mut solc = match self {
            Self::Specific(solc) => solc.clone(),

            #[cfg(feature = "svm-solc")]
            Self::AutoDetect => Solc::find_or_install(&input.version)?,
        };
        solc.base_path.clone_from(&input.cli_settings.base_path);
        solc.allow_paths.clone_from(&input.cli_settings.allow_paths);
        solc.include_paths.clone_from(&input.cli_settings.include_paths);
        solc.extra_args.clone_from(&input.cli_settings.extra_args);

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

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SolcVersionedInput {
    pub version: Version,
    #[serde(flatten)]
    pub input: SolcInput,
    #[serde(flatten)]
    cli_settings: CliSettings,
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
        let SolcSettings { settings, cli_settings } = settings;
        let input = SolcInput::new(language, sources, settings).sanitized(&version);

        Self { version, input, cli_settings }
    }

    fn language(&self) -> Self::Language {
        self.input.language
    }

    fn version(&self) -> &Version {
        &self.version
    }

    fn sources(&self) -> impl Iterator<Item = (&Path, &Source)> {
        self.input.sources.iter().map(|(path, source)| (path.as_path(), source))
    }

    fn compiler_name(&self) -> Cow<'static, str> {
        "Solc".into()
    }

    fn strip_prefix(&mut self, base: &Path) {
        self.input.strip_prefix(base);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CliSettings {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extra_args: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub allow_paths: BTreeSet<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub include_paths: BTreeSet<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct SolcSettings {
    /// JSON settings expected by Solc
    #[serde(flatten)]
    pub settings: Settings,
    /// Additional CLI args configuration
    #[serde(flatten)]
    pub cli_settings: CliSettings,
}

impl Deref for SolcSettings {
    type Target = Settings;

    fn deref(&self) -> &Self::Target {
        &self.settings
    }
}

impl DerefMut for SolcSettings {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.settings
    }
}

impl CompilerSettings for SolcSettings {
    fn update_output_selection(&mut self, f: impl FnOnce(&mut OutputSelection) + Copy) {
        f(&mut self.settings.output_selection)
    }

    fn can_use_cached(&self, other: &Self) -> bool {
        let Self {
            settings:
                Settings {
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
                    eof_version,
                },
            ..
        } = self;

        *stop_after == other.settings.stop_after
            && *remappings == other.settings.remappings
            && *optimizer == other.settings.optimizer
            && *model_checker == other.settings.model_checker
            && *metadata == other.settings.metadata
            && *evm_version == other.settings.evm_version
            && *via_ir == other.settings.via_ir
            && *debug == other.settings.debug
            && *libraries == other.settings.libraries
            && *eof_version == other.settings.eof_version
            && output_selection.is_subset_of(&other.settings.output_selection)
    }

    fn with_remappings(mut self, remappings: &[Remapping]) -> Self {
        self.settings.remappings = remappings.to_vec();

        self
    }

    fn with_allow_paths(mut self, allowed_paths: &BTreeSet<PathBuf>) -> Self {
        self.cli_settings.allow_paths.clone_from(allowed_paths);
        self
    }

    fn with_base_path(mut self, base_path: &Path) -> Self {
        self.cli_settings.base_path = Some(base_path.to_path_buf());
        self
    }

    fn with_include_paths(mut self, include_paths: &BTreeSet<PathBuf>) -> Self {
        self.cli_settings.include_paths.clone_from(include_paths);
        self
    }
}

impl ParsedSource for SolData {
    type Language = SolcLanguage;

    fn parse(content: &str, file: &std::path::Path) -> Result<Self> {
        Ok(Self::parse(content, file))
    }

    fn version_req(&self) -> Option<&semver::VersionReq> {
        self.version_req.as_ref()
    }

    fn resolve_imports<C>(
        &self,
        _paths: &crate::ProjectPathsConfig<C>,
        _include_paths: &mut BTreeSet<PathBuf>,
    ) -> Result<Vec<PathBuf>> {
        Ok(self.imports.iter().map(|i| i.data().path().to_path_buf()).collect_vec())
    }

    fn language(&self) -> Self::Language {
        if self.is_yul {
            SolcLanguage::Yul
        } else {
            SolcLanguage::Solidity
        }
    }

    fn compilation_dependencies<'a>(
        &self,
        imported_nodes: impl Iterator<Item = (&'a Path, &'a Self)>,
    ) -> impl Iterator<Item = &'a Path>
    where
        Self: 'a,
    {
        imported_nodes.filter_map(|(path, node)| (!node.libraries.is_empty()).then_some(path))
    }
}

impl CompilationError for Error {
    fn is_warning(&self) -> bool {
        self.severity.is_warning()
    }
    fn is_error(&self) -> bool {
        self.severity.is_error()
    }

    fn source_location(&self) -> Option<SourceLocation> {
        self.source_location.clone()
    }

    fn severity(&self) -> Severity {
        self.severity
    }

    fn error_code(&self) -> Option<u64> {
        self.error_code
    }
}

#[cfg(test)]
mod tests {
    use foundry_compilers_artifacts::{CompilerOutput, SolcLanguage};
    use semver::Version;

    use crate::{
        buildinfo::RawBuildInfo,
        compilers::{
            solc::{SolcCompiler, SolcVersionedInput},
            CompilerInput,
        },
        AggregatedCompilerOutput,
    };

    #[test]
    fn can_parse_declaration_error() {
        let s = r#"{
  "errors": [
    {
      "component": "general",
      "errorCode": "7576",
      "formattedMessage": "DeclarationError: Undeclared identifier. Did you mean \"revert\"?\n  --> /Users/src/utils/UpgradeProxy.sol:35:17:\n   |\n35 |                 refert(\"Transparent ERC1967 proxies do not have upgradeable implementations\");\n   |                 ^^^^^^\n\n",
      "message": "Undeclared identifier. Did you mean \"revert\"?",
      "severity": "error",
      "sourceLocation": {
        "end": 1623,
        "file": "/Users/src/utils/UpgradeProxy.sol",
        "start": 1617
      },
      "type": "DeclarationError"
    }
  ],
  "sources": { }
}"#;

        let out: CompilerOutput = serde_json::from_str(s).unwrap();
        assert_eq!(out.errors.len(), 1);

        let out_converted = crate::compilers::CompilerOutput {
            errors: out.errors,
            contracts: Default::default(),
            sources: Default::default(),
        };

        let v: Version = "0.8.12".parse().unwrap();
        let input = SolcVersionedInput::build(
            Default::default(),
            Default::default(),
            SolcLanguage::Solidity,
            v.clone(),
        );
        let build_info = RawBuildInfo::new(&input, &out_converted, true).unwrap();
        let mut aggregated = AggregatedCompilerOutput::<SolcCompiler>::default();
        aggregated.extend(v, build_info, out_converted);
        assert!(!aggregated.is_unchanged());
    }
}

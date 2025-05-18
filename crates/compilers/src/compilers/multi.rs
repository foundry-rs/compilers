use super::{
    resolc::{Resolc, ResolcVersionedInput},
    restrictions::CompilerSettingsRestrictions,
    solc::{SolcCompiler, SolcSettings, SolcVersionedInput, SOLC_EXTENSIONS},
    vyper::{
        input::VyperVersionedInput, parser::VyperParsedSource, Vyper, VyperLanguage,
        VYPER_EXTENSIONS,
    },
    CompilationError, Compiler, CompilerInput, CompilerOutput, CompilerSettings, CompilerVersion,
    Language, ParsedSource, SimpleCompilerName,
};
use crate::{
    artifacts::vyper::{VyperCompilationError, VyperSettings},
    resolver::parse::SolData,
    settings::VyperRestrictions,
    solc::SolcRestrictions,
};
use foundry_compilers_artifacts::{
    error::SourceLocation,
    output_selection::OutputSelection,
    remappings::Remapping,
    sources::{Source, Sources},
    Contract, Error, Severity, SolcLanguage,
};
use foundry_compilers_core::error::{Result, SolcError};
use semver::Version;
use serde::{Deserialize, Serialize};
use std::{
    borrow::Cow,
    collections::BTreeSet,
    fmt,
    path::{Path, PathBuf},
};

/// Compiler capable of compiling both Solidity and Vyper sources.
#[derive(Clone, Debug)]
pub struct MultiCompiler {
    pub solidity: SolidityCompiler,
    pub vyper: Option<Vyper>,
}

#[derive(Clone, Debug, Default)]
/// Compiler capable of compiling Solidity.
pub enum SolidityCompiler {
    Solc(SolcCompiler),
    Resolc(Resolc),
    #[default]
    MissingInstallation,
}

impl Default for MultiCompiler {
    fn default() -> Self {
        let vyper = Vyper::new("vyper").ok();

        #[cfg(feature = "svm-solc")]
        let solc = SolidityCompiler::Solc(SolcCompiler::AutoDetect);

        #[cfg(not(feature = "svm-solc"))]
        let solc = crate::solc::Solc::new("solc")
            .map(SolcCompiler::Specific)
            .ok()
            .map(SolidityCompiler::Solc)
            .unwrap_or_default();

        Self { vyper, solidity: solc }
    }
}

impl MultiCompiler {
    pub fn new(solc: Option<SolidityCompiler>, vyper_path: Option<PathBuf>) -> Result<Self> {
        let vyper = vyper_path.map(Vyper::new).transpose()?;
        Ok(Self { solidity: solc.unwrap_or_default(), vyper })
    }
}

/// Languages supported by the [MultiCompiler].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MultiCompilerLanguage {
    Solc(SolcLanguage),
    Vyper(VyperLanguage),
}

impl From<SolcLanguage> for MultiCompilerLanguage {
    fn from(language: SolcLanguage) -> Self {
        Self::Solc(language)
    }
}

impl From<VyperLanguage> for MultiCompilerLanguage {
    fn from(language: VyperLanguage) -> Self {
        Self::Vyper(language)
    }
}

impl Language for MultiCompilerLanguage {
    const FILE_EXTENSIONS: &'static [&'static str] = &["sol", "vy", "vyi", "yul"];
}

impl fmt::Display for MultiCompilerLanguage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Solc(lang) => lang.fmt(f),
            Self::Vyper(lang) => lang.fmt(f),
        }
    }
}

/// Source parser for the [MultiCompiler]. Recognizes Solc and Vyper sources.
#[derive(Clone, Debug)]
pub enum MultiCompilerParsedSource {
    Solc(SolData),
    Vyper(VyperParsedSource),
}

impl MultiCompilerParsedSource {
    fn solc(&self) -> Option<&SolData> {
        match self {
            Self::Solc(parsed) => Some(parsed),
            _ => None,
        }
    }

    fn vyper(&self) -> Option<&VyperParsedSource> {
        match self {
            Self::Vyper(parsed) => Some(parsed),
            _ => None,
        }
    }
}

/// Compilation error which may occur when compiling Solidity or Vyper sources.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum MultiCompilerError {
    Solc(Error),
    Vyper(VyperCompilationError),
}

impl fmt::Display for MultiCompilerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Solc(error) => error.fmt(f),
            Self::Vyper(error) => error.fmt(f),
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct MultiCompilerRestrictions {
    pub solc: SolcRestrictions,
    pub vyper: VyperRestrictions,
}

impl CompilerSettingsRestrictions for MultiCompilerRestrictions {
    fn merge(self, other: Self) -> Option<Self> {
        Some(Self { solc: self.solc.merge(other.solc)?, vyper: self.vyper.merge(other.vyper)? })
    }
}

/// Settings for the [MultiCompiler]. Includes settings for both Solc and Vyper compilers.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiCompilerSettings {
    pub solc: SolcSettings,
    pub vyper: VyperSettings,
}

impl CompilerSettings for MultiCompilerSettings {
    type Restrictions = MultiCompilerRestrictions;

    fn can_use_cached(&self, other: &Self) -> bool {
        self.solc.can_use_cached(&other.solc) && self.vyper.can_use_cached(&other.vyper)
    }

    fn update_output_selection(&mut self, f: impl FnOnce(&mut OutputSelection) + Copy) {
        self.solc.update_output_selection(f);
        self.vyper.update_output_selection(f);
    }

    fn with_allow_paths(self, allowed_paths: &BTreeSet<PathBuf>) -> Self {
        Self {
            solc: self.solc.with_allow_paths(allowed_paths),
            vyper: self.vyper.with_allow_paths(allowed_paths),
        }
    }

    fn with_base_path(self, base_path: &Path) -> Self {
        Self {
            solc: self.solc.with_base_path(base_path),
            vyper: self.vyper.with_base_path(base_path),
        }
    }

    fn with_include_paths(self, include_paths: &BTreeSet<PathBuf>) -> Self {
        Self {
            solc: self.solc.with_include_paths(include_paths),
            vyper: self.vyper.with_include_paths(include_paths),
        }
    }

    fn with_remappings(self, remappings: &[Remapping]) -> Self {
        Self {
            solc: self.solc.with_remappings(remappings),
            vyper: self.vyper.with_remappings(remappings),
        }
    }

    fn satisfies_restrictions(&self, restrictions: &Self::Restrictions) -> bool {
        self.solc.satisfies_restrictions(&restrictions.solc)
            && self.vyper.satisfies_restrictions(&restrictions.vyper)
    }
}

impl From<MultiCompilerSettings> for SolcSettings {
    fn from(settings: MultiCompilerSettings) -> Self {
        settings.solc
    }
}

impl From<MultiCompilerSettings> for VyperSettings {
    fn from(settings: MultiCompilerSettings) -> Self {
        settings.vyper
    }
}

/// Input for the [MultiCompiler]. Either Solc or Vyper input.
#[derive(Clone, Debug, Serialize)]
#[serde(untagged)]
pub enum MultiCompilerInput {
    Solc(SolcVersionedInput),
    Vyper(VyperVersionedInput),
}

impl CompilerInput for MultiCompilerInput {
    type Language = MultiCompilerLanguage;
    type Settings = MultiCompilerSettings;

    fn build(
        sources: Sources,
        settings: Self::Settings,
        language: Self::Language,
        version: Version,
    ) -> Self {
        match language {
            MultiCompilerLanguage::Solc(language) => {
                Self::Solc(SolcVersionedInput::build(sources, settings.solc, language, version))
            }
            MultiCompilerLanguage::Vyper(language) => {
                Self::Vyper(VyperVersionedInput::build(sources, settings.vyper, language, version))
            }
        }
    }

    fn language(&self) -> Self::Language {
        match self {
            Self::Solc(input) => MultiCompilerLanguage::Solc(input.language()),
            Self::Vyper(input) => MultiCompilerLanguage::Vyper(input.language()),
        }
    }

    fn strip_prefix(&mut self, base: &Path) {
        match self {
            Self::Solc(input) => input.strip_prefix(base),
            Self::Vyper(input) => input.strip_prefix(base),
        }
    }

    fn version(&self) -> &Version {
        match self {
            Self::Solc(input) => input.version(),
            Self::Vyper(input) => input.version(),
        }
    }

    fn sources(&self) -> impl Iterator<Item = (&Path, &Source)> {
        let ret: Box<dyn Iterator<Item = _>> = match self {
            Self::Solc(input) => Box::new(input.sources()),
            Self::Vyper(input) => Box::new(input.sources()),
        };

        ret
    }
}

impl Compiler for MultiCompiler {
    type Input = MultiCompilerInput;
    type CompilationError = MultiCompilerError;
    type ParsedSource = MultiCompilerParsedSource;
    type Settings = MultiCompilerSettings;
    type Language = MultiCompilerLanguage;
    type CompilerContract = Contract;

    fn compiler_version(&self, input: &Self::Input) -> Version {
        match input {
            MultiCompilerInput::Solc(sol) => match &self.solidity {
                SolidityCompiler::Solc(solc) => solc.compiler_version(sol),
                SolidityCompiler::Resolc(r) => {
                    let input = sol.clone();
                    let input = ResolcVersionedInput::build(
                        input.input.sources,
                        SolcSettings {
                            settings: input.input.settings,
                            cli_settings: input.cli_settings,
                            extra_settings: input.extra_settings,
                        },
                        input.input.language,
                        input.version,
                    );
                    r.compiler_version(&input)
                }
                SolidityCompiler::MissingInstallation => sol.version().clone(),
            },
            MultiCompilerInput::Vyper(v) => self
                .vyper
                .as_ref()
                .map(|vyper| vyper.compiler_version(v))
                .unwrap_or_else(|| v.version().clone()),
        }
    }

    fn compiler_name(&self, input: &Self::Input) -> Cow<'static, str> {
        match input {
            MultiCompilerInput::Solc(_) => match &self.solidity {
                SolidityCompiler::Solc(_) => SolcCompiler::compiler_name_default(),
                SolidityCompiler::Resolc(_) => Resolc::compiler_name_default(),
                SolidityCompiler::MissingInstallation => "No applicable compilers installed".into(),
            },
            MultiCompilerInput::Vyper(_) => Vyper::compiler_name_default(),
        }
    }

    fn compile(
        &self,
        input: &Self::Input,
    ) -> Result<CompilerOutput<Self::CompilationError, Self::CompilerContract>> {
        match input {
            MultiCompilerInput::Solc(input) => match &self.solidity {
                SolidityCompiler::Solc(solc_compiler) => Compiler::compile(solc_compiler, input)
                    .map(|res| res.map_err(MultiCompilerError::Solc)),
                SolidityCompiler::Resolc(resolc) => {
                    let input = input.clone();
                    let input = ResolcVersionedInput::build(
                        input.input.sources,
                        SolcSettings {
                            settings: input.input.settings,
                            cli_settings: input.cli_settings,
                            extra_settings: input.extra_settings,
                        },
                        input.input.language,
                        input.version,
                    );
                    Compiler::compile(resolc, &input)
                        .map(|res| res.map_err(MultiCompilerError::Solc))
                }
                SolidityCompiler::MissingInstallation => {
                    Err(SolcError::msg("No solidity compiler is available"))
                }
            },
            MultiCompilerInput::Vyper(input) => {
                if let Some(vyper) = &self.vyper {
                    Compiler::compile(vyper, input)
                        .map(|res| res.map_err(MultiCompilerError::Vyper))
                } else {
                    Err(SolcError::msg("vyper compiler is not available"))
                }
            }
        }
    }

    fn available_versions(&self, language: &Self::Language) -> Vec<CompilerVersion> {
        match language {
            MultiCompilerLanguage::Solc(language) => match &self.solidity {
                SolidityCompiler::Solc(solc_compiler) => solc_compiler.available_versions(language),
                SolidityCompiler::Resolc(resolc) => resolc.available_versions(language),
                SolidityCompiler::MissingInstallation => Default::default(),
            },
            MultiCompilerLanguage::Vyper(language) => {
                self.vyper.as_ref().map(|v| v.available_versions(language)).unwrap_or_default()
            }
        }
    }
}

impl ParsedSource for MultiCompilerParsedSource {
    type Language = MultiCompilerLanguage;

    fn parse(content: &str, file: &std::path::Path) -> Result<Self> {
        let Some(extension) = file.extension().and_then(|e| e.to_str()) else {
            return Err(SolcError::msg("failed to resolve file extension"));
        };

        if SOLC_EXTENSIONS.contains(&extension) {
            <SolData as ParsedSource>::parse(content, file).map(Self::Solc)
        } else if VYPER_EXTENSIONS.contains(&extension) {
            VyperParsedSource::parse(content, file).map(Self::Vyper)
        } else {
            Err(SolcError::msg("unexpected file extension"))
        }
    }

    fn version_req(&self) -> Option<&semver::VersionReq> {
        match self {
            Self::Solc(parsed) => parsed.version_req(),
            Self::Vyper(parsed) => parsed.version_req(),
        }
    }

    fn contract_names(&self) -> &[String] {
        match self {
            Self::Solc(parsed) => parsed.contract_names(),
            Self::Vyper(parsed) => parsed.contract_names(),
        }
    }

    fn language(&self) -> Self::Language {
        match self {
            Self::Solc(parsed) => MultiCompilerLanguage::Solc(parsed.language()),
            Self::Vyper(parsed) => MultiCompilerLanguage::Vyper(parsed.language()),
        }
    }

    fn resolve_imports<C>(
        &self,
        paths: &crate::ProjectPathsConfig<C>,
        include_paths: &mut BTreeSet<PathBuf>,
    ) -> Result<Vec<PathBuf>> {
        match self {
            Self::Solc(parsed) => parsed.resolve_imports(paths, include_paths),
            Self::Vyper(parsed) => parsed.resolve_imports(paths, include_paths),
        }
    }

    fn compilation_dependencies<'a>(
        &self,
        imported_nodes: impl Iterator<Item = (&'a Path, &'a Self)>,
    ) -> impl Iterator<Item = &'a Path>
    where
        Self: 'a,
    {
        match self {
            Self::Solc(parsed) => parsed
                .compilation_dependencies(
                    imported_nodes.filter_map(|(path, node)| node.solc().map(|node| (path, node))),
                )
                .collect::<Vec<_>>(),
            Self::Vyper(parsed) => parsed
                .compilation_dependencies(
                    imported_nodes.filter_map(|(path, node)| node.vyper().map(|node| (path, node))),
                )
                .collect::<Vec<_>>(),
        }
        .into_iter()
    }
}

impl CompilationError for MultiCompilerError {
    fn is_warning(&self) -> bool {
        match self {
            Self::Solc(error) => error.is_warning(),
            Self::Vyper(error) => error.is_warning(),
        }
    }
    fn is_error(&self) -> bool {
        match self {
            Self::Solc(error) => error.is_error(),
            Self::Vyper(error) => error.is_error(),
        }
    }

    fn source_location(&self) -> Option<SourceLocation> {
        match self {
            Self::Solc(error) => error.source_location(),
            Self::Vyper(error) => error.source_location(),
        }
    }

    fn severity(&self) -> Severity {
        match self {
            Self::Solc(error) => error.severity(),
            Self::Vyper(error) => error.severity(),
        }
    }

    fn error_code(&self) -> Option<u64> {
        match self {
            Self::Solc(error) => error.error_code(),
            Self::Vyper(error) => error.error_code(),
        }
    }
}

use super::{
    solc::{SolcCompiler, SolcLanguage, SolcVersionedInput},
    vyper::{
        error::VyperCompilationError, input::VyperVersionedInput, parser::VyperParsedSource, Vyper,
        VyperLanguage, VyperSettings,
    },
    CompilationError, Compiler, CompilerInput, CompilerOutput, CompilerSettings, CompilerVersion,
    Language, ParsedSource,
};
use crate::{
    artifacts::{output_selection::OutputSelection, Error, Settings as SolcSettings, Sources},
    error::{Result, SolcError},
    remappings::Remapping,
    resolver::parse::SolData,
};
use semver::Version;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    fmt,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone)]
pub struct MultiCompiler {
    pub solc: SolcCompiler,
    pub vyper: Option<Vyper>,
}

#[cfg(feature = "svm-solc")]
impl Default for MultiCompiler {
    fn default() -> Self {
        let vyper = Vyper::new("vyper").ok();

        Self { solc: SolcCompiler::default(), vyper }
    }
}

impl MultiCompiler {
    pub fn new(solc: SolcCompiler, vyper_path: Option<PathBuf>) -> Result<Self> {
        let vyper = vyper_path.map(Vyper::new).transpose()?;
        Ok(Self { solc, vyper })
    }
}

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
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

#[derive(Debug, Clone)]
pub enum MultiCompilerParsedSource {
    Solc(SolData),
    Vyper(VyperParsedSource),
}

#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum MultiCompilerError {
    Solc(Error),
    Vyper(VyperCompilationError),
}

impl Language for MultiCompilerLanguage {
    const FILE_EXTENSIONS: &'static [&'static str] = &["sol", "vy", "yul"];
}

impl fmt::Display for MultiCompilerLanguage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Solc(lang) => lang.fmt(f),
            Self::Vyper(lang) => lang.fmt(f),
        }
    }
}

#[derive(Default, Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct MultiCompilerSettings {
    pub solc: SolcSettings,
    pub vyper: VyperSettings,
}

impl CompilerSettings for MultiCompilerSettings {
    fn can_use_cached(&self, other: &Self) -> bool {
        self.solc.can_use_cached(&other.solc) && self.vyper.can_use_cached(&other.vyper)
    }

    fn update_output_selection(&mut self, f: impl FnOnce(&mut OutputSelection) + Copy) {
        f(&mut self.solc.output_selection);
        f(&mut self.vyper.output_selection);
    }
}

#[derive(Debug, Clone, Serialize)]
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

    fn compiler_name(&self) -> String {
        match self {
            Self::Solc(input) => input.compiler_name(),
            Self::Vyper(input) => input.compiler_name(),
        }
    }

    fn language(&self) -> Self::Language {
        match self {
            Self::Solc(input) => MultiCompilerLanguage::Solc(input.language()),
            Self::Vyper(input) => MultiCompilerLanguage::Vyper(input.language()),
        }
    }

    fn sources(&self) -> &Sources {
        match self {
            Self::Solc(input) => input.sources(),
            Self::Vyper(input) => input.sources(),
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

    fn with_allow_paths(self, allowed_paths: BTreeSet<PathBuf>) -> Self {
        match self {
            Self::Solc(input) => Self::Solc(input.with_allow_paths(allowed_paths)),
            Self::Vyper(input) => Self::Vyper(input.with_allow_paths(allowed_paths)),
        }
    }

    fn with_base_path(self, base_path: PathBuf) -> Self {
        match self {
            Self::Solc(input) => Self::Solc(input.with_base_path(base_path)),
            Self::Vyper(input) => Self::Vyper(input.with_base_path(base_path)),
        }
    }

    fn with_include_paths(self, include_paths: BTreeSet<PathBuf>) -> Self {
        match self {
            Self::Solc(input) => Self::Solc(input.with_include_paths(include_paths)),
            Self::Vyper(input) => Self::Vyper(input.with_include_paths(include_paths)),
        }
    }

    fn with_remappings(self, remappings: Vec<Remapping>) -> Self {
        match self {
            Self::Solc(input) => Self::Solc(input.with_remappings(remappings)),
            Self::Vyper(input) => Self::Vyper(input.with_remappings(remappings)),
        }
    }
}

impl Compiler for MultiCompiler {
    type Input = MultiCompilerInput;
    type CompilationError = MultiCompilerError;
    type ParsedSource = MultiCompilerParsedSource;
    type Settings = MultiCompilerSettings;
    type Language = MultiCompilerLanguage;

    fn compile(&self, input: &Self::Input) -> Result<CompilerOutput<Self::CompilationError>> {
        match input {
            MultiCompilerInput::Solc(input) => {
                self.solc.compile(input).map(|res| res.map_err(MultiCompilerError::Solc))
            }
            MultiCompilerInput::Vyper(input) => {
                if let Some(vyper) = &self.vyper {
                    vyper.compile(input).map(|res| res.map_err(MultiCompilerError::Vyper))
                } else {
                    Err(SolcError::msg("vyper compiler is not available"))
                }
            }
        }
    }

    fn available_versions(&self, language: &Self::Language) -> Vec<CompilerVersion> {
        match language {
            MultiCompilerLanguage::Solc(language) => self.solc.available_versions(language),
            MultiCompilerLanguage::Vyper(language) => {
                self.vyper.as_ref().map(|v| v.available_versions(language)).unwrap_or_default()
            }
        }
    }
}

impl ParsedSource for MultiCompilerParsedSource {
    type Language = MultiCompilerLanguage;

    fn parse(content: &str, file: &std::path::Path) -> Result<Self> {
        match file.extension().and_then(|e| e.to_str()) {
            Some("sol" | "yul") => <SolData as ParsedSource>::parse(content, file).map(Self::Solc),
            Some("vy") => VyperParsedSource::parse(content, file).map(Self::Vyper),
            _ => Err(SolcError::msg("unexpected file extension")),
        }
    }

    fn version_req(&self) -> Option<&semver::VersionReq> {
        match self {
            Self::Solc(parsed) => parsed.version_req(),
            Self::Vyper(parsed) => parsed.version_req(),
        }
    }

    fn resolve_imports<C>(&self, paths: &crate::ProjectPathsConfig<C>) -> Result<Vec<PathBuf>> {
        match self {
            Self::Solc(parsed) => parsed.resolve_imports(paths),
            Self::Vyper(parsed) => parsed.resolve_imports(paths),
        }
    }

    fn language(&self) -> Self::Language {
        match self {
            Self::Solc(parsed) => MultiCompilerLanguage::Solc(parsed.language()),
            Self::Vyper(parsed) => MultiCompilerLanguage::Vyper(parsed.language()),
        }
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

    fn source_location(&self) -> Option<crate::artifacts::error::SourceLocation> {
        match self {
            Self::Solc(error) => error.source_location(),
            Self::Vyper(error) => error.source_location(),
        }
    }

    fn severity(&self) -> crate::artifacts::error::Severity {
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

impl fmt::Display for MultiCompilerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Solc(error) => error.fmt(f),
            Self::Vyper(error) => error.fmt(f),
        }
    }
}
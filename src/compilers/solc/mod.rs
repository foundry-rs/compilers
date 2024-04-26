mod error;
pub use error::SolcError;

mod vm;
use itertools::Itertools;
pub use vm::SolcVersionManager;

use super::{
    CompilationError, Compiler, CompilerInput, CompilerOutput, CompilerSettings, ParsedSource,
};
use crate::{
    artifacts::{
        output_selection::OutputSelection, CompilerInput as SolcInput,
        CompilerOutput as SolcOutput, Error, Settings as SolcSettings, Sources,
    },
    resolver::parse::SolData,
    Solc,
};
use semver::Version;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
};

impl Compiler for Solc {
    type Input = SolcInput;
    type Error = SolcError;
    type CompilationError = crate::artifacts::Error;
    type ParsedSource = SolData;
    type Settings = SolcSettings;

    fn compile(
        &self,
        mut input: Self::Input,
        base_path: Option<PathBuf>,
        include_paths: BTreeSet<PathBuf>,
        allow_paths: BTreeSet<PathBuf>,
    ) -> Result<(Self::Input, CompilerOutput<Self::CompilationError>), Self::Error> {
        let mut cmd = self.configure_cmd(base_path.clone(), include_paths, allow_paths);

        if let Some(ref base_path) = base_path {
            // Strip prefix from all sources to ensure deterministic metadata.
            input.strip_prefix(base_path);
        }

        trace!(input=%serde_json::to_string(&input).unwrap_or_else(|e| e.to_string()));
        debug!(?cmd, "compiling");

        let mut child = cmd.spawn()?;
        debug!("spawned");

        let stdin = child.stdin.as_mut().unwrap();
        serde_json::to_writer(stdin, &input)?;
        debug!("wrote JSON input to stdin");

        let output = child.wait_with_output()?;
        debug!(%output.status, output.stderr = ?String::from_utf8_lossy(&output.stderr), "finished");

        if output.status.success() {
            // Only run UTF-8 validation once.
            let output = std::str::from_utf8(&output.stdout).map_err(|_| SolcError::InvalidUtf8)?;
            let mut solc_output: SolcOutput = serde_json::from_str(output)?;

            if let Some(ref base_path) = base_path {
                solc_output.join_all(base_path);
            }

            let output = CompilerOutput {
                errors: solc_output.errors,
                contracts: solc_output.contracts,
                sources: solc_output.sources,
            };

            Ok((input, output))
        } else {
            Err(SolcError::solc_output(Some(self.version.clone()), &output))
        }
    }

    fn version(&self) -> &Version {
        &self.version
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
                language: "Solidity".to_string(),
                sources: solidity_sources,
                settings: settings.clone(),
            });
        }
        if !yul_sources.is_empty() {
            res.push(Self { language: "Yul".to_string(), sources: yul_sources, settings });
        }
        res
    }

    fn sources(&self) -> &Sources {
        &self.sources
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

    fn resolve_imports(&self, _paths: &crate::ProjectPathsConfig) -> Vec<PathBuf> {
        return self.imports.iter().map(|i| i.data().path().to_path_buf()).collect_vec();
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

use super::{
    solc::SolcError, CompilationError, Compiler, CompilerError, CompilerInput, CompilerSettings,
    ParsedSource,
};
use crate::{
    artifacts::{output_selection::OutputSelection, serde_helpers, Error, Severity, Sources},
    compilers::CompilerOutput,
    resolver::parse::capture_outer_and_inner,
    utils, CompilerOutput as SolcOutput, EvmVersion, ProjectPathsConfig,
};
use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

#[derive(Debug, Serialize, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VyperSettings {
    #[serde(
        default,
        with = "serde_helpers::display_from_str_opt",
        skip_serializing_if = "Option::is_none"
    )]
    pub evm_version: Option<EvmVersion>,
    pub output_selection: OutputSelection,
}

impl CompilerSettings for VyperSettings {
    fn output_selection_mut(&mut self) -> &mut OutputSelection {
        &mut self.output_selection
    }

    fn can_use_cached(&self, other: &Self) -> bool {
        self.evm_version == other.evm_version
            && self.output_selection.is_subset_of(&other.output_selection)
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct VyperCompilationError {
    pub message: String,
    pub severity: Severity,
}

impl CompilationError for VyperCompilationError {
    fn is_warning(&self) -> bool {
        self.severity.is_warning()
    }

    fn is_error(&self) -> bool {
        self.severity.is_error()
    }

    fn source_location(&self) -> Option<crate::artifacts::error::SourceLocation> {
        None
    }

    fn severity(&self) -> Severity {
        self.severity
    }

    fn error_code(&self) -> Option<u64> {
        None
    }
}

#[derive(Debug, Clone)]
pub struct Vyper {
    pub path: PathBuf,
    pub version: Version,
}

#[derive(Debug)]
pub struct VyperParsedSource {
    version_req: Option<VersionReq>,
}

impl ParsedSource for VyperParsedSource {
    fn parse(content: &str, _file: &Path) -> Self {
        let version_req = capture_outer_and_inner(content, &utils::RE_VYPER_VERSION, &["version"])
            .first()
            .and_then(|(cap, _)| VersionReq::parse(cap.as_str()).ok());
        VyperParsedSource { version_req }
    }

    fn version_req(&self) -> Option<&VersionReq> {
        self.version_req.as_ref()
    }

    fn resolve_imports(&self, _paths: &ProjectPathsConfig) -> Vec<PathBuf> {
        vec![]
    }
}

#[derive(Debug, thiserror::Error)]
pub enum VyperError {}

impl CompilerError for VyperError {
    fn compiler_version(&self) -> Option<&Version> {
        None
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VyperInput {
    pub language: String,
    pub sources: Sources,
    pub settings: VyperSettings,
}

pub struct VyperOutput {
    errors: Vec<VyperCompilationError>,
}

impl CompilerInput for VyperInput {
    type Settings = VyperSettings;

    fn build(sources: Sources, settings: Self::Settings, _version: &Version) -> Vec<Self> {
        vec![VyperInput { language: "Vyper".to_string(), sources, settings }]
    }

    fn sources(&self) -> &Sources {
        &self.sources
    }
}

impl Compiler for Vyper {
    type Settings = VyperSettings;
    type CompilationError = Error;
    type ParsedSource = VyperParsedSource;
    type Error = SolcError;
    type Input = VyperInput;

    fn compile(
        &self,
        input: Self::Input,
    ) -> Result<(Self::Input, super::CompilerOutput<Self::CompilationError>), Self::Error> {
        let mut cmd = Command::new(&self.path);
        cmd.stdin(Stdio::piped()).stderr(Stdio::piped()).stdout(Stdio::piped());

        cmd.arg("--standard-json");

        let mut child = cmd.spawn()?;
        debug!("spawned");

        let stdin = child.stdin.as_mut().unwrap();
        serde_json::to_writer(stdin, &input)?;

        println!("{:?}", serde_json::to_string(&input));
        debug!("wrote JSON input to stdin");

        let output = child.wait_with_output()?;
        debug!(%output.status, output.stderr = ?String::from_utf8_lossy(&output.stderr), "finished");

        println!("{:?}", output);

        if output.status.success() {
            // Only run UTF-8 validation once.
            let output = std::str::from_utf8(&output.stdout).map_err(|_| SolcError::InvalidUtf8)?;
            let mut solc_output: SolcOutput = serde_json::from_str(output)?;

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

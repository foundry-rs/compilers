use crate::{
    error::{Result, SolcError},
    resolver::parse::SolData,
    solc::{Solc, SolcCompiler, SolcSettings},
    Compiler, CompilerVersion, SimpleCompilerName,
};
use foundry_compilers_artifacts::{resolc::ResolcCompilerOutput, Contract, Error, SolcLanguage};
use itertools::Itertools;
use semver::Version;
use serde::Serialize;
use std::{
    io,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    str::FromStr,
};

use super::{ResolcInput, ResolcVersionedInput};

#[derive(Clone, Debug)]
pub struct Resolc {
    pub resolc: PathBuf,
    pub resolc_version: Version,
    pub solc: SolcCompiler,
}

impl Compiler for Resolc {
    type CompilerContract = Contract;
    type Input = ResolcVersionedInput;
    type CompilationError = Error;
    type ParsedSource = SolData;
    type Settings = SolcSettings;
    type Language = SolcLanguage;

    fn compiler_name(&self, _input: &Self::Input) -> std::borrow::Cow<'static, str> {
        Self::compiler_name_default()
    }

    /// Instead of using specific sols version we are going to autodetect
    /// Installed versions
    fn available_versions(&self, language: &SolcLanguage) -> Vec<CompilerVersion> {
        self.solc
            .available_versions(language)
            .into_iter()
            .filter(|version| match version {
                CompilerVersion::Installed(version) | CompilerVersion::Remote(version) => {
                    version.minor >= 8 && version.patch <= 28
                }
            })
            .collect::<Vec<_>>()
    }

    fn compile(
        &self,
        input: &Self::Input,
    ) -> Result<crate::compilers::CompilerOutput<Error, Self::CompilerContract>, SolcError> {
        let solc = self.solc(input)?;
        let results = self.compile_output::<ResolcInput>(&solc, &input.input)?;
        let output = std::str::from_utf8(&results).map_err(|_| SolcError::InvalidUtf8)?;
        let results: ResolcCompilerOutput =
            serde_json::from_str(output).map_err(|e| SolcError::msg(e.to_string()))?;
        Ok(results.into())
    }
}

impl SimpleCompilerName for Resolc {
    fn compiler_name_default() -> std::borrow::Cow<'static, str> {
        "resolc and solc".into()
    }
}

impl Resolc {
    pub fn new(resolc_path: impl Into<PathBuf>, solc_compiler: SolcCompiler) -> Result<Self> {
        let resolc_path = resolc_path.into();
        let resolc_version = Self::get_version_for_path(&resolc_path)?;
        Ok(Self { resolc_version, resolc: resolc_path, solc: solc_compiler })
    }

    fn solc(&self, _input: &ResolcVersionedInput) -> Result<Solc> {
        let solc = match &self.solc {
            SolcCompiler::Specific(solc) => solc.clone(),

            #[cfg(feature = "svm-solc")]
            SolcCompiler::AutoDetect => Solc::find_or_install(&_input.solc_version)?,
        };

        Ok(solc)
    }

    pub fn get_version_for_path(path: &Path) -> Result<Version> {
        let mut cmd = Command::new(path);
        cmd.arg("--version").stdin(Stdio::piped()).stderr(Stdio::piped()).stdout(Stdio::piped());
        debug!("Getting Resolc version");
        let output = cmd.output().map_err(map_io_err(path))?;
        trace!(?output);
        let version = version_from_output(output)?;
        debug!(%version);
        Ok(version)
    }

    #[instrument(name = "compile", level = "debug", skip_all)]
    pub fn compile_output<T: Serialize>(
        &self,
        solc: &Solc,
        input: &ResolcInput,
    ) -> Result<Vec<u8>> {
        let mut cmd = self.configure_cmd(solc);
        if !solc.allow_paths.is_empty() {
            cmd.arg("--allow-paths");
            cmd.arg(solc.allow_paths.iter().map(|p| p.display()).join(","));
        }
        if let Some(base_path) = &solc.base_path {
            for path in solc.include_paths.iter().filter(|p| p.as_path() != base_path.as_path()) {
                cmd.arg("--include-path").arg(path);
            }

            cmd.arg("--base-path").arg(base_path);
            cmd.current_dir(base_path);
        }

        let child = if matches!(&input.language, SolcLanguage::Solidity) {
            cmd.arg("--solc");
            cmd.arg(&solc.solc);
            cmd.arg("--standard-json");
            let mut child = cmd.spawn().map_err(map_io_err(&self.resolc))?;
            let mut stdin = io::BufWriter::new(child.stdin.take().unwrap());
            serde_json::to_writer(&mut stdin, &input)?;
            stdin.flush().map_err(map_io_err(&self.resolc))?;
            child
        } else {
            cmd.arg("--yul");
            cmd.arg(format!(
                "{}",
                &input
                    .sources
                    .first_key_value()
                    .map(|k| k.0.to_string_lossy())
                    .ok_or_else(|| SolcError::msg("No Yul sources available"))?
            ));
            cmd.arg("--bin");
            cmd.spawn().map_err(map_io_err(&self.resolc))?
        };

        debug!("Spawned");

        let output = child.wait_with_output().map_err(map_io_err(&self.resolc))?;
        debug!("Finished compiling with standard json with status {:?}", output.status);

        compile_output(output)
    }

    fn configure_cmd(&self, solc: &Solc) -> Command {
        let mut cmd = Command::new(&self.resolc);
        cmd.stdin(Stdio::piped()).stderr(Stdio::piped()).stdout(Stdio::piped());
        cmd.args(&solc.extra_args);
        cmd
    }
}

fn map_io_err(resolc_path: &Path) -> impl FnOnce(std::io::Error) -> SolcError + '_ {
    move |err| SolcError::io(err, resolc_path)
}

fn version_from_output(output: Output) -> Result<Version> {
    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let version = stdout
            .lines()
            .filter(|l| !l.trim().is_empty())
            .find(|l| l.contains("version"))
            .ok_or_else(|| SolcError::msg("Version not found in resolc output"))?;

        version
            .split_whitespace()
            .find(|s| s.starts_with("0.") || s.starts_with("v0."))
            .and_then(|s| {
                let trimmed = s.trim_start_matches('v').split('+').next().unwrap_or(s);
                Version::from_str(trimmed).ok()
            })
            .ok_or_else(|| SolcError::msg("Unable to retrieve version from resolc output"))
    } else {
        Err(SolcError::solc_output(&output))
    }
}

fn compile_output(output: Output) -> Result<Vec<u8>> {
    // @TODO: Handle YUL output
    if output.status.success() {
        Ok(output.stdout)
    } else {
        Err(SolcError::solc_output(&output))
    }
}

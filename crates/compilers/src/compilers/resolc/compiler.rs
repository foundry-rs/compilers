use crate::{
    error::{Result, SolcError},
    resolver::parse::SolData,
    solc::{Solc, SolcCompiler, SolcSettings},
    Compiler, CompilerVersion, SimpleCompilerName,
};
use foundry_compilers_artifacts::{resolc::ResolcCompilerOutput, Contract, Error, SolcLanguage};
use itertools::Itertools;
use rvm::Binary;
use semver::{Comparator, Prerelease, Version, VersionReq};
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
    pub supported_solc_versions: semver::VersionReq,
    pub solc: SolcCompiler,
}

impl Compiler for Resolc {
    type CompilerContract = Contract;
    type Input = ResolcVersionedInput;
    type CompilationError = Error;
    type ParsedSource = SolData;
    type Settings = SolcSettings;
    type Language = SolcLanguage;

    fn compiler_version(&self, _input: &Self::Input) -> Version {
        self.resolc_version.clone()
    }

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
                    self.supported_solc_versions.matches(version)
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
        "Resolc and Solc".into()
    }
}

impl Resolc {
    pub fn new(resolc_path: impl Into<PathBuf>, solc_compiler: SolcCompiler) -> Result<Self> {
        let resolc_path = resolc_path.into();
        let resolc_version = Self::get_version_for_path(&resolc_path)?;
        let supported_solc_versions = Self::supported_solc_versions(&resolc_path)?;
        Ok(Self {
            resolc_version,
            resolc: resolc_path,
            solc: solc_compiler,
            supported_solc_versions,
        })
    }

    pub fn find_installed(
        resolc_version: &Version,
        solc_compiler: SolcCompiler,
    ) -> Result<Option<Self>> {
        let solc_version = match &solc_compiler {
            SolcCompiler::Specific(solc) => Some(solc.version_short()),
            #[cfg(feature = "svm-solc")]
            SolcCompiler::AutoDetect => None,
        };

        let version_manager =
            rvm::VersionManager::new(true).map_err(|e| SolcError::Message(e.to_string()))?;
        let available = match version_manager.list_available(solc_version) {
            ok @ Ok(_) => ok,
            Err(rvm::Error::NoVersionsInstalled) => return Ok(None),
            err => err,
        }
        .map_err(|e| SolcError::Message(e.to_string()))?;

        available
            .iter()
            .filter(|x| x.version() == resolc_version)
            .filter_map(|x| x.local())
            .next_back()
            .map(|path| Self::new(path, solc_compiler))
            .transpose()
    }

    pub fn find_or_install(resolc_version: &Version, solc_compiler: SolcCompiler) -> Result<Self> {
        if let Some(resolc) = Self::find_installed(resolc_version, solc_compiler.clone())? {
            Ok(resolc)
        } else {
            Self::install(Some(resolc_version), solc_compiler)
        }
    }

    pub fn install(resolc_version: Option<&Version>, solc_compiler: SolcCompiler) -> Result<Self> {
        let solc_version = match &solc_compiler {
            SolcCompiler::Specific(solc) => Some(solc.version_short()),
            #[cfg(feature = "svm-solc")]
            SolcCompiler::AutoDetect => None,
        };
        let version_manager =
            rvm::VersionManager::new(false).map_err(|e| SolcError::Message(e.to_string()))?;

        let versions: Vec<Binary> = version_manager
            .list_available(solc_version.clone())
            .map_err(|e| SolcError::Message(e.to_string()))?
            .into_iter()
            .filter(|x| resolc_version.is_none_or(|version| version == x.version()))
            .collect();

        let binary = versions.into_iter().next_back().expect("Can't be empty");

        let binary_info = match binary {
            Binary::Remote(binary_info) => binary_info,
            Binary::Local { path, info } => {
                let supported_solc_versions = semver::VersionReq {
                    comparators: vec![
                        Comparator {
                            op: semver::Op::GreaterEq,
                            major: info.first_supported_solc_version.major,
                            minor: Some(info.first_supported_solc_version.minor),
                            patch: Some(info.first_supported_solc_version.patch),
                            pre: Prerelease::default(),
                        },
                        Comparator {
                            op: semver::Op::LessEq,
                            major: info.last_supported_solc_version.major,
                            minor: Some(info.last_supported_solc_version.minor),
                            patch: Some(info.last_supported_solc_version.patch),
                            pre: Prerelease::default(),
                        },
                    ],
                };
                return Ok(Self {
                    resolc_version: info.version,
                    resolc: path,
                    solc: solc_compiler,
                    supported_solc_versions,
                });
            }
        };

        let (path, resolc_version, supported_solc_versions) = {
            let (path, binary_info) = {
                let bin = version_manager
                    .get_or_install(&binary_info.version, solc_version)
                    .map_err(|e| SolcError::Message(e.to_string()))?;
                (bin.local().expect("should be installed").to_path_buf(), binary_info)
            };
            let supported_solc_versions = semver::VersionReq {
                comparators: vec![
                    Comparator {
                        op: semver::Op::GreaterEq,
                        major: binary_info.first_supported_solc_version.major,
                        minor: Some(binary_info.first_supported_solc_version.minor),
                        patch: Some(binary_info.first_supported_solc_version.patch),
                        pre: Prerelease::default(),
                    },
                    Comparator {
                        op: semver::Op::LessEq,
                        major: binary_info.last_supported_solc_version.major,
                        minor: Some(binary_info.last_supported_solc_version.minor),
                        patch: Some(binary_info.last_supported_solc_version.patch),
                        pre: Prerelease::default(),
                    },
                ],
            };

            (path, binary_info.version, supported_solc_versions)
        };

        Ok(Self { resolc_version, resolc: path, solc: solc_compiler, supported_solc_versions })
    }

    fn supported_solc_versions(path: &Path) -> Result<semver::VersionReq> {
        let mut cmd = Command::new(path);
        cmd.arg("--supported-solc-versions")
            .stdin(Stdio::piped())
            .stderr(Stdio::piped())
            .stdout(Stdio::piped());
        debug!("Getting Resolc supported `solc` versions");
        let output = cmd.output().map_err(map_io_err(path))?;
        trace!(?output);
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let version = VersionReq::parse(stdout.trim())?;
            debug!(%version);
            Ok(version)
        } else {
            Err(SolcError::Message(
                "`resolc` failed to get rang eof supported `solc` versions".to_owned(),
            ))
        }
    }

    pub(crate) fn solc(&self, _input: &ResolcVersionedInput) -> Result<Solc> {
        match &self.solc {
            SolcCompiler::Specific(solc) => Ok(solc.clone()),

            #[cfg(feature = "svm-solc")]
            SolcCompiler::AutoDetect => {
                if self.supported_solc_versions.matches(&_input.solc_version) {
                    Solc::find_or_install(&_input.solc_version)
                } else {
                    Err(SolcError::Message(format!(
                        "autodetected `solc` version v{} is not supported by `resolc` v{}. Set explicit `solc` version",
                        &_input.solc_version, self.resolc_version
                    )))
                }
            }
        }
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

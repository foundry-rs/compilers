use self::{
    error::VyperCompilationError,
    input::{VyperInput, VyperVersionedInput},
    parser::VyperParsedSource,
};
use super::{Compiler, CompilerOutput, Language};
use crate::{
    artifacts::Source,
    error::{Result, SolcError},
};
use core::fmt;
use semver::Version;
use serde::{de::DeserializeOwned, Serialize};
use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
    str::FromStr,
};

pub mod error;
pub mod input;
pub mod parser;
pub mod settings;
pub use settings::VyperSettings;

pub type VyperCompilerOutput = CompilerOutput<VyperCompilationError>;

/// File extensions that are recognized as Vyper source files.
pub const VYPER_EXTENSIONS: &[&str] = &["vy"];

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
#[non_exhaustive]
pub struct VyperLanguage;

impl Language for VyperLanguage {
    const FILE_EXTENSIONS: &'static [&'static str] = VYPER_EXTENSIONS;
}

impl fmt::Display for VyperLanguage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Vyper")
    }
}

#[derive(Debug, Clone)]
pub struct Vyper {
    pub path: PathBuf,
    pub version: Version,
}

impl Vyper {
    /// Creates a new instance of the Vyper compiler. Uses the `vyper` binary in the system `PATH`.
    pub fn new(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let version = Self::version(path)?;
        Ok(Self { path: path.into(), version })
    }

    /// Convenience function for compiling all sources under the given path
    pub fn compile_source(&self, path: impl AsRef<Path>) -> Result<VyperCompilerOutput> {
        let path = path.as_ref();
        let input =
            VyperInput::new(Source::read_all_from(path, VYPER_EXTENSIONS)?, Default::default());
        self.compile(&input)
    }

    /// Same as [`Self::compile()`], but only returns those files which are included in the
    /// `CompilerInput`.
    ///
    /// In other words, this removes those files from the `VyperCompilerOutput` that are __not__
    /// included in the provided `CompilerInput`.
    ///
    /// # Examples
    pub fn compile_exact(&self, input: &VyperInput) -> Result<VyperCompilerOutput> {
        let mut out = self.compile(input)?;
        out.retain_files(input.sources.keys().map(|p| p.as_path()));
        Ok(out)
    }

    /// Compiles with `--standard-json` and deserializes the output as [`VyperCompilerOutput`].
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use foundry_compilers::{CompilerInput, Solc};
    ///
    /// let solc = Solc::default();
    /// let input = CompilerInput::new("./contracts")?;
    /// let output = solc.compile(&input)?;
    /// # Ok::<_, Box<dyn std::error::Error>>(())
    /// ```
    pub fn compile<T: Serialize>(&self, input: &T) -> Result<VyperCompilerOutput> {
        self.compile_as(input)
    }

    /// Compiles with `--standard-json` and deserializes the output as the given `D`.
    pub fn compile_as<T: Serialize, D: DeserializeOwned>(&self, input: &T) -> Result<D> {
        let output = self.compile_output(input)?;

        // Only run UTF-8 validation once.
        let output = std::str::from_utf8(&output).map_err(|_| SolcError::InvalidUtf8)?;

        trace!("vyper compiler output: {}", output);

        Ok(serde_json::from_str(output)?)
    }

    /// Compiles with `--standard-json` and returns the raw `stdout` output.
    #[instrument(name = "compile", level = "debug", skip_all)]
    pub fn compile_output<T: Serialize>(&self, input: &T) -> Result<Vec<u8>> {
        let mut cmd = Command::new(&self.path);
        cmd.arg("--standard-json")
            .stdin(Stdio::piped())
            .stderr(Stdio::piped())
            .stdout(Stdio::piped());

        trace!(input=%serde_json::to_string(input).unwrap_or_else(|e| e.to_string()));
        debug!(?cmd, "compiling");

        let mut child = cmd.spawn().map_err(self.map_io_err())?;
        debug!("spawned");

        let stdin = child.stdin.as_mut().unwrap();
        serde_json::to_writer(stdin, input)?;
        debug!("wrote JSON input to stdin");

        let output = child.wait_with_output().map_err(self.map_io_err())?;
        debug!(%output.status, output.stderr = ?String::from_utf8_lossy(&output.stderr), "finished");

        if output.status.success() {
            Ok(output.stdout)
        } else {
            Err(SolcError::solc_output(&output))
        }
    }

    /// Invokes `vyper --version` and parses the output as a SemVer [`Version`].
    #[instrument(level = "debug", skip_all)]
    pub fn version(vyper: impl Into<PathBuf>) -> Result<Version> {
        let vyper = vyper.into();
        let mut cmd = Command::new(vyper.clone());
        cmd.arg("--version").stdin(Stdio::piped()).stderr(Stdio::piped()).stdout(Stdio::piped());
        debug!(?cmd, "getting Solc version");
        let output = cmd.output().map_err(|e| SolcError::io(e, vyper))?;
        trace!(?output);
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            Ok(Version::from_str(stdout.trim())?)
        } else {
            Err(SolcError::solc_output(&output))
        }
    }

    fn map_io_err(&self) -> impl FnOnce(std::io::Error) -> SolcError + '_ {
        move |err| SolcError::io(err, &self.path)
    }
}

impl Compiler for Vyper {
    type Settings = VyperSettings;
    type CompilationError = VyperCompilationError;
    type ParsedSource = VyperParsedSource;
    type Input = VyperVersionedInput;
    type Language = VyperLanguage;

    fn compile(&self, input: &Self::Input) -> Result<VyperCompilerOutput> {
        self.compile(input)
    }

    fn available_versions(&self, _language: &Self::Language) -> Vec<super::CompilerVersion> {
        vec![super::CompilerVersion::Installed(self.version.clone())]
    }
}

use self::input::{ZkSolcInput, ZkSolcVersionedInput};
use crate::error::{Result, SolcError};
use foundry_compilers_artifacts::zksolc::CompilerOutput;

use itertools::Itertools;
use semver::Version;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    str::FromStr,
};

pub mod input;
pub mod settings;
pub use settings::ZkSolcSettings;

#[cfg(feature = "async")]
use std::os::unix::prelude::PermissionsExt;
#[cfg(feature = "async")]
use tokio::{
    fs::{create_dir_all, set_permissions, File},
    io::copy,
};

pub const ZKSOLC: &str = "zksolc";
pub const ZKSYNC_SOLC_RELEASE: Version = Version::new(1, 0, 1);

#[derive(Debug, Clone, Serialize)]
enum ZkSolcOS {
    Linux,
    MacAMD,
    MacARM,
}

fn get_operating_system() -> Result<ZkSolcOS> {
    match std::env::consts::OS {
        "linux" => Ok(ZkSolcOS::Linux),
        "macos" | "darwin" => match std::env::consts::ARCH {
            "aarch64" => Ok(ZkSolcOS::MacARM),
            _ => Ok(ZkSolcOS::MacAMD),
        },
        _ => Err(SolcError::msg(format!("Unsupported operating system {}", std::env::consts::OS))),
    }
}

impl ZkSolcOS {
    fn get_compiler(&self) -> &str {
        match self {
            Self::Linux => "zksolc-linux-amd64-musl-",
            Self::MacAMD => "zksolc-macosx-amd64-",
            Self::MacARM => "zksolc-macosx-arm64-",
        }
    }

    fn get_solc_prefix(&self) -> &str {
        match self {
            Self::Linux => "solc-linux-amd64-",
            Self::MacAMD => "solc-macosx-amd64-",
            Self::MacARM => "solc-macosx-arm64-",
        }
    }

    #[cfg(feature = "async")]
    fn get_download_uri(&self) -> &str {
        match self {
            Self::Linux => "linux-amd64-musl",
            Self::MacAMD => "macosx-amd64",
            Self::MacARM => "macosx-arm64",
        }
    }
}

/// Abstraction over `zksolc` command line utility
///
/// Supports sync and async functions.
///
/// By default the zksolc path is configured as follows, with descending priority:
///   1. `ZKSOLC_PATH` environment variable
///   2. `zksolc` otherwise
#[derive(Debug, Clone, Eq, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ZkSolc {
    /// Path to the `zksolc` executable
    pub zksolc: PathBuf,
    /// Value for --base path
    pub base_path: Option<PathBuf>,
    /// Value for --allow-paths arg.
    pub allow_paths: BTreeSet<PathBuf>,
    /// Value for --include-paths arg.
    pub include_paths: BTreeSet<PathBuf>,
    /// Value for --solc arg
    pub solc: Option<PathBuf>,
}

impl Default for ZkSolc {
    fn default() -> Self {
        if let Ok(zksolc) = std::env::var("ZKSOLC_PATH") {
            return Self::new(zksolc);
        }

        Self::new(ZKSOLC)
    }
}

impl ZkSolc {
    /// A new instance which points to `zksolc`
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            zksolc: path.into(),
            base_path: None,
            allow_paths: Default::default(),
            include_paths: Default::default(),
            solc: None,
        }
    }

    /// Sets zksolc's base path
    pub fn with_base_path(mut self, base_path: impl Into<PathBuf>) -> Self {
        self.base_path = Some(base_path.into());
        self
    }

    /// Compiles with `--standard-json` and deserializes the output as [`CompilerOutput`].
    pub fn compile(&self, input: &mut ZkSolcVersionedInput) -> Result<CompilerOutput> {
        let mut zksolc = self.clone();
        // TODO: maybe we can just override the input
        if input.input.settings.solc.is_some() {
            zksolc.solc = input.input.settings.solc.clone();
        } else {
            let solc_version_without_metadata = format!(
                "{}.{}.{}",
                input.solc_version.major, input.solc_version.minor, input.solc_version.patch
            );
            let maybe_solc = Self::find_solc_installed_version(&solc_version_without_metadata)?;
            if let Some(solc) = maybe_solc {
                zksolc.solc = Some(solc);
            } else {
                // TODO: respect offline settings although it requires moving where we
                // check and get zksolc solc pathj
                #[cfg(feature = "async")]
                {
                    let installed_solc_path =
                        Self::solc_blocking_install(&solc_version_without_metadata)?;
                    zksolc.solc = Some(installed_solc_path);
                }
            }
        }

        zksolc.base_path.clone_from(&input.base_path);
        zksolc.allow_paths.clone_from(&input.allow_paths);
        zksolc.include_paths.clone_from(&input.include_paths);

        let (output, recompiled_with_dml) = zksolc.compile_output(&input.input)?;
        // TODO:  We set the input's with the detect missing libraries flag
        // if recompilation was attempted so cache is stored with the right
        // input that generated the output.
        // Maybe we should add this value as part of the output somehow and then
        // check for that when we evaluate writting the cache
        if recompiled_with_dml {
            input.input.settings.detect_missing_libraries = true;
        }

        // Only run UTF-8 validation once.
        let output = std::str::from_utf8(&output).map_err(|_| SolcError::InvalidUtf8)?;

        Ok(serde_json::from_str(output)?)
    }

    /// Compiles with `--standard-json` and returns the raw `stdout` output.
    #[instrument(name = "compile", level = "debug", skip_all)]
    pub fn compile_output(&self, input: &ZkSolcInput) -> Result<(Vec<u8>, bool)> {
        let mut cmd = Command::new(&self.zksolc);
        let mut recompiled_with_dml = false;

        if !self.allow_paths.is_empty() {
            cmd.arg("--allow-paths");
            cmd.arg(self.allow_paths.iter().map(|p| p.display()).join(","));
        }

        if let Some(base_path) = &self.base_path {
            for path in self.include_paths.iter().filter(|p| p.as_path() != base_path.as_path()) {
                cmd.arg("--include-path").arg(path);
            }

            cmd.arg("--base-path").arg(base_path);

            cmd.current_dir(base_path);
        }

        // don't pass solc argument in yul mode (avoid verification)
        if !input.is_yul() {
            if let Some(solc) = &self.solc {
                cmd.arg("--solc").arg(solc);
            }
        }

        cmd.arg("--standard-json");
        cmd.stdin(Stdio::piped()).stderr(Stdio::piped()).stdout(Stdio::piped());

        trace!(input=%serde_json::to_string(input).unwrap_or_else(|e| e.to_string()));
        debug!(?cmd, "compiling");

        let mut child = cmd.spawn().map_err(self.map_io_err())?;
        debug!("spawned");

        let stdin = child.stdin.as_mut().unwrap();
        serde_json::to_writer(stdin, input)?;
        debug!("wrote JSON input to stdin");

        let output = child.wait_with_output().map_err(self.map_io_err())?;
        debug!(%output.status, output.stderr = ?String::from_utf8_lossy(&output.stderr), "finished");

        let missing_libs_error: &[u8] = b"not found in the project".as_slice();

        let output = if !output.status.success()
            && output
                .stderr
                .windows(missing_libs_error.len())
                .any(|window| window == missing_libs_error)
        {
            trace!("Re-Running compiler with missing libraries detection");
            recompiled_with_dml = true;
            cmd.arg("--detect-missing-libraries");
            let mut child = cmd.spawn().map_err(self.map_io_err())?;
            debug!("spawned");
            let stdin = child.stdin.as_mut().unwrap();
            serde_json::to_writer(stdin, input)?;
            debug!("wrote JSON input to stdin");

            debug!(%output.status, output.stderr = ?String::from_utf8_lossy(&output.stderr), "finished");
            child.wait_with_output().map_err(self.map_io_err())?
        } else {
            output
        };

        compile_output(output, recompiled_with_dml)
    }

    /// Invokes `zksolc --version` and parses the output as a SemVer [`Version`], stripping the
    /// pre-release and build metadata.
    pub fn version_short(&self) -> Result<Version> {
        let version = self.version()?;
        Ok(Version::new(version.major, version.minor, version.patch))
    }

    /// Invokes `zksolc --version` and parses the output as a SemVer [`Version`].
    #[instrument(level = "debug", skip_all)]
    pub fn version(&self) -> Result<Version> {
        let mut cmd = Command::new(&self.zksolc);
        cmd.arg("--version").stdin(Stdio::piped()).stderr(Stdio::piped()).stdout(Stdio::piped());
        debug!(?cmd, "getting ZkSolc version");
        let output = cmd.output().map_err(self.map_io_err())?;
        trace!(?output);
        let version = version_from_output(output)?;
        debug!(%version);
        Ok(version)
    }

    fn map_io_err(&self) -> impl FnOnce(std::io::Error) -> SolcError + '_ {
        move |err| SolcError::io(err, &self.zksolc)
    }

    fn compilers_dir() -> Result<PathBuf> {
        let mut compilers_dir = dirs::home_dir()
            .ok_or(SolcError::msg("Could not build SolcManager - homedir not found"))?;
        compilers_dir.push(".zksync");
        Ok(compilers_dir)
    }

    fn compiler_path(version: &Version) -> Result<PathBuf> {
        let os = get_operating_system()?;
        Ok(Self::compilers_dir()?.join(format!("{}v{}", os.get_compiler(), version)))
    }

    fn solc_path(version_str: &str) -> Result<PathBuf> {
        let os = get_operating_system()?;
        Ok(Self::compilers_dir()?.join(format!(
            "{}{}-{}",
            os.get_solc_prefix(),
            version_str,
            ZKSYNC_SOLC_RELEASE
        )))
    }

    /// Install zksolc version and block the thread
    // TODO: Maybe this (and the whole module) goes behind a zksync feature installed
    #[cfg(feature = "async")]
    pub fn blocking_install(version: &Version) -> Result<Self> {
        use crate::utils::RuntimeOrHandle;

        trace!("blocking installing solc version \"{}\"", version);
        // TODO: Evaluate report support
        //crate::report::solc_installation_start(version);
        // An async block is used because the underlying `reqwest::blocking::Client` does not behave
        // well inside of a Tokio runtime. See: https://github.com/seanmonstar/reqwest/issues/1017
        let install = RuntimeOrHandle::new().block_on(async {
            let os = get_operating_system()?;
            let download_uri = os.get_download_uri();
            let full_download_url = format!(
                "https://github.com/matter-labs/zksolc-bin/releases/download/v{version}/zksolc-{download_uri}-v{version}",
            );

            let compiler_path = Self::compiler_path(version)?;

            let client = reqwest::Client::new();
            let response = client
                .get(full_download_url)
                .send()
                .await
                .map_err(|e| SolcError::msg(format!("Failed to download file: {e}")))?;

            if response.status().is_success() {
                let compilers_dir = Self::compilers_dir()?;
                if !compilers_dir.exists() {
                    create_dir_all(compilers_dir).await.map_err(|e| {
                        SolcError::msg(format!("Could not create compilers path: {e}"))
                    })?;
                }
                let mut output_file = File::create(&compiler_path)
                    .await
                    .map_err(|e| SolcError::msg(format!("Failed to create output file: {e}")))?;

                let content = response
                    .bytes()
                    .await
                    .map_err(|e| SolcError::msg(format!("failed to download file: {e}")))?;

                copy(&mut content.as_ref(), &mut output_file).await.map_err(|e| {
                    SolcError::msg(format!("Failed to write the downloaded file: {e}"))
                })?;

                set_permissions(&compiler_path, PermissionsExt::from_mode(0o755)).await.map_err(
                    |e| SolcError::msg(format!("Failed to set zksync compiler permissions: {e}")),
                )?;
            } else {
                return Err(SolcError::msg(format!(
                    "Failed to download file: status code {}",
                    response.status()
                )));
            }
            Ok(compiler_path)
        });

        match install {
            Ok(path) => {
                //crate::report::solc_installation_success(version);
                Ok(Self::new(path))
            }
            Err(err) => {
                //crate::report::solc_installation_error(version, &err.to_string());
                Err(err)
            }
        }
    }

    #[cfg(feature = "async")]
    pub fn solc_blocking_install(version_str: &str) -> Result<PathBuf> {
        use crate::utils::RuntimeOrHandle;

        trace!("blocking installing solc version \"{}\"", version_str);
        // TODO: Evaluate report support
        //crate::report::solc_installation_start(version);
        // An async block is used because the underlying `reqwest::blocking::Client` does not behave
        // well inside of a Tokio runtime. See: https://github.com/seanmonstar/reqwest/issues/1017
        RuntimeOrHandle::new().block_on(async {
            let os = get_operating_system()?;
            let solc_prefix = os.get_solc_prefix();
            let full_download_url = format!(
                "https://github.com/matter-labs/era-solidity/releases/download/{version_str}-{ZKSYNC_SOLC_RELEASE}/{solc_prefix}{version_str}-{ZKSYNC_SOLC_RELEASE}",
            );

            let solc_path = Self::solc_path(version_str)?;

            let client = reqwest::Client::new();
            let response = client
                .get(full_download_url)
                .send()
                .await
                .map_err(|e| SolcError::msg(format!("Failed to download file: {e}")))?;

            if response.status().is_success() {
                let compilers_dir = Self::compilers_dir()?;
                if !compilers_dir.exists() {
                    create_dir_all(compilers_dir).await.map_err(|e| {
                        SolcError::msg(format!("Could not create compilers path: {e}"))
                    })?;
                }
                let mut output_file = File::create(&solc_path)
                    .await
                    .map_err(|e| SolcError::msg(format!("Failed to create output file: {e}")))?;

                let content = response
                    .bytes()
                    .await
                    .map_err(|e| SolcError::msg(format!("failed to download file: {e}")))?;

                copy(&mut content.as_ref(), &mut output_file).await.map_err(|e| {
                    SolcError::msg(format!("Failed to write the downloaded file: {e}"))
                })?;

                set_permissions(&solc_path, PermissionsExt::from_mode(0o755)).await.map_err(
                    |e| SolcError::msg(format!("Failed to set zksync compiler permissions: {e}")),
                )?;
            } else {
                return Err(SolcError::msg(format!(
                    "Failed to download file: status code {}",
                    response.status()
                )));
            }
            Ok(solc_path)
        })
    }

    pub fn find_installed_version(version: &Version) -> Result<Option<Self>> {
        let zksolc = Self::compiler_path(version)?;

        if !zksolc.is_file() {
            return Ok(None);
        }
        Ok(Some(Self::new(zksolc)))
    }

    pub fn find_solc_installed_version(version_str: &str) -> Result<Option<PathBuf>> {
        let solc = Self::solc_path(version_str)?;

        if !solc.is_file() {
            return Ok(None);
        }
        Ok(Some(solc))
    }
}

fn compile_output(output: Output, recompiled_with_dml: bool) -> Result<(Vec<u8>, bool)> {
    if output.status.success() {
        Ok((output.stdout, recompiled_with_dml))
    } else {
        Err(SolcError::solc_output(&output))
    }
}

fn version_from_output(output: Output) -> Result<Version> {
    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let version = stdout
            .lines()
            .filter(|l| !l.trim().is_empty())
            .last()
            .ok_or_else(|| SolcError::msg("Version not found in zksolc output"))?;

        version
            .split_whitespace()
            .find_map(|s| {
                let trimmed = s.trim_start_matches('v');
                Version::from_str(trimmed).ok()
            })
            .ok_or_else(|| SolcError::msg("Unable to retrieve version from zksolc output"))
    } else {
        Err(SolcError::solc_output(&output))
    }
}

impl AsRef<Path> for ZkSolc {
    fn as_ref(&self) -> &Path {
        &self.zksolc
    }
}

impl<T: Into<PathBuf>> From<T> for ZkSolc {
    fn from(zksolc: T) -> Self {
        Self::new(zksolc.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn zksolc() -> ZkSolc {
        ZkSolc::default()
    }

    #[test]
    fn zksolc_version_works() {
        zksolc().version().unwrap();
    }

    /*
    #[test]
    fn zksolc_compile_works() {
        let input = include_str!("../../../../../test-data/zksync/in/compiler-in-1.json");
        let mut input: ZkSolcInput = serde_json::from_str(input).unwrap();
        let out = zksolc().compile(&mut input).unwrap();
        assert!(!out.has_error());
    }

    #[test]
    fn zksolc_can_compile_with_remapped_links() {
        let mut input: ZkSolcInput = serde_json::from_str(include_str!(
            "../../../../../test-data/zksync/library-remapping-in.json"
        ))
        .unwrap();
        let out = zksolc().compile(&mut input).unwrap();
        let (_, mut contracts) = out.split();
        let contract = contracts.remove("LinkTest").unwrap();
        let bytecode = &contract.get_bytecode().unwrap().object;
        assert!(!bytecode.is_unlinked());
    }

    #[test]
    fn zksolc_can_compile_with_remapped_links_temp_dir() {
        let mut input: ZkSolcInput = serde_json::from_str(include_str!(
            "../../../../../test-data/zksync/library-remapping-in-2.json"
        ))
        .unwrap();
        let out = zksolc().compile(&mut input).unwrap();
        let (_, mut contracts) = out.split();
        let contract = contracts.remove("LinkTest").unwrap();
        let bytecode = &contract.get_bytecode().unwrap().object;
        assert!(!bytecode.is_unlinked());
    }
    */
}

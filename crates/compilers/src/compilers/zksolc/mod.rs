use self::input::{ZkSolcInput, ZkSolcVersionedInput};
use crate::{
    error::{Result, SolcError},
    resolver::parse::SolData,
    solc::SolcCompiler,
    CompilationError, Compiler, CompilerVersion,
};
use foundry_compilers_artifacts::{
    solc::error::SourceLocation,
    zksolc::{error::Error, CompilerOutput},
    Severity, SolcLanguage,
};

use itertools::Itertools;
use semver::Version;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    str::FromStr,
};

#[cfg(feature = "async")]
use std::{
    fs::{self, create_dir_all, set_permissions, File},
    io::Write,
};

#[cfg(target_family = "unix")]
#[cfg(feature = "async")]
use std::os::unix::fs::PermissionsExt;

pub mod input;
pub mod settings;
pub use settings::ZkSettings;
pub use settings::ZkSolcSettings;

pub const ZKSOLC: &str = "zksolc";
pub const ZKSYNC_SOLC_RELEASE: Version = Version::new(1, 0, 1);
pub const ZKSOLC_VERSION: Version = Version::new(1, 5, 2);

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

    fn get_download_uri(&self) -> &str {
        match self {
            Self::Linux => "linux-amd64-musl",
            Self::MacAMD => "macosx-amd64",
            Self::MacARM => "macosx-arm64",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ZkSolcCompiler {
    pub zksolc: PathBuf,
    pub solc: SolcCompiler,
}

#[cfg(feature = "project-util")]
impl Default for ZkSolcCompiler {
    fn default() -> Self {
        let zksolc = ZkSolc::new_from_version(&ZKSOLC_VERSION).expect("Could not install zksolc");
        Self { zksolc: zksolc.zksolc, solc: Default::default() }
    }
}

impl Compiler for ZkSolcCompiler {
    type Input = ZkSolcVersionedInput;
    type CompilationError = Error;
    type ParsedSource = SolData;
    type Settings = ZkSolcSettings;
    type Language = SolcLanguage;

    fn compile(
        &self,
        _input: &Self::Input,
    ) -> Result<crate::compilers::CompilerOutput<Self::CompilationError>> {
        // This method cannot be implemented until CompilerOutput is decoupled from
        // evm Contract
        panic!(
            "`Compiler::create` not supported for `ZkSolcCompiler`, should call `zksync_compile`."
        );
    }

    // NOTE: This is used in the context of matching source files to compiler version so
    // the solc versions are returned
    fn available_versions(&self, _language: &Self::Language) -> Vec<CompilerVersion> {
        match &self.solc {
            SolcCompiler::Specific(solc) => vec![CompilerVersion::Installed(Version::new(
                solc.version.major,
                solc.version.minor,
                solc.version.patch,
            ))],
            SolcCompiler::AutoDetect => {
                let mut all_versions = ZkSolc::solc_installed_versions()
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
                    ZkSolc::solc_available_versions()
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

impl ZkSolcCompiler {
    pub fn zksync_compile(&self, input: &ZkSolcVersionedInput) -> Result<CompilerOutput> {
        let mut zksolc = ZkSolc::new(self.zksolc.clone());

        match &self.solc {
            SolcCompiler::Specific(solc) => zksolc.solc = Some(solc.solc.clone()),
            SolcCompiler::AutoDetect => {
                #[cfg(test)]
                crate::take_solc_installer_lock!(_lock);

                let solc_version_without_metadata = format!(
                    "{}.{}.{}",
                    input.solc_version.major, input.solc_version.minor, input.solc_version.patch
                );
                let maybe_solc =
                    ZkSolc::find_solc_installed_version(&solc_version_without_metadata)?;
                if let Some(solc) = maybe_solc {
                    zksolc.solc = Some(solc);
                } else {
                    #[cfg(feature = "async")]
                    {
                        let installed_solc_path =
                            ZkSolc::solc_blocking_install(&solc_version_without_metadata)?;
                        zksolc.solc = Some(installed_solc_path);
                    }
                }
            }
        }

        zksolc.base_path.clone_from(&input.cli_settings.base_path);
        zksolc.allow_paths.clone_from(&input.cli_settings.allow_paths);
        zksolc.include_paths.clone_from(&input.cli_settings.include_paths);

        zksolc.compile(&input.input)
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

    pub fn new_from_version(version: &Version) -> Result<Self> {
        let maybe_zksolc = Self::find_installed_version(version)?;

        if let Some(zksolc) = maybe_zksolc {
            Ok(zksolc)
        } else {
            Self::blocking_install(version)
        }
    }

    /// Sets zksolc's base path
    pub fn with_base_path(mut self, base_path: impl Into<PathBuf>) -> Self {
        self.base_path = Some(base_path.into());
        self
    }

    /// Compiles with `--standard-json` and deserializes the output as [`CompilerOutput`].
    pub fn compile(&self, input: &ZkSolcInput) -> Result<CompilerOutput> {
        let output = self.compile_output(input)?;
        // Only run UTF-8 validation once.
        let output = std::str::from_utf8(&output).map_err(|_| SolcError::InvalidUtf8)?;

        Ok(serde_json::from_str(output)?)
    }

    pub fn solc_installed_versions() -> Vec<Version> {
        if let Ok(dir) = Self::compilers_dir() {
            let os = get_operating_system().unwrap();
            let solc_prefix = os.get_solc_prefix();
            let mut versions: Vec<Version> = walkdir::WalkDir::new(dir)
                .max_depth(1)
                .into_iter()
                .filter_map(std::result::Result::ok)
                .filter(|e| e.file_type().is_file())
                .filter_map(|e| e.file_name().to_str().map(|s| s.to_string()))
                .filter(|e| e.ends_with(&ZKSYNC_SOLC_RELEASE.to_string()))
                .filter_map(|e| {
                    e.strip_prefix(solc_prefix)
                        .and_then(|s| s.split('-').next())
                        .and_then(|s| Version::parse(s).ok())
                })
                .collect();
            versions.sort();
            versions
        } else {
            vec![]
        }
    }

    pub fn solc_available_versions() -> Vec<Version> {
        let mut ret = vec![];
        let min_max_patch_by_minor_versions =
            vec![(4, 12, 26), (5, 0, 17), (6, 0, 12), (7, 0, 6), (8, 0, 26)];
        for (minor, min_patch, max_patch) in min_max_patch_by_minor_versions {
            for i in min_patch..=max_patch {
                ret.push(Version::new(0, minor, i));
            }
        }

        ret
    }

    /// Compiles with `--standard-json` and returns the raw `stdout` output.
    #[instrument(name = "compile", level = "debug", skip_all)]
    pub fn compile_output(&self, input: &ZkSolcInput) -> Result<Vec<u8>> {
        let mut cmd = Command::new(&self.zksolc);

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

        compile_output(output)
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
    #[cfg(feature = "async")]
    pub fn blocking_install(version: &Version) -> Result<Self> {
        let os = get_operating_system()?;
        let os_namespace = os.get_download_uri();
        let download_url = if version.pre.is_empty() {
            format!(
                "https://github.com/matter-labs/zksolc-bin/releases/download/v{version}/zksolc-{os_namespace}-v{version}",
            )
        } else {
            let pre = version.pre.as_str();
            // Use version as string without pre-release and build metadata
            let version_str = version.to_string();
            let version_str = version_str.split('-').next().unwrap();
            // Matter Labs uses a different repositiry for pre-releases
            format!(
                "https://github.com/matter-labs/era-compiler-solidity/releases/download/{pre}/zksolc-{os_namespace}-v{version_str}",
            )
        };
        let compilers_dir = Self::compilers_dir()?;
        if !compilers_dir.exists() {
            create_dir_all(compilers_dir)
                .map_err(|e| SolcError::msg(format!("Could not create compilers path: {e}")))?;
        }
        let compiler_path = Self::compiler_path(version)?;
        let lock_path = lock_file_path("zksolc", &version.to_string());

        let label = format!("zksolc-{version}");
        let install = compiler_blocking_install(compiler_path, lock_path, &download_url, &label);

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

    /// Install zksync solc version and block the thread
    #[cfg(feature = "async")]
    pub fn solc_blocking_install(version_str: &str) -> Result<PathBuf> {
        let os = get_operating_system()?;
        let solc_os_namespace = os.get_solc_prefix();
        let download_url = format!(
            "https://github.com/matter-labs/era-solidity/releases/download/{version_str}-{ZKSYNC_SOLC_RELEASE}/{solc_os_namespace}{version_str}-{ZKSYNC_SOLC_RELEASE}",
        );

        let compilers_dir = Self::compilers_dir()?;
        if !compilers_dir.exists() {
            create_dir_all(compilers_dir)
                .map_err(|e| SolcError::msg(format!("Could not create compilers path: {e}")))?;
        }
        let solc_path = Self::solc_path(version_str)?;
        let lock_path = lock_file_path("solc", version_str);

        let label = format!("solc-{version_str}");
        compiler_blocking_install(solc_path, lock_path, &download_url, &label)
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

fn compile_output(output: Output) -> Result<Vec<u8>> {
    if output.status.success() {
        Ok(output.stdout)
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

#[cfg(feature = "async")]
fn compiler_blocking_install(
    compiler_path: PathBuf,
    lock_path: PathBuf,
    download_url: &str,
    label: &str,
) -> Result<PathBuf> {
    use foundry_compilers_core::utils::RuntimeOrHandle;
    trace!("blocking installing {label}");
    //trace!("blocking installing {label}");
    // An async block is used because the underlying `reqwest::blocking::Client` does not behave
    // well inside of a Tokio runtime. See: https://github.com/seanmonstar/reqwest/issues/1017
    RuntimeOrHandle::new().block_on(async {
        let client = reqwest::Client::new();
        let response = client
            .get(download_url)
            .send()
            .await
            .map_err(|e| SolcError::msg(format!("Failed to download {label} file: {e}")))?;

        if response.status().is_success() {
            let content = response
                .bytes()
                .await
                .map_err(|e| SolcError::msg(format!("failed to download {label} file: {e}")))?;
            trace!("downloaded {label}");

            // lock file to indicate that installation of this compiler version will be in progress.
            // wait until lock file is released, possibly by another parallel thread trying to
            // install the same compiler version.
            trace!("try to get lock for {label}");
            let _lock = try_lock_file(lock_path)?;
            trace!("got lock for {label}");

            // Only write to file if it is not there. The check is doneafter adquiring the lock
            // to ensure the thread remains blocked until the required compiler is
            // fully installed
            if !compiler_path.exists() {
                trace!("creating binary for {label}");
                //trace!("creating binary for {label}");
                let mut output_file = File::create(&compiler_path).map_err(|e| {
                    SolcError::msg(format!("Failed to create output {label} file: {e}"))
                })?;

                output_file.write_all(&content).map_err(|e| {
                    SolcError::msg(format!("Failed to write the downloaded {label} file: {e}"))
                })?;

                set_permissions(&compiler_path, PermissionsExt::from_mode(0o755)).map_err(|e| {
                    SolcError::msg(format!("Failed to set {label} permissions: {e}"))
                })?;
            } else {
                trace!("found binary for {label}");
            }
        } else {
            return Err(SolcError::msg(format!(
                "Failed to download {label} file: status code {}",
                response.status()
            )));
        }
        trace!("{label} instalation completed");
        Ok(compiler_path)
    })
}

/// Creates the file and locks it exclusively, this will block if the file is currently locked
#[cfg(feature = "async")]
fn try_lock_file(lock_path: PathBuf) -> Result<LockFile> {
    use fs4::FileExt;
    let _lock_file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .read(true)
        .write(true)
        .open(&lock_path)
        .map_err(|_| SolcError::msg("Error creating lock file"))?;
    _lock_file.lock_exclusive().map_err(|_| SolcError::msg("Error taking the lock"))?;
    Ok(LockFile { lock_path, _lock_file })
}

/// Represents a lockfile that's removed once dropped
#[cfg(feature = "async")]
struct LockFile {
    _lock_file: File,
    lock_path: PathBuf,
}

#[cfg(feature = "async")]
impl Drop for LockFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.lock_path);
    }
}

/// Returns the lockfile to use for a specific file
#[cfg(feature = "async")]
fn lock_file_path(compiler: &str, version: &str) -> PathBuf {
    ZkSolc::compilers_dir()
        .expect("could not detect zksolc compilers directory")
        .join(format!(".lock-{compiler}-{version}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn zksolc() -> ZkSolc {
        let mut zksolc = ZkSolc::new_from_version(&ZKSOLC_VERSION).unwrap();
        let solc_version = "0.8.26";

        crate::take_solc_installer_lock!(_lock);
        let maybe_solc = ZkSolc::find_solc_installed_version(solc_version).unwrap();
        if let Some(solc) = maybe_solc {
            zksolc.solc = Some(solc);
        } else {
            {
                let installed_solc_path = ZkSolc::solc_blocking_install(solc_version).unwrap();
                zksolc.solc = Some(installed_solc_path);
            }
        }
        zksolc
    }

    #[test]
    fn zksolc_version_works() {
        zksolc().version().unwrap();
    }

    #[test]
    fn zksolc_compile_works() {
        let input = include_str!("../../../../../test-data/zksync/in/compiler-in-1.json");
        let input: ZkSolcInput = serde_json::from_str(input).unwrap();
        let out = zksolc().compile(&input).unwrap();
        assert!(!out.has_error());
    }

    #[test]
    fn zksolc_can_compile_with_remapped_links() {
        let input: ZkSolcInput = serde_json::from_str(include_str!(
            "../../../../../test-data/zksync/library-remapping-in.json"
        ))
        .unwrap();
        let out = zksolc().compile(&input).unwrap();
        let (_, mut contracts) = out.split();
        let contract = contracts.remove("LinkTest").unwrap();
        let bytecode = &contract.evm.unwrap().bytecode.unwrap().object;
        assert!(!bytecode.is_unlinked());
    }

    #[test]
    fn zksolc_can_compile_with_remapped_links_temp_dir() {
        let input: ZkSolcInput = serde_json::from_str(include_str!(
            "../../../../../test-data/zksync/library-remapping-in-2.json"
        ))
        .unwrap();
        let out = zksolc().compile(&input).unwrap();
        let (_, mut contracts) = out.split();
        let contract = contracts.remove("LinkTest").unwrap();
        let bytecode = &contract.evm.unwrap().bytecode.unwrap().object;
        assert!(!bytecode.is_unlinked());
    }
}

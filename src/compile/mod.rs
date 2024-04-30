use crate::{
    artifacts::Source,
    error::{Result, SolcError},
    resolver::parse::SolData,
    utils, CompilerInput, CompilerOutput,
};
use itertools::Itertools;
use once_cell::sync::Lazy;
use semver::{Version, VersionReq};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    fmt,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    str::FromStr,
};

pub mod many;

pub mod output;
pub use output::{contracts, info, sources};

pub mod project;

/// The name of the `solc` binary on the system
pub const SOLC: &str = "solc";

/// Support for configuring the EVM version
/// <https://blog.soliditylang.org/2018/03/08/solidity-0.4.21-release-announcement/>
pub const BYZANTIUM_SOLC: Version = Version::new(0, 4, 21);

/// Bug fix for configuring the EVM version with Constantinople
/// <https://blog.soliditylang.org/2018/03/08/solidity-0.4.21-release-announcement/>
pub const CONSTANTINOPLE_SOLC: Version = Version::new(0, 4, 22);

/// Petersburg support
/// <https://blog.soliditylang.org/2019/03/05/solidity-0.5.5-release-announcement/>
pub const PETERSBURG_SOLC: Version = Version::new(0, 5, 5);

/// Istanbul support
/// <https://blog.soliditylang.org/2019/12/09/solidity-0.5.14-release-announcement/>
pub const ISTANBUL_SOLC: Version = Version::new(0, 5, 14);

/// Berlin support
/// <https://blog.soliditylang.org/2021/06/10/solidity-0.8.5-release-announcement/>
pub const BERLIN_SOLC: Version = Version::new(0, 8, 5);

/// London support
/// <https://blog.soliditylang.org/2021/08/11/solidity-0.8.7-release-announcement/>
pub const LONDON_SOLC: Version = Version::new(0, 8, 7);

/// Paris support
/// <https://blog.soliditylang.org/2023/02/01/solidity-0.8.18-release-announcement/>
pub const PARIS_SOLC: Version = Version::new(0, 8, 18);

/// Shanghai support
/// <https://blog.soliditylang.org/2023/05/10/solidity-0.8.20-release-announcement/>
pub const SHANGHAI_SOLC: Version = Version::new(0, 8, 20);

/// Cancun support
/// <https://soliditylang.org/blog/2024/01/26/solidity-0.8.24-release-announcement/>
pub const CANCUN_SOLC: Version = Version::new(0, 8, 24);

// `--base-path` was introduced in 0.6.9 <https://github.com/ethereum/solidity/releases/tag/v0.6.9>
pub static SUPPORTS_BASE_PATH: Lazy<VersionReq> =
    Lazy::new(|| VersionReq::parse(">=0.6.9").unwrap());

// `--include-path` was introduced in 0.8.8 <https://github.com/ethereum/solidity/releases/tag/v0.8.8>
pub static SUPPORTS_INCLUDE_PATH: Lazy<VersionReq> =
    Lazy::new(|| VersionReq::parse(">=0.8.8").unwrap());

/// take the lock in tests, we use this to enforce that
/// a test does not run while a compiler version is being installed
///
/// This ensures that only one thread installs a missing `solc` exe.
/// Instead of taking this lock in `Solc::blocking_install`, the lock should be taken before
/// installation is detected.
#[cfg(feature = "svm-solc")]
#[cfg(test)]
pub(crate) fn take_solc_installer_lock() -> impl Drop {
    static LOCK: Lazy<std::sync::Mutex<()>> = Lazy::new(|| std::sync::Mutex::new(()));
    LOCK.lock().unwrap()
}

/// A list of upstream Solc releases, used to check which version
/// we should download.
/// The boolean value marks whether there was an error accessing the release list
pub static RELEASES: Lazy<(svm::Releases, Vec<Version>, bool)> =
    Lazy::new(|| match serde_json::from_str::<svm::Releases>(svm_builds::RELEASE_LIST_JSON) {
        Ok(releases) => {
            let sorted_versions = releases.clone().into_versions();
            (releases, sorted_versions, true)
        }
        Err(err) => {
            error!("{:?}", err);
            Default::default()
        }
    });

/// A `Solc` version is either installed (available locally) or can be downloaded, from the remote
/// endpoint
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SolcVersion {
    Installed(Version),
    Remote(Version),
}

impl SolcVersion {
    /// Whether this version is installed
    pub fn is_installed(&self) -> bool {
        matches!(self, SolcVersion::Installed(_))
    }
}

impl AsRef<Version> for SolcVersion {
    fn as_ref(&self) -> &Version {
        match self {
            SolcVersion::Installed(v) | SolcVersion::Remote(v) => v,
        }
    }
}

impl From<SolcVersion> for Version {
    fn from(s: SolcVersion) -> Version {
        match s {
            SolcVersion::Installed(v) | SolcVersion::Remote(v) => v,
        }
    }
}

impl fmt::Display for SolcVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_ref())
    }
}

/// Abstraction over `solc` command line utility
///
/// Supports sync and async functions.
///
/// By default the solc path is configured as follows, with descending priority:
///   1. `SOLC_PATH` environment variable
///   2. [svm](https://github.com/roynalnaruto/svm-rs)'s  `global_version` (set via `svm use
///      <version>`), stored at `<svm_home>/.global_version`
///   3. `solc` otherwise
#[derive(Debug, Clone, Eq, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Solc {
    /// Path to the `solc` executable
    pub solc: PathBuf,
    /// Compiler version.
    pub version: Version,
}

impl Solc {
    /// A new instance which points to `solc`. Invokes `solc --version` to determine the version.
    pub fn new(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let version = Self::version(path)?;
        Ok(Self::new_with_version(path, version))
    }

    /// A new instance which points to `solc` with the given version
    pub fn new_with_version(path: impl Into<PathBuf>, version: Version) -> Self {
        Solc { solc: path.into(), version }
    }

    /// Returns the directory in which [svm](https://github.com/roynalnaruto/svm-rs) stores all versions
    ///
    /// This will be:
    /// - `~/.svm` on unix, if it exists
    /// - $XDG_DATA_HOME (~/.local/share/svm) if the svm folder does not exist.
    pub fn svm_home() -> Option<PathBuf> {
        if let Some(home_dir) = home::home_dir() {
            let home_dot_svm = home_dir.join(".svm");
            if home_dot_svm.exists() {
                return Some(home_dot_svm);
            }
        }
        dirs::data_dir().map(|dir| dir.join("svm"))
    }

    /// Returns the `semver::Version` [svm](https://github.com/roynalnaruto/svm-rs)'s `.global_version` is currently set to.
    ///  `global_version` is configured with (`svm use <version>`)
    ///
    /// This will read the version string (eg: "0.8.9") that the  `~/.svm/.global_version` file
    /// contains
    pub fn svm_global_version() -> Option<Version> {
        let home = Self::svm_home()?;
        let version = std::fs::read_to_string(home.join(".global_version")).ok()?;
        Version::parse(&version).ok()
    }

    /// Returns the list of all solc instances installed at `SVM_HOME`
    pub fn installed_versions() -> Vec<SolcVersion> {
        Self::svm_home()
            .map(|home| {
                utils::installed_versions(home)
                    .unwrap_or_default()
                    .into_iter()
                    .map(SolcVersion::Installed)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Returns the list of all versions that are available to download and marking those which are
    /// already installed.
    #[cfg(feature = "svm-solc")]
    pub fn all_versions() -> Vec<SolcVersion> {
        let mut all_versions = Self::installed_versions();
        let mut uniques = all_versions
            .iter()
            .map(|v| {
                let v = v.as_ref();
                (v.major, v.minor, v.patch)
            })
            .collect::<std::collections::HashSet<_>>();
        all_versions.extend(
            RELEASES
                .1
                .clone()
                .into_iter()
                .filter(|v| uniques.insert((v.major, v.minor, v.patch)))
                .map(SolcVersion::Remote),
        );
        all_versions.sort_unstable();
        all_versions
    }

    /// Assuming the `versions` array is sorted, it returns the first element which satisfies
    /// the provided [`VersionReq`]
    pub fn find_matching_installation(
        versions: &[Version],
        required_version: &VersionReq,
    ) -> Option<Version> {
        // iterate in reverse to find the last match
        versions.iter().rev().find(|version| required_version.matches(version)).cloned()
    }

    /// Given a Solidity source, it detects the latest compiler version which can be used
    /// to build it, and returns it.
    ///
    /// If the required compiler version is not installed, it also proceeds to install it.
    #[cfg(feature = "svm-solc")]
    pub fn detect_version(source: &Source) -> Result<Version> {
        // detects the required solc version
        let sol_version = Self::source_version_req(source)?;
        Self::ensure_installed(&sol_version)
    }

    /// Given a Solidity version requirement, it detects the latest compiler version which can be
    /// used to build it, and returns it.
    ///
    /// If the required compiler version is not installed, it also proceeds to install it.
    #[cfg(feature = "svm-solc")]
    pub fn ensure_installed(sol_version: &VersionReq) -> Result<Version> {
        #[cfg(test)]
        let _lock = take_solc_installer_lock();

        // load the local / remote versions
        let versions = utils::installed_versions(svm::data_dir()).unwrap_or_default();

        let local_versions = Self::find_matching_installation(&versions, sol_version);
        let remote_versions = Self::find_matching_installation(&RELEASES.1, sol_version);
        // if there's a better upstream version than the one we have, install it
        Ok(match (local_versions, remote_versions) {
            (Some(local), None) => local,
            (Some(local), Some(remote)) => {
                if remote > local {
                    Self::blocking_install(&remote)?;
                    remote
                } else {
                    local
                }
            }
            (None, Some(version)) => {
                Self::blocking_install(&version)?;
                version
            }
            // do nothing otherwise
            _ => return Err(SolcError::VersionNotFound),
        })
    }

    /// Parses the given source looking for the `pragma` definition and
    /// returns the corresponding SemVer version requirement.
    pub fn source_version_req(source: &Source) -> Result<VersionReq> {
        let version =
            utils::find_version_pragma(&source.content).ok_or(SolcError::PragmaNotFound)?;
        Self::version_req(version.as_str())
    }

    /// Returns the corresponding SemVer version requirement for the solidity version.
    ///
    /// Note: This is a workaround for the fact that `VersionReq::parse` does not support whitespace
    /// separators and requires comma separated operators. See [VersionReq].
    pub fn version_req(version: &str) -> Result<VersionReq> {
        Ok(SolData::parse_version_req(version)?)
    }

    /// Installs the provided version of Solc in the machine under the svm dir and returns the
    /// [Solc] instance pointing to the installation.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use foundry_compilers::{Solc, ISTANBUL_SOLC};
    ///
    /// # async fn run() -> Result<(), Box<dyn std::error::Error>> {
    /// let solc = Solc::install(&ISTANBUL_SOLC).await?;
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(feature = "svm-solc")]
    pub async fn install(version: &Version) -> std::result::Result<Self, svm::SvmError> {
        trace!("installing solc version \"{}\"", version);
        crate::report::solc_installation_start(version);
        match svm::install(version).await {
            Ok(path) => {
                crate::report::solc_installation_success(version);
                Ok(Solc::new_with_version(path, version.clone()))
            }
            Err(err) => {
                crate::report::solc_installation_error(version, &err.to_string());
                Err(err)
            }
        }
    }

    /// Blocking version of `Self::install`
    #[cfg(feature = "svm-solc")]
    pub fn blocking_install(version: &Version) -> std::result::Result<Self, svm::SvmError> {
        use crate::utils::RuntimeOrHandle;

        trace!("blocking installing solc version \"{}\"", version);
        crate::report::solc_installation_start(version);
        // The async version `svm::install` is used instead of `svm::blocking_intsall`
        // because the underlying `reqwest::blocking::Client` does not behave well
        // inside of a Tokio runtime. See: https://github.com/seanmonstar/reqwest/issues/1017
        match RuntimeOrHandle::new().block_on(svm::install(version)) {
            Ok(path) => {
                crate::report::solc_installation_success(version);
                Ok(Solc::new_with_version(path, version.clone()))
            }
            Err(err) => {
                crate::report::solc_installation_error(version, &err.to_string());
                Err(err)
            }
        }
    }

    /// Verify that the checksum for this version of solc is correct. We check against the SHA256
    /// checksum from the build information published by [binaries.soliditylang.org](https://binaries.soliditylang.org/)
    #[cfg(feature = "svm-solc")]
    pub fn verify_checksum(&self) -> Result<()> {
        let version = self.version_short();
        let mut version_path = svm::version_path(version.to_string().as_str());
        version_path.push(format!("solc-{}", version.to_string().as_str()));
        trace!(target:"solc", "reading solc binary for checksum {:?}", version_path);
        let content =
            std::fs::read(&version_path).map_err(|err| SolcError::io(err, version_path.clone()))?;

        if !RELEASES.2 {
            // we skip checksum verification because the underlying request to fetch release info
            // failed so we have nothing to compare against
            return Ok(());
        }

        #[cfg(windows)]
        {
            // Prior to 0.7.2, binaries are released as exe files which are hard to verify: <https://github.com/foundry-rs/foundry/issues/5601>
            // <https://binaries.soliditylang.org/windows-amd64/list.json>
            const V0_7_2: Version = Version::new(0, 7, 2);
            if version < V0_7_2 {
                return Ok(());
            }
        }

        use sha2::Digest;
        let mut hasher = sha2::Sha256::new();
        hasher.update(content);
        let checksum_calc = &hasher.finalize()[..];

        let checksum_found = &RELEASES
            .0
            .get_checksum(&version)
            .ok_or_else(|| SolcError::ChecksumNotFound { version: version.clone() })?;

        if checksum_calc == checksum_found {
            Ok(())
        } else {
            use alloy_primitives::hex;
            let expected = hex::encode(checksum_found);
            let detected = hex::encode(checksum_calc);
            warn!(target: "solc", "checksum mismatch for {:?}, expected {}, but found {} for file {:?}", version, expected, detected, version_path);
            Err(SolcError::ChecksumMismatch { version, expected, detected, file: version_path })
        }
    }

    /// Invokes `solc --version` and parses the output as a SemVer [`Version`], stripping the
    /// pre-release and build metadata.
    pub fn version_short(&self) -> Version {
        Version::new(self.version.major, self.version.minor, self.version.patch)
    }

    /// Invokes `solc --version` and parses the output as a SemVer [`Version`].
    #[instrument(level = "debug", skip_all)]
    pub fn version(solc: impl Into<PathBuf>) -> Result<Version> {
        let solc = solc.into();
        let mut cmd = Command::new(solc.clone());
        cmd.arg("--version").stdin(Stdio::piped()).stderr(Stdio::piped()).stdout(Stdio::piped());
        debug!(?cmd, "getting Solc version");
        let output = cmd.output().map_err(|e| SolcError::io(e, solc))?;
        trace!(?output);
        let version = version_from_output(output)?;
        debug!(%version);
        Ok(version)
    }

    fn map_io_err(&self) -> impl FnOnce(std::io::Error) -> SolcError + '_ {
        move |err| SolcError::io(err, &self.solc)
    }

    pub fn configure_cmd(
        &self,
        base_path: Option<PathBuf>,
        include_paths: &BTreeSet<PathBuf>,
        allow_paths: &BTreeSet<PathBuf>,
    ) -> Command {
        let mut cmd = Command::new(&self.solc);
        cmd.stdin(Stdio::piped()).stderr(Stdio::piped()).stdout(Stdio::piped());

        if !allow_paths.is_empty() {
            cmd.arg("--allow-paths");
            cmd.arg(allow_paths.iter().map(|p| p.display()).join(","));
        }
        if let Some(base_path) = base_path {
            if SUPPORTS_BASE_PATH.matches(&self.version) {
                if SUPPORTS_INCLUDE_PATH.matches(&self.version) {
                    // `--base-path` and `--include-path` conflict if set to the same path, so
                    // as a precaution, we ensure here that the `--base-path` is not also used
                    // for `--include-path`
                    for path in include_paths.iter().filter(|p| p.as_path() != base_path.as_path())
                    {
                        cmd.arg("--include-path").arg(path);
                    }
                }

                cmd.arg("--base-path").arg(&base_path);
            }

            cmd.current_dir(&base_path);
        }

        cmd.arg("--standard-json");

        cmd
    }
}

#[cfg(feature = "async")]
#[cfg(ignore)]
impl Solc {
    /// Convenience function for compiling all sources under the given path
    pub async fn async_compile_source(&self, path: impl AsRef<Path>) -> Result<CompilerOutput> {
        self.async_compile(&CompilerInput::with_sources(Source::async_read_all_from(path).await?))
            .await
    }

    /// Run `solc --stand-json` and return the `solc`'s output as
    /// `CompilerOutput`
    pub async fn async_compile<T: Serialize>(&self, input: &T) -> Result<CompilerOutput> {
        self.async_compile_as(input).await
    }

    /// Run `solc --stand-json` and return the `solc`'s output as the given json
    /// output
    pub async fn async_compile_as<T: Serialize, D: DeserializeOwned>(
        &self,
        input: &T,
    ) -> Result<D> {
        let output = self.async_compile_output(input).await?;
        Ok(serde_json::from_slice(&output)?)
    }

    pub async fn async_compile_output<T: Serialize>(&self, input: &T) -> Result<Vec<u8>> {
        use tokio::io::AsyncWriteExt;
        let content = serde_json::to_vec(input)?;
        let mut cmd = tokio::process::Command::new(&self.solc);
        if let Some(ref base_path) = self.base_path {
            cmd.current_dir(base_path);
        }
        let mut child = cmd
            .args(&self.args)
            .arg("--standard-json")
            .stdin(Stdio::piped())
            .stderr(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .map_err(self.map_io_err())?;
        let stdin = child.stdin.as_mut().unwrap();
        stdin.write_all(&content).await.map_err(self.map_io_err())?;
        stdin.flush().await.map_err(self.map_io_err())?;
        compile_output(child.wait_with_output().await.map_err(self.map_io_err())?)
    }

    pub async fn async_version(&self) -> Result<Version> {
        let mut cmd = tokio::process::Command::new(&self.solc);
        cmd.arg("--version").stdin(Stdio::piped()).stderr(Stdio::piped()).stdout(Stdio::piped());
        debug!(?cmd, "getting version");
        let output = cmd.output().await.map_err(self.map_io_err())?;
        let version = version_from_output(output)?;
        debug!(%version);
        Ok(version)
    }

    /// Compiles all `CompilerInput`s with their associated `Solc`.
    ///
    /// This will buffer up to `n` `solc` processes and then return the `CompilerOutput`s in the
    /// order in which they complete. No more than `n` futures will be buffered at any point in
    /// time, and less than `n` may also be buffered depending on the state of each future.
    ///
    /// # Examples
    ///
    /// Compile 2 `CompilerInput`s at once
    ///
    /// ```no_run
    /// use foundry_compilers::{CompilerInput, Solc};
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let solc1 = Solc::default();
    /// let solc2 = Solc::default();
    /// let input1 = CompilerInput::new("contracts")?[0].clone();
    /// let input2 = CompilerInput::new("src")?[0].clone();
    ///
    /// let outputs = Solc::compile_many([(solc1, input1), (solc2, input2)], 2).await.flattened()?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn compile_many<I>(jobs: I, n: usize) -> crate::many::CompiledMany
    where
        I: IntoIterator<Item = (Solc, CompilerInput)>,
    {
        use futures_util::stream::StreamExt;

        let outputs = futures_util::stream::iter(
            jobs.into_iter()
                .map(|(solc, input)| async { (solc.async_compile(&input).await, solc, input) }),
        )
        .buffer_unordered(n)
        .collect::<Vec<_>>()
        .await;

        crate::many::CompiledMany::new(outputs)
    }
}

fn version_from_output(output: Output) -> Result<Version> {
    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let version = stdout
            .lines()
            .filter(|l| !l.trim().is_empty())
            .last()
            .ok_or_else(|| SolcError::msg("Version not found in Solc output"))?;
        // NOTE: semver doesn't like `+` in g++ in build metadata which is invalid semver
        Ok(Version::from_str(&version.trim_start_matches("Version: ").replace(".g++", ".gcc"))?)
    } else {
        Err(SolcError::solc_output(&output))
    }
}

impl AsRef<Path> for Solc {
    fn as_ref(&self) -> &Path {
        &self.solc
    }
}

#[cfg(test)]
#[cfg(ignore)]
mod tests {
    use super::*;
    use crate::{
        compilers::{solc::SolcVersionManager, CompilerVersionManager, VersionManagerError},
        Artifact,
    };

    #[test]
    fn test_version_parse() {
        let req = Solc::version_req(">=0.6.2 <0.8.21").unwrap();
        let semver_req: VersionReq = ">=0.6.2,<0.8.21".parse().unwrap();
        assert_eq!(req, semver_req);
    }

    fn solc() -> Solc {
        Solc::default()
    }

    #[test]
    fn solc_version_works() {
        solc().version().unwrap();
    }

    #[test]
    fn can_parse_version_metadata() {
        let _version = Version::from_str("0.6.6+commit.6c089d02.Linux.gcc").unwrap();
    }

    #[cfg(feature = "async")]
    #[tokio::test]
    async fn async_solc_version_works() {
        let _version = solc().async_version().await.unwrap();
    }

    #[test]
    fn solc_compile_works() {
        let input = include_str!("../../test-data/in/compiler-in-1.json");
        let input: CompilerInput = serde_json::from_str(input).unwrap();
        let out = solc().compile(&input).unwrap();
        let other = solc().compile(&serde_json::json!(input)).unwrap();
        assert_eq!(out, other);
    }

    #[test]
    fn solc_metadata_works() {
        let input = include_str!("../../test-data/in/compiler-in-1.json");
        let mut input: CompilerInput = serde_json::from_str(input).unwrap();
        input.settings.push_output_selection("metadata");
        let out = solc().compile(&input).unwrap();
        for (_, c) in out.split().1.contracts_iter() {
            assert!(c.metadata.is_some());
        }
    }

    #[test]
    fn can_compile_with_remapped_links() {
        let input: CompilerInput =
            serde_json::from_str(include_str!("../../test-data/library-remapping-in.json"))
                .unwrap();
        let out = solc().compile(&input).unwrap();
        let (_, mut contracts) = out.split();
        let contract = contracts.remove("LinkTest").unwrap();
        let bytecode = &contract.get_bytecode().unwrap().object;
        assert!(!bytecode.is_unlinked());
    }

    #[test]
    fn can_compile_with_remapped_links_temp_dir() {
        let input: CompilerInput =
            serde_json::from_str(include_str!("../../test-data/library-remapping-in-2.json"))
                .unwrap();
        let out = solc().compile(&input).unwrap();
        let (_, mut contracts) = out.split();
        let contract = contracts.remove("LinkTest").unwrap();
        let bytecode = &contract.get_bytecode().unwrap().object;
        assert!(!bytecode.is_unlinked());
    }

    #[cfg(feature = "async")]
    #[tokio::test]
    async fn async_solc_compile_works() {
        let input = include_str!("../../test-data/in/compiler-in-1.json");
        let input: CompilerInput = serde_json::from_str(input).unwrap();
        let out = solc().async_compile(&input).await.unwrap();
        let other = solc().async_compile(&serde_json::json!(input)).await.unwrap();
        assert_eq!(out, other);
    }

    #[cfg(feature = "async")]
    #[tokio::test]
    async fn async_solc_compile_works2() {
        let input = include_str!("../../test-data/in/compiler-in-2.json");
        let input: CompilerInput = serde_json::from_str(input).unwrap();
        let out = solc().async_compile(&input).await.unwrap();
        let other = solc().async_compile(&serde_json::json!(input)).await.unwrap();
        assert_eq!(out, other);
        let sync_out = solc().compile(&input).unwrap();
        assert_eq!(out, sync_out);
    }

    #[test]
    fn test_version_req() {
        let versions = ["=0.1.2", "^0.5.6", ">=0.7.1", ">0.8.0"];
        let sources = versions.iter().map(|version| source(version));

        sources.zip(versions).for_each(|(source, version)| {
            let version_req = Solc::source_version_req(&source).unwrap();
            assert_eq!(version_req, VersionReq::from_str(version).unwrap());
        });

        // Solidity defines version ranges with a space, whereas the semver package
        // requires them to be separated with a comma
        let version_range = ">=0.8.0 <0.9.0";
        let source = source(version_range);
        let version_req = Solc::source_version_req(&source).unwrap();
        assert_eq!(version_req, VersionReq::from_str(">=0.8.0,<0.9.0").unwrap());
    }

    #[test]
    #[cfg(feature = "svm-solc")]
    fn test_detect_version() {
        for (pragma, expected) in [
            // pinned
            ("=0.4.14", "0.4.14"),
            // pinned too
            ("0.4.14", "0.4.14"),
            // The latest patch is 0.4.26
            ("^0.4.14", "0.4.26"),
            // range
            (">=0.4.0 <0.5.0", "0.4.26"),
            // latest - this has to be updated every time a new version is released.
            // Requires the SVM version list to be updated as well.
            (">=0.5.0", "0.8.25"),
        ] {
            let source = source(pragma);
            let res = Solc::detect_version(&source).unwrap();
            assert_eq!(res, Version::from_str(expected).unwrap());
        }
    }

    #[test]
    #[cfg(feature = "full")]
    #[cfg(ignore)]
    fn test_find_installed_version_path() {
        // This test does not take the lock by default, so we need to manually add it here.
        let _lock = take_solc_installer_lock();
        let ver = "0.8.6";
        let version = Version::from_str(ver).unwrap();
        if utils::installed_versions(svm::data_dir())
            .map(|versions| !versions.contains(&version))
            .unwrap_or_default()
        {
            Solc::blocking_install(&version).unwrap();
        }
        let res = Solc::find_svm_installed_version(version.to_string()).unwrap().unwrap();
        let expected = svm::data_dir().join(ver).join(format!("solc-{ver}"));
        assert_eq!(res.solc, expected);
    }

    #[test]
    #[cfg(feature = "svm-solc")]
    fn can_install_solc_in_tokio_rt() {
        let version = Version::from_str("0.8.6").unwrap();
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(async { Solc::blocking_install(&version) });
        assert!(result.is_ok());
    }

    #[test]
    fn does_not_find_not_installed_version() {
        let ver = "1.1.1";
        let version = Version::from_str(ver).unwrap();
        let res = SolcVersionManager.get_installed(&version);
        assert!(matches!(res, Err(VersionManagerError::VersionNotInstalled(_))));
    }

    #[test]
    fn test_find_latest_matching_installation() {
        let versions = ["0.4.24", "0.5.1", "0.5.2"]
            .iter()
            .map(|version| Version::from_str(version).unwrap())
            .collect::<Vec<_>>();

        let required = VersionReq::from_str(">=0.4.24").unwrap();

        let got = Solc::find_matching_installation(&versions, &required).unwrap();
        assert_eq!(got, versions[2]);
    }

    #[test]
    fn test_no_matching_installation() {
        let versions = ["0.4.24", "0.5.1", "0.5.2"]
            .iter()
            .map(|version| Version::from_str(version).unwrap())
            .collect::<Vec<_>>();

        let required = VersionReq::from_str(">=0.6.0").unwrap();
        let got = Solc::find_matching_installation(&versions, &required);
        assert!(got.is_none());
    }

    ///// helpers

    fn source(version: &str) -> Source {
        Source::new(format!("pragma solidity {version};\n"))
    }
}

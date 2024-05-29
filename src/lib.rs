#![doc = include_str!("../README.md")]
#![warn(rustdoc::all)]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![deny(unused_must_use, rust_2018_idioms)]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]

#[macro_use]
extern crate tracing;

#[macro_use]
pub mod error;

pub mod artifacts;
pub use artifacts::{CompilerOutput, EvmVersion, SolcInput};

pub mod sourcemap;

mod artifact_output;
pub use artifact_output::*;

pub mod buildinfo;

pub mod cache;

pub mod flatten;

pub mod hh;
use compilers::{multi::MultiCompiler, Compiler, CompilerSettings};
pub use filter::SparseOutputFileFilter;
pub use hh::{HardhatArtifact, HardhatArtifacts};

pub mod resolver;
pub use resolver::Graph;

pub mod compilers;

mod compile;
pub use compile::{
    output::{AggregatedCompilerOutput, ProjectCompileOutput},
    *,
};

mod config;
pub use config::{PathStyle, ProjectPaths, ProjectPathsConfig, SolcConfig};

pub mod remappings;

mod filter;
pub use filter::{
    FileFilter, FilteredSources, SourceCompilationKind, SparseOutputFilter, TestFileFilter,
};
use solang_parser::pt::SourceUnitPart;

pub mod report;

pub mod utils;

use crate::{
    artifacts::{Source, SourceFile, Sources, StandardJsonCompilerInput},
    cache::CompilerCache,
    error::{SolcError, SolcIoError},
    sources::{VersionedSourceFile, VersionedSourceFiles},
};
use artifacts::{contract::Contract, output_selection::OutputSelection, Settings, Severity};
use compile::output::contracts::VersionedContracts;
use error::Result;
use semver::Version;
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

/// Utilities for creating, mocking and testing of (temporary) projects
#[cfg(feature = "project-util")]
pub mod project_util;

/// Represents a project workspace and handles `solc` compiling of all contracts in that workspace.
#[derive(Clone, Debug)]
pub struct Project<C: Compiler = MultiCompiler, T: ArtifactOutput = ConfigurableArtifacts> {
    pub compiler: C,
    /// Compiler versions locked for specific languages.
    pub locked_versions: HashMap<C::Language, Version>,
    /// The layout of the project
    pub paths: ProjectPathsConfig<C::Language>,
    /// The compiler settings
    pub settings: C::Settings,
    /// Whether caching is enabled
    pub cached: bool,
    /// Whether to output build information with each solc call.
    pub build_info: bool,
    /// Whether writing artifacts to disk is enabled
    pub no_artifacts: bool,
    /// Handles all artifacts related tasks, reading and writing from the artifact dir.
    pub artifacts: T,
    /// Errors/Warnings which match these error codes are not going to be logged
    pub ignored_error_codes: Vec<u64>,
    /// Errors/Warnings which match these file paths are not going to be logged
    pub ignored_file_paths: Vec<PathBuf>,
    /// The minimum severity level that is treated as a compiler error
    pub compiler_severity_filter: Severity,
    /// Maximum number of `solc` processes to run simultaneously.
    solc_jobs: usize,
    /// Offline mode, if set, network access (download solc) is disallowed
    pub offline: bool,
    /// Windows only config value to ensure the all paths use `/` instead of `\\`, same as `solc`
    ///
    /// This is a noop on other platforms
    pub slash_paths: bool,
}

impl Project {
    /// Convenience function to call `ProjectBuilder::default()`.
    ///
    /// # Examples
    ///
    /// Configure with `ConfigurableArtifacts` artifacts output:
    ///
    /// ```
    /// use foundry_compilers::Project;
    ///
    /// let config = Project::builder().build()?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// To configure any a project with any `ArtifactOutput` use either:
    ///
    /// ```
    /// use foundry_compilers::Project;
    ///
    /// let config = Project::builder().build()?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// or use the builder directly:
    ///
    /// ```
    /// use foundry_compilers::{ConfigurableArtifacts, ProjectBuilder};
    ///
    /// let config = ProjectBuilder::<ConfigurableArtifacts>::default().build()?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn builder() -> ProjectBuilder {
        ProjectBuilder::default()
    }
}

impl<T: ArtifactOutput, C: Compiler> Project<C, T> {
    /// Returns the handler that takes care of processing all artifacts
    pub fn artifacts_handler(&self) -> &T {
        &self.artifacts
    }
}

impl<C: Compiler, T: ArtifactOutput> Project<C, T>
where
    C::Settings: Into<Settings>,
{
    /// Returns standard-json-input to compile the target contract
    pub fn standard_json_input(
        &self,
        target: impl AsRef<Path>,
    ) -> Result<StandardJsonCompilerInput> {
        let target = target.as_ref();
        trace!("Building standard-json-input for {:?}", target);
        let graph = Graph::<C::ParsedSource>::resolve(&self.paths)?;
        let target_index = graph.files().get(target).ok_or_else(|| {
            SolcError::msg(format!("cannot resolve file at {:?}", target.display()))
        })?;

        let mut sources = Vec::new();
        let mut unique_paths = HashSet::new();
        let (path, source) = graph.node(*target_index).unpack();
        unique_paths.insert(path.clone());
        sources.push((path, source));
        sources.extend(
            graph
                .all_imported_nodes(*target_index)
                .map(|index| graph.node(index).unpack())
                .filter(|(p, _)| unique_paths.insert(p.to_path_buf())),
        );

        let root = self.root();
        let sources = sources
            .into_iter()
            .map(|(path, source)| (rebase_path(root, path), source.clone()))
            .collect();

        let mut settings = self.settings.clone().into();
        // strip the path to the project root from all remappings
        settings.remappings = self
            .paths
            .remappings
            .clone()
            .into_iter()
            .map(|r| r.into_relative(self.root()).to_relative_remapping())
            .collect::<Vec<_>>();

        let input = StandardJsonCompilerInput::new(sources, settings);

        Ok(input)
    }
}

impl<T: ArtifactOutput, C: Compiler> Project<C, T> {
    /// Returns the path to the artifacts directory
    pub fn artifacts_path(&self) -> &PathBuf {
        &self.paths.artifacts
    }

    /// Returns the path to the sources directory
    pub fn sources_path(&self) -> &PathBuf {
        &self.paths.sources
    }

    /// Returns the path to the cache file
    pub fn cache_path(&self) -> &PathBuf {
        &self.paths.cache
    }

    /// Returns the path to the `build-info` directory nested in the artifacts dir
    pub fn build_info_path(&self) -> &PathBuf {
        &self.paths.build_infos
    }

    /// Returns the root directory of the project
    pub fn root(&self) -> &PathBuf {
        &self.paths.root
    }

    /// Convenience function to read the cache file.
    /// See also [CompilerCache::read_joined()]
    pub fn read_cache_file(&self) -> Result<CompilerCache<C::Settings>> {
        CompilerCache::read_joined(&self.paths)
    }

    /// Sets the maximum number of parallel `solc` processes to run simultaneously.
    ///
    /// # Panics
    ///
    /// if `jobs == 0`
    pub fn set_solc_jobs(&mut self, jobs: usize) {
        assert!(jobs > 0);
        self.solc_jobs = jobs;
    }

    /// Returns all sources found under the project's configured sources path
    #[instrument(skip_all, fields(name = "sources"))]
    pub fn sources(&self) -> Result<Sources> {
        self.paths.read_sources()
    }

    /// Emit the cargo [`rerun-if-changed`](https://doc.rust-lang.org/cargo/reference/build-scripts.html#cargorerun-if-changedpath) instruction.
    ///
    /// This tells Cargo to re-run the build script if a file inside the project's sources directory
    /// has changed.
    ///
    /// Use this if you compile a project in a `build.rs` file.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use foundry_compilers::{Project, ProjectPathsConfig};
    ///
    /// // Configure the project with all its paths, solc, cache etc.
    /// // where the root dir is the current Rust project.
    /// let paths = ProjectPathsConfig::hardhat(env!("CARGO_MANIFEST_DIR"))?;
    /// let project = Project::builder().paths(paths).build()?;
    /// let output = project.compile()?;
    ///
    /// // Tell Cargo to rerun this build script that if a source file changes.
    /// project.rerun_if_sources_changed();
    /// # Ok::<_, Box<dyn std::error::Error>>(())
    /// ```
    pub fn rerun_if_sources_changed(&self) {
        println!("cargo:rerun-if-changed={}", self.paths.sources.display())
    }

    pub fn compile(&self) -> Result<ProjectCompileOutput<C::CompilationError, T>> {
        project::ProjectCompiler::new(self)?.compile()
    }

    /// Convenience function to compile a single solidity file with the project's settings.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use foundry_compilers::Project;
    ///
    /// let project = Project::builder().build()?;
    /// let output = project.compile_file("example/Greeter.sol")?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn compile_file(
        &self,
        file: impl Into<PathBuf>,
    ) -> Result<ProjectCompileOutput<C::CompilationError, T>> {
        let file = file.into();
        let source = Source::read(&file)?;
        project::ProjectCompiler::with_sources(self, Sources::from([(file, source)]))?.compile()
    }

    /// Convenience function to compile a series of solidity files with the project's settings.
    /// Same as [`Self::compile()`] but with the given `files` as input.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use foundry_compilers::Project;
    ///
    /// let project = Project::builder().build()?;
    /// let output = project.compile_files(["examples/Foo.sol", "examples/Bar.sol"])?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn compile_files<P, I>(
        &self,
        files: I,
    ) -> Result<ProjectCompileOutput<C::CompilationError, T>>
    where
        I: IntoIterator<Item = P>,
        P: Into<PathBuf>,
    {
        let sources = Source::read_all(files)?;

        project::ProjectCompiler::with_sources(self, sources)?.compile()
    }

    /// Convenience function to compile only files that match the provided [FileFilter].
    ///
    /// Same as [`Self::compile()`] but with only with the input files that match
    /// [`FileFilter::is_match()`].
    ///
    /// # Examples
    ///
    /// Only compile test files:
    ///
    /// ```no_run
    /// use foundry_compilers::{Project, TestFileFilter};
    ///
    /// let project = Project::builder().build()?;
    /// let output = project.compile_sparse(Box::new(TestFileFilter::default()))?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// Apply a custom filter:
    ///
    /// ```no_run
    /// use foundry_compilers::Project;
    /// use std::path::Path;
    ///
    /// let project = Project::builder().build()?;
    /// let output = project.compile_sparse(Box::new(|path: &Path| path.ends_with("Greeter.sol")))?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn compile_sparse(
        &self,
        filter: Box<dyn SparseOutputFileFilter<C::ParsedSource>>,
    ) -> Result<ProjectCompileOutput<C::CompilationError, T>> {
        let sources =
            Source::read_all(self.paths.input_files().into_iter().filter(|p| filter.is_match(p)))?;

        project::ProjectCompiler::with_sources(self, sources)?.with_sparse_output(filter).compile()
    }

    /// Removes the project's artifacts and cache file
    ///
    /// If the cache file was the only file in the folder, this also removes the empty folder.
    ///
    /// # Examples
    ///
    /// ```
    /// use foundry_compilers::Project;
    ///
    /// let project = Project::builder().build()?;
    /// let _ = project.compile()?;
    /// assert!(project.artifacts_path().exists());
    /// assert!(project.cache_path().exists());
    ///
    /// project.cleanup();
    /// assert!(!project.artifacts_path().exists());
    /// assert!(!project.cache_path().exists());
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn cleanup(&self) -> std::result::Result<(), SolcIoError> {
        trace!("clean up project");
        if self.cache_path().exists() {
            std::fs::remove_file(self.cache_path())
                .map_err(|err| SolcIoError::new(err, self.cache_path()))?;
            if let Some(cache_folder) =
                self.cache_path().parent().filter(|cache_folder| self.root() != cache_folder)
            {
                // remove the cache folder if the cache file was the only file
                if cache_folder
                    .read_dir()
                    .map_err(|err| SolcIoError::new(err, cache_folder))?
                    .next()
                    .is_none()
                {
                    std::fs::remove_dir(cache_folder)
                        .map_err(|err| SolcIoError::new(err, cache_folder))?;
                }
            }
            trace!("removed cache file \"{}\"", self.cache_path().display());
        }

        // clean the artifacts dir
        if self.artifacts_path().exists() && self.root() != self.artifacts_path() {
            std::fs::remove_dir_all(self.artifacts_path())
                .map_err(|err| SolcIoError::new(err, self.artifacts_path().clone()))?;
            trace!("removed artifacts dir \"{}\"", self.artifacts_path().display());
        }

        // also clean the build-info dir, in case it's not nested in the artifacts dir
        if self.build_info_path().exists() && self.root() != self.build_info_path() {
            std::fs::remove_dir_all(self.build_info_path())
                .map_err(|err| SolcIoError::new(err, self.build_info_path().clone()))?;
            tracing::trace!("removed build-info dir \"{}\"", self.build_info_path().display());
        }

        Ok(())
    }

    /// Runs solc compiler without requesting any output and collects a mapping from contract names
    /// to source files containing artifact with given name.
    fn collect_contract_names_solc(&self) -> Result<HashMap<String, Vec<PathBuf>>>
    where
        T: Clone,
        C: Clone,
    {
        let mut temp_project = (*self).clone();
        temp_project.no_artifacts = true;
        temp_project.settings.update_output_selection(|selection| {
            *selection = OutputSelection::common_output_selection(["abi".to_string()]);
        });

        let output = temp_project.compile()?;

        if output.has_compiler_errors() {
            return Err(SolcError::msg(output));
        }

        let contracts = output.into_artifacts().fold(
            HashMap::new(),
            |mut contracts: HashMap<_, Vec<_>>, (id, _)| {
                contracts.entry(id.name).or_default().push(id.source);
                contracts
            },
        );

        Ok(contracts)
    }

    /// Parses project sources via solang parser, collecting mapping from contract name to source
    /// files containing artifact with given name. On parser failure, fallbacks to
    /// [Self::collect_contract_names_solc].
    fn collect_contract_names(&self) -> Result<HashMap<String, Vec<PathBuf>>>
    where
        T: Clone,
        C: Clone,
    {
        let graph = Graph::<C::ParsedSource>::resolve(&self.paths)?;
        let mut contracts: HashMap<String, Vec<PathBuf>> = HashMap::new();

        for file in graph.files().keys() {
            let src = fs::read_to_string(file).map_err(|e| SolcError::io(e, file))?;
            let Ok((parsed, _)) = solang_parser::parse(&src, 0) else {
                return self.collect_contract_names_solc();
            };

            for part in parsed.0 {
                if let SourceUnitPart::ContractDefinition(contract) = part {
                    if let Some(name) = contract.name {
                        contracts.entry(name.name).or_default().push(file.clone());
                    }
                }
            }
        }

        Ok(contracts)
    }

    /// Finds the path of the contract with the given name.
    /// Throws error if multiple or no contracts with the same name are found.
    pub fn find_contract_path(&self, target_name: &str) -> Result<PathBuf>
    where
        T: Clone,
        C: Clone,
    {
        let mut contracts = self.collect_contract_names()?;

        if contracts.get(target_name).map_or(true, |paths| paths.is_empty()) {
            return Err(SolcError::msg(format!(
                "No contract found with the name `{}`",
                target_name
            )));
        }
        let mut paths = contracts.remove(target_name).unwrap();
        if paths.len() > 1 {
            return Err(SolcError::msg(format!(
                "Multiple contracts found with the name `{}`",
                target_name
            )));
        }

        Ok(paths.remove(0))
    }
}

pub struct ProjectBuilder<C: Compiler = MultiCompiler, T: ArtifactOutput = ConfigurableArtifacts> {
    /// The layout of the
    paths: Option<ProjectPathsConfig<C::Language>>,
    /// Compiler versions locked for specific languages.
    locked_versions: HashMap<C::Language, Version>,
    /// How solc invocation should be configured.
    settings: Option<C::Settings>,
    /// Whether caching is enabled, default is true.
    cached: bool,
    /// Whether to output build information with each solc call.
    build_info: bool,
    /// Whether writing artifacts to disk is enabled, default is true.
    no_artifacts: bool,
    /// Use offline mode
    offline: bool,
    /// Whether to slash paths of the `ProjectCompilerOutput`
    slash_paths: bool,
    /// handles all artifacts related tasks
    artifacts: T,
    /// Which error codes to ignore
    pub ignored_error_codes: Vec<u64>,
    /// Which file paths to ignore
    pub ignored_file_paths: Vec<PathBuf>,
    /// The minimum severity level that is treated as a compiler error
    compiler_severity_filter: Severity,
    solc_jobs: Option<usize>,
}

impl<C: Compiler, T: ArtifactOutput> ProjectBuilder<C, T> {
    /// Create a new builder with the given artifacts handler
    pub fn new(artifacts: T) -> Self {
        Self {
            paths: None,
            cached: true,
            build_info: false,
            no_artifacts: false,
            offline: false,
            slash_paths: true,
            artifacts,
            ignored_error_codes: Vec::new(),
            ignored_file_paths: Vec::new(),
            compiler_severity_filter: Severity::Error,
            solc_jobs: None,
            settings: None,
            locked_versions: Default::default(),
        }
    }

    #[must_use]
    pub fn paths(mut self, paths: ProjectPathsConfig<C::Language>) -> Self {
        self.paths = Some(paths);
        self
    }

    #[must_use]
    pub fn settings(mut self, settings: C::Settings) -> Self {
        self.settings = Some(settings);
        self
    }

    #[must_use]
    pub fn ignore_error_code(mut self, code: u64) -> Self {
        self.ignored_error_codes.push(code);
        self
    }

    #[must_use]
    pub fn ignore_error_codes(mut self, codes: impl IntoIterator<Item = u64>) -> Self {
        for code in codes {
            self = self.ignore_error_code(code);
        }
        self
    }

    pub fn ignore_paths(mut self, paths: Vec<PathBuf>) -> Self {
        self.ignored_file_paths = paths;
        self
    }

    #[must_use]
    pub fn set_compiler_severity_filter(mut self, compiler_severity_filter: Severity) -> Self {
        self.compiler_severity_filter = compiler_severity_filter;
        self
    }

    /// Disables cached builds
    #[must_use]
    pub fn ephemeral(self) -> Self {
        self.set_cached(false)
    }

    /// Sets the cache status
    #[must_use]
    pub fn set_cached(mut self, cached: bool) -> Self {
        self.cached = cached;
        self
    }

    /// Sets the build info value
    #[must_use]
    pub fn set_build_info(mut self, build_info: bool) -> Self {
        self.build_info = build_info;
        self
    }

    /// Activates offline mode
    ///
    /// Prevents network possible access to download/check solc installs
    #[must_use]
    pub fn offline(self) -> Self {
        self.set_offline(true)
    }

    /// Sets the offline status
    #[must_use]
    pub fn set_offline(mut self, offline: bool) -> Self {
        self.offline = offline;
        self
    }

    /// Sets whether to slash all paths on windows
    ///
    /// If set to `true` all `\\` separators are replaced with `/`, same as solc
    #[must_use]
    pub fn set_slashed_paths(mut self, slashed_paths: bool) -> Self {
        self.slash_paths = slashed_paths;
        self
    }

    /// Disables writing artifacts to disk
    #[must_use]
    pub fn no_artifacts(self) -> Self {
        self.set_no_artifacts(true)
    }

    /// Sets the no artifacts status
    #[must_use]
    pub fn set_no_artifacts(mut self, artifacts: bool) -> Self {
        self.no_artifacts = artifacts;
        self
    }

    /// Sets the maximum number of parallel `solc` processes to run simultaneously.
    ///
    /// # Panics
    ///
    /// `jobs` must be at least 1
    #[must_use]
    pub fn solc_jobs(mut self, jobs: usize) -> Self {
        assert!(jobs > 0);
        self.solc_jobs = Some(jobs);
        self
    }

    /// Sets the number of parallel `solc` processes to `1`, no parallelization
    #[must_use]
    pub fn single_solc_jobs(self) -> Self {
        self.solc_jobs(1)
    }

    #[must_use]
    pub fn locked_version(mut self, lang: impl Into<C::Language>, version: Version) -> Self {
        self.locked_versions.insert(lang.into(), version);
        self
    }

    #[must_use]
    pub fn locked_versions(mut self, versions: HashMap<C::Language, Version>) -> Self {
        self.locked_versions = versions;
        self
    }

    /// Set arbitrary `ArtifactOutputHandler`
    pub fn artifacts<A: ArtifactOutput>(self, artifacts: A) -> ProjectBuilder<C, A> {
        let ProjectBuilder {
            paths,
            cached,
            no_artifacts,
            ignored_error_codes,
            compiler_severity_filter,
            solc_jobs,
            offline,
            build_info,
            slash_paths,
            ignored_file_paths,
            settings,
            locked_versions,
            ..
        } = self;
        ProjectBuilder {
            paths,
            cached,
            no_artifacts,
            offline,
            slash_paths,
            artifacts,
            ignored_error_codes,
            ignored_file_paths,
            compiler_severity_filter,
            solc_jobs,
            build_info,
            settings,
            locked_versions,
        }
    }

    pub fn build(self, compiler: C) -> Result<Project<C, T>> {
        let Self {
            paths,
            cached,
            no_artifacts,
            artifacts,
            ignored_error_codes,
            ignored_file_paths,
            compiler_severity_filter,
            solc_jobs,
            offline,
            build_info,
            slash_paths,
            settings,
            locked_versions,
        } = self;

        let mut paths = paths.map(Ok).unwrap_or_else(ProjectPathsConfig::current_hardhat)?;

        if slash_paths {
            // ensures we always use `/` paths
            paths.slash_paths();
        }

        Ok(Project {
            compiler,
            paths,
            cached,
            build_info,
            no_artifacts,
            artifacts,
            ignored_error_codes,
            ignored_file_paths,
            compiler_severity_filter,
            solc_jobs: solc_jobs
                .or_else(|| std::thread::available_parallelism().ok().map(|n| n.get()))
                .unwrap_or(1),
            offline,
            slash_paths,
            settings: settings.unwrap_or_default(),
            locked_versions,
        })
    }
}

impl<C: Compiler, T: ArtifactOutput + Default> Default for ProjectBuilder<C, T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}

impl<T: ArtifactOutput, C: Compiler> ArtifactOutput for Project<C, T> {
    type Artifact = T::Artifact;

    fn on_output<CP>(
        &self,
        contracts: &VersionedContracts,
        sources: &VersionedSourceFiles,
        layout: &ProjectPathsConfig<CP>,
        ctx: OutputContext<'_>,
    ) -> Result<Artifacts<Self::Artifact>> {
        self.artifacts_handler().on_output(contracts, sources, layout, ctx)
    }

    fn handle_artifacts(
        &self,
        contracts: &VersionedContracts,
        artifacts: &Artifacts<Self::Artifact>,
    ) -> Result<()> {
        self.artifacts_handler().handle_artifacts(contracts, artifacts)
    }

    fn output_file_name(name: impl AsRef<str>) -> PathBuf {
        T::output_file_name(name)
    }

    fn output_file_name_versioned(name: impl AsRef<str>, version: &Version) -> PathBuf {
        T::output_file_name_versioned(name, version)
    }

    fn output_file(contract_file: impl AsRef<Path>, name: impl AsRef<str>) -> PathBuf {
        T::output_file(contract_file, name)
    }

    fn output_file_versioned(
        contract_file: impl AsRef<Path>,
        name: impl AsRef<str>,
        version: &Version,
    ) -> PathBuf {
        T::output_file_versioned(contract_file, name, version)
    }

    fn contract_name(file: impl AsRef<Path>) -> Option<String> {
        T::contract_name(file)
    }

    fn output_exists(
        contract_file: impl AsRef<Path>,
        name: impl AsRef<str>,
        root: impl AsRef<Path>,
    ) -> bool {
        T::output_exists(contract_file, name, root)
    }

    fn read_cached_artifact(path: impl AsRef<Path>) -> Result<Self::Artifact> {
        T::read_cached_artifact(path)
    }

    fn read_cached_artifacts<P, I>(files: I) -> Result<BTreeMap<PathBuf, Self::Artifact>>
    where
        I: IntoIterator<Item = P>,
        P: Into<PathBuf>,
    {
        T::read_cached_artifacts(files)
    }

    fn contract_to_artifact(
        &self,
        file: &Path,
        name: &str,
        contract: Contract,
        source_file: Option<&SourceFile>,
    ) -> Self::Artifact {
        self.artifacts_handler().contract_to_artifact(file, name, contract, source_file)
    }

    fn output_to_artifacts<CP>(
        &self,
        contracts: &VersionedContracts,
        sources: &VersionedSourceFiles,
        ctx: OutputContext<'_>,
        layout: &ProjectPathsConfig<CP>,
    ) -> Artifacts<Self::Artifact> {
        self.artifacts_handler().output_to_artifacts(contracts, sources, ctx, layout)
    }

    fn standalone_source_file_to_artifact(
        &self,
        path: &Path,
        file: &VersionedSourceFile,
    ) -> Option<Self::Artifact> {
        self.artifacts_handler().standalone_source_file_to_artifact(path, file)
    }

    fn is_dirty(&self, artifact_file: &ArtifactFile<Self::Artifact>) -> Result<bool> {
        self.artifacts_handler().is_dirty(artifact_file)
    }

    fn handle_cached_artifacts(&self, artifacts: &Artifacts<Self::Artifact>) -> Result<()> {
        self.artifacts_handler().handle_cached_artifacts(artifacts)
    }
}

// Rebases the given path to the base directory lexically.
//
// For instance, given the base `/home/user/project` and the path `/home/user/project/src/A.sol`,
// this function returns `src/A.sol`.
//
// This function transforms a path into a form that is relative to the base directory. The returned
// path starts either with a normal component (e.g., `src`) or a parent directory component (i.e.,
// `..`). It also converts the path into a UTF-8 string and replaces all separators with forward
// slashes (`/`), if they're not.
//
// The rebasing process can be conceptualized as follows:
//
// 1. Remove the leading components from the path that match those in the base.
// 2. Prepend `..` components to the path, matching the number of remaining components in the base.
//
// # Examples
//
// `rebase_path("/home/user/project", "/home/user/project/src/A.sol")` returns `src/A.sol`. The
// common part, `/home/user/project`, is removed from the path.
//
// `rebase_path("/home/user/project", "/home/user/A.sol")` returns `../A.sol`. First, the common
// part, `/home/user`, is removed, leaving `A.sol`. Next, as `project` remains in the base, `..` is
// prepended to the path.
//
// On Windows, paths like `a\b\c` are converted to `a/b/c`.
//
// For more examples, see the test.
fn rebase_path(base: impl AsRef<Path>, path: impl AsRef<Path>) -> PathBuf {
    use path_slash::PathExt;

    let mut base_components = base.as_ref().components();
    let mut path_components = path.as_ref().components();

    let mut new_path = PathBuf::new();

    while let Some(path_component) = path_components.next() {
        let base_component = base_components.next();

        if Some(path_component) != base_component {
            if base_component.is_some() {
                new_path.extend(
                    std::iter::repeat(std::path::Component::ParentDir)
                        .take(base_components.count() + 1),
                );
            }

            new_path.push(path_component);
            new_path.extend(path_components);

            break;
        }
    }

    new_path.to_slash_lossy().into_owned().into()
}

#[cfg(test)]
#[cfg(feature = "svm-solc")]
mod tests {
    use super::*;
    use crate::remappings::Remapping;

    #[test]
    #[cfg_attr(windows, ignore = "<0.7 solc is flaky")]
    fn test_build_all_versions() {
        let paths = ProjectPathsConfig::builder()
            .root("./test-data/test-contract-versions")
            .sources("./test-data/test-contract-versions")
            .build()
            .unwrap();
        let project = Project::builder()
            .paths(paths)
            .no_artifacts()
            .ephemeral()
            .build(Default::default())
            .unwrap();
        let contracts = project.compile().unwrap().succeeded().into_output().contracts;
        // Contracts A to F
        assert_eq!(contracts.contracts().count(), 3);
    }

    #[test]
    fn test_build_many_libs() {
        let root = utils::canonicalize("./test-data/test-contract-libs").unwrap();

        let paths = ProjectPathsConfig::builder()
            .root(&root)
            .sources(root.join("src"))
            .lib(root.join("lib1"))
            .lib(root.join("lib2"))
            .remappings(
                Remapping::find_many(root.join("lib1"))
                    .into_iter()
                    .chain(Remapping::find_many(root.join("lib2"))),
            )
            .build()
            .unwrap();
        let project = Project::builder()
            .paths(paths)
            .no_artifacts()
            .ephemeral()
            .no_artifacts()
            .build(Default::default())
            .unwrap();
        let contracts = project.compile().unwrap().succeeded().into_output().contracts;
        assert_eq!(contracts.contracts().count(), 3);
    }

    #[test]
    fn test_build_remappings() {
        let root = utils::canonicalize("./test-data/test-contract-remappings").unwrap();
        let paths = ProjectPathsConfig::builder()
            .root(&root)
            .sources(root.join("src"))
            .lib(root.join("lib"))
            .remappings(Remapping::find_many(root.join("lib")))
            .build()
            .unwrap();
        let project = Project::builder()
            .no_artifacts()
            .paths(paths)
            .ephemeral()
            .build(Default::default())
            .unwrap();
        let contracts = project.compile().unwrap().succeeded().into_output().contracts;
        assert_eq!(contracts.contracts().count(), 2);
    }

    #[test]
    fn can_rebase_path() {
        assert_eq!(rebase_path("a/b", "a/b/c"), PathBuf::from("c"));
        assert_eq!(rebase_path("a/b", "a/c"), PathBuf::from("../c"));
        assert_eq!(rebase_path("a/b", "c"), PathBuf::from("../../c"));

        assert_eq!(
            rebase_path("/home/user/project", "/home/user/project/A.sol"),
            PathBuf::from("A.sol")
        );
        assert_eq!(
            rebase_path("/home/user/project", "/home/user/project/src/A.sol"),
            PathBuf::from("src/A.sol")
        );
        assert_eq!(
            rebase_path("/home/user/project", "/home/user/project/lib/forge-std/src/Test.sol"),
            PathBuf::from("lib/forge-std/src/Test.sol")
        );
        assert_eq!(
            rebase_path("/home/user/project", "/home/user/A.sol"),
            PathBuf::from("../A.sol")
        );
        assert_eq!(rebase_path("/home/user/project", "/home/A.sol"), PathBuf::from("../../A.sol"));
        assert_eq!(rebase_path("/home/user/project", "/A.sol"), PathBuf::from("../../../A.sol"));
        assert_eq!(
            rebase_path("/home/user/project", "/tmp/A.sol"),
            PathBuf::from("../../../tmp/A.sol")
        );

        assert_eq!(
            rebase_path("/Users/ah/temp/verif", "/Users/ah/temp/remapped/Child.sol"),
            PathBuf::from("../remapped/Child.sol")
        );
        assert_eq!(
            rebase_path("/Users/ah/temp/verif", "/Users/ah/temp/verif/../remapped/Parent.sol"),
            PathBuf::from("../remapped/Parent.sol")
        );
    }
}

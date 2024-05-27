use crate::{
    artifact_output::{ArtifactId, Artifacts},
    artifacts::error::Severity,
    buildinfo::RawBuildInfo,
    compile::output::{
        info::ContractInfoRef,
        sources::{VersionedSourceFile, VersionedSourceFiles},
    },
    output::Builds,
    zksync::{
        artifact_output::{
            artifacts_artifacts, artifacts_into_artifacts, contract_name, zk::ZkContractArtifact,
        },
        compile::output::contracts::{VersionedContract, VersionedContracts},
    },
};
use foundry_compilers_artifacts::{
    zksolc::{
        contract::{CompactContractRef, Contract},
        error::Error,
        CompilerOutput,
    },
    ErrorFilter, SolcLanguage,
};
use semver::Version;
use serde::{Deserialize, Serialize};
use std::{
    fmt,
    path::{Path, PathBuf},
};
use yansi::Paint;

pub mod contracts;

#[derive(Clone)]
pub struct ProjectCompileOutput {
    /// contains the aggregated `CompilerOutput`
    pub compiler_output: AggregatedCompilerOutput,
    /// all artifact files from `output` that were freshly compiled and written
    pub compiled_artifacts: Artifacts<ZkContractArtifact>,
    /// All artifacts that were read from cache
    pub cached_artifacts: Artifacts<ZkContractArtifact>,
    /// errors that should be omitted
    pub ignored_error_codes: Vec<u64>,
    /// paths that should be omitted
    pub ignored_file_paths: Vec<PathBuf>,
    /// set minimum level of severity that is treated as an error
    pub compiler_severity_filter: Severity,
    /// all build infos that were just compiled
    pub builds: Builds<SolcLanguage>,
}

impl ProjectCompileOutput {
    /// Converts all `\\` separators in _all_ paths to `/`
    pub fn slash_paths(&mut self) {
        self.compiler_output.slash_paths();
        self.compiled_artifacts.slash_paths();
        self.cached_artifacts.slash_paths();
    }

    /// All artifacts together with their contract file name and name `<file name>:<name>`.
    ///
    /// This returns a chained iterator of both cached and recompiled contract artifacts.
    pub fn artifact_ids(&self) -> impl Iterator<Item = (ArtifactId, &ZkContractArtifact)> {
        let Self { cached_artifacts, compiled_artifacts, .. } = self;
        artifacts_artifacts(cached_artifacts).chain(artifacts_artifacts(compiled_artifacts))
    }

    /// All artifacts together with their contract file name and name `<file name>:<name>`
    ///
    /// This returns a chained iterator of both cached and recompiled contract artifacts
    pub fn into_artifacts(self) -> impl Iterator<Item = (ArtifactId, ZkContractArtifact)> {
        let Self { cached_artifacts, compiled_artifacts, .. } = self;
        artifacts_into_artifacts(cached_artifacts)
            .chain(artifacts_into_artifacts(compiled_artifacts))
    }

    pub fn with_stripped_file_prefixes(mut self, base: impl AsRef<Path>) -> Self {
        let base = base.as_ref();
        self.cached_artifacts = self.cached_artifacts.into_stripped_file_prefixes(base);
        self.compiled_artifacts = self.compiled_artifacts.into_stripped_file_prefixes(base);
        self.compiler_output.strip_prefix_all(base);
        self
    }

    /// Returns whether this type does not contain compiled contracts.
    pub fn is_unchanged(&self) -> bool {
        self.compiler_output.is_unchanged()
    }

    /// Returns whether any errors were emitted by the compiler.
    pub fn has_compiler_errors(&self) -> bool {
        self.compiler_output.has_error(
            &self.ignored_error_codes,
            &self.ignored_file_paths,
            &self.compiler_severity_filter,
        )
    }

    pub fn versioned_artifacts(
        &self,
    ) -> impl Iterator<Item = (String, (&ZkContractArtifact, &Version))> {
        self.cached_artifacts
            .artifact_files()
            .chain(self.compiled_artifacts.artifact_files())
            .filter_map(|artifact| {
                contract_name(&artifact.file)
                    .map(|name| (name, (&artifact.artifact, &artifact.version)))
            })
    }

    pub fn artifacts(&self) -> impl Iterator<Item = (String, &ZkContractArtifact)> {
        self.versioned_artifacts().map(|(name, (artifact, _))| (name, artifact))
    }

    pub fn output(&self) -> &AggregatedCompilerOutput {
        &self.compiler_output
    }

    pub fn into_output(self) -> AggregatedCompilerOutput {
        self.compiler_output
    }

    /// Finds the artifact with matching path and name
    pub fn find(
        &self,
        path: impl AsRef<str>,
        contract: impl AsRef<str>,
    ) -> Option<&ZkContractArtifact> {
        let contract_path = path.as_ref();
        let contract_name = contract.as_ref();
        if let artifact @ Some(_) = self.compiled_artifacts.find(contract_path, contract_name) {
            return artifact;
        }
        self.cached_artifacts.find(contract_path, contract_name)
    }

    /// Returns the set of `Artifacts` that were cached and got reused during
    /// [`crate::Project::compile()`]
    pub fn cached_artifacts(&self) -> &Artifacts<ZkContractArtifact> {
        &self.cached_artifacts
    }

    /// Returns the set of `Artifacts` that were compiled with `zksolc` in
    /// [`crate::Project::compile()`]
    pub fn compiled_artifacts(&self) -> &Artifacts<ZkContractArtifact> {
        &self.compiled_artifacts
    }

    /// Removes the artifact with matching path and name
    pub fn remove(
        &mut self,
        path: impl AsRef<str>,
        contract: impl AsRef<str>,
    ) -> Option<ZkContractArtifact> {
        let contract_path = path.as_ref();
        let contract_name = contract.as_ref();
        if let artifact @ Some(_) = self.compiled_artifacts.remove(contract_path, contract_name) {
            return artifact;
        }
        self.cached_artifacts.remove(contract_path, contract_name)
    }

    /// Removes the _first_ contract with the given name from the set
    pub fn remove_first(&mut self, contract_name: impl AsRef<str>) -> Option<ZkContractArtifact> {
        let contract_name = contract_name.as_ref();
        if let artifact @ Some(_) = self.compiled_artifacts.remove_first(contract_name) {
            return artifact;
        }
        self.cached_artifacts.remove_first(contract_name)
    }

    /// Removes the contract with matching path and name using the `<path>:<contractname>` pattern
    /// where `path` is optional.
    ///
    /// If the `path` segment is `None`, then the first matching `Contract` is returned, see
    /// [Self::remove_first]
    pub fn remove_contract<'a>(
        &mut self,
        info: impl Into<ContractInfoRef<'a>>,
    ) -> Option<ZkContractArtifact> {
        let ContractInfoRef { path, name } = info.into();
        if let Some(path) = path {
            self.remove(path, name)
        } else {
            self.remove_first(name)
        }
    }
}

impl fmt::Display for ProjectCompileOutput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.compiler_output.is_unchanged() {
            f.write_str("Nothing to compile")
        } else {
            self.compiler_output
                .diagnostics(
                    &self.ignored_error_codes,
                    &self.ignored_file_paths,
                    self.compiler_severity_filter,
                )
                .fmt(f)
        }
    }
}

/// The aggregated output of (multiple) compile jobs
///
/// This is effectively a solc version aware `CompilerOutput`
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AggregatedCompilerOutput {
    /// all errors from all `CompilerOutput`
    pub errors: Vec<Error>,
    /// All source files combined with the solc version used to compile them
    pub sources: VersionedSourceFiles,
    /// All compiled contracts combined with the solc version used to compile them
    pub contracts: VersionedContracts,
    // All the `BuildInfo`s of zksolc invocations.
    pub build_infos: Vec<RawBuildInfo<SolcLanguage>>,
}

impl AggregatedCompilerOutput {
    /// Converts all `\\` separators in _all_ paths to `/`
    pub fn slash_paths(&mut self) {
        self.sources.slash_paths();
        self.contracts.slash_paths();
    }

    /// Whether the output contains a compiler error
    ///
    /// This adheres to the given `compiler_severity_filter` and also considers [Error] with the
    /// given [Severity] as errors. For example [Severity::Warning] will consider [Error]s with
    /// [Severity::Warning] and [Severity::Error] as errors.
    pub fn has_error(
        &self,
        ignored_error_codes: &[u64],
        ignored_file_paths: &[PathBuf],
        compiler_severity_filter: &Severity,
    ) -> bool {
        self.errors.iter().any(|err| {
            if err.is_error() {
                // [Severity::Error] is always treated as an error
                return true;
            }
            // check if the filter is set to something higher than the error's severity
            if compiler_severity_filter.ge(&err.severity) {
                if compiler_severity_filter.is_warning() {
                    // skip ignored error codes and file path from warnings
                    let filter = ErrorFilter::new(ignored_error_codes, ignored_file_paths);
                    return self.has_warning(filter);
                }
                return true;
            }
            false
        })
    }

    /// Checks if there are any compiler warnings that are not ignored by the specified error codes
    /// and file paths.
    pub fn has_warning<'a>(&self, filter: impl Into<ErrorFilter<'a>>) -> bool {
        let filter: ErrorFilter<'_> = filter.into();
        self.errors.iter().any(|error| {
            if !error.severity.is_warning() {
                return false;
            }

            let is_code_ignored = filter.is_code_ignored(error.error_code);

            let is_file_ignored = error
                .source_location
                .as_ref()
                .map_or(false, |location| filter.is_file_ignored(Path::new(&location.file)));

            // Only consider warnings that are not ignored by either code or file path.
            // Hence, return `true` for warnings that are not ignored, making the function
            // return `true` if any such warnings exist.
            !(is_code_ignored || is_file_ignored)
        })
    }

    pub fn diagnostics<'a>(
        &'a self,
        ignored_error_codes: &'a [u64],
        ignored_file_paths: &'a [PathBuf],
        compiler_severity_filter: Severity,
    ) -> OutputDiagnostics<'a> {
        OutputDiagnostics {
            compiler_output: self,
            ignored_error_codes,
            ignored_file_paths,
            compiler_severity_filter,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.contracts.is_empty()
    }

    pub fn is_unchanged(&self) -> bool {
        self.contracts.is_empty() && self.errors.is_empty()
    }

    /// adds a new `CompilerOutput` to the aggregated output
    pub fn extend(
        &mut self,
        version: Version,
        build_info: RawBuildInfo<SolcLanguage>,
        output: CompilerOutput,
    ) {
        let build_id = build_info.id.clone();
        self.build_infos.push(build_info);

        let CompilerOutput { errors, sources, contracts, .. } = output;
        self.errors.extend(errors);

        for (path, source_file) in sources {
            let sources = self.sources.as_mut().entry(path).or_default();
            sources.push(VersionedSourceFile {
                source_file,
                version: version.clone(),
                build_id: build_id.clone(),
            });
        }

        for (file_name, new_contracts) in contracts {
            let contracts = self.contracts.as_mut().entry(file_name).or_default();
            for (contract_name, contract) in new_contracts {
                let versioned = contracts.entry(contract_name).or_default();
                versioned.push(VersionedContract {
                    contract,
                    version: version.clone(),
                    build_id: build_id.clone(),
                });
            }
        }
    }

    /// Finds the _first_ contract with the given name
    pub fn find_first(&self, contract: impl AsRef<str>) -> Option<CompactContractRef<'_>> {
        self.contracts.find_first(contract)
    }

    /// Removes the _first_ contract with the given name from the set
    pub fn remove_first(&mut self, contract: impl AsRef<str>) -> Option<Contract> {
        self.contracts.remove_first(contract)
    }

    /// Removes the contract with matching path and name
    pub fn remove(
        &mut self,
        path: impl AsRef<Path>,
        contract: impl AsRef<str>,
    ) -> Option<Contract> {
        self.contracts.remove(path, contract)
    }

    /// Removes the contract with matching path and name using the `<path>:<contractname>` pattern
    /// where `path` is optional.
    ///
    /// If the `path` segment is `None`, then the first matching `Contract` is returned, see
    /// [Self::remove_first]
    pub fn remove_contract<'a>(
        &mut self,
        info: impl Into<ContractInfoRef<'a>>,
    ) -> Option<Contract> {
        let ContractInfoRef { path, name } = info.into();
        if let Some(path) = path {
            self.remove(Path::new(path.as_ref()), name)
        } else {
            self.remove_first(name)
        }
    }

    /// Iterate over all contracts and their names
    pub fn contracts_iter(&self) -> impl Iterator<Item = (&String, &Contract)> {
        self.contracts.contracts()
    }

    /// Iterate over all contracts and their names
    pub fn contracts_into_iter(self) -> impl Iterator<Item = (String, Contract)> {
        self.contracts.into_contracts()
    }

    /// Returns an iterator over (`file`, `name`, `Contract`)
    pub fn contracts_with_files_iter(
        &self,
    ) -> impl Iterator<Item = (&PathBuf, &String, &Contract)> {
        self.contracts.contracts_with_files()
    }

    /// Returns an iterator over (`file`, `name`, `Contract`)
    pub fn contracts_with_files_into_iter(
        self,
    ) -> impl Iterator<Item = (PathBuf, String, Contract)> {
        self.contracts.into_contracts_with_files()
    }

    /// Returns an iterator over (`file`, `name`, `Contract`, `Version`)
    pub fn contracts_with_files_and_version_iter(
        &self,
    ) -> impl Iterator<Item = (&PathBuf, &String, &Contract, &Version)> {
        self.contracts.contracts_with_files_and_version()
    }

    /// Returns an iterator over (`file`, `name`, `Contract`, `Version`)
    pub fn contracts_with_files_and_version_into_iter(
        self,
    ) -> impl Iterator<Item = (PathBuf, String, Contract, Version)> {
        self.contracts.into_contracts_with_files_and_version()
    }

    /// Given the contract file's path and the contract's name, tries to return the contract's
    /// bytecode, runtime bytecode, and ABI.
    pub fn get(
        &self,
        path: impl AsRef<Path>,
        contract: impl AsRef<str>,
    ) -> Option<CompactContractRef<'_>> {
        self.contracts.get(path, contract)
    }

    /// Returns the output's source files and contracts separately, wrapped in helper types that
    /// provide several helper methods
    pub fn split(self) -> (VersionedSourceFiles, VersionedContracts) {
        (self.sources, self.contracts)
    }

    /// Joins all file path with `root`
    pub fn join_all(&mut self, root: impl AsRef<Path>) -> &mut Self {
        let root = root.as_ref();
        self.contracts.join_all(root);
        self.sources.join_all(root);
        self
    }

    /// Strips the given prefix from all file paths to make them relative to the given
    /// `base` argument.
    ///
    /// Convenience method for [Self::strip_prefix_all()] that consumes the type.
    pub fn with_stripped_file_prefixes(mut self, base: impl AsRef<Path>) -> Self {
        let base = base.as_ref();
        self.contracts.strip_prefix_all(base);
        self.sources.strip_prefix_all(base);
        self
    }

    /// Removes `base` from all contract paths
    pub fn strip_prefix_all(&mut self, base: impl AsRef<Path>) -> &mut Self {
        let base = base.as_ref();
        self.contracts.strip_prefix_all(base);
        self.sources.strip_prefix_all(base);
        self
    }
}

/// Helper type to implement display for solc errors
#[derive(Clone, Debug)]
pub struct OutputDiagnostics<'a> {
    /// output of the compiled project
    compiler_output: &'a AggregatedCompilerOutput,
    /// the error codes to ignore
    ignored_error_codes: &'a [u64],
    /// the file paths to ignore
    ignored_file_paths: &'a [PathBuf],
    /// set minimum level of severity that is treated as an error
    compiler_severity_filter: Severity,
}

impl<'a> OutputDiagnostics<'a> {
    /// Returns true if there is at least one error of high severity
    pub fn has_error(&self) -> bool {
        self.compiler_output.has_error(
            self.ignored_error_codes,
            self.ignored_file_paths,
            &self.compiler_severity_filter,
        )
    }

    /// Returns true if there is at least one warning
    pub fn has_warning(&self) -> bool {
        let filter = ErrorFilter::new(self.ignored_error_codes, self.ignored_file_paths);
        self.compiler_output.has_warning(filter)
    }

    /// Returns true if the contract is a expected to be a test
    fn is_test<T: AsRef<str>>(&self, contract_path: T) -> bool {
        if contract_path.as_ref().ends_with(".t.sol") {
            return true;
        }

        self.compiler_output.find_first(&contract_path).map_or(false, |contract| {
            contract.abi.map_or(false, |abi| abi.functions.contains_key("IS_TEST"))
        })
    }
}

impl<'a> fmt::Display for OutputDiagnostics<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Compiler run ")?;
        if self.has_error() {
            Paint::red("failed:")
        } else if self.has_warning() {
            Paint::yellow("successful with warnings:")
        } else {
            Paint::green("successful!")
        }
        .fmt(f)?;

        for err in &self.compiler_output.errors {
            let mut ignored = false;
            if err.severity.is_warning() {
                if let Some(code) = err.error_code {
                    if let Some(source_location) = &err.source_location {
                        // we ignore spdx and contract size warnings in test
                        // files. if we are looking at one of these warnings
                        // from a test file we skip
                        ignored =
                            self.is_test(&source_location.file) && (code == 1878 || code == 5574);

                        // we ignore warnings coming from ignored files
                        let source_path = Path::new(&source_location.file);
                        ignored |= self
                            .ignored_file_paths
                            .iter()
                            .any(|ignored_path| source_path.starts_with(ignored_path));
                    }

                    ignored |= self.ignored_error_codes.contains(&code);
                }
            }

            if !ignored {
                f.write_str("\n")?;
                err.fmt(f)?;
            }
        }

        Ok(())
    }
}

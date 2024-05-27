use crate::{
    artifact_output::{ArtifactFile, Artifacts, ArtifactsMap, OutputContext},
    artifacts::{DevDoc, SourceFile, StorageLayout, UserDoc},
    compile::output::sources::VersionedSourceFiles,
    config::ProjectPathsConfig,
    error::{Result, SolcIoError},
    zksync::compile::output::contracts::VersionedContracts,
};
use alloy_json_abi::JsonAbi;
use foundry_compilers_artifacts::{
    zksolc::{
        bytecode::Bytecode,
        contract::{CompactContractBytecodeCow, Contract},
        Evm,
    },
    SolcLanguage,
};
use path_slash::PathBufExt;
use semver::Version;
use serde::{Deserialize, Serialize};
use std::{
    borrow::Cow,
    collections::{BTreeMap, HashSet},
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ZkContractArtifact {
    pub abi: Option<JsonAbi>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bytecode: Option<Bytecode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assembly: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub method_identifiers: Option<BTreeMap<String, String>>,
    //#[serde(default, skip_serializing_if = "Vec::is_empty")]
    //pub generated_sources: Vec<GeneratedSource>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storage_layout: Option<StorageLayout>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub userdoc: Option<UserDoc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub devdoc: Option<DevDoc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ir_optimized: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub factory_dependencies: Option<BTreeMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub missing_libraries: Option<Vec<String>>,
    /// The identifier of the source file
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<u32>,
}

impl<'a> From<&'a ZkContractArtifact> for CompactContractBytecodeCow<'a> {
    fn from(artifact: &'a ZkContractArtifact) -> Self {
        CompactContractBytecodeCow {
            abi: artifact.abi.as_ref().map(Cow::Borrowed),
            bytecode: artifact.bytecode.as_ref().map(Cow::Borrowed),
        }
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Default)]
pub struct ZkArtifactOutput();

impl ZkArtifactOutput {
    fn contract_to_artifact(
        &self,
        _file: &Path,
        _name: &str,
        contract: Contract,
        source_file: Option<&SourceFile>,
    ) -> ZkContractArtifact {
        let mut artifact_bytecode = None;
        let mut artifact_method_identifiers = None;
        let mut artifact_assembly = None;

        let Contract {
            abi,
            metadata,
            userdoc,
            devdoc,
            storage_layout,
            evm,
            ir_optimized,
            hash,
            factory_dependencies,
            missing_libraries,
        } = contract;

        if let Some(evm) = evm {
            let Evm {
                assembly,
                bytecode,
                method_identifiers,
                extra_metadata: _,
                legacy_assembly: _,
            } = evm;

            artifact_bytecode = bytecode.map(Into::into);
            artifact_method_identifiers = Some(method_identifiers);
            artifact_assembly = assembly;
        }

        ZkContractArtifact {
            abi,
            hash,
            factory_dependencies,
            missing_libraries,
            storage_layout: Some(storage_layout),
            bytecode: artifact_bytecode,
            assembly: artifact_assembly,
            method_identifiers: artifact_method_identifiers,
            metadata,
            userdoc: Some(userdoc),
            devdoc: Some(devdoc),
            ir_optimized,
            id: source_file.as_ref().map(|s| s.id),
        }
    }

    pub fn on_output(
        &self,
        contracts: &VersionedContracts,
        sources: &VersionedSourceFiles,
        layout: &ProjectPathsConfig<SolcLanguage>,
        ctx: OutputContext<'_>,
    ) -> Result<Artifacts<ZkContractArtifact>> {
        let mut artifacts = self.output_to_artifacts(contracts, sources, ctx, layout);
        fs::create_dir_all(&layout.zksync_artifacts).map_err(|err| {
            error!(dir=?layout.zksync_artifacts, "Failed to create artifacts folder");
            SolcIoError::new(err, &layout.zksync_artifacts)
        })?;

        artifacts.join_all(&layout.zksync_artifacts);
        artifacts.write_all()?;

        Ok(artifacts)
    }

    /// Returns the file name for the contract's artifact
    /// `Greeter.json`
    fn output_file_name(name: impl AsRef<str>) -> PathBuf {
        format!("{}.json", name.as_ref()).into()
    }

    /// Returns the file name for the contract's artifact and the given version
    /// `Greeter.0.8.11.json`
    fn output_file_name_versioned(name: impl AsRef<str>, version: &Version) -> PathBuf {
        format!("{}.{}.{}.{}.json", name.as_ref(), version.major, version.minor, version.patch)
            .into()
    }

    /// Returns the appropriate file name for the conflicting file.
    ///
    /// This should ensure that the resulting `PathBuf` is conflict free, which could be possible if
    /// there are two separate contract files (in different folders) that contain the same contract:
    ///
    /// `src/A.sol::A`
    /// `src/nested/A.sol::A`
    ///
    /// Which would result in the same `PathBuf` if only the file and contract name is taken into
    /// account, [`Self::output_file`].
    ///
    /// This return a unique output file
    fn conflict_free_output_file(
        already_taken: &HashSet<String>,
        conflict: PathBuf,
        contract_file: impl AsRef<Path>,
        artifacts_folder: impl AsRef<Path>,
    ) -> PathBuf {
        let artifacts_folder = artifacts_folder.as_ref();
        let mut rel_candidate = conflict;
        if let Ok(stripped) = rel_candidate.strip_prefix(artifacts_folder) {
            rel_candidate = stripped.to_path_buf();
        }
        #[allow(clippy::redundant_clone)] // false positive
        let mut candidate = rel_candidate.clone();
        let contract_file = contract_file.as_ref();
        let mut current_parent = contract_file.parent();

        while let Some(parent_name) = current_parent.and_then(|f| f.file_name()) {
            // this is problematic if both files are absolute
            candidate = Path::new(parent_name).join(&candidate);
            let out_path = artifacts_folder.join(&candidate);
            if !already_taken.contains(&out_path.to_slash_lossy().to_lowercase()) {
                trace!("found alternative output file={:?} for {:?}", out_path, contract_file);
                return out_path;
            }
            current_parent = current_parent.and_then(|f| f.parent());
        }

        // this means we haven't found an alternative yet, which shouldn't actually happen since
        // `contract_file` are unique, but just to be safe, handle this case in which case
        // we simply numerate the parent folder

        trace!("no conflict free output file found after traversing the file");

        let mut num = 1;

        loop {
            // this will attempt to find an alternate path by numerating the first component in the
            // path: `<root>+_<num>/....sol`
            let mut components = rel_candidate.components();
            let first = components.next().expect("path not empty");
            let name = first.as_os_str();
            let mut numerated = OsString::with_capacity(name.len() + 2);
            numerated.push(name);
            numerated.push("_");
            numerated.push(num.to_string());

            let candidate: PathBuf = Some(numerated.as_os_str())
                .into_iter()
                .chain(components.map(|c| c.as_os_str()))
                .collect();
            if !already_taken.contains(&candidate.to_slash_lossy().to_lowercase()) {
                trace!("found alternative output file={:?} for {:?}", candidate, contract_file);
                return candidate;
            }

            num += 1;
        }
    }

    /// Returns the path to the contract's artifact location based on the contract's file and name
    ///
    /// This returns `contract.sol/contract.json` by default
    fn output_file(contract_file: impl AsRef<Path>, name: impl AsRef<str>) -> PathBuf {
        let name = name.as_ref();
        contract_file
            .as_ref()
            .file_name()
            .map(Path::new)
            .map(|p| p.join(Self::output_file_name(name)))
            .unwrap_or_else(|| Self::output_file_name(name))
    }

    /// Returns the path to the contract's artifact location based on the contract's file, name and
    /// version
    ///
    /// This returns `contract.sol/contract.0.8.11.json` by default
    fn output_file_versioned(
        contract_file: impl AsRef<Path>,
        name: impl AsRef<str>,
        version: &Version,
    ) -> PathBuf {
        let name = name.as_ref();
        contract_file
            .as_ref()
            .file_name()
            .map(Path::new)
            .map(|p| p.join(Self::output_file_name_versioned(name, version)))
            .unwrap_or_else(|| Self::output_file_name_versioned(name, version))
    }

    /// Generates a path for an artifact based on already taken paths by either cached or compiled
    /// artifacts.
    fn get_artifact_path(
        ctx: &OutputContext<'_>,
        already_taken: &HashSet<String>,
        file: &Path,
        name: &str,
        artifacts_folder: &Path,
        version: &Version,
        versioned: bool,
    ) -> PathBuf {
        // if an artifact for the contract already exists (from a previous compile job)
        // we reuse the path, this will make sure that even if there are conflicting
        // files (files for witch `T::output_file()` would return the same path) we use
        // consistent output paths
        if let Some(existing_artifact) = ctx.existing_artifact(file, name, version) {
            trace!("use existing artifact file {:?}", existing_artifact,);
            existing_artifact.to_path_buf()
        } else {
            let path = if versioned {
                Self::output_file_versioned(file, name, version)
            } else {
                Self::output_file(file, name)
            };

            let path = artifacts_folder.join(path);

            if already_taken.contains(&path.to_slash_lossy().to_lowercase()) {
                // preventing conflict
                Self::conflict_free_output_file(already_taken, path, file, artifacts_folder)
            } else {
                path
            }
        }
    }

    /// Convert the compiler output into a set of artifacts
    ///
    /// **Note:** This does only convert, but _NOT_ write the artifacts to disk, See
    /// [`Self::on_output()`]
    pub fn output_to_artifacts(
        &self,
        contracts: &VersionedContracts,
        sources: &VersionedSourceFiles,
        ctx: OutputContext<'_>,
        layout: &ProjectPathsConfig<SolcLanguage>,
    ) -> Artifacts<ZkContractArtifact> {
        let mut artifacts = ArtifactsMap::new();

        // this tracks all the `SourceFile`s that we successfully mapped to a contract
        let mut non_standalone_sources = HashSet::new();

        // prepopulate taken paths set with cached artifacts
        let mut taken_paths_lowercase = ctx
            .existing_artifacts
            .values()
            .flat_map(|artifacts| artifacts.values().flat_map(|artifacts| artifacts.values()))
            .map(|a| a.path.to_slash_lossy().to_lowercase())
            .collect::<HashSet<_>>();

        let mut files = contracts.keys().collect::<Vec<_>>();
        // Iterate starting with top-most files to ensure that they get the shortest paths.
        files.sort_by(|file1, file2| {
            (file1.components().count(), file1).cmp(&(file2.components().count(), file2))
        });
        for file in files {
            for (name, versioned_contracts) in &contracts[file] {
                for contract in versioned_contracts {
                    // track `SourceFile`s that can be mapped to contracts
                    let source_file = sources.find_file_and_version(file, &contract.version);

                    if let Some(source) = source_file {
                        non_standalone_sources.insert((source.id, &contract.version));
                    }

                    let artifact_path = Self::get_artifact_path(
                        &ctx,
                        &taken_paths_lowercase,
                        file,
                        name,
                        layout.zksync_artifacts.as_path(),
                        &contract.version,
                        versioned_contracts.len() > 1,
                    );

                    taken_paths_lowercase.insert(artifact_path.to_slash_lossy().to_lowercase());

                    trace!(
                        "use artifact file {:?} for contract file {} {}",
                        artifact_path,
                        file.display(),
                        contract.version
                    );

                    let artifact = self.contract_to_artifact(
                        file,
                        name,
                        contract.contract.clone(),
                        source_file,
                    );

                    let artifact = ArtifactFile {
                        artifact,
                        file: artifact_path,
                        version: contract.version.clone(),
                        build_id: contract.build_id.clone(),
                    };

                    artifacts
                        .entry(file.to_path_buf())
                        .or_default()
                        .entry(name.to_string())
                        .or_default()
                        .push(artifact);
                }
            }
        }
        Artifacts(artifacts)
    }
}

use crate::{
    artifact_output::{ArtifactFile, ArtifactOutput, Artifacts, ArtifactsMap, OutputContext},
    artifacts::{DevDoc, SourceFile, StorageLayout, UserDoc},
    compile::output::sources::VersionedSourceFiles,
    config::ProjectPathsConfig,
    error::{Result, SolcIoError},
    zksync::compile::output::contracts::VersionedContracts,
};
use alloy_json_abi::JsonAbi;
use foundry_compilers_artifacts::{
    solc::{
        CompactBytecode, CompactContract, CompactContractBytecode, CompactContractBytecodeCow,
        CompactDeployedBytecode,
    },
    zksolc::{bytecode::Bytecode, contract::Contract, Evm},
    SolcLanguage,
};
use path_slash::PathBufExt;
use serde::{Deserialize, Serialize};
use std::{
    borrow::Cow,
    collections::{BTreeMap, HashSet},
    fs,
    path::Path,
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

// CompactContract variants
// TODO: for zkEvm, the distinction between bytecode and deployed_bytecode makes little sense,
// and there some fields that the ouptut doesn't provide (e.g: source_map)
// However, we implement these because we get the Artifact trait and can reuse lots of
// the crate's helpers without needing to duplicate everything. Maybe there's a way
// we can get all these without having to add the same bytecode twice on each struct.
// Ideally the Artifacts trait would not be coupled to a specific Contract type
impl<'a> From<&'a ZkContractArtifact> for CompactContractBytecodeCow<'a> {
    fn from(artifact: &'a ZkContractArtifact) -> Self {
        // TODO: artifact.abi might have None, we need to get this field from solc_metadata
        CompactContractBytecodeCow {
            abi: artifact.abi.as_ref().map(Cow::Borrowed),
            bytecode: artifact.bytecode.clone().map(|b| Cow::Owned(CompactBytecode::from(b))),
            deployed_bytecode: artifact
                .bytecode
                .clone()
                .map(|b| Cow::Owned(CompactDeployedBytecode::from(b))),
        }
    }
}

impl From<ZkContractArtifact> for CompactContractBytecode {
    fn from(c: ZkContractArtifact) -> Self {
        Self {
            abi: c.abi.map(Into::into),
            deployed_bytecode: c.bytecode.clone().map(|b| b.into()),
            bytecode: c.bytecode.clone().map(|b| b.into()),
        }
    }
}

impl From<ZkContractArtifact> for CompactContract {
    fn from(c: ZkContractArtifact) -> Self {
        // TODO: c.abi might have None, we need to get this field from solc_metadata
        Self {
            bin: c.bytecode.clone().map(|b| b.object),
            bin_runtime: c.bytecode.clone().map(|b| b.object),
            abi: c.abi,
        }
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Default)]
pub struct ZkArtifactOutput();

impl ArtifactOutput for ZkArtifactOutput {
    type Artifact = ZkContractArtifact;

    fn contract_to_artifact(
        &self,
        _file: &Path,
        _name: &str,
        _contract: foundry_compilers_artifacts::Contract,
        _source_file: Option<&SourceFile>,
    ) -> Self::Artifact {
        panic!("Unsupported use zksync_contract_to_artifact instead");
    }

    fn standalone_source_file_to_artifact(
        &self,
        _path: &Path,
        _file: &crate::VersionedSourceFile,
    ) -> Option<Self::Artifact> {
        None
    }
}

impl ZkArtifactOutput {
    fn zksync_contract_to_artifact(
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

    pub fn zksync_on_output(
        &self,
        contracts: &VersionedContracts,
        sources: &VersionedSourceFiles,
        layout: &ProjectPathsConfig<SolcLanguage>,
        ctx: OutputContext<'_>,
    ) -> Result<Artifacts<ZkContractArtifact>> {
        let mut artifacts = self.zksync_output_to_artifacts(contracts, sources, ctx, layout);
        fs::create_dir_all(&layout.artifacts).map_err(|err| {
            error!(dir=?layout.artifacts, "Failed to create artifacts folder");
            SolcIoError::new(err, &layout.artifacts)
        })?;

        artifacts.join_all(&layout.artifacts);
        artifacts.write_all()?;

        Ok(artifacts)
    }

    /// Convert the compiler output into a set of artifacts
    ///
    /// **Note:** This does only convert, but _NOT_ write the artifacts to disk, See
    /// [`Self::on_output()`]
    pub fn zksync_output_to_artifacts(
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
                        layout.artifacts.as_path(),
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

                    let artifact = self.zksync_contract_to_artifact(
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

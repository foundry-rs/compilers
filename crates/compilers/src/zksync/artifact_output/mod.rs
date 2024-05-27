use std::{
    borrow::Cow,
    path::{Path, PathBuf},
};

use semver::Version;

use crate::{
    artifact_output::{ArtifactId, Artifacts},
    artifacts::bytecode::BytecodeObject,
    zksync::artifact_output::zk::ZkContractArtifact,
};
use foundry_compilers_artifacts::zksolc::{
    bytecode::Bytecode, contract::CompactContractBytecodeCow,
};

pub mod files;
pub mod zk;

pub trait Artifact {
    /// Returns the reference to the `bytecode`
    fn get_bytecode(&self) -> Option<Cow<'_, Bytecode>> {
        self.get_contract_bytecode().bytecode
    }

    /// Returns the reference to the `bytecode` object
    fn get_bytecode_object(&self) -> Option<Cow<'_, BytecodeObject>> {
        let val = match self.get_bytecode()? {
            Cow::Borrowed(b) => Cow::Borrowed(&b.object),
            Cow::Owned(b) => Cow::Owned(b.object),
        };
        Some(val)
    }

    /// Returns the reference of container type for abi, compact bytecode and deployed bytecode if
    /// available
    fn get_contract_bytecode(&self) -> CompactContractBytecodeCow<'_>;
}

impl<T> Artifact for T
where
    for<'a> &'a T: Into<CompactContractBytecodeCow<'a>>,
{
    fn get_contract_bytecode(&self) -> CompactContractBytecodeCow<'_> {
        self.into()
    }
}

// solc Artifacts overrides (for methods that require the
// `ArtifactOutput` trait)

/// Returns an iterator over _all_ artifacts and `<file name:contract name>`.
pub fn artifacts_artifacts(
    artifacts: &Artifacts<ZkContractArtifact>,
) -> impl Iterator<Item = (ArtifactId, &ZkContractArtifact)> + '_ {
    artifacts.0.iter().flat_map(|(file, contract_artifacts)| {
        contract_artifacts.iter().flat_map(move |(_contract_name, artifacts)| {
            let source = file;
            artifacts.iter().filter_map(move |artifact| {
                contract_name(&artifact.file).map(|name| {
                    (
                        ArtifactId {
                            path: PathBuf::from(&artifact.file),
                            name,
                            source: source.clone(),
                            version: artifact.version.clone(),
                            build_id: artifact.build_id.clone(),
                        }
                        .with_slashed_paths(),
                        &artifact.artifact,
                    )
                })
            })
        })
    })
}

pub fn artifacts_into_artifacts(
    artifacts: Artifacts<ZkContractArtifact>,
) -> impl Iterator<Item = (ArtifactId, ZkContractArtifact)> {
    artifacts.0.into_iter().flat_map(|(file, contract_artifacts)| {
        contract_artifacts.into_iter().flat_map(move |(_contract_name, artifacts)| {
            let source = PathBuf::from(file.clone());
            artifacts.into_iter().filter_map(move |artifact| {
                contract_name(&artifact.file).map(|name| {
                    (
                        ArtifactId {
                            path: PathBuf::from(&artifact.file),
                            name,
                            build_id: artifact.build_id,
                            source: source.clone(),
                            version: artifact.version,
                        }
                        .with_slashed_paths(),
                        artifact.artifact,
                    )
                })
            })
        })
    })
}

// ArtifactOutput trait methods that don't require self are
// defined as standalone functions here (We don't redefine the
// trait for zksolc)

/// Returns the file name for the contract's artifact
/// `Greeter.json`
fn output_file_name(name: impl AsRef<str>) -> PathBuf {
    format!("{}.json", name.as_ref()).into()
}

/// Returns the file name for the contract's artifact and the given version
/// `Greeter.0.8.11.json`
fn output_file_name_versioned(name: impl AsRef<str>, version: &Version) -> PathBuf {
    format!("{}.{}.{}.{}.json", name.as_ref(), version.major, version.minor, version.patch).into()
}

/// Returns the path to the contract's artifact location based on the contract's file and name
///
/// This returns `contract.sol/contract.json` by default
pub fn output_file(contract_file: impl AsRef<Path>, name: impl AsRef<str>) -> PathBuf {
    let name = name.as_ref();
    contract_file
        .as_ref()
        .file_name()
        .map(Path::new)
        .map(|p| p.join(output_file_name(name)))
        .unwrap_or_else(|| output_file_name(name))
}

/// Returns the path to the contract's artifact location based on the contract's file, name and
/// version
///
/// This returns `contract.sol/contract.0.8.11.json` by default
pub fn output_file_versioned(
    contract_file: impl AsRef<Path>,
    name: impl AsRef<str>,
    version: &Version,
) -> PathBuf {
    let name = name.as_ref();
    contract_file
        .as_ref()
        .file_name()
        .map(Path::new)
        .map(|p| p.join(output_file_name_versioned(name, version)))
        .unwrap_or_else(|| output_file_name_versioned(name, version))
}

pub fn contract_name(file: impl AsRef<Path>) -> Option<String> {
    file.as_ref().file_stem().and_then(|s| s.to_str().map(|s| s.to_string()))
}

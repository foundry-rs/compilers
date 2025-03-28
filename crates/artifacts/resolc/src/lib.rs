//! Resolc artifact types.

use std::{collections::BTreeMap, path::PathBuf};

pub mod contract;
use contract::ResolcContract;
use foundry_compilers_artifacts_solc::{
    Bytecode, DeployedBytecode, Error, FileToContractsMap, SourceFile,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
pub struct ResolcCompilerOutput {
    /// The file-contract hashmap.
    #[serde(default)]
    pub contracts: FileToContractsMap<ResolcContract>,
    /// The source code mapping data.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub sources: BTreeMap<PathBuf, SourceFile>,
    /// The compilation errors and warnings.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<Error>,
    /// The `solc` compiler version.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// The `solc` compiler long version.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub long_version: Option<String>,
    /// The `resolc` compiler version.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revive_version: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RecursiveFunction {
    /// The function name.
    pub name: String,
    /// The creation code function block tag.
    pub creation_tag: Option<usize>,
    /// The runtime code function block tag.
    pub runtime_tag: Option<usize>,
    /// The number of input arguments.
    #[serde(rename = "totalParamSize")]
    pub input_size: usize,
    /// The number of output arguments.
    #[serde(rename = "totalRetParamSize")]
    pub output_size: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct ResolcEVM {
    /// The contract PolkaVM assembly code.
    #[serde(rename = "assembly", skip_serializing_if = "Option::is_none")]
    pub assembly_text: Option<String>,
    /// The contract bytecode.
    /// Is reset by that of PolkaVM before yielding the compiled project artifacts.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytecode: Option<Bytecode>,
    /// The deployed bytecode of the contract.
    /// It is overwritten with the PolkaVM blob before yielding the compiled project artifacts.
    /// Hence it will be the same as the runtime code but we keep both for compatibility reasons.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deployed_bytecode: Option<Bytecode>,
    /// The contract function signatures.
    #[serde(default, skip_serializing_if = "::std::collections::BTreeMap::is_empty")]
    pub method_identifiers: BTreeMap<String, String>,
}

impl From<ResolcEVM> for foundry_compilers_artifacts_solc::Evm {
    fn from(evm: ResolcEVM) -> Self {
        Self {
            bytecode: evm.bytecode.clone(),
            deployed_bytecode: Some(DeployedBytecode {
                bytecode: evm.deployed_bytecode.or(evm.bytecode),
                immutable_references: BTreeMap::new(),
            }),
            method_identifiers: evm.method_identifiers,
            assembly: evm.assembly_text,
            legacy_assembly: None,
            gas_estimates: None,
        }
    }
}

pub type ResolcContracts = FileToContractsMap<ResolcContract>;

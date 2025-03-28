//! Resolc artifact types.

use std::{collections::BTreeMap, path::PathBuf};

pub mod contract;
use alloy_primitives::{hex::FromHex, Bytes};
use contract::ResolcContract;
use foundry_compilers_artifacts_solc::{
    Bytecode, BytecodeObject, DeployedBytecode, Error, FileToContractsMap, SourceFile,
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
#[derive(Debug, Default, Serialize, Deserialize, Clone, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExtraMetadata {
    /// The list of recursive functions.
    #[serde(default = "Vec::new")]
    pub recursive_functions: Vec<RecursiveFunction>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct ResolcEVM {
    /// The contract EVM legacy assembly code.
    #[serde(rename = "legacyAssembly", skip_serializing_if = "Option::is_none")]
    pub assembly: Option<serde_json::Value>,
    /// The contract PolkaVM assembly code.
    #[serde(rename = "assembly", skip_serializing_if = "Option::is_none")]
    pub assembly_text: Option<String>,
    /// The contract bytecode.
    /// Is reset by that of PolkaVM before yielding the compiled project artifacts.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytecode: Option<ResolcBytecode>,
    /// The deployed bytecode of the contract.
    /// It is overwritten with the PolkaVM blob before yielding the compiled project artifacts.
    /// Hence it will be the same as the runtime code but we keep both for compatibility reasons.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deployed_bytecode: Option<ResolcBytecode>,
    /// The contract function signatures.
    #[serde(default, skip_serializing_if = "::std::collections::BTreeMap::is_empty")]
    pub method_identifiers: BTreeMap<String, String>,
    /// The extra EVMLA metadata.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extra_metadata: Option<ExtraMetadata>,
}

/// The `solc --standard-json` output contract EVM deployed bytecode.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ResolcBytecode {
    /// The bytecode object.
    pub object: String,
}

impl ResolcBytecode {
    /// A shortcut constructor.
    pub fn new(object: String) -> Self {
        Self { object }
    }
}

impl From<ResolcBytecode> for Bytecode {
    fn from(value: ResolcBytecode) -> Self {
        let object = Bytes::from_hex(value.object).expect("Value wasn't correctly encoded");
        Self {
            function_debug_data: BTreeMap::new(),
            object: BytecodeObject::Bytecode(object),
            opcodes: None,
            source_map: None,
            generated_sources: vec![],
            link_references: BTreeMap::new(),
        }
    }
}

impl From<ResolcEVM> for foundry_compilers_artifacts_solc::Evm {
    fn from(evm: ResolcEVM) -> Self {
        Self {
            bytecode: evm.bytecode.clone().map(Into::into),
            deployed_bytecode: Some(DeployedBytecode {
                bytecode: evm.deployed_bytecode.or(evm.bytecode).map(Into::into),
                immutable_references: BTreeMap::new(),
            }),
            method_identifiers: evm.method_identifiers,
            assembly: evm.assembly_text,
            legacy_assembly: evm.assembly,
            gas_estimates: None,
        }
    }
}

pub type ResolcContracts = FileToContractsMap<ResolcContract>;

//! Contract related types.
use crate::Evm;
use alloy_json_abi::JsonAbi;
use foundry_compilers_artifacts_solc::{
    CompactContractBytecode, CompactContractBytecodeCow, CompactContractRef, DevDoc, StorageLayout,
    UserDoc,
};
use serde::{Deserialize, Serialize};
use std::{borrow::Cow, collections::BTreeMap};

/// Represents a compiled solidity contract
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Contract {
    pub abi: Option<JsonAbi>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
    #[serde(default)]
    pub userdoc: UserDoc,
    #[serde(default)]
    pub devdoc: DevDoc,
    /// The contract optimized IR code.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ir_optimized: Option<String>,
    /// The contract storage layout.
    #[serde(default, skip_serializing_if = "StorageLayout::is_empty")]
    pub storage_layout: StorageLayout,
    /// The contract EraVM bytecode hash.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,
    /// The contract factory dependencies.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub factory_dependencies: Option<BTreeMap<String, String>>,
    /// The contract missing libraries.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub missing_libraries: Option<Vec<String>>,
    /// EVM-related outputs
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evm: Option<Evm>,
}

// CompactContract variants
// TODO: for zkEvm, the distinction between bytecode and deployed_bytecode makes little sense,
// and there some fields that the ouptut doesn't provide (e.g: source_map)
// However, we implement these because we get the Artifact trait and can reuse lots of
// the crate's helpers without needing to duplicate everything. Maybe there's a way
// we can get all these without having to add the same bytecode twice on each struct.
// Ideally the Artifacts trait would not be coupled to a specific Contract type
impl<'a> From<&'a Contract> for CompactContractBytecodeCow<'a> {
    fn from(artifact: &'a Contract) -> Self {
        let (bytecode, deployed_bytecode) = if let Some(ref evm) = artifact.evm {
            (
                evm.bytecode.clone().map(Into::into).map(Cow::Owned),
                evm.bytecode.clone().map(Into::into).map(Cow::Owned),
            )
        } else {
            (None, None)
        };
        CompactContractBytecodeCow {
            abi: artifact.abi.as_ref().map(Cow::Borrowed),
            bytecode,
            deployed_bytecode,
        }
    }
}

impl From<Contract> for CompactContractBytecode {
    fn from(c: Contract) -> Self {
        let bytecode = if let Some(evm) = c.evm { evm.bytecode } else { None };
        Self {
            abi: c.abi.map(Into::into),
            deployed_bytecode: bytecode.clone().map(|b| b.into()),
            bytecode: bytecode.clone().map(|b| b.into()),
        }
    }
}

impl<'a> From<&'a Contract> for CompactContractRef<'a> {
    fn from(c: &'a Contract) -> Self {
        let (bin, bin_runtime) = if let Some(ref evm) = c.evm {
            (evm.bytecode.as_ref().map(|c| &c.object), evm.bytecode.as_ref().map(|c| &c.object))
        } else {
            (None, None)
        };

        Self { abi: c.abi.as_ref(), bin, bin_runtime }
    }
}

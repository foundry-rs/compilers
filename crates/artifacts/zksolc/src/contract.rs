//! Contract related types.
use crate::{bytecode::Bytecode, Evm};
use alloy_json_abi::JsonAbi;
use alloy_primitives::Bytes;
use foundry_compilers_artifacts_solc::{bytecode::BytecodeObject, DevDoc, StorageLayout, UserDoc};
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

/// Helper type to serialize while borrowing from `Contract`
#[derive(Copy, Clone, Debug, Serialize)]
pub struct CompactContractRef<'a> {
    pub abi: Option<&'a JsonAbi>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bin: Option<&'a BytecodeObject>,
}

impl<'a> CompactContractRef<'a> {
    /// Clones the referenced values and returns as tuples
    pub fn into_parts(self) -> (Option<JsonAbi>, Option<Bytes>) {
        CompactContract::from(self).into_parts()
    }

    /// Returns the individual parts of this contract.
    ///
    /// If the values are `None`, then `Default` is returned.
    pub fn into_parts_or_default(self) -> (JsonAbi, Bytes) {
        CompactContract::from(self).into_parts_or_default()
    }

    pub fn bytecode(&self) -> Option<&Bytes> {
        self.bin.as_ref().and_then(|bin| bin.as_bytes())
    }
}

impl<'a> From<&'a Contract> for CompactContractRef<'a> {
    fn from(c: &'a Contract) -> Self {
        let bin =
            if let Some(ref evm) = c.evm { evm.bytecode.as_ref().map(|c| &c.object) } else { None };

        Self { abi: c.abi.as_ref(), bin }
    }
}

/// The general purpose minimal representation of a contract's abi with bytecode
/// Unlike `CompactContractSome` all fields are optional so that every possible compiler output can
/// be represented by it
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
pub struct CompactContract {
    /// The Ethereum Contract ABI. If empty, it is represented as an empty
    /// array. See <https://docs.soliditylang.org/en/develop/abi-spec.html>
    pub abi: Option<JsonAbi>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bin: Option<BytecodeObject>,
}

impl CompactContract {
    /// Returns the contents of this type as a single tuple of abi, bytecode and deployed bytecode
    pub fn into_parts(self) -> (Option<JsonAbi>, Option<Bytes>) {
        (self.abi, self.bin.and_then(|bin| bin.into_bytes()))
    }

    /// Returns the individual parts of this contract.
    ///
    /// If the values are `None`, then `Default` is returned.
    pub fn into_parts_or_default(self) -> (JsonAbi, Bytes) {
        (
            self.abi.unwrap_or_default(),
            self.bin.and_then(|bin| bin.into_bytes()).unwrap_or_default(),
        )
    }
}

impl<'a> From<CompactContractRef<'a>> for CompactContract {
    fn from(c: CompactContractRef<'a>) -> Self {
        Self { abi: c.abi.cloned(), bin: c.bin.cloned() }
    }
}

/// A [CompactContractBytecode] that is either owns or borrows its content
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CompactContractBytecodeCow<'a> {
    pub abi: Option<Cow<'a, JsonAbi>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bytecode: Option<Cow<'a, Bytecode>>,
}

impl<'a> From<&'a Contract> for CompactContractBytecodeCow<'a> {
    fn from(artifact: &'a Contract) -> Self {
        let bytecode = if let Some(ref evm) = artifact.evm {
            evm.bytecode.clone().map(Into::into).map(Cow::Owned)
        } else {
            None
        };
        CompactContractBytecodeCow { abi: artifact.abi.as_ref().map(Cow::Borrowed), bytecode }
    }
}

use std::collections::{BTreeMap, HashSet};

use alloy_json_abi::JsonAbi;
use foundry_compilers_artifacts_solc::{DevDoc, LosslessMetadata, StorageLayout, UserDoc};
use serde::{Deserialize, Serialize};

use crate::ResolcEVM;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct ResolcContract {
    /// The contract ABI.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub abi: Option<JsonAbi>,
    /// The contract metadata.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
    /// The contract developer documentation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub devdoc: Option<DevDoc>,
    /// The contract user documentation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub userdoc: Option<UserDoc>,
    /// The contract storage layout.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storage_layout: Option<StorageLayout>,
    /// Contract's bytecode and related objects
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evm: Option<ResolcEVM>,
    /// The contract IR code.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ir: Option<String>,
    /// The contract optimized IR code.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ir_optimized: Option<String>,
    /// The contract PolkaVM bytecode hash.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,
    /// The contract factory dependencies.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub factory_dependencies: Option<BTreeMap<String, String>>,
    /// The contract missing libraries.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub missing_libraries: Option<HashSet<String>>,
}

impl From<ResolcContract> for foundry_compilers_artifacts_solc::Contract {
    fn from(contract: ResolcContract) -> Self {
        let meta = match contract.metadata.as_ref() {
            Some(meta) => match meta {
                serde_json::Value::Object(map) => {
                    if let Some(meta) = map.get("solc_metadata") {
                        serde_json::from_value::<LosslessMetadata>(meta.clone()).ok()
                    } else {
                        None
                    }
                }
                _ => None,
            },
            None => Default::default(),
        };

        Self {
            abi: contract.abi,
            evm: contract.evm.map(Into::into),
            metadata: meta,
            userdoc: contract.userdoc.unwrap_or_default(),
            devdoc: contract.devdoc.unwrap_or_default(),
            ir: contract.ir,
            storage_layout: contract.storage_layout.unwrap_or_default(),
            transient_storage_layout: Default::default(),
            ewasm: None,
            ir_optimized: contract.ir_optimized,
            ir_optimized_ast: None,
        }
    }
}

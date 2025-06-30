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
            Some(serde_json::Value::Object(map)) => {
                map.get("solc_metadata")
                    .and_then(|solc_metadata| {
                        serde_json::from_value::<LosslessMetadata>(solc_metadata.clone()).ok()
                    })
                    .map(|mut solc_metadata| {
                        // Extract and inject revive compiler information if available.
                        if let Some(revive_version) =
                            map.get("revive_version").and_then(|v| v.as_str())
                        {
                            solc_metadata.metadata.compiler.additional_information.insert(
                                "revive_version".to_string(),
                                serde_json::Value::String(revive_version.to_string()),
                            );

                            solc_metadata.raw_metadata =
                                serde_json::to_string(&solc_metadata.metadata).unwrap();
                        }
                        solc_metadata
                    })
            }
            _ => None,
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn make_solc_metadata(version: &str, revive_version: Option<&str>) -> serde_json::Value {
        let meta = json!({
            "compiler": { "version": version },
            "language": "Solidity",
            "output": { "abi": [], "evm": { "bytecode": { "object": "0x" } } },
            "settings": { "compilationTarget": {}, "evmVersion": "paris", "libraries": {}, "metadata": {}, "optimizer": {}, "remappings": [] },
            "sources": {},
            "version": 1
        });
        let solc_metadata_string = serde_json::to_string(&meta).unwrap();
        match revive_version {
            Some(revive) => {
                json!({ "solc_metadata": solc_metadata_string, "revive_version": revive })
            }
            None => json!({ "solc_metadata": solc_metadata_string }),
        }
    }

    fn make_contract(metadata: Option<serde_json::Value>) -> ResolcContract {
        ResolcContract {
            abi: Some(JsonAbi::default()),
            metadata,
            devdoc: Some(DevDoc::default()),
            userdoc: Some(UserDoc::default()),
            storage_layout: Some(StorageLayout::default()),
            evm: Some(ResolcEVM::default()),
            ir: Some("test_ir".to_string()),
            ir_optimized: Some("test_ir_optimized".to_string()),
            hash: Some("test_hash".to_string()),
            factory_dependencies: None,
            missing_libraries: None,
        }
    }

    #[test]
    fn conversion_without_metadata() {
        let contract = make_contract(None);
        let solc_contract: foundry_compilers_artifacts_solc::Contract = contract.into();

        assert!(solc_contract.metadata.is_none());
    }

    #[test]
    fn conversion_with_metadata_and_versions() {
        // No revive_version
        let contract = make_contract(Some(make_solc_metadata("0.8.19", None)));
        let solc_contract: foundry_compilers_artifacts_solc::Contract = contract.into();

        assert_eq!(solc_contract.metadata.as_ref().unwrap().metadata.compiler.version, "0.8.19");

        assert!(!solc_contract
            .metadata
            .as_ref()
            .unwrap()
            .metadata
            .compiler
            .additional_information
            .contains_key("revive_version"));

        // With revive_version
        let contract = make_contract(Some(make_solc_metadata("0.8.19", Some("0.1.0-dev.13"))));
        let solc_contract: foundry_compilers_artifacts_solc::Contract = contract.into();
        let meta = solc_contract.metadata.unwrap();

        assert_eq!(meta.metadata.compiler.version, "0.8.19");
        assert_eq!(
            meta.metadata.compiler.additional_information["revive_version"],
            json!("0.1.0-dev.13")
        );
    }

    #[test]
    fn conversion_with_different_compiler_versions() {
        let contract1 = make_contract(Some(make_solc_metadata("0.8.19", Some("0.1.0-dev.13"))));
        let contract2 = make_contract(Some(make_solc_metadata("0.8.25", Some("0.1.0-dev.15"))));

        let solc_contract1: foundry_compilers_artifacts_solc::Contract = contract1.into();
        let solc_contract2: foundry_compilers_artifacts_solc::Contract = contract2.into();

        assert_eq!(solc_contract1.metadata.as_ref().unwrap().metadata.compiler.version, "0.8.19");
        assert_eq!(solc_contract2.metadata.as_ref().unwrap().metadata.compiler.version, "0.8.25");
    }
}

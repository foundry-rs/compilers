use std::collections::BTreeMap;

use foundry_compilers_artifacts_solc::{
    bytecode::{serialize_bytecode_without_prefix, BytecodeObject},
    CompactBytecode, CompactDeployedBytecode,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Bytecode {
    #[serde(serialize_with = "serialize_bytecode_without_prefix")]
    pub object: BytecodeObject,
}

// NOTE: distinction between bytecode and deployed bytecode make no sense of zkEvm, but
// we implement these conversions in order to be able to use the Artifacts trait.
impl From<Bytecode> for CompactBytecode {
    fn from(bcode: Bytecode) -> Self {
        Self { object: bcode.object, source_map: None, link_references: BTreeMap::default() }
    }
}

impl From<Bytecode> for CompactDeployedBytecode {
    fn from(bcode: Bytecode) -> Self {
        Self { bytecode: Some(bcode.into()), immutable_references: BTreeMap::default() }
    }
}

use crate::{
    artifacts::{output_selection::OutputSelection, serde_helpers},
    compilers::CompilerSettings,
    EvmVersion,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Clone, Deserialize, Eq, PartialEq)]
pub enum VyperOptimizationMode {
    Gas,
    Codesize,
    None,
}

#[derive(Debug, Serialize, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VyperSettings {
    #[serde(
        default,
        with = "serde_helpers::display_from_str_opt",
        skip_serializing_if = "Option::is_none"
    )]
    pub evm_version: Option<EvmVersion>,
    /// Optimization mode
    pub optimize: Option<VyperOptimizationMode>,
    /// Whether or not the bytecode should include Vyper's signature
    pub bytecode_metadata: Option<bool>,
    pub output_selection: OutputSelection,
}

impl CompilerSettings for VyperSettings {
    fn output_selection_mut(&mut self) -> &mut OutputSelection {
        &mut self.output_selection
    }

    fn can_use_cached(&self, other: &Self) -> bool {
        let Self { evm_version, optimize, bytecode_metadata, output_selection } = self;
        return evm_version == &other.evm_version
            && optimize == &other.optimize
            && bytecode_metadata == &other.bytecode_metadata
            && output_selection.is_subset_of(&other.output_selection);
    }
}

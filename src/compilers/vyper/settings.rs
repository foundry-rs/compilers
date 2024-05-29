use std::{collections::BTreeMap, path::Path};

use crate::{
    artifacts::{
        output_selection::{FileOutputSelection, OutputSelection},
        serde_helpers,
    },
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

#[derive(Debug, Serialize, Clone, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct VyperSettings {
    #[serde(
        default,
        with = "serde_helpers::display_from_str_opt",
        skip_serializing_if = "Option::is_none"
    )]
    pub evm_version: Option<EvmVersion>,
    /// Optimization mode
    #[serde(skip_serializing_if = "Option::is_none")]
    pub optimize: Option<VyperOptimizationMode>,
    /// Whether or not the bytecode should include Vyper's signature
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytecode_metadata: Option<bool>,
    pub output_selection: OutputSelection,
}

impl VyperSettings {
    pub fn strip_prefix(&mut self, base: impl AsRef<Path>) {
        let base = base.as_ref();

        self.output_selection = OutputSelection(
            std::mem::take(&mut self.output_selection.0)
                .into_iter()
                .map(|(file, selection)| {
                    (
                        Path::new(&file)
                            .strip_prefix(base)
                            .map(|p| format!("{}", p.display()))
                            .unwrap_or(file),
                        selection,
                    )
                })
                .collect(),
        );
    }
}

impl CompilerSettings for VyperSettings {
    fn update_output_selection(&mut self, f: impl FnOnce(&mut OutputSelection)) {
        f(&mut self.output_selection)
    }

    fn can_use_cached(&self, other: &Self) -> bool {
        let Self { evm_version, optimize, bytecode_metadata, output_selection } = self;
        evm_version == &other.evm_version
            && optimize == &other.optimize
            && bytecode_metadata == &other.bytecode_metadata
            && output_selection.is_subset_of(&other.output_selection)
    }

    fn minimal_output_selection() -> FileOutputSelection {
        // Vyper throws an error if empty selection is specified, so we are only requesting ABI.
        BTreeMap::from([("*".to_string(), vec!["abi".to_string()])])
    }
}

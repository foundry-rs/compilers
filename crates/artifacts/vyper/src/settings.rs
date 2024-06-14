use foundry_compilers_artifacts_solc::{
    output_selection::OutputSelection, serde_helpers, EvmVersion,
};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Serialize, Clone, Copy, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
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

    /// During caching we prune output selection for some of the sources, however, Vyper will reject
    /// [] as an output selection, so we are adding "abi" as a default output selection which is
    /// cheap to be produced.
    pub fn sanitize_output_selection(&mut self) {
        self.output_selection.0.values_mut().for_each(|selection| {
            selection.values_mut().for_each(|selection| {
                if selection.is_empty() {
                    selection.push("abi".to_string())
                }
            })
        });
    }
}

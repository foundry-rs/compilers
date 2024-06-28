use foundry_compilers_artifacts_solc::{
    output_selection::OutputSelection, serde_helpers, EvmVersion,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VyperOptimizationMode {
    Gas,
    Codesize,
    None,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
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
    #[serde(rename = "search_paths", skip_serializing_if = "Option::is_none")]
    pub search_paths: Option<BTreeSet<PathBuf>>,
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
        self.search_paths = self.search_paths.as_ref().map(|paths| {
            paths.iter().map(|p| p.strip_prefix(base).unwrap_or(p.as_path()).into()).collect()
        });
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

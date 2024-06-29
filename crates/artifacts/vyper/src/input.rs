use super::VyperSettings;
use foundry_compilers_artifacts_solc::sources::Sources;
use foundry_compilers_core::utils::strip_prefix_owned;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Extension of Vyper interface file.
pub const VYPER_INTERFACE_EXTENSION: &str = "vyi";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VyperInput {
    pub language: String,
    pub sources: Sources,
    pub interfaces: Sources,
    pub settings: VyperSettings,
}

impl VyperInput {
    pub fn new(sources: Sources, mut settings: VyperSettings) -> Self {
        let mut new_sources = Sources::new();
        let mut interfaces = Sources::new();

        for (path, content) in sources {
            if path.extension().map_or(false, |ext| ext == VYPER_INTERFACE_EXTENSION) {
                // Interface .vyi files should be removed from the output selection.
                settings.output_selection.0.remove(path.to_string_lossy().as_ref());
                interfaces.insert(path, content);
            } else {
                new_sources.insert(path, content);
            }
        }

        settings.sanitize_output_selection();
        Self { language: "Vyper".to_string(), sources: new_sources, interfaces, settings }
    }

    pub fn strip_prefix(&mut self, base: &Path) {
        self.sources = std::mem::take(&mut self.sources)
            .into_iter()
            .map(|(path, s)| (strip_prefix_owned(path, base), s))
            .collect();

        self.interfaces = std::mem::take(&mut self.interfaces)
            .into_iter()
            .map(|(path, s)| (strip_prefix_owned(path, base), s))
            .collect();

        self.settings.strip_prefix(base)
    }
}

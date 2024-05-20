use std::path::Path;

use super::settings::VyperSettings;
use crate::{artifacts::Sources, compilers::CompilerInput};
use semver::Version;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct VyperInput {
    pub language: String,
    pub sources: Sources,
    pub settings: VyperSettings,
}

impl CompilerInput for VyperInput {
    type Settings = VyperSettings;

    fn build(sources: Sources, settings: Self::Settings, _version: &Version) -> Vec<Self> {
        vec![VyperInput { language: "Vyper".to_string(), sources, settings }]
    }

    fn sources(&self) -> &Sources {
        &self.sources
    }

    fn compiler_name(&self) -> String {
        "Vyper".to_string()
    }

    fn strip_prefix(&mut self, base: &Path) {
        self.sources = std::mem::take(&mut self.sources)
            .into_iter()
            .map(|(path, s)| (path.strip_prefix(base).map(Into::into).unwrap_or(path), s))
            .collect();

        self.settings.strip_prefix(base)
    }
}

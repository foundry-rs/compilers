use std::path::Path;

use super::{settings::VyperSettings, VyperLanguage};
use crate::{artifacts::Sources, compilers::CompilerInput};
use semver::Version;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct VyperInput {
    pub language: String,
    pub sources: Sources,
    pub settings: VyperSettings,
}

#[derive(Debug, Serialize)]
pub struct VyperVersionedInput {
    #[serde(flatten)]
    pub input: VyperInput,
    #[serde(skip)]
    pub version: Version,
}

impl VyperInput {
    pub fn new(sources: Sources, settings: VyperSettings) -> Self {
        VyperInput { language: "Vyper".to_string(), sources, settings }
    }

    pub fn strip_prefix(&mut self, base: &Path) {
        self.sources = std::mem::take(&mut self.sources)
            .into_iter()
            .map(|(path, s)| (path.strip_prefix(base).map(Into::into).unwrap_or(path), s))
            .collect();

        self.settings.strip_prefix(base)
    }
}

impl CompilerInput for VyperVersionedInput {
    type Settings = VyperSettings;
    type Language = VyperLanguage;

    fn build(
        sources: Sources,
        settings: Self::Settings,
        _language: Self::Language,
        version: Version,
    ) -> Self {
        Self { input: VyperInput::new(sources, settings), version }
    }

    fn sources(&self) -> &Sources {
        &self.input.sources
    }

    fn compiler_name(&self) -> String {
        "Vyper".to_string()
    }

    fn strip_prefix(&mut self, base: &Path) {
        self.input.strip_prefix(base);
    }

    fn language(&self) -> Self::Language {
        VyperLanguage
    }

    fn version(&self) -> &Version {
        &self.version
    }
}

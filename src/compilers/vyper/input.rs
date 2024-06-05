use std::{borrow::Cow, path::Path};

use super::{settings::VyperSettings, VyperLanguage, VYPER_INTERFACE_EXTENSION};
use crate::{artifacts::Sources, compilers::CompilerInput};
use semver::Version;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VyperInput {
    pub language: String,
    pub sources: Sources,
    pub interfaces: Sources,
    pub settings: VyperSettings,
}

#[derive(Debug, Clone, Serialize)]
pub struct VyperVersionedInput {
    #[serde(flatten)]
    pub input: VyperInput,
    #[serde(skip)]
    pub version: Version,
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
        VyperInput { language: "Vyper".to_string(), sources: new_sources, interfaces, settings }
    }

    pub fn strip_prefix(&mut self, base: &Path) {
        self.sources = std::mem::take(&mut self.sources)
            .into_iter()
            .map(|(path, s)| (path.strip_prefix(base).map(Into::into).unwrap_or(path), s))
            .collect();

        self.interfaces = std::mem::take(&mut self.interfaces)
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

    fn compiler_name(&self) -> Cow<'static, str> {
        "Vyper".into()
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

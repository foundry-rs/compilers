use foundry_compilers_artifacts::{SolcLanguage, Source, Sources};
use foundry_compilers_core::utils::strip_prefix_owned;
use semver::Version;
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, path::Path};

use crate::{
    solc::{SolcSettings, SolcVersionedInput},
    CompilerInput, CompilerSettings,
};

#[derive(Debug, Clone, Serialize)]
pub struct ResolcVersionedInput {
    #[serde(flatten)]
    pub input: ResolcInput,
    pub solc_version: Version,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolcInput {
    pub language: SolcLanguage,
    pub sources: Sources,
    pub settings: SolcSettings,
}

impl Default for ResolcInput {
    fn default() -> Self {
        Self {
            language: SolcLanguage::Solidity,
            sources: Sources::default(),
            settings: SolcSettings::default(),
        }
    }
}

impl From<SolcVersionedInput> for ResolcVersionedInput {
    fn from(value: SolcVersionedInput) -> Self {
        Self::build(
            value.input.sources,
            SolcSettings { settings: value.input.settings, cli_settings: value.cli_settings },
            value.input.language,
            value.version,
        )
    }
}

impl CompilerInput for ResolcVersionedInput {
    type Settings = SolcSettings;
    type Language = SolcLanguage;

    fn build(
        sources: Sources,
        settings: Self::Settings,
        language: Self::Language,
        version: Version,
    ) -> Self {
        let hash_set = HashSet::from([
            "abi",
            "metadata",
            "devdoc",
            "userdoc",
            "evm.methodIdentifiers",
            "storageLayout",
            "ast",
            "irOptimized",
            "evm.legacyAssembly",
            "evm.bytecode",
            "evm.deployedBytecode",
            "evm.assembly",
            "ir",
        ]);
        let solc_settings = settings.settings.sanitized(&version, language);

        let mut settings =
            Self::Settings { settings: solc_settings, cli_settings: settings.cli_settings };
        settings.update_output_selection(|selection| {
            for (_, key) in selection.0.iter_mut() {
                for (_, value) in key.iter_mut() {
                    value.retain(|item| hash_set.contains(item.as_str()));
                }
            }
        });
        let input = ResolcInput::new(language, sources, settings);
        Self { input, solc_version: version }
    }

    fn language(&self) -> Self::Language {
        self.input.language
    }

    fn version(&self) -> &Version {
        &self.solc_version
    }

    fn sources(&self) -> impl Iterator<Item = (&Path, &Source)> {
        self.input.sources.iter().map(|(path, source)| (path.as_path(), source))
    }

    fn strip_prefix(&mut self, base: &Path) {
        self.input.strip_prefix(base);
    }
}

impl ResolcInput {
    fn new(language: SolcLanguage, sources: Sources, settings: SolcSettings) -> Self {
        Self { language, sources, settings }
    }

    pub fn strip_prefix(&mut self, base: &Path) {
        self.sources = std::mem::take(&mut self.sources)
            .into_iter()
            .map(|(path, s)| (strip_prefix_owned(path, base), s))
            .collect();

        self.settings.strip_prefix(base);
    }
}

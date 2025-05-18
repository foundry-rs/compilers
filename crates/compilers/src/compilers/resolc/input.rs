use foundry_compilers_artifacts::{SolcLanguage, Source, Sources};
use foundry_compilers_core::utils::strip_prefix_owned;
use semver::Version;
use serde::{Deserialize, Serialize, Serializer};
use std::{collections::HashSet, path::Path};

use crate::{solc::SolcSettings, CompilerInput, CompilerSettings};

#[derive(Clone, Default, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolcOptimizer {
    #[serde(default)]
    pub mode: Option<char>,
}

#[derive(Clone, Default, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolkaVMSettings {
    // #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub heap_size: Option<u32>,
    // #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub stack_size: Option<u32>,
}

#[derive(Clone, Default, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolcSettings {
    // #[serde(skip_serializing_if = "should_skip_polkavm")]
    #[serde(default)]
    pub polkavm: PolkaVMSettings,
    //#[serde(skip)]
    #[serde(default)]
    pub resolc_optimizer: ResolcOptimizer,
}

impl ResolcSettings {
    pub fn new(
        optimizer_mode: Option<char>,
        heap_size: Option<u32>,
        stack_size: Option<u32>,
    ) -> Self {
        Self {
            resolc_optimizer: ResolcOptimizer { mode: optimizer_mode },
            polkavm: PolkaVMSettings { heap_size, stack_size },
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ResolcVersionedInput {
    #[serde(flatten)]
    pub input: ResolcInput,
    pub solc_version: Version,
}

#[derive(Debug, Clone, Deserialize)]
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

impl Serialize for ResolcInput {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeMap;
        use serde_json::{Map, Value};

        let mut map = serializer.serialize_map(None)?;

        // language and sources
        map.serialize_entry("language", &self.language)?;
        map.serialize_entry("sources", &self.sources)?;

        // Serialize settings to a JSON object
        let mut settings_val =
            serde_json::to_value(&self.settings.settings).map_err(serde::ser::Error::custom)?;

        let settings_obj = settings_val.as_object_mut().ok_or_else(|| {
            serde::ser::Error::custom("Expected `settings` to serialize to object")
        })?;

        // Inject optimizer.mode
        let optimizer_val =
            settings_obj.entry("optimizer").or_insert_with(|| Value::Object(Map::new()));

        let optimizer_obj = optimizer_val
            .as_object_mut()
            .ok_or_else(|| serde::ser::Error::custom("Expected `optimizer` to be an object"))?;

        if let Some(mode) = self.settings.extra_settings.resolc_optimizer.mode {
            optimizer_obj.insert("mode".to_string(), Value::String(mode.to_string()));
        }

        // Finally insert modified settings under "settings"
        map.serialize_entry("settings", settings_obj)?;

        map.end()
    }
}

// impl From<SolcVersionedInput> for ResolcVersionedInput {
//     fn from(value: SolcVersionedInput) -> Self {
//         Self::build(
//             value.input.sources,
//             SolcSettings { settings: value.input.settings, cli_settings: value.cli_settings,
// extra_settings: value.extra_settings},             value.input.language,
//             value.version,
//         )
//     }
// }

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

        let mut settings = Self::Settings {
            settings: solc_settings,
            cli_settings: settings.cli_settings,
            extra_settings: settings.extra_settings,
        };
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

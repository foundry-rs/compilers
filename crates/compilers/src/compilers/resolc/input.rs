use foundry_compilers_artifacts::{
    output_selection::OutputSelection, serde_helpers, DebuggingSettings, EofVersion, EvmVersion,
    Libraries, ModelCheckerSettings, OptimizerDetails, Remapping, Settings, SettingsMetadata,
    SolcLanguage, Source, Sources,
};
use foundry_compilers_core::utils::strip_prefix_owned;
use semver::Version;
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, path::Path};

use crate::{
    solc::{CliSettings, SolcSettings},
    CompilerInput, CompilerSettings,
};

#[derive(Clone, Default, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolcOptimizer {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<char>,
}

#[derive(Clone, Default, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolkaVMSettings {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub heap_size: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stack_size: Option<u32>,
}

#[derive(Clone, Default, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolcSettings {
    #[serde(default)]
    pub polkavm: PolkaVMSettings,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolcVersionedInput {
    #[serde(flatten)]
    pub input: ResolcInput,
    #[serde(flatten)]
    pub cli_settings: CliSettings,
    pub solc_version: Version,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolcInput {
    pub language: SolcLanguage,
    pub sources: Sources,
    pub settings: ResolcSettingsInput,
}

impl Default for ResolcInput {
    fn default() -> Self {
        Self {
            language: SolcLanguage::Solidity,
            sources: Sources::default(),
            settings: ResolcSettingsInput::default(),
        }
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
        let cli_settings = settings.cli_settings.clone();
        let input = ResolcInput::new(language, sources, settings);
        Self { input, cli_settings, solc_version: version }
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
        Self { language, sources, settings: settings.into() }
    }

    pub fn strip_prefix(&mut self, base: &Path) {
        self.sources = std::mem::take(&mut self.sources)
            .into_iter()
            .map(|(path, s)| (strip_prefix_owned(path, base), s))
            .collect();

        self.settings.strip_prefix(base);
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolcSettingsInput {
    /// Stop compilation after the given stage.
    /// since 0.8.11: only "parsing" is valid here
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stop_after: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub remappings: Vec<Remapping>,
    /// Custom Optimizer settings
    #[serde(default)]
    pub optimizer: ResolcOptimizerInput,
    /// Model Checker options.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_checker: Option<ModelCheckerSettings>,
    /// Metadata settings
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<SettingsMetadata>,
    /// This field can be used to select desired outputs based
    /// on file and contract names.
    /// If this field is omitted, then the compiler loads and does type
    /// checking, but will not generate any outputs apart from errors.
    #[serde(default)]
    pub output_selection: OutputSelection,
    #[serde(
        default,
        with = "serde_helpers::display_from_str_opt",
        skip_serializing_if = "Option::is_none"
    )]
    pub evm_version: Option<EvmVersion>,
    /// Change compilation pipeline to go through the Yul intermediate representation. This is
    /// false by default.
    #[serde(rename = "viaIR", default, skip_serializing_if = "Option::is_none")]
    pub via_ir: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub debug: Option<DebuggingSettings>,
    /// Addresses of the libraries. If not all libraries are given here,
    /// it can result in unlinked objects whose output data is different.
    ///
    /// The top level key is the name of the source file where the library is used.
    /// If remappings are used, this source file should match the global path
    /// after remappings were applied.
    /// If this key is an empty string, that refers to a global level.
    #[serde(default)]
    pub libraries: Libraries,
    /// Specify EOF version to produce.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub eof_version: Option<EofVersion>,
    #[serde(default)]
    pub polkavm: PolkaVMSettings,
}

impl From<SolcSettings> for ResolcSettingsInput {
    fn from(settings: SolcSettings) -> Self {
        let SolcSettings {
            settings:
                Settings {
                    stop_after,
                    remappings,
                    optimizer,
                    model_checker,
                    metadata,
                    output_selection,
                    evm_version,
                    via_ir,
                    debug,
                    libraries,
                    eof_version,
                },
            extra_settings: ResolcSettings { polkavm, resolc_optimizer },
            ..
        } = settings;

        Self {
            stop_after,
            remappings,
            optimizer: ResolcOptimizerInput {
                enabled: optimizer.enabled,
                runs: optimizer.runs,
                details: optimizer.details,
                mode: resolc_optimizer.mode,
            },
            model_checker,
            metadata,
            output_selection,
            evm_version,
            via_ir,
            debug,
            libraries,
            eof_version,
            polkavm,
        }
    }
}

impl ResolcSettingsInput {
    /// Adds `ast` to output
    #[must_use]
    pub fn with_ast(mut self) -> Self {
        let output = self.output_selection.as_mut().entry("*".to_string()).or_default();
        output.insert(String::new(), vec!["ast".to_string()]);
        self
    }

    pub fn strip_prefix(&mut self, base: &Path) {
        self.remappings.iter_mut().for_each(|r| {
            r.strip_prefix(base);
        });

        self.libraries.libs = std::mem::take(&mut self.libraries.libs)
            .into_iter()
            .map(|(file, libs)| (file.strip_prefix(base).map(Into::into).unwrap_or(file), libs))
            .collect();

        self.output_selection = OutputSelection(
            std::mem::take(&mut self.output_selection.0)
                .into_iter()
                .map(|(file, selection)| {
                    (
                        Path::new(&file)
                            .strip_prefix(base)
                            .map(|p| p.display().to_string())
                            .unwrap_or(file),
                        selection,
                    )
                })
                .collect(),
        );

        if let Some(mut model_checker) = self.model_checker.take() {
            model_checker.contracts = model_checker
                .contracts
                .into_iter()
                .map(|(path, contracts)| {
                    (
                        Path::new(&path)
                            .strip_prefix(base)
                            .map(|p| p.display().to_string())
                            .unwrap_or(path),
                        contracts,
                    )
                })
                .collect();
            self.model_checker = Some(model_checker);
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolcOptimizerInput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runs: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<char>,
    /// Switch optimizer components on or off in detail.
    /// The "enabled" switch above provides two defaults which can be
    /// tweaked here. If "details" is given, "enabled" can be omitted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub details: Option<OptimizerDetails>,
}

impl Default for ResolcOptimizerInput {
    fn default() -> Self {
        Self { enabled: Some(false), runs: Some(200), details: None, mode: None }
    }
}

impl Default for ResolcSettingsInput {
    fn default() -> Self {
        Self {
            stop_after: None,
            optimizer: Default::default(),
            metadata: None,
            output_selection: OutputSelection::default_output_selection(),
            evm_version: Some(EvmVersion::default()),
            via_ir: None,
            debug: None,
            libraries: Default::default(),
            remappings: Default::default(),
            model_checker: None,
            eof_version: None,
            polkavm: Default::default(),
        }
        .with_ast()
    }
}

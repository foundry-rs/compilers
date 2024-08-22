use super::{settings::ZkSolcSettings, ZkSettings};
use crate::{
    compilers::{solc::SolcLanguage, CompilerInput},
    solc,
};
use foundry_compilers_artifacts::{remappings::Remapping, solc::serde_helpers, Source, Sources};
use semver::Version;
use serde::{Deserialize, Serialize};
use std::{
    borrow::Cow,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Serialize)]
pub struct ZkSolcVersionedInput {
    #[serde(flatten)]
    pub input: ZkSolcInput,
    pub solc_version: Version,
    pub cli_settings: solc::CliSettings,
}

impl CompilerInput for ZkSolcVersionedInput {
    type Settings = ZkSolcSettings;
    type Language = SolcLanguage;

    // WARN: version is the solc version and NOT the zksolc version
    // This is because we use solc's version resolution to figure
    // out what solc to pair zksolc with.
    fn build(
        sources: Sources,
        settings: Self::Settings,
        language: Self::Language,
        version: Version,
    ) -> Self {
        let ZkSolcSettings { settings, cli_settings } = settings;
        let input = ZkSolcInput { language, sources, settings }.sanitized(&version);

        Self { solc_version: version, input, cli_settings }
    }

    fn language(&self) -> Self::Language {
        self.input.language
    }

    // TODO: This is the solc_version and not the zksolc version. We store this here because
    // the input is not associated with a zksolc version and we use solc's version resolution
    // features to know what solc to use to compile a file with. We should think about
    // how to best honor this api so the value is not confusing.
    fn version(&self) -> &Version {
        &self.solc_version
    }

    fn sources(&self) -> impl Iterator<Item = (&Path, &Source)> {
        self.input.sources.iter().map(|(path, source)| (path.as_path(), source))
    }

    fn compiler_name(&self) -> Cow<'static, str> {
        "zksolc and solc".into()
    }

    fn strip_prefix(&mut self, base: &Path) {
        self.input.strip_prefix(base);
    }
}

/// Input type `solc` expects.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ZkSolcInput {
    pub language: SolcLanguage,
    pub sources: Sources,
    pub settings: ZkSettings,
}

/// Default `language` field is set to `"Solidity"`.
impl Default for ZkSolcInput {
    fn default() -> Self {
        Self {
            language: SolcLanguage::Solidity,
            sources: Sources::default(),
            settings: ZkSettings::default(),
        }
    }
}

impl ZkSolcInput {
    /// Removes the `base` path from all source files
    pub fn strip_prefix(&mut self, base: impl AsRef<Path>) {
        let base = base.as_ref();
        self.sources = std::mem::take(&mut self.sources)
            .into_iter()
            .map(|(path, s)| (path.strip_prefix(base).map(Into::into).unwrap_or(path), s))
            .collect();

        self.settings.strip_prefix(base);
    }
    /// The flag indicating whether the current [CompilerInput] is
    /// constructed for the yul sources
    pub fn is_yul(&self) -> bool {
        self.language == SolcLanguage::Yul
    }
    /// Consumes the type and returns a [ZkSolcInput::sanitized] version
    pub fn sanitized(mut self, version: &Version) -> Self {
        self.settings.sanitize(version);
        self
    }

    pub fn with_remappings(mut self, remappings: Vec<Remapping>) -> Self {
        if self.language == SolcLanguage::Yul {
            if !remappings.is_empty() {
                warn!("omitting remappings supplied for the yul sources");
            }
        } else {
            self.settings.remappings = remappings;
        }

        self
    }
}

/// A `CompilerInput` representation used for verify
///
/// This type is an alternative `CompilerInput` but uses non-alphabetic ordering of the `sources`
/// and instead emits the (Path -> Source) path in the same order as the pairs in the `sources`
/// `Vec`. This is used over a map, so we can determine the order in which etherscan will display
/// the verified contracts
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StandardJsonCompilerInput {
    pub language: SolcLanguage,
    #[serde(with = "serde_helpers::tuple_vec_map")]
    pub sources: Vec<(PathBuf, Source)>,
    pub settings: ZkSettings,
}

impl StandardJsonCompilerInput {
    pub fn new(sources: Vec<(PathBuf, Source)>, settings: ZkSettings) -> Self {
        Self { language: SolcLanguage::Solidity, sources, settings }
    }
}

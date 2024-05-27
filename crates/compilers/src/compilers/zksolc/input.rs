use super::settings::ZkSolcSettings;
use crate::compilers::{solc::SolcLanguage, CompilerInput};
use foundry_compilers_artifacts::{remappings::Remapping, Source, Sources};
use semver::Version;
use serde::{Deserialize, Serialize};
use std::{
    borrow::Cow,
    collections::BTreeSet,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Serialize)]
pub struct ZkSolcVersionedInput {
    #[serde(flatten)]
    pub input: ZkSolcInput,
    pub allow_paths: BTreeSet<PathBuf>,
    pub base_path: Option<PathBuf>,
    pub include_paths: BTreeSet<PathBuf>,
    pub solc_version: Version,
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
        let input = ZkSolcInput { language, sources, settings };

        Self {
            solc_version: version,
            input,
            base_path: None,
            include_paths: Default::default(),
            allow_paths: Default::default(),
        }
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

    fn with_remappings(mut self, remappings: Vec<Remapping>) -> Self {
        self.input = self.input.with_remappings(remappings);

        self
    }

    fn compiler_name(&self) -> Cow<'static, str> {
        "ZkSolc".into()
    }

    fn strip_prefix(&mut self, base: &Path) {
        self.input.strip_prefix(base);
    }

    fn with_allow_paths(mut self, allowed_paths: BTreeSet<PathBuf>) -> Self {
        self.allow_paths = allowed_paths;
        self
    }

    fn with_base_path(mut self, base_path: PathBuf) -> Self {
        self.base_path = Some(base_path);
        self
    }

    fn with_include_paths(mut self, include_paths: BTreeSet<PathBuf>) -> Self {
        self.include_paths = include_paths;
        self
    }
}

/// Input type `solc` expects.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ZkSolcInput {
    pub language: SolcLanguage,
    pub sources: Sources,
    pub settings: ZkSolcSettings,
}

/// Default `language` field is set to `"Solidity"`.
impl Default for ZkSolcInput {
    fn default() -> Self {
        Self {
            language: SolcLanguage::Solidity,
            sources: Sources::default(),
            settings: ZkSolcSettings::default(),
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

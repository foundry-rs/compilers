use super::VyperSettings;
use foundry_compilers_artifacts_solc::sources::Sources;
use foundry_compilers_core::utils::strip_prefix_owned;
use semver::Version;
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
    pub fn new(sources: Sources, mut settings: VyperSettings, version: &Version) -> Self {
        let mut new_sources = Sources::new();
        let mut interfaces = Sources::new();

        for (path, content) in sources {
            if path.extension().is_some_and(|ext| ext == VYPER_INTERFACE_EXTENSION) {
                // Interface .vyi files should be removed from the output selection.
                settings.output_selection.0.remove(path.to_string_lossy().as_ref());
                interfaces.insert(path, content);
            } else {
                new_sources.insert(path, content);
            }
        }

        settings.sanitize(version);
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

    /// This will remove/adjust values in the [`VyperInput`] that are not compatible with this
    /// version
    pub fn sanitize(&mut self, version: &Version) {
        self.settings.sanitize(version);
    }

    /// Consumes the type and returns a [VyperInput::sanitized] version
    pub fn sanitized(mut self, version: &Version) -> Self {
        self.sanitize(version);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::VyperSettings;
    use foundry_compilers_artifacts_solc::EvmVersion;

    #[test]
    fn normalizes_evm_version_on_creation() {
        // Vyper 0.4.3 only supports up to Prague, not Osaka
        let vyper_version = Version::new(0, 4, 3);
        let settings =
            VyperSettings { evm_version: Some(EvmVersion::Osaka), ..Default::default() };

        let input = VyperInput::new(Sources::new(), settings, &vyper_version);

        // Should be normalized to Prague (max supported by 0.4.3)
        assert_eq!(input.settings.evm_version, Some(EvmVersion::Prague));
    }

    #[test]
    fn normalizes_evm_version_for_older_vyper() {
        // Vyper 0.3.7 only supports up to Paris
        let vyper_version = Version::new(0, 3, 7);
        let settings =
            VyperSettings { evm_version: Some(EvmVersion::Cancun), ..Default::default() };

        let input = VyperInput::new(Sources::new(), settings, &vyper_version);

        // Should be normalized to Paris (max supported by 0.3.7)
        assert_eq!(input.settings.evm_version, Some(EvmVersion::Paris));
    }

    #[test]
    fn keeps_supported_evm_version() {
        let vyper_version = Version::new(0, 4, 3);
        let settings =
            VyperSettings { evm_version: Some(EvmVersion::Cancun), ..Default::default() };

        let input = VyperInput::new(Sources::new(), settings, &vyper_version);

        // Cancun is supported by 0.4.3, should remain unchanged
        assert_eq!(input.settings.evm_version, Some(EvmVersion::Cancun));
    }
}

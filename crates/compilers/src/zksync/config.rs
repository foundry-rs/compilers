use crate::zksolc::settings::ZkSolcSettings;
use serde::{Deserialize, Serialize};

/// The config to use when compiling the contracts
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Default)]
pub struct ZkSolcConfig {
    /// How the file was compiled
    pub settings: ZkSolcSettings,
}

impl From<ZkSolcConfig> for ZkSolcSettings {
    fn from(config: ZkSolcConfig) -> Self {
        config.settings
    }
}

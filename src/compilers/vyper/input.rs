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
}

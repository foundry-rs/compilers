use std::{collections::BTreeSet, path::PathBuf};

pub use crate::artifacts::vyper::VyperSettings;
use crate::{
    compilers::{restrictions::CompilerSettingsRestrictions, CompilerSettings},
    solc::Restriction,
    Result,
};
use foundry_compilers_artifacts::{output_selection::OutputSelection, EvmVersion};

#[derive(Clone, Copy, Debug, Default)]
pub struct VyperRestrictions {
    pub evm_version: Restriction<EvmVersion>,
}

impl CompilerSettingsRestrictions for VyperRestrictions {
    fn merge(self, other: Self) -> Result<Self> {
        let Self { evm_version } = self;
        let evm_version = evm_version.merge(other.evm_version)?;
        Ok(Self { evm_version })
    }
}

impl CompilerSettings for VyperSettings {
    type Restrictions = VyperRestrictions;

    fn update_output_selection(&mut self, f: impl FnOnce(&mut OutputSelection)) {
        f(&mut self.output_selection)
    }

    fn can_use_cached(&self, other: &Self) -> bool {
        let Self {
            evm_version,
            optimize,
            bytecode_metadata,
            output_selection,
            search_paths,
            experimental_codegen,
        } = self;
        evm_version == &other.evm_version
            && optimize == &other.optimize
            && bytecode_metadata == &other.bytecode_metadata
            && output_selection.is_subset_of(&other.output_selection)
            && search_paths == &other.search_paths
            && experimental_codegen == &other.experimental_codegen
    }

    fn with_include_paths(mut self, include_paths: &BTreeSet<PathBuf>) -> Self {
        self.search_paths = Some(include_paths.clone());
        self
    }

    fn satisfies_restrictions(&self, restrictions: &Self::Restrictions) -> bool {
        restrictions.evm_version.satisfies(self.evm_version)
    }

    fn apply_restrictions(&self, restrictions: &Self::Restrictions) -> Self {
        let VyperRestrictions { evm_version } = restrictions;

        let mut settings = self.clone();
        settings.evm_version = evm_version.apply(settings.evm_version);
        settings
    }
}

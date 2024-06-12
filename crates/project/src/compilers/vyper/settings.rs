use crate::compilers::CompilerSettings;
use foundry_compilers_artifacts::{output_selection::OutputSelection, vyper::VyperSettings};

impl CompilerSettings for VyperSettings {
    fn update_output_selection(&mut self, f: impl FnOnce(&mut OutputSelection)) {
        f(&mut self.output_selection)
    }

    fn can_use_cached(&self, other: &Self) -> bool {
        let Self { evm_version, optimize, bytecode_metadata, output_selection } = self;
        evm_version == &other.evm_version
            && optimize == &other.optimize
            && bytecode_metadata == &other.bytecode_metadata
            && output_selection.is_subset_of(&other.output_selection)
    }
}

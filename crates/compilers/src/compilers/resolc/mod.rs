mod compiler;
mod input;
pub use compiler::Resolc;
use foundry_compilers_artifacts::{resolc::ResolcCompilerOutput, solc::error::Error, Contract};
pub use input::{ResolcInput, ResolcVersionedInput};

impl From<ResolcCompilerOutput> for super::CompilerOutput<Error, Contract> {
    fn from(output: ResolcCompilerOutput) -> Self {
        Self {
            errors: output.errors,
            contracts: output
                .contracts
                .into_iter()
                .map(|(k, v)| (k, v.into_iter().map(|(k, v)| (k, v.into())).collect()))
                .collect(),
            sources: output.sources,
            metadata: Default::default(),
        }
    }
}

use foundry_compilers_artifacts::vyper::{error::VyperCompilationError, output::VyperOutput};

impl From<VyperOutput> for super::CompilerOutput<VyperCompilationError> {
    fn from(output: VyperOutput) -> Self {
        super::CompilerOutput {
            errors: output.errors,
            contracts: output
                .contracts
                .into_iter()
                .map(|(k, v)| (k, v.into_iter().map(|(k, v)| (k, v.into())).collect()))
                .collect(),
            sources: output.sources.into_iter().map(|(k, v)| (k, v.into())).collect(),
        }
    }
}

//! Meta crate reexporting all artifacts types.

#![cfg_attr(not(test), warn(unused_crate_dependencies))]

pub use foundry_compilers_artifacts_solc as solc;
pub use foundry_compilers_artifacts_vyper as vyper;
pub use solc::*;

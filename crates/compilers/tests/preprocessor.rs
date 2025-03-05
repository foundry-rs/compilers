//! preprocessor tests

use foundry_compilers::{
    preprocessor::TestOptimizerPreprocessor,
    project::ProjectCompiler,
    solc::{SolcCompiler, SolcLanguage},
    ProjectBuilder, ProjectPathsConfig,
};
use foundry_compilers_core::utils::canonicalize;
use std::{env, path::Path};

#[test]
fn can_handle_constructors_and_creation_code() {
    let root =
        canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../test-data/preprocessor"))
            .unwrap();

    let paths = ProjectPathsConfig::builder()
        .sources(root.join("src"))
        .tests(root.join("test"))
        .root(&root)
        .build::<SolcLanguage>()
        .unwrap();

    let project = ProjectBuilder::<SolcCompiler>::new(Default::default())
        .paths(paths)
        .build(SolcCompiler::default())
        .unwrap();

    // TODO: figure out how to set root to parsing context.
    let cur_dir = env::current_dir().unwrap();
    env::set_current_dir(root).unwrap();
    let compiled = ProjectCompiler::new(&project)
        .unwrap()
        .with_preprocessor(TestOptimizerPreprocessor)
        .compile()
        .expect("failed to compile");
    compiled.assert_success();
    env::set_current_dir(cur_dir).unwrap();
}

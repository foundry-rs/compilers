use crate::{
    apply_updates,
    multi::{MultiCompiler, MultiCompilerInput, MultiCompilerLanguage},
    preprocessor::{
        data::{collect_preprocessor_data, create_deploy_helpers},
        deps::{remove_bytecode_dependencies, PreprocessorDependencies},
    },
    project::Preprocessor,
    replace_source_content,
    solc::{SolcCompiler, SolcVersionedInput},
    Compiler, ProjectPathsConfig, Result,
};
use alloy_primitives::hex;
use foundry_compilers_artifacts::{SolcLanguage, Source};
use foundry_compilers_core::{error::SolcError, utils};
use itertools::Itertools;
use md5::Digest;
use solar_parse::{
    ast::{FunctionKind, ItemKind, Span, Visibility},
    interface::{
        diagnostics::EmittedDiagnostics, source_map::FileName, BytePos, Session, SourceMap,
    },
    Parser,
};
use solar_sema::{thread_local::ThreadLocal, ParsingContext};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

mod data;
mod deps;

/// Represents location of an item in the source map.
/// Used to generate source code updates.
#[derive(Debug)]
pub struct SourceMapLocation {
    /// Source map location start.
    start: usize,
    /// Source map location end.
    end: usize,
}

impl SourceMapLocation {
    /// Creates source map location from an item location within a source file.
    fn from_span(source_map: &SourceMap, span: Span) -> Self {
        let range = span.to_range();
        let start_pos = BytePos::from_usize(range.start);
        let end_pos = BytePos::from_usize(range.end);
        Self {
            start: source_map.lookup_byte_offset(start_pos).pos.to_usize(),
            end: source_map.lookup_byte_offset(end_pos).pos.to_usize(),
        }
    }
}

#[derive(Debug)]
pub struct TestOptimizerPreprocessor;

impl Preprocessor<SolcCompiler> for TestOptimizerPreprocessor {
    fn preprocess(
        &self,
        _solc: &SolcCompiler,
        mut input: SolcVersionedInput,
        paths: &ProjectPathsConfig<SolcLanguage>,
        mocks: &mut HashSet<PathBuf>,
    ) -> Result<SolcVersionedInput> {
        let sources = &mut input.input.sources;
        // Skip if we are not preprocessing any tests or scripts. Avoids unnecessary AST parsing.
        if sources.iter().all(|(path, _)| !is_test_or_script(path, paths)) {
            trace!("no tests or sources to preprocess");
            return Ok(input);
        }

        let sess = Session::builder().with_buffer_emitter(Default::default()).build();
        let _ = sess.enter_parallel(|| -> solar_parse::interface::Result {
            let mut parsing_context = ParsingContext::new(&sess);
            // Set remappings into HIR parsing context.
            for remapping in &paths.remappings {
                parsing_context
                    .file_resolver
                    .add_import_map(PathBuf::from(&remapping.name), PathBuf::from(&remapping.path));
            }
            // Load and parse test and script contracts only (dependencies are automatically
            // resolved).
            let preprocessed_paths = sources
                .into_iter()
                .filter(|(path, source)| {
                    is_test_or_script(path, paths) && !source.content.is_empty()
                })
                .map(|(path, _)| path.clone())
                .collect_vec();
            parsing_context.load_files(&preprocessed_paths)?;

            let hir_arena = ThreadLocal::new();
            if let Some(gcx) = parsing_context.parse_and_lower(&hir_arena)? {
                let hir = &gcx.get().hir;
                // Collect tests and scripts dependencies and identify mock contracts.
                let deps = PreprocessorDependencies::new(
                    &sess,
                    hir,
                    &preprocessed_paths,
                    &paths.paths_relative().sources,
                    &paths.root,
                    mocks,
                );
                // Collect data of source contracts referenced in tests and scripts.
                let data = collect_preprocessor_data(&sess, hir, &deps.referenced_contracts);

                // Extend existing sources with preprocessor deploy helper sources.
                sources.extend(create_deploy_helpers(&data));

                // Generate and apply preprocessor source updates.
                apply_updates(sources, remove_bytecode_dependencies(hir, &deps, &data));
            }

            Ok(())
        });

        // Return if any diagnostics emitted during content parsing.
        if let Err(err) = sess.emitted_errors().unwrap() {
            trace!("failed preprocessing {err}");
            return Err(SolcError::Message(err.to_string()));
        }

        Ok(input)
    }
}

impl Preprocessor<MultiCompiler> for TestOptimizerPreprocessor {
    fn preprocess(
        &self,
        compiler: &MultiCompiler,
        input: <MultiCompiler as Compiler>::Input,
        paths: &ProjectPathsConfig<MultiCompilerLanguage>,
        mocks: &mut HashSet<PathBuf>,
    ) -> Result<<MultiCompiler as Compiler>::Input> {
        match input {
            MultiCompilerInput::Solc(input) => {
                if let Some(solc) = &compiler.solc {
                    let paths = paths.clone().with_language::<SolcLanguage>();
                    let input = self.preprocess(solc, input, &paths, mocks)?;
                    Ok(MultiCompilerInput::Solc(input))
                } else {
                    Ok(MultiCompilerInput::Solc(input))
                }
            }
            MultiCompilerInput::Vyper(input) => Ok(MultiCompilerInput::Vyper(input)),
        }
    }
}

/// Helper function to compute hash of [`interface_representation`] of the source.
pub(crate) fn interface_representation_hash(source: &Source, file: &PathBuf) -> String {
    let Ok(repr) = interface_representation(&source.content, file) else {
        return source.content_hash();
    };
    let mut hasher = md5::Md5::new();
    hasher.update(&repr);
    let result = hasher.finalize();
    hex::encode(result)
}

/// Helper function to remove parts of the contract which do not alter its interface:
///   - Internal functions
///   - External functions bodies
///
/// Preserves all libraries and interfaces.
fn interface_representation(content: &str, file: &PathBuf) -> Result<String, EmittedDiagnostics> {
    let mut spans_to_remove: Vec<Span> = Vec::new();
    let sess =
        solar_parse::interface::Session::builder().with_buffer_emitter(Default::default()).build();
    sess.enter(|| {
        let arena = solar_parse::ast::Arena::new();
        let filename = FileName::Real(file.to_path_buf());
        let Ok(mut parser) = Parser::from_source_code(&sess, &arena, filename, content.to_string())
        else {
            return;
        };
        let Ok(ast) = parser.parse_file().map_err(|e| e.emit()) else { return };
        for item in ast.items {
            let ItemKind::Contract(contract) = &item.kind else {
                continue;
            };

            if contract.kind.is_interface() || contract.kind.is_library() {
                continue;
            }

            for contract_item in contract.body.iter() {
                if let ItemKind::Function(function) = &contract_item.kind {
                    let is_exposed = match function.kind {
                        // Function with external or public visibility
                        FunctionKind::Function => {
                            function.header.visibility >= Some(Visibility::Public)
                        }
                        FunctionKind::Constructor
                        | FunctionKind::Fallback
                        | FunctionKind::Receive => true,
                        FunctionKind::Modifier => false,
                    };

                    // If function is not exposed we remove the entire span (signature and
                    // body). Otherwise we keep function signature and
                    // remove only the body.
                    if !is_exposed {
                        spans_to_remove.push(contract_item.span);
                    } else {
                        spans_to_remove.push(function.body_span);
                    }
                }
            }
        }
    });

    // Return if any diagnostics emitted during content parsing.
    if let Err(err) = sess.emitted_errors().unwrap() {
        trace!("failed parsing {file:?}: {err}");
        return Err(err);
    }

    let content =
        replace_source_content(content, spans_to_remove.iter().map(|span| (span.to_range(), "")))
            .replace("\n", "");
    Ok(utils::RE_TWO_OR_MORE_SPACES.replace_all(&content, "").to_string())
}

/// Checks if the given path is a test/script file.
fn is_test_or_script<L>(path: &Path, paths: &ProjectPathsConfig<L>) -> bool {
    let test_dir = paths.tests.strip_prefix(&paths.root).unwrap_or(&paths.root);
    let script_dir = paths.scripts.strip_prefix(&paths.root).unwrap_or(&paths.root);
    path.starts_with(test_dir) || path.starts_with(script_dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_interface_representation() {
        let content = r#"
library Lib {
    function libFn() internal {
        // logic to keep
    }
}
contract A {
    function a() external {}
    function b() public {}
    function c() internal {
        // logic logic logic
    }
    function d() private {}
    function e() external {
        // logic logic logic
    }
}"#;

        let result = interface_representation(content, &PathBuf::new()).unwrap();
        assert_eq!(
            result,
            r#"library Lib {function libFn() internal {// logic to keep}}contract A {function a() externalfunction b() publicfunction e() external }"#
        );
    }
}

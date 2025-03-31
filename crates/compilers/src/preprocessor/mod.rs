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
use foundry_compilers_artifacts::SolcLanguage;
use foundry_compilers_core::{error::SolcError, utils};
use solar_parse::{
    ast::{FunctionKind, ItemKind, Span, Visibility},
    interface::{diagnostics::EmittedDiagnostics, source_map::FileName, Session, SourceMap},
    Parser,
};
use solar_sema::{thread_local::ThreadLocal, ParsingContext};
use std::{
    collections::HashSet,
    ops::Range,
    path::{Path, PathBuf},
};

mod data;
mod deps;

/// Returns the range of the given span in the source map.
#[track_caller]
fn span_to_range(source_map: &SourceMap, span: Span) -> Range<usize> {
    source_map.span_to_source(span).unwrap().1
}

#[derive(Debug)]
pub struct TestOptimizerPreprocessor;

impl Preprocessor<SolcCompiler> for TestOptimizerPreprocessor {
    fn preprocess(
        &self,
        _solc: &SolcCompiler,
        input: &mut SolcVersionedInput,
        paths: &ProjectPathsConfig<SolcLanguage>,
        mocks: &mut HashSet<PathBuf>,
    ) -> Result<()> {
        let sources = &mut input.input.sources;
        // Skip if we are not preprocessing any tests or scripts. Avoids unnecessary AST parsing.
        if sources.iter().all(|(path, _)| !paths.is_test_or_script(path)) {
            trace!("no tests or sources to preprocess");
            return Ok(());
        }

        let sess = Session::builder().with_buffer_emitter(Default::default()).build();
        let _ = sess.enter_parallel(|| -> solar_parse::interface::Result {
            // Set up the parsing context with the project paths.
            let mut parsing_context = ParsingContext::new(&sess);
            parsing_context.file_resolver.set_current_dir(&paths.root);
            for remapping in &paths.remappings {
                parsing_context.file_resolver.add_import_remapping(
                    solar_sema::interface::config::ImportRemapping {
                        context: remapping.context.clone().unwrap_or_default(),
                        prefix: remapping.name.clone(),
                        path: remapping.path.clone(),
                    },
                );
            }
            parsing_context.file_resolver.add_include_paths(paths.include_paths.iter().cloned());

            // Add the sources into the context.
            let mut preprocessed_paths = vec![];
            for (path, source) in sources.iter() {
                if paths.is_test_or_script(path) {
                    if let Ok(src_file) =
                        sess.source_map().new_source_file(path.clone(), source.content.as_str())
                    {
                        parsing_context.add_file(src_file);
                        preprocessed_paths.push(path.clone());
                    }
                }
            }

            // Parse and preprocess.
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

        Ok(())
    }
}

impl Preprocessor<MultiCompiler> for TestOptimizerPreprocessor {
    fn preprocess(
        &self,
        compiler: &MultiCompiler,
        input: &mut <MultiCompiler as Compiler>::Input,
        paths: &ProjectPathsConfig<MultiCompilerLanguage>,
        mocks: &mut HashSet<PathBuf>,
    ) -> Result<()> {
        // Preprocess only Solc compilers.
        let MultiCompilerInput::Solc(input) = input else { return Ok(()) };

        let Some(solc) = &compiler.solc else { return Ok(()) };

        let paths = paths.clone().with_language::<SolcLanguage>();
        self.preprocess(solc, input, &paths, mocks)
    }
}

pub(crate) fn interface_repr_hash(content: &str, path: &Path) -> Option<String> {
    let src = interface_repr(content, path).ok()?;
    Some(foundry_compilers_artifacts::Source::content_hash_of(&src))
}

pub(crate) fn interface_repr(content: &str, path: &Path) -> Result<String, EmittedDiagnostics> {
    parse_one_source(content, path, |ast| interface_representation_ast(content, &ast))
}

pub(crate) fn parse_one_source<R>(
    content: &str,
    path: &Path,
    f: impl FnOnce(solar_sema::ast::SourceUnit<'_>) -> R,
) -> Result<R, EmittedDiagnostics> {
    let sess = Session::builder().with_buffer_emitter(Default::default()).build();
    let res = sess.enter(|| -> solar_parse::interface::Result<_> {
        let arena = solar_parse::ast::Arena::new();
        let filename = FileName::Real(path.to_path_buf());
        let mut parser = Parser::from_source_code(&sess, &arena, filename, content.to_string())?;
        let ast = parser.parse_file().map_err(|e| e.emit())?;
        Ok(f(ast))
    });

    // Return if any diagnostics emitted during content parsing.
    if let Err(err) = sess.emitted_errors().unwrap() {
        trace!("failed parsing {path:?}:\n{err}");
        return Err(err);
    }

    Ok(res.unwrap())
}

/// Helper function to remove parts of the contract which do not alter its interface:
///   - Internal functions
///   - External functions bodies
///
/// Preserves all libraries and interfaces.
pub(crate) fn interface_representation_ast(
    content: &str,
    ast: &solar_parse::ast::SourceUnit<'_>,
) -> String {
    let mut spans_to_remove: Vec<Span> = Vec::new();
    for item in ast.items.iter() {
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
                    FunctionKind::Constructor | FunctionKind::Fallback | FunctionKind::Receive => {
                        true
                    }
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
    let content =
        replace_source_content(content, spans_to_remove.iter().map(|span| (span.to_range(), "")))
            .replace("\n", "");
    utils::RE_TWO_OR_MORE_SPACES.replace_all(&content, "").into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

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

        let result = interface_repr(content, Path::new("")).unwrap();
        assert_eq!(
            result,
            r#"library Lib {function libFn() internal {// logic to keep}}contract A {function a() externalfunction b() publicfunction e() external }"#
        );
    }
}

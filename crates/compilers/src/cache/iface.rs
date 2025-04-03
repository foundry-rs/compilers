use crate::{parse_one_source, replace_source_content};
use solar_sema::{
    ast::{self, Span},
    interface::diagnostics::EmittedDiagnostics,
};
use std::path::Path;

pub(crate) fn interface_repr_hash(content: &str, path: &Path) -> Option<String> {
    let src = interface_repr(content, path).ok()?;
    Some(foundry_compilers_artifacts::Source::content_hash_of(&src))
}

pub(crate) fn interface_repr(content: &str, path: &Path) -> Result<String, EmittedDiagnostics> {
    parse_one_source(content, path, |ast| interface_representation_ast(content, &ast))
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
        let ast::ItemKind::Contract(contract) = &item.kind else {
            continue;
        };

        if contract.kind.is_interface() || contract.kind.is_library() {
            continue;
        }

        for contract_item in contract.body.iter() {
            if let ast::ItemKind::Function(function) = &contract_item.kind {
                let is_exposed = match function.kind {
                    // Function with external or public visibility
                    ast::FunctionKind::Function => {
                        function.header.visibility >= Some(ast::Visibility::Public)
                    }
                    ast::FunctionKind::Constructor
                    | ast::FunctionKind::Fallback
                    | ast::FunctionKind::Receive => true,
                    ast::FunctionKind::Modifier => false,
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
    crate::utils::RE_TWO_OR_MORE_SPACES.replace_all(&content, "").into_owned()
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

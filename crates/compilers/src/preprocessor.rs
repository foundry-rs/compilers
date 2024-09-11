use super::project::Preprocessor;
use crate::{
    flatten::{apply_updates, Updates},
    multi::{MultiCompiler, MultiCompilerInput},
    solc::{SolcCompiler, SolcVersionedInput},
    Compiler, Result, SolcError,
};
use alloy_primitives::hex;
use foundry_compilers_artifacts::{
    ast::SourceLocation,
    output_selection::OutputSelection,
    visitor::{Visitor, Walk},
    ContractDefinition, ContractKind, Expression, FunctionCall, MemberAccess, NewExpression,
    Source, SourceUnit, SourceUnitPart, Sources, TypeName,
};
use foundry_compilers_core::utils;
use md5::{digest::typenum::Exp, Digest};
use solang_parser::{
    diagnostics::Diagnostic,
    helpers::CodeLocation,
    pt::{ContractPart, ContractTy, FunctionAttribute, FunctionTy, SourceUnitPart, Visibility},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

pub(crate) fn interface_representation(content: &str) -> Result<String, Vec<Diagnostic>> {
    let (source_unit, _) = solang_parser::parse(&content, 0)?;
    let mut locs_to_remove = Vec::new();

    for part in source_unit.0 {
        if let SourceUnitPart::ContractDefinition(contract) = part {
            if matches!(contract.ty, ContractTy::Interface(_) | ContractTy::Library(_)) {
                continue;
            }
            for part in contract.parts {
                if let ContractPart::FunctionDefinition(func) = part {
                    let is_exposed = func.ty == FunctionTy::Function
                        && func.attributes.iter().any(|attr| {
                            matches!(
                                attr,
                                FunctionAttribute::Visibility(
                                    Visibility::External(_) | Visibility::Public(_)
                                )
                            )
                        })
                        || matches!(
                            func.ty,
                            FunctionTy::Constructor | FunctionTy::Fallback | FunctionTy::Receive
                        );

                    if !is_exposed {
                        locs_to_remove.push(func.loc);
                    }

                    if let Some(ref body) = func.body {
                        locs_to_remove.push(body.loc());
                    }
                }
            }
        }
    }

    let mut content = content.to_string();
    let mut offset = 0;

    for loc in locs_to_remove {
        let start = loc.start() - offset;
        let end = loc.end() - offset;

        content.replace_range(start..end, "");
        offset += end - start;
    }

    let content = content.replace("\n", "");
    Ok(utils::RE_TWO_OR_MORE_SPACES.replace_all(&content, "").to_string())
}

pub(crate) fn interface_representation_hash(source: &Source) -> String {
    let Ok(repr) = interface_representation(&source.content) else { return source.content_hash() };
    let mut hasher = md5::Md5::new();
    hasher.update(&repr);
    let result = hasher.finalize();
    hex::encode(result)
}

#[derive(Debug)]
pub struct ItemLocation {
    start: usize,
    end: usize,
}

impl ItemLocation {
    fn try_from_loc(loc: SourceLocation) -> Option<ItemLocation> {
        Some(ItemLocation { start: loc.start?, end: loc.start? + loc.length? })
    }
}

#[derive(Debug)]
enum BytecodeDependencyKind {
    CreationCode,
    New(ItemLocation, String),
}

#[derive(Debug)]
struct BytecodeDependency {
    kind: BytecodeDependencyKind,
    loc: ItemLocation,
    referenced_contract: usize,
}

#[derive(Debug)]
struct BytecodeDependencyCollector<'a> {
    source: &'a str,
    dependencies: Vec<BytecodeDependency>,
}

impl BytecodeDependencyCollector<'_> {
    fn new(source: &str) -> BytecodeDependencyCollector<'_> {
        BytecodeDependencyCollector { source, dependencies: Vec::new() }
    }
}

impl Visitor for BytecodeDependencyCollector<'_> {
    fn visit_function_call(&mut self, call: &FunctionCall) {
        let (new_loc, expr) = match &call.expression {
            Expression::NewExpression(expr) => (expr.src, expr),
            Expression::FunctionCallOptions(expr) => {
                if let Expression::NewExpression(new_expr) = &expr.expression {
                    (expr.src, new_expr)
                } else {
                    return;
                }
            }
            _ => return,
        };

        let TypeName::UserDefinedTypeName(type_name) = &expr.type_name else { return };

        let Some(loc) = ItemLocation::try_from_loc(call.src) else { return };
        let Some(name_loc) = ItemLocation::try_from_loc(type_name.src) else { return };
        let Some(new_loc) = ItemLocation::try_from_loc(new_loc) else { return };
        let name = &self.source[name_loc.start..name_loc.end];

        self.dependencies.push(BytecodeDependency {
            kind: BytecodeDependencyKind::New(new_loc, name.to_string()),
            loc,
            referenced_contract: type_name.referenced_declaration as usize,
        });
    }

    fn visit_member_access(&mut self, access: &MemberAccess) {
        if access.member_name != "creationCode" {
            return;
        }

        let Expression::FunctionCall(call) = &access.expression else { return };

        let Expression::Identifier(ident) = &call.expression else { return };

        if ident.name != "type" {
            return;
        }

        let Some(Expression::Identifier(ident)) = call.arguments.first() else { return };

        let Some(referenced) = ident.referenced_declaration else { return };

        let Some(loc) = ItemLocation::try_from_loc(access.src) else { return };

        self.dependencies.push(BytecodeDependency {
            kind: BytecodeDependencyKind::CreationCode,
            loc,
            referenced_contract: referenced as usize,
        });
    }
}

struct TestOptimizer<'a> {
    asts: BTreeMap<PathBuf, SourceUnit>,
    dirty: &'a Vec<PathBuf>,
    sources: &'a mut Sources,
}

impl TestOptimizer<'_> {
    fn new<'a>(
        asts: BTreeMap<PathBuf, SourceUnit>,
        dirty: &'a Vec<PathBuf>,
        sources: &'a mut Sources,
    ) -> TestOptimizer<'a> {
        TestOptimizer { asts, dirty, sources }
    }

    fn optimize(self) {
        let mut updates = Updates::default();
        let ignored_contracts = self.collect_ignored_contracts();
        self.rename_contracts_to_abstract(&ignored_contracts, &mut updates);
        self.remove_bytecode_dependencies(&ignored_contracts, &mut updates);

        apply_updates(self.sources, updates);
    }

    fn collect_ignored_contracts(&self) -> BTreeSet<usize> {
        let mut ignored_sources = BTreeSet::new();

        for (path, ast) in &self.asts {
            if path.to_str().unwrap().contains("test") || path.to_str().unwrap().contains("script")
            {
                ignored_sources.insert(ast.id);
            } else if self.dirty.contains(path) {
                ignored_sources.insert(ast.id);

                for node in &ast.nodes {
                    if let SourceUnitPart::ImportDirective(import) = node {
                        ignored_sources.insert(import.source_unit);
                    }
                }
            }
        }

        let mut ignored_contracts = BTreeSet::new();

        for ast in self.asts.values() {
            if ignored_sources.contains(&ast.id) {
                for node in &ast.nodes {
                    if let SourceUnitPart::ContractDefinition(contract) = node {
                        ignored_contracts.insert(contract.id);
                    }
                }
            }
        }

        ignored_contracts
    }

    fn rename_contracts_to_abstract(
        &self,
        ignored_contracts: &BTreeSet<usize>,
        updates: &mut Updates,
    ) {
        for (path, ast) in &self.asts {
            for node in &ast.nodes {
                if let SourceUnitPart::ContractDefinition(contract) = node {
                    if ignored_contracts.contains(&contract.id) {
                        continue;
                    }
                    if matches!(contract.kind, ContractKind::Contract) && !contract.is_abstract {
                        if let Some(start) = contract.src.start {
                            updates.entry(path.clone()).or_default().insert((
                                start,
                                start,
                                "abstract ".to_string(),
                            ));
                        }
                    }
                }
            }
        }
    }

    fn remove_bytecode_dependencies(
        &self,
        ignored_contracts: &BTreeSet<usize>,
        updates: &mut Updates,
    ) {
        for (path, ast) in &self.asts {
            let src = self.sources.get(path).unwrap().content.as_str();
            let mut collector = BytecodeDependencyCollector::new(src);
            ast.walk(&mut collector);
            let updates = updates.entry(path.clone()).or_default();
            for dep in collector.dependencies {
                match dep.kind {
                    BytecodeDependencyKind::CreationCode => {
                        updates.insert((dep.loc.start, dep.loc.end, "bytes(\"\")".to_string()));
                    }
                    BytecodeDependencyKind::New(new_loc, name) => {
                        updates.insert((
                            new_loc.start,
                            new_loc.end,
                            format!("{name}(payable(address(uint160(uint256(keccak256(abi.encode"),
                        ));
                        updates.insert((dep.loc.end, dep.loc.end, format!("))))))")));
                    }
                };
            }
        }
    }
}

#[derive(Debug)]
pub struct TestOptimizerPreprocessor;

impl Preprocessor<SolcCompiler> for TestOptimizerPreprocessor {
    fn preprocess(
        &self,
        solc: &SolcCompiler,
        mut input: SolcVersionedInput,
        dirty: &Vec<PathBuf>,
    ) -> Result<SolcVersionedInput> {
        let prev_output_selection = std::mem::replace(
            &mut input.input.settings.output_selection,
            OutputSelection::ast_output_selection(),
        );
        let output = solc.compile(&input)?;

        input.input.settings.output_selection = prev_output_selection;

        if let Some(e) = output.errors.iter().find(|e| e.severity.is_error()) {
            return Err(SolcError::msg(e));
        }

        let asts = output
            .sources
            .into_iter()
            .filter_map(|(path, source)| {
                if !input.input.sources.contains_key(&path) {
                    return None;
                }

                Some((|| {
                    let ast = source.ast.ok_or_else(|| SolcError::msg("missing AST"))?;
                    let ast: SourceUnit = serde_json::from_str(&serde_json::to_string(&ast)?)?;
                    Ok((path, ast))
                })())
            })
            .collect::<Result<BTreeMap<_, _>>>()?;

        TestOptimizer::new(asts, dirty, &mut input.input.sources).optimize();

        Ok(input)
    }
}

impl Preprocessor<MultiCompiler> for TestOptimizerPreprocessor {
    fn preprocess(
        &self,
        compiler: &MultiCompiler,
        input: <MultiCompiler as Compiler>::Input,
        dirty: &Vec<PathBuf>,
    ) -> Result<<MultiCompiler as Compiler>::Input> {
        match input {
            MultiCompilerInput::Solc(input) => {
                if let Some(solc) = &compiler.solc {
                    let input = self.preprocess(solc, input, dirty)?;
                    Ok(MultiCompilerInput::Solc(input))
                } else {
                    Ok(MultiCompilerInput::Solc(input))
                }
            }
            MultiCompilerInput::Vyper(input) => Ok(MultiCompilerInput::Vyper(input)),
        }
    }
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

        let result = interface_representation(content).unwrap();
        assert_eq!(
            result,
            r#"library Lib {function libFn() internal {// logic to keep}}contract A {function a() externalfunction b() publicfunction e() external }"#
        );
    }
}

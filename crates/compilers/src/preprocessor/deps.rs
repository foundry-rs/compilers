use crate::{
    preprocessor::{
        data::{ContractData, PreprocessorData},
        SourceMapLocation,
    },
    Updates,
};
use itertools::Itertools;
use solar_parse::interface::Session;
use solar_sema::{
    ast::Span,
    hir::{ContractId, Expr, ExprKind, Hir, TypeKind, Visit},
    interface::{data_structures::Never, source_map::FileName, SourceMap},
};
use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    ops::ControlFlow,
    path::PathBuf,
};

/// Holds data about referenced source contracts and bytecode dependencies.
pub struct PreprocessorDependencies {
    // Mapping test contract id -> test contract bytecode dependencies.
    pub bytecode_deps: BTreeMap<u32, Vec<BytecodeDependency>>,
    // Referenced contract ids.
    pub referenced_contracts: HashSet<u32>,
}

impl PreprocessorDependencies {
    pub fn new(sess: &Session, hir: &Hir<'_>, paths: &[PathBuf]) -> Self {
        let mut inner = BTreeMap::new();
        let mut references = HashSet::default();
        for contract in Hir::contracts(hir) {
            let source = Hir::source(hir, contract.source);

            let FileName::Real(path) = &source.file.name else {
                continue;
            };

            // Collect dependencies only for tests and scripts.
            if !paths.contains(path) {
                continue;
            }

            let mut deps_collector =
                BytecodeDependencyCollector::new(sess.source_map(), hir, source.file.src.as_str());
            // Analyze current contract.
            deps_collector.walk_contract(contract);
            // Ignore empty test contracts declared in source files with other contracts.
            if !deps_collector.dependencies.is_empty() {
                inner.insert(contract.linearized_bases[0].get(), deps_collector.dependencies);
            }
            // Record collected referenced contract ids.
            references.extend(deps_collector.referenced_contracts);
        }
        Self { bytecode_deps: inner, referenced_contracts: references }
    }
}

/// Represents a bytecode dependency kind.
#[derive(Debug)]
enum BytecodeDependencyKind {
    /// `type(Contract).creationCode`
    CreationCode,
    /// `new Contract`. Holds the name of the contract and args length.
    New(String, usize),
}

/// Represents a single bytecode dependency.
#[derive(Debug)]
pub struct BytecodeDependency {
    /// Dependency kind.
    kind: BytecodeDependencyKind,
    /// Source map location of this dependency.
    loc: SourceMapLocation,
    /// HIR id of referenced contract.
    referenced_contract: u32,
}

/// Walks over contract HIR and collects [`BytecodeDependency`]s and referenced contracts.
struct BytecodeDependencyCollector<'hir> {
    /// Source map, used for determining contract item locations.
    source_map: &'hir SourceMap,
    /// Parsed HIR.
    hir: &'hir Hir<'hir>,
    /// Source content of current contract.
    src: &'hir str,
    /// Dependencies collected for current contract.
    dependencies: Vec<BytecodeDependency>,
    /// HIR ids of contracts referenced from current contract.
    referenced_contracts: HashSet<u32>,
}

impl<'hir> BytecodeDependencyCollector<'hir> {
    fn new(source_map: &'hir SourceMap, hir: &'hir Hir<'hir>, src: &'hir str) -> Self {
        Self {
            source_map,
            hir,
            src,
            dependencies: vec![],
            referenced_contracts: HashSet::default(),
        }
    }
}

impl<'hir> Visit<'hir> for BytecodeDependencyCollector<'hir> {
    type BreakValue = Never;

    fn hir(&self) -> &'hir Hir<'hir> {
        self.hir
    }

    fn visit_expr(&mut self, expr: &'hir Expr<'hir>) -> ControlFlow<Self::BreakValue> {
        match &expr.kind {
            ExprKind::New(ty) => {
                if let TypeKind::Custom(item_id) = ty.kind {
                    if let Some(contract_id) = item_id.as_contract() {
                        let name_loc = SourceMapLocation::from_span(self.source_map, ty.span);
                        let name = &self.src[name_loc.start..name_loc.end];
                        // TODO: check if there's a better way to determine where constructor call
                        // ends.
                        let args_len = self.src[name_loc.end..].split_once(';').unwrap().0.len();
                        self.dependencies.push(BytecodeDependency {
                            kind: BytecodeDependencyKind::New(name.to_string(), args_len),
                            loc: SourceMapLocation::from_span(
                                self.source_map,
                                Span::new(expr.span.lo(), expr.span.hi()),
                            ),
                            referenced_contract: contract_id.get(),
                        });
                        self.referenced_contracts.insert(contract_id.get());
                    }
                }
            }
            ExprKind::Member(member_expr, ident) => {
                if ident.name.to_string() == "creationCode" {
                    if let ExprKind::TypeCall(ty) = &member_expr.kind {
                        if let TypeKind::Custom(contract_id) = &ty.kind {
                            if let Some(contract_id) = contract_id.as_contract() {
                                self.dependencies.push(BytecodeDependency {
                                    kind: BytecodeDependencyKind::CreationCode,
                                    loc: SourceMapLocation::from_span(self.source_map, expr.span),
                                    referenced_contract: contract_id.get(),
                                });
                                self.referenced_contracts.insert(contract_id.get());
                            }
                        }
                    }
                }
            }
            _ => {}
        }
        self.walk_expr(expr)
    }
}

/// Goes over all test/script files and replaces bytecode dependencies with cheatcode
/// invocations.
pub fn remove_bytecode_dependencies(
    hir: &Hir<'_>,
    deps: &PreprocessorDependencies,
    data: &PreprocessorData,
) -> Updates {
    let mut updates = Updates::default();
    for (contract_id, deps) in &deps.bytecode_deps {
        let contract = Hir::contract(hir, ContractId::new(*contract_id));
        let source = Hir::source(hir, contract.source);
        let FileName::Real(path) = &source.file.name else {
            continue;
        };

        let updates = updates.entry(path.clone()).or_default();
        let mut used_helpers = BTreeSet::new();

        let vm_interface_name = format!("VmContractHelper{contract_id}");
        let vm = format!("{vm_interface_name}(0x7109709ECfa91a80626fF3989D68f67F5b1DD12D)");

        for dep in deps {
            let Some(ContractData { artifact, constructor_data, .. }) =
                data.get(&dep.referenced_contract)
            else {
                continue;
            };

            match &dep.kind {
                BytecodeDependencyKind::CreationCode => {
                    // for creation code we need to just call getCode
                    updates.insert((
                        dep.loc.start,
                        dep.loc.end,
                        format!("{vm}.getCode(\"{artifact}\")"),
                    ));
                }
                BytecodeDependencyKind::New(name, args_length) => {
                    if constructor_data.is_none() {
                        // if there's no constructor, we can just call deployCode with one
                        // argument
                        updates.insert((
                            dep.loc.start,
                            dep.loc.end + args_length,
                            format!("{name}(payable({vm}.deployCode(\"{artifact}\")))"),
                        ));
                    } else {
                        // if there's a constructor, we use our helper
                        used_helpers.insert(dep.referenced_contract);
                        updates.insert((
                            dep.loc.start,
                            dep.loc.end,
                            format!(
                                "deployCode{id}(DeployHelper{id}.ConstructorArgs",
                                id = dep.referenced_contract
                            ),
                        ));
                        updates.insert((
                            dep.loc.end + args_length,
                            dep.loc.end + args_length,
                            ")".to_string(),
                        ));
                    }
                }
            };
        }
        let helper_imports = used_helpers.into_iter().map(|id| {
            format!(
                "import {{DeployHelper{id}, encodeArgs{id}, deployCode{id}}} from \"foundry-pp/DeployHelper{id}.sol\";",
            )
        }).join("\n");
        updates.insert((
            source.file.src.len(),
            source.file.src.len(),
            format!(
                r#"
{helper_imports}

interface {vm_interface_name} {{
    function deployCode(string memory _artifact, bytes memory _data) external returns (address);
    function deployCode(string memory _artifact) external returns (address);
    function getCode(string memory _artifact) external returns (bytes memory);
}}"#
            ),
        ));
    }
    updates
}

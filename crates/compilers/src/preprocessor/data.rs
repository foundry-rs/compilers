use crate::preprocessor::SourceMapLocation;
use foundry_compilers_artifacts::{Source, Sources};
use itertools::Itertools;
use solar_parse::interface::{Session, SourceMap};
use solar_sema::{
    hir::{Contract, ContractId, Hir},
    interface::source_map::FileName,
};
use std::{
    collections::{BTreeMap, HashSet},
    path::{Path, PathBuf},
};

/// Keeps data about project contracts definitions referenced from tests and scripts.
/// HIR id -> Contract data definition mapping.
pub type PreprocessorData = BTreeMap<u32, ContractData>;

/// Collects preprocessor data from referenced contracts.
pub fn collect_preprocessor_data(
    sess: &Session,
    hir: &Hir<'_>,
    libs: &[PathBuf],
    referenced_contracts: HashSet<u32>,
) -> PreprocessorData {
    let mut data = PreprocessorData::default();
    for contract_id in referenced_contracts {
        let contract = Hir::contract(hir, ContractId::new(contract_id));
        let source = Hir::source(hir, contract.source);

        let FileName::Real(path) = &source.file.name else {
            continue;
        };

        // Do not include external dependencies / libs.
        // TODO: better to include only files from project src in order to avoid processing mocks
        // within test dir.
        if libs.iter().any(|lib_paths| path.starts_with(lib_paths)) {
            continue;
        }

        let contract_data = ContractData::new(hir, contract, path, source, sess.source_map());
        data.insert(contract_data.hir_id, contract_data);
    }
    data
}

/// Creates helper libraries for contracts with a non-empty constructor.
///
/// See [`ContractData::build_helper`] for more details.
pub fn create_deploy_helpers(data: &BTreeMap<u32, ContractData>) -> Sources {
    let mut deploy_helpers = Sources::new();
    for (hir_id, contract) in data {
        if let Some(code) = contract.build_helper() {
            let path = format!("foundry-pp/DeployHelper{hir_id}.sol");
            deploy_helpers.insert(path.into(), Source::new(code));
        }
    }
    deploy_helpers
}

/// Keeps data about a contract constructor.
#[derive(Debug)]
pub struct ContractConstructorData {
    /// ABI encoded args.
    pub abi_encode_args: String,
    /// Constructor struct fields.
    pub struct_fields: String,
}

/// Keeps data about a single contract definition.
#[derive(Debug)]
pub struct ContractData {
    /// HIR Id of the contract.
    hir_id: u32,
    /// Path of the source file.
    path: PathBuf,
    /// Name of the contract
    name: String,
    /// Constructor parameters, if any.
    pub constructor_data: Option<ContractConstructorData>,
    /// Artifact string to pass into cheatcodes.
    pub artifact: String,
}

impl ContractData {
    fn new(
        hir: &Hir<'_>,
        contract: &Contract<'_>,
        path: &Path,
        source: &solar_sema::hir::Source<'_>,
        source_map: &SourceMap,
    ) -> Self {
        let artifact = format!("{}:{}", path.display(), contract.name);

        // Process data for contracts with constructor and parameters.
        let constructor_data = contract
            .ctor
            .map(|ctor_id| Hir::function(hir, ctor_id))
            .filter(|ctor| !ctor.parameters.is_empty())
            .map(|ctor| {
                let abi_encode_args = ctor
                    .parameters
                    .iter()
                    .map(|param_id| {
                        format!("args.{}", Hir::variable(hir, *param_id).name.unwrap().name)
                    })
                    .join(", ");
                let struct_fields = ctor
                    .parameters
                    .iter()
                    .map(|param_id| {
                        let src = source.file.src.as_str();
                        let loc = SourceMapLocation::from_span(
                            source_map,
                            Hir::variable(hir, *param_id).span,
                        );
                        src[loc.start..loc.end].replace(" memory ", " ").replace(" calldata ", " ")
                    })
                    .join("; ");
                ContractConstructorData { abi_encode_args, struct_fields }
            });

        Self {
            hir_id: contract.linearized_bases[0].get(),
            path: path.to_path_buf(),
            name: contract.name.to_string(),
            constructor_data,
            artifact,
        }
    }

    /// If contract has a non-empty constructor, generates a helper source file for it containing a
    /// helper to encode constructor arguments.
    ///
    /// This is needed because current preprocessing wraps the arguments, leaving them unchanged.
    /// This allows us to handle nested new expressions correctly. However, this requires us to have
    /// a way to wrap both named and unnamed arguments. i.e you can't do abi.encode({arg: val}).
    ///
    /// This function produces a helper struct + a helper function to encode the arguments. The
    /// struct is defined in scope of an abstract contract inheriting the contract containing the
    /// constructor. This is done as a hack to allow us to inherit the same scope of definitions.
    ///
    /// The resulted helper looks like this:
    /// ```solidity
    /// import "lib/openzeppelin-contracts/contracts/token/ERC20.sol";
    ///
    /// abstract contract DeployHelper335 is ERC20 {
    ///     struct ConstructorArgs {
    ///         string name;
    ///         string symbol;
    ///     }
    /// }
    ///
    /// function encodeArgs335(DeployHelper335.ConstructorArgs memory args) pure returns (bytes memory) {
    ///     return abi.encode(args.name, args.symbol);
    /// }
    /// ```
    ///
    /// Example usage:
    /// ```solidity
    /// new ERC20(name, symbol)
    /// ```
    /// becomes
    /// ```solidity
    /// vm.deployCode("artifact path", encodeArgs335(DeployHelper335.ConstructorArgs(name, symbol)))
    /// ```
    /// With named arguments:
    /// ```solidity
    /// new ERC20({name: name, symbol: symbol})
    /// ```
    /// becomes
    /// ```solidity
    /// vm.deployCode("artifact path", encodeArgs335(DeployHelper335.ConstructorArgs({name: name, symbol: symbol})))
    /// ```
    pub fn build_helper(&self) -> Option<String> {
        let Self { hir_id, path, name, constructor_data, artifact } = self;

        let Some(constructor_details) = constructor_data else { return None };
        let struct_fields = &constructor_details.struct_fields;
        let abi_encode_args = &constructor_details.abi_encode_args;
        let vm_interface_name = format!("VmContractHelper{hir_id}");
        let vm = format!("{vm_interface_name}(0x7109709ECfa91a80626fF3989D68f67F5b1DD12D)");

        let helper = format!(
            r#"
pragma solidity >=0.4.0;

import "{path}";

abstract contract DeployHelper{hir_id} is {name} {{
    struct ConstructorArgs {{
        {struct_fields};
    }}
}}

function encodeArgs{hir_id}(DeployHelper{hir_id}.ConstructorArgs memory args) pure returns (bytes memory) {{
    return abi.encode({abi_encode_args});
}}

function deployCode{hir_id}(DeployHelper{hir_id}.ConstructorArgs memory args) returns({name}) {{
    return {name}(payable({vm}.deployCode("{artifact}", encodeArgs{hir_id}(args))));
}}

interface {vm_interface_name} {{
    function deployCode(string memory _artifact, bytes memory _data) external returns (address);
    function deployCode(string memory _artifact) external returns (address);
    function getCode(string memory _artifact) external returns (bytes memory);
}}
        "#,
            path = path.display(),
        );

        Some(helper)
    }
}

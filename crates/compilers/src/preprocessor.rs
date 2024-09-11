use alloy_primitives::hex;
use foundry_compilers_artifacts::Source;
use foundry_compilers_core::utils;
use md5::Digest;
use solang_parser::{
    diagnostics::Diagnostic,
    helpers::CodeLocation,
    pt::{ContractPart, ContractTy, FunctionAttribute, FunctionTy, SourceUnitPart, Visibility},
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

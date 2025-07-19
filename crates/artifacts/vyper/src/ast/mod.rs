use serde::{Deserialize, Serialize};

pub mod visitor;
mod macros;

use macros::{vyper_node};
use crate::ast::macros::node_group;

vyper_node!(
    struct Module {
        // Module-specific fields
        source_sha256sum: String,
        name: Option<String>,
        path: String,
        resolved_path: String,
        source_id: u64,
        is_interface: bool,
        doc_string: Option<DocStr>,
        settings: Settings,

        // AST content
        body: Vec<TopLevelItem>,
    }
);

node_group!(
    TopLevelItem;

    // Function definitions
    FunctionDef,

    // Variable declarations
    VariableDecl,

    // Type definitions
    StructDef,
    EventDef,
    FlagDef,
    InterfaceDef,

    // Import statements
    Import,
    ImportFrom,

    // Special declarations
    ImplementsDecl,
    UsesDecl,
    InitializesDecl,
    ExportsDecl,

    // Docstrings at top level
    DocStr,
);

vyper_node!(
    struct FunctionDef {
        args: Arguments,
        body: Vec<Statement>,
        pos: Option<String>,
        doc_string: Option<String>,
        decorator_list: Vec<Expression>,
        name: String,
        returns: Option<Box<Expression>>,
    }
);

vyper_node!(
    struct VariableDecl {
        annotation: Box<Expression>,
        value: Option<String>,
        is_transient: bool,
        is_constant: bool,
        is_reentrant: bool,
        is_public: bool,
        target: Box<Expression>,
        is_immutable: bool,
        #[serde(rename = "type")]
        ttype: Type,
    }
);

vyper_node!(struct StructDef {});

vyper_node!(
    struct EventDef {
        name: String,
        body: Vec<Statement>,
        doc_string: Option<String>,
    }
);

vyper_node!(struct FlagDef {});

vyper_node!(struct InterfaceDef {});

vyper_node!(struct Import {});

vyper_node!(struct ImportFrom {});

vyper_node!(struct ImplementsDecl {});

vyper_node!(struct UsesDecl {});

vyper_node!(struct InitializesDecl {});

vyper_node!(struct ExportsDecl {});

vyper_node!(struct DocStr {});

node_group!(
    Statement;

    // Assignment statements
    Assign,
    AnnAssign,
    AugAssign,

    // Control flow statements
    Return,
    If,
    For,
    Break,
    Continue,
    Pass,

    // Exception handling
    Raise,
    Assert,

    // Vyper-specific statements
    Log,

    // Expression statements
    Expr,
    NamedExpr,
);

vyper_node!(
    struct Assign {
        value: Box<Expression>,
        target: Box<Expression>,
    }
);

vyper_node!(
    struct AnnAssign {
        annotation: Box<Expression>,
        value: Option<Box<Expression>>,
        target: Box<Expression>,
    }
);

vyper_node!(struct AugAssign {});

vyper_node!(
    struct Return {
        value: Option<Box<Expression>>,
    }
);

vyper_node!(struct If {});

vyper_node!(struct For {});

vyper_node!(struct Break {});

vyper_node!(struct Continue {});

vyper_node!(struct Pass {});

vyper_node!(struct Raise {});

vyper_node!(
    struct Assert {
        msg: Box<Expression>,
        test: Box<Expression>,
    }
);

vyper_node!(
    struct Log {
        #[serde(rename = "type")]
        ttype: Type,
    }
);

vyper_node!(struct Expr {});

vyper_node!(struct NamedExpr {});

node_group!(
    Expression;

    // Literals
    Constant,
    Int,
    Decimal,
    Hex,
    Str,
    Bytes,
    HexBytes,
    NameConstant,
    Ellipsis,

    // Collections
    List,
    Tuple,
    Dict,

    // Names and access
    Name,
    Attribute,
    Subscript,

    // Operations
    UnaryOp,
    BinOp,
    BoolOp,
    Compare,

    // Function calls
    Call,
    ExtCall,
    StaticCall,

    // Control flow expressions
    IfExp,

    // Function parameters (special context)
    #[serde(rename = "arg")]
    Arg,
    #[serde(rename = "arguments")]
    Arguments,
    #[serde(rename = "keyword")]
    Keyword,
);

vyper_node!(struct Constant {}); // TODO

vyper_node!(
    struct Int {
        value: u64,
        #[serde(rename = "type")]
        ttype: Type,
    }
);

vyper_node!(struct Decimal {}); // TODO

vyper_node!(struct Hex {}); // TODO

vyper_node!(
    struct Str {
        value: String,
        #[serde(rename = "type")]
        ttype: Type,
    }
);

vyper_node!(struct Bytes {}); // TODO

vyper_node!(struct HexBytes {}); // TODO

vyper_node!(struct NameConstant {}); // TODO

vyper_node!(struct Ellipsis {}); // TODO

vyper_node!(struct List {}); // TODO

vyper_node!(struct Tuple {}); // TODO

vyper_node!(struct Dict {}); // TODO

vyper_node!(
    struct Name {
        id: String,
        #[serde(rename = "type")]
        ttype: Option<Type>,
        variable_reads: Option<Vec<VariableAccess>>,
    }
);

vyper_node!(
    struct Attribute {
        value: Box<Expression>,
        attr: String,
        #[serde(rename = "type")]
        ttype: Type,
        variable_reads: Option<Vec<VariableAccess>>,
        variable_writes: Option<Vec<VariableAccess>>,
    }
);

vyper_node!(struct Subscript {}); // TODO

vyper_node!(struct UnaryOp {}); // TODO

vyper_node!(struct BinOp {}); // TODO

vyper_node!(struct BoolOp {}); // TODO

vyper_node!(
    struct Compare {
        left: Box<Expression>,
        right: Box<Expression>,
        op: Box<ComparisonOperator>,
        #[serde(rename = "type")]
        ttype: Type,
    }
);

vyper_node!(
    struct Call {
        args: Vec<Expression>,
        keywords: Vec<Keyword>,
        func: Box<Expression>,
        #[serde(rename = "type")]
        ttype: Type,
    }
);

vyper_node!(struct ExtCall {}); // TODO

vyper_node!(struct StaticCall {}); // TODO

vyper_node!(struct IfExp {}); // TODO

vyper_node!(
    struct Arg {
        annotation: Box<Expression>,
        arg: String,
    }
);

vyper_node!(
    struct Arguments {
        default: Option<String>,
        args: Vec<Arg>,
        defaults: Vec<String>,
    }
);

vyper_node!(struct Keyword {}); // TODO

node_group!(
    UnaryOperator;

    USub,
    Not,
    Invert,
);

vyper_node!(struct USub {}); // TODO
vyper_node!(struct Not {}); // TODO
vyper_node!(struct Invert {}); // TODO

node_group!(
    BinaryOperator;

    // Arithmetic
    Add,
    Sub,
    Mult,
    Div,
    FloorDiv,
    Mod,
    Pow,

    // Bitwise
    BitAnd,
    BitOr,
    BitXor,
    LShift,
    RShift,
);

vyper_node!(struct Add {});
vyper_node!(struct Sub {});
vyper_node!(struct Mult {});
vyper_node!(struct Div {});
vyper_node!(struct FloorDiv {});
vyper_node!(struct Mod {});
vyper_node!(struct Pow {});
vyper_node!(struct BitAnd {});
vyper_node!(struct BitOr {});
vyper_node!(struct BitXor {});
vyper_node!(struct LShift {});
vyper_node!(struct RShift {});

node_group!(
    BooleanOperator;

    And,
    Or,
);

vyper_node!(struct And {});
vyper_node!(struct Or {});

node_group!(
    ComparisonOperator;

    Eq,
    NotEq,
    Lt,
    LtE,
    Gt,
    GtE,
    In,
    NotIn,
);

vyper_node!(struct Eq {});
vyper_node!(struct NotEq {});
vyper_node!(struct Lt {});
vyper_node!(struct LtE {});
vyper_node!(struct Gt {});
vyper_node!(struct GtE {});
vyper_node!(struct In {});
vyper_node!(struct NotIn {});

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settings {}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VariableAccess {
    pub name: String,
    pub decl_node: DeclNode,
    pub access_path: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeclNode {
    pub node_id: u64,
    pub source_id: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
// #[serde(rename_all = "snake_case", tag = "typeclass")]
pub struct Type {
    pub name: Option<String>,
    pub typeclass: Option<String>,
    pub is_signed: Option<bool>,
    pub bits: Option<u32>,
    pub type_t: Option<TypedType>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypedType {
    pub name: String,
    pub type_decl_node: Option<TypeDeclNode>,
    pub typeclass: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypeDeclNode {
    pub node_id: i64,
    pub source_id: i64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, path::Path};

    #[test]
    fn can_parse_ast() {
        fs::read_dir(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../test-data").join("vyper_ast"),
        )
        .unwrap()
        .for_each(|path| {
            let path = path.unwrap().path();
            let path_str = path.to_string_lossy();

            let input = fs::read_to_string(&path).unwrap();
            let deserializer = &mut serde_json::Deserializer::from_str(&input);
            let result: Result<Module, _> = serde_path_to_error::deserialize(deserializer);
            match result {
                Err(e) => {
                    println!("... {path_str} fail: {e}");
                    panic!();
                }
                Ok(_) => {
                    println!("... {path_str} ok");
                }
            }
        })
    }
}

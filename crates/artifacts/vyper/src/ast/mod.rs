use serde::{Deserialize, Serialize};

mod macros;
pub mod visitor;

use crate::ast::macros::{basic_vyper_nodes, node_group};
use macros::vyper_node;

vyper_node!(
    struct Module {
        // Module-specific fields
        source_sha256sum: Option<String>,
        name: Option<String>,
        path: Option<String>,
        resolved_path: Option<String>,
        source_id: Option<i32>,
        is_interface: Option<bool>,
        doc_string: Option<DocStr>,
        settings: Option<Settings>,

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
        doc_string: Option<DocStr>,
        decorator_list: Vec<Expression>,
        name: String,
        returns: Option<Box<Expression>>,
    }
);

vyper_node!(
    struct VariableDecl {
        annotation: Box<Expression>,
        value: Option<Box<Expression>>,
        target: Box<Expression>,
        is_transient: bool,
        is_constant: bool,
        is_reentrant: bool,
        is_public: bool,
        is_immutable: bool,
        #[serde(rename = "type")]
        ttype: Option<Type>,
    }
);

vyper_node!(
    struct StructDef {
        name: String,
        doc_string: Option<DocStr>,
        body: Vec<Statement>,
    }
);

vyper_node!(
    struct EventDef {
        name: String,
        doc_string: Option<DocStr>,
        body: Vec<Statement>,
    }
);

vyper_node!(
    struct FlagDef {
        name: String,
        doc_string: Option<DocStr>,
        body: Vec<Statement>,
    }
);

vyper_node!(
    struct InterfaceDef {
        name: String,
        doc_string: Option<DocStr>,
        body: Vec<FunctionDef>,
    }
);

vyper_node!(
    struct Import {
        alias: Option<String>,
        name: String,
        import_info: ImportInfo
    }
);

vyper_node!(
    struct ImportFrom {
        alias: Option<String>,
        name: String,
        import_info: ImportInfo,
        module: Option<String>,
        level: u32,
    }
);

vyper_node!(
    struct ImplementsDecl {
        annotation: Box<Expression>,
    }
);

vyper_node!(
    struct UsesDecl {
        annotation: Box<Expression>,
    }
);

vyper_node!(
    struct InitializesDecl {
        annotation: Box<Expression>,
    }
);

vyper_node!(
    struct ExportsDecl {
        annotation: Box<Expression>,
    }
);

vyper_node!(
    struct DocStr {
        value: String,
    }
);

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

vyper_node!(
    struct AugAssign {}
);

vyper_node!(
    struct Return {
        value: Option<Box<Expression>>,
    }
);

vyper_node!(
    struct If {
        body: Vec<Statement>,
        test: Box<Expression>,
        orelse: Vec<Statement>,
    }
);

vyper_node!(
    struct For {
        iter: Box<Expression>,
        target: Box<Statement>,
        body: Vec<Statement>,
    }
);

vyper_node!(
    struct Break {}
);

vyper_node!(
    struct Continue {}
);

vyper_node!(
    struct Pass {}
);

vyper_node!(
    struct Raise {
        exc: Option<Box<Expression>>,
    }
);

vyper_node!(
    struct Assert {
        msg: Box<Expression>,
        test: Box<Expression>,
    }
);

vyper_node!(
    struct Log {
        value: Call,
        #[serde(rename = "type")]
        ttype: Option<Type>,
    }
);

vyper_node!(
    struct Expr {
        value: Box<Expression>,
    }
);

vyper_node!(
    struct NamedExpr {
        target: Box<Expression>,
        value: Box<Expression>,
    }
);

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

vyper_node!(
    struct Constant {
        value: serde_json::Value,
        #[serde(rename = "type")]
        ttype: Option<Type>,
    }
);

vyper_node!(
    struct Int {
        value: u64,
        #[serde(rename = "type")]
        ttype: Option<Type>,
    }
);

vyper_node!(
    struct Decimal {
        value: String,
        #[serde(rename = "type")]
        ttype: Option<Type>,
    }
);

vyper_node!(
    struct Hex {
        value: String,
        #[serde(rename = "type")]
        ttype: Option<Type>,
    }
);

vyper_node!(
    struct Str {
        value: String,
        #[serde(rename = "type")]
        ttype: Option<Type>,
    }
);

vyper_node!(
    struct Bytes {
        value: String,
        #[serde(rename = "type")]
        ttype: Option<Type>,
    }
);

vyper_node!(
    struct HexBytes {
        value: String,
        #[serde(rename = "type")]
        ttype: Option<Type>,
    }
);

vyper_node!(
    struct NameConstant {
        value: serde_json::Value,
        #[serde(rename = "type")]
        ttype: Option<Type>,
    }
);

vyper_node!(
    struct Ellipsis {
        value: String,
        #[serde(rename = "type")]
        ttype: Option<Type>,
    }
);

vyper_node!(
    struct List {
        elements: Vec<Expression>,
        #[serde(rename = "type")]
        ttype: Option<Type>,
    }
);

vyper_node!(
    struct Tuple {
        elements: Vec<Expression>,
    }
);

vyper_node!(
    struct Dict {
        keys: Vec<Expression>,
        values: Vec<Expression>,
    }
);

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
        ttype: Option<Type>,
        variable_reads: Option<Vec<VariableAccess>>,
        variable_writes: Option<Vec<VariableAccess>>,
    }
);

vyper_node!(
    struct Subscript {
        slice: Box<Expression>,
        value: Box<Expression>,
        #[serde(rename = "type")]
        ttype: Option<Type>,
        variable_reads: Option<Vec<VariableAccess>>,
        variable_writes: Option<Vec<VariableAccess>>,
    }
);

vyper_node!(
    struct UnaryOp {
        operand: Box<Expression>,
        #[serde(rename = "type")]
        ttype: Option<Type>,
    }
);

vyper_node!(
    struct BinOp {
        left: Box<Expression>,
        right: Box<Expression>,
        op: Box<BinaryOperator>,
        #[serde(rename = "type")]
        ttype: Option<Type>,
    }
);

vyper_node!(
    struct BoolOp {
        values: Vec<Expression>,
        op: Box<BooleanOperator>,
        #[serde(rename = "type")]
        ttype: Option<Type>,
    }
);

vyper_node!(
    struct Compare {
        left: Box<Expression>,
        right: Box<Expression>,
        op: Box<ComparisonOperator>,
        #[serde(rename = "type")]
        ttype: Option<Type>,
    }
);

vyper_node!(
    struct Call {
        args: Vec<Expression>,
        keywords: Vec<Keyword>,
        func: Box<Expression>,
        #[serde(rename = "type")]
        ttype: Option<Type>,
    }
);

vyper_node!(
    struct ExtCall {
        value: Box<Expression>,
        #[serde(rename = "type")]
        ttype: Option<Type>,
    }
);

vyper_node!(
    struct StaticCall {
        value: Box<Expression>,
        #[serde(rename = "type")]
        ttype: Option<Type>,
    }
);

vyper_node!(
    struct IfExp {
        test: Box<Expression>,
        body: Box<Expression>,
        orelse: Box<Expression>,
        #[serde(rename = "type")]
        ttype: Option<Type>,
    }
);

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

vyper_node!(
    struct Keyword {
        arg: String,
        value: Box<Expression>,
    }
);

node_group!(
    UnaryOperator;

    USub,
    Not,
    Invert,
);

basic_vyper_nodes!(
    USub, Not, Invert
);

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

basic_vyper_nodes!(Add, Sub, Mult, Div, FloorDiv, Mod, Pow, BitAnd, BitOr, BitXor, LShift, RShift);

node_group!(
    BooleanOperator;

    And,
    Or,
);

basic_vyper_nodes!(And, Or);

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

basic_vyper_nodes!(Eq, NotEq, Lt, LtE, Gt, GtE, In, NotIn);

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
    pub node_id: i32,
    pub source_id: i32,
}

// TODO these types could probably be converted to an enum because some fields are exclusive.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Type {
    pub name: Option<String>,
    pub value_type: Option<Box<Type>>,
    pub m: Option<u32>,
    pub typeclass: Option<String>,
    pub is_signed: Option<bool>,
    pub length: Option<u32>,
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
    pub node_id: i32,
    pub source_id: i32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportInfo {
    alias: Option<String>,
    qualified_module_name: String,
    source_id: i32,
    path: String,
    resolved_path: String,
    file_sha256sum: String,
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

use serde::{Deserialize, Serialize};

mod visitor;

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "ast_type")]
pub enum Node {
    Module(Module),
    Name(Name),
    VariableDecl(VariableDecl),
    AnnAssign(AnnAssign),
    EventDef(EventDef),
    FunctionDef(FunctionDef),
    #[serde(rename = "arguments")]
    Arguments(Arguments),
    Assign(Assign),
    Attribute(Attribute),
    Assert(Assert),
    Int(Int),
    Str(Str),
    Eq(Eq),
    Gt(Gt),
    Log(Log),
    Return(Return),
    #[serde(rename = "arg")]
    Arg(Arg),
    Compare(Compare),
    Call(Call),
    NotEq(NotEq),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Module {
    source_sha256sum: String,
    name: Option<String>,
    path: String,
    source_id: u64,
    is_interface: bool,
    doc_string: Option<String>,
    src: String,
    body: Vec<Node>,
    node_id: u64,
    end_col_offset: u64,
    col_offset: u64,
    settings: Settings,
    end_lineno: u64,
    resolved_path: String,
    lineno: u64,
    #[serde(rename = "type")]
    ttype: Type
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Settings {}

#[derive(Debug, Serialize, Deserialize)]
pub struct Name {
    end_lineno: u64,
    lineno: u64,
    id: String,
    col_offset: u64,
    node_id: u64,
    end_col_offset: u64,
    src: String,
    #[serde(rename = "type")]
    ttype: Option<Type>,
    variable_reads: Option<Vec<VariableAccess>>
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VariableDecl {
    end_lineno: u64,
    annotation: Box<Node>,
    src: String,
    value: Option<String>,
    col_offset: u64,
    is_transient: bool,
    is_constant: bool,
    is_reentrant: bool,
    is_public: bool,
    node_id: u64,
    target: Box<Node>,
    end_col_offset: u64,
    is_immutable: bool,
    lineno: u64,
    #[serde(rename = "type")]
    ttype: Type
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EventDef {
    end_lineno: u64,
    src: String,
    col_offset: u64,
    body: Vec<Node>,
    node_id: u64,
    doc_string: Option<String>,
    lineno: u64,
    end_col_offset: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FunctionDef {
    end_lineno: u64,
    src: String,
    col_offset: u64,
    args: Box<Node>,
    body: Vec<Node>,
    pos: Option<String>,
    node_id: u64,
    doc_string: Option<String>,
    decorator_list: Vec<Node>,
    name: String,
    lineno: u64,
    returns: Option<Box<Node>>,
    end_col_offset: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AnnAssign {
    end_lineno: u64,
    annotation: Box<Node>,
    src: String,
    col_offset: u64,
    value: Option<Box<Node>>,
    node_id: u64,
    lineno: u64,
    target: Box<Node>,
    end_col_offset: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Arguments {
    end_lineno: u64,
    default: Option<String>,
    src: String,
    col_offset: u64,
    args: Vec<Node>,
    node_id: u64,
    lineno: u64,
    defaults: Vec<String>,
    end_col_offset: u64
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Assign {
    end_lineno: u64,
    src: String,
    col_offset: u64,
    value: Box<Node>,
    node_id: u64,
    lineno: u64,
    target: Box<Node>,
    end_col_offset: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Attribute {
    end_lineno: u64,
    src: String,
    col_offset: u64,
    value: Box<Node>,
    node_id: u64,
    attr: String,
    lineno: u64,
    end_col_offset: u64,
    #[serde(rename = "type")]
    ttype: Type,
    variable_reads: Option<Vec<VariableAccess>>,
    variable_writes: Option<Vec<VariableAccess>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Assert {
    end_lineno: u64,
    lineno: u64,
    col_offset: u64,
    msg: Box<Node>,
    node_id: u64,
    test: Box<Node>,
    end_col_offset: u64,
    src: String
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Int {
    end_lineno: u64,
    lineno: u64,
    col_offset: u64,
    node_id: u64,
    value: u64,
    end_col_offset: u64,
    src: String,
    #[serde(rename = "type")]
    ttype: Type
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Str {
    end_lineno: u64,
    lineno: u64,
    col_offset: u64,
    node_id: u64,
    value: String,
    end_col_offset: u64,
    src: String,
    #[serde(rename = "type")]
    ttype: Type
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Eq {
    end_lineno: u64,
    lineno: u64,
    col_offset: u64,
    node_id: u64,
    end_col_offset: u64,
    src: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Gt {
    end_lineno: u64,
    lineno: u64,
    col_offset: u64,
    node_id: u64,
    end_col_offset: u64,
    src: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Log {
    end_lineno: u64,
    lineno: u64,
    col_offset: u64,
    node_id: u64,
    end_col_offset: u64,
    src: String,
    #[serde(rename = "type")]
    ttype: Type
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Return {
    end_lineno: u64,
    lineno: u64,
    col_offset: u64,
    node_id: u64,
    end_col_offset: u64,
    src: String,
    value: Box<Node>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Arg {
    end_lineno: u64,
    lineno: u64,
    col_offset: u64,
    node_id: u64,
    end_col_offset: u64,
    src: String,
    annotation: Box<Node>,
    arg: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Compare {
    left: Box<Node>,
    right: Box<Node>,
    op: Box<Node>,
    end_lineno: u64,
    lineno: u64,
    col_offset: u64,
    node_id: u64,
    end_col_offset: u64,
    src: String,
    #[serde(rename = "type")]
    ttype: Type
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Call {
    lineno: u64,
    node_id: u64,
    col_offset: u64,
    args: Vec<Node>,
    end_col_offset: u64,
    end_lineno: u64,
    src: String,
    keywords: Vec<Node>,
    func: Box<Node>,
    #[serde(rename = "type")]
    ttype: Type,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NotEq {
    lineno: u64,
    node_id: u64,
    col_offset: u64,
    end_col_offset: u64,
    end_lineno: u64,
    src: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VariableAccess {
    name: String,
    decl_node: DeclNode,
    access_path: Vec<String>
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeclNode {
    node_id: u64,
    source_id: u64
}

#[derive(Debug, Serialize, Deserialize)]
// #[serde(rename_all = "snake_case", tag = "typeclass")]
pub struct Type {
    pub name: Option<String>,
    pub typeclass: Option<String>,
    pub is_signed: Option<bool>,
    pub bits: Option<u32>,
    pub type_t: Option<TypedType>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TypedType {
    pub name: String,
    pub type_decl_node: Option<TypeDeclNode>,
    pub typeclass: Option<String>
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ModuleType {
    name: String,
    type_decl_node: TypeDeclNode,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StringType {
    length: u64,
    name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BuiltinFunctionType {
    name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TypeDeclNode {
    pub node_id: i64,
    pub source_id: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct IntegerType {
    is_signed: bool,
    bits: u32,
    name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EventType {
    name: String,
    type_decl_node: TypeDeclNode
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, path::Path};

    #[test]
    fn can_parse_ast() {
        fs::read_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../test-data").join("vyper_ast"))
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
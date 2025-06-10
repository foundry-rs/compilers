//! Bindings for solc's `ast` output field

use crate::serde_helpers;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{collections::BTreeMap, fmt, fmt::Write, str::FromStr};

/// Represents the AST field in the solc output
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ast {
    #[serde(rename = "absolutePath")]
    pub absolute_path: String,
    pub id: usize,
    #[serde(default, rename = "exportedSymbols")]
    pub exported_symbols: BTreeMap<String, Vec<usize>>,
    #[serde(rename = "nodeType")]
    pub node_type: NodeType,
    #[serde(with = "serde_helpers::display_from_str")]
    pub src: SourceLocation,
    #[serde(default)]
    pub nodes: Vec<Node>,

    /// Node attributes that were not deserialized.
    #[serde(flatten)]
    pub other: BTreeMap<String, serde_json::Value>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Node {
    /// The node ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<usize>,

    /// The node type.
    #[serde(rename = "nodeType")]
    pub node_type: NodeType,

    /// The location of the node in the source file.
    #[serde(with = "serde_helpers::display_from_str")]
    pub src: SourceLocation,

    /// Child nodes for some node types.
    #[serde(default)]
    pub nodes: Vec<Node>,

    /// Body node for some node types.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<Box<Node>>,

    /// Node attributes that were not deserialized.
    #[serde(flatten)]
    pub other: BTreeMap<String, serde_json::Value>,
}

impl Node {
    /// Deserialize a serialized node attribute.
    pub fn attribute<D: DeserializeOwned>(&self, key: &str) -> Option<D> {
        // TODO: Can we avoid this clone?
        self.other.get(key).and_then(|v| serde_json::from_value(v.clone()).ok())
    }
}

/// Represents the source location of a node: `<start byte>:<length>:<source index>`.
///
/// The `length` and `index` can be -1 which is represented as `None`
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SourceLocation {
    pub start: usize,
    pub length: Option<usize>,
    pub index: Option<usize>,
}

impl FromStr for SourceLocation {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let invalid_location = move || format!("{s} invalid source location");

        let mut split = s.split(':');
        let start = split
            .next()
            .ok_or_else(invalid_location)?
            .parse::<usize>()
            .map_err(|_| invalid_location())?;
        let length = split
            .next()
            .ok_or_else(invalid_location)?
            .parse::<isize>()
            .map_err(|_| invalid_location())?;
        let index = split
            .next()
            .ok_or_else(invalid_location)?
            .parse::<isize>()
            .map_err(|_| invalid_location())?;

        let length = if length < 0 { None } else { Some(length as usize) };
        let index = if index < 0 { None } else { Some(index as usize) };

        Ok(Self { start, length, index })
    }
}

impl fmt::Display for SourceLocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.start.fmt(f)?;
        f.write_char(':')?;
        if let Some(length) = self.length {
            length.fmt(f)?;
        } else {
            f.write_str("-1")?;
        }
        f.write_char(':')?;
        if let Some(index) = self.index {
            index.fmt(f)?;
        } else {
            f.write_str("-1")?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NodeType {
    // Expressions
    Assignment,
    BinaryOperation,
    Conditional,
    ElementaryTypeNameExpression,
    FunctionCall,
    FunctionCallOptions,
    Identifier,
    IndexAccess,
    IndexRangeAccess,
    Literal,
    MemberAccess,
    NewExpression,
    TupleExpression,
    UnaryOperation,

    // Statements
    Block,
    Break,
    Continue,
    DoWhileStatement,
    EmitStatement,
    ExpressionStatement,
    ForStatement,
    IfStatement,
    InlineAssembly,
    PlaceholderStatement,
    Return,
    RevertStatement,
    TryStatement,
    UncheckedBlock,
    VariableDeclarationStatement,
    VariableDeclaration,
    WhileStatement,

    // Yul statements
    YulAssignment,
    YulBlock,
    YulBreak,
    YulCase,
    YulContinue,
    YulExpressionStatement,
    YulLeave,
    YulForLoop,
    YulFunctionDefinition,
    YulIf,
    YulSwitch,
    YulVariableDeclaration,

    // Yul expressions
    YulFunctionCall,
    YulIdentifier,
    YulLiteral,

    // Yul literals
    YulLiteralValue,
    YulHexValue,
    YulTypedName,

    // Definitions
    ContractDefinition,
    FunctionDefinition,
    EventDefinition,
    ErrorDefinition,
    ModifierDefinition,
    StructDefinition,
    EnumDefinition,
    UserDefinedValueTypeDefinition,

    // Directives
    PragmaDirective,
    ImportDirective,
    UsingForDirective,

    // Misc
    SourceUnit,
    InheritanceSpecifier,
    ElementaryTypeName,
    FunctionTypeName,
    ParameterList,
    TryCatchClause,
    ModifierInvocation,

    /// An unknown AST node type.
    Other(String),
}

impl serde::Serialize for NodeType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            NodeType::Assignment => serializer.serialize_str("Assignment"),
            NodeType::BinaryOperation => serializer.serialize_str("BinaryOperation"),
            NodeType::Conditional => serializer.serialize_str("Conditional"),
            NodeType::ElementaryTypeNameExpression => {
                serializer.serialize_str("ElementaryTypeNameExpression")
            }
            NodeType::FunctionCall => serializer.serialize_str("FunctionCall"),
            NodeType::FunctionCallOptions => serializer.serialize_str("FunctionCallOptions"),
            NodeType::Identifier => serializer.serialize_str("Identifier"),
            NodeType::IndexAccess => serializer.serialize_str("IndexAccess"),
            NodeType::IndexRangeAccess => serializer.serialize_str("IndexRangeAccess"),
            NodeType::Literal => serializer.serialize_str("Literal"),
            NodeType::MemberAccess => serializer.serialize_str("MemberAccess"),
            NodeType::NewExpression => serializer.serialize_str("NewExpression"),
            NodeType::TupleExpression => serializer.serialize_str("TupleExpression"),
            NodeType::UnaryOperation => serializer.serialize_str("UnaryOperation"),
            NodeType::Block => serializer.serialize_str("Block"),
            NodeType::Break => serializer.serialize_str("Break"),
            NodeType::Continue => serializer.serialize_str("Continue"),
            NodeType::DoWhileStatement => serializer.serialize_str("DoWhileStatement"),
            NodeType::EmitStatement => serializer.serialize_str("EmitStatement"),
            NodeType::ExpressionStatement => serializer.serialize_str("ExpressionStatement"),
            NodeType::ForStatement => serializer.serialize_str("ForStatement"),
            NodeType::IfStatement => serializer.serialize_str("IfStatement"),
            NodeType::InlineAssembly => serializer.serialize_str("InlineAssembly"),
            NodeType::PlaceholderStatement => serializer.serialize_str("PlaceholderStatement"),
            NodeType::Return => serializer.serialize_str("Return"),
            NodeType::RevertStatement => serializer.serialize_str("RevertStatement"),
            NodeType::TryStatement => serializer.serialize_str("TryStatement"),
            NodeType::UncheckedBlock => serializer.serialize_str("UncheckedBlock"),
            NodeType::VariableDeclarationStatement => {
                serializer.serialize_str("VariableDeclarationStatement")
            }
            NodeType::VariableDeclaration => serializer.serialize_str("VariableDeclaration"),
            NodeType::WhileStatement => serializer.serialize_str("WhileStatement"),
            NodeType::YulAssignment => serializer.serialize_str("YulAssignment"),
            NodeType::YulBlock => serializer.serialize_str("YulBlock"),
            NodeType::YulBreak => serializer.serialize_str("YulBreak"),
            NodeType::YulCase => serializer.serialize_str("YulCase"),
            NodeType::YulContinue => serializer.serialize_str("YulContinue"),
            NodeType::YulExpressionStatement => serializer.serialize_str("YulExpressionStatement"),
            NodeType::YulLeave => serializer.serialize_str("YulLeave"),
            NodeType::YulForLoop => serializer.serialize_str("YulForLoop"),
            NodeType::YulFunctionDefinition => serializer.serialize_str("YulFunctionDefinition"),
            NodeType::YulIf => serializer.serialize_str("YulIf"),
            NodeType::YulSwitch => serializer.serialize_str("YulSwitch"),
            NodeType::YulVariableDeclaration => serializer.serialize_str("YulVariableDeclaration"),
            NodeType::YulFunctionCall => serializer.serialize_str("YulFunctionCall"),
            NodeType::YulIdentifier => serializer.serialize_str("YulIdentifier"),
            NodeType::YulLiteral => serializer.serialize_str("YulLiteral"),
            NodeType::YulLiteralValue => serializer.serialize_str("YulLiteralValue"),
            NodeType::YulHexValue => serializer.serialize_str("YulHexValue"),
            NodeType::YulTypedName => serializer.serialize_str("YulTypedName"),
            NodeType::ContractDefinition => serializer.serialize_str("ContractDefinition"),
            NodeType::FunctionDefinition => serializer.serialize_str("FunctionDefinition"),
            NodeType::EventDefinition => serializer.serialize_str("EventDefinition"),
            NodeType::ErrorDefinition => serializer.serialize_str("ErrorDefinition"),
            NodeType::ModifierDefinition => serializer.serialize_str("ModifierDefinition"),
            NodeType::StructDefinition => serializer.serialize_str("StructDefinition"),
            NodeType::EnumDefinition => serializer.serialize_str("EnumDefinition"),
            NodeType::UserDefinedValueTypeDefinition => {
                serializer.serialize_str("UserDefinedValueTypeDefinition")
            }
            NodeType::PragmaDirective => serializer.serialize_str("PragmaDirective"),
            NodeType::ImportDirective => serializer.serialize_str("ImportDirective"),
            NodeType::UsingForDirective => serializer.serialize_str("UsingForDirective"),
            NodeType::SourceUnit => serializer.serialize_str("SourceUnit"),
            NodeType::InheritanceSpecifier => serializer.serialize_str("InheritanceSpecifier"),
            NodeType::ElementaryTypeName => serializer.serialize_str("ElementaryTypeName"),
            NodeType::FunctionTypeName => serializer.serialize_str("FunctionTypeName"),
            NodeType::ParameterList => serializer.serialize_str("ParameterList"),
            NodeType::TryCatchClause => serializer.serialize_str("TryCatchClause"),
            NodeType::ModifierInvocation => serializer.serialize_str("ModifierInvocation"),
            NodeType::Other(s) => serializer.serialize_str(s),
        }
    }
}

impl<'de> serde::Deserialize<'de> for NodeType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(match s.as_str() {
            "Assignment" => NodeType::Assignment,
            "BinaryOperation" => NodeType::BinaryOperation,
            "Conditional" => NodeType::Conditional,
            "ElementaryTypeNameExpression" => NodeType::ElementaryTypeNameExpression,
            "FunctionCall" => NodeType::FunctionCall,
            "FunctionCallOptions" => NodeType::FunctionCallOptions,
            "Identifier" => NodeType::Identifier,
            "IndexAccess" => NodeType::IndexAccess,
            "IndexRangeAccess" => NodeType::IndexRangeAccess,
            "Literal" => NodeType::Literal,
            "MemberAccess" => NodeType::MemberAccess,
            "NewExpression" => NodeType::NewExpression,
            "TupleExpression" => NodeType::TupleExpression,
            "UnaryOperation" => NodeType::UnaryOperation,
            "Block" => NodeType::Block,
            "Break" => NodeType::Break,
            "Continue" => NodeType::Continue,
            "DoWhileStatement" => NodeType::DoWhileStatement,
            "EmitStatement" => NodeType::EmitStatement,
            "ExpressionStatement" => NodeType::ExpressionStatement,
            "ForStatement" => NodeType::ForStatement,
            "IfStatement" => NodeType::IfStatement,
            "InlineAssembly" => NodeType::InlineAssembly,
            "PlaceholderStatement" => NodeType::PlaceholderStatement,
            "Return" => NodeType::Return,
            "RevertStatement" => NodeType::RevertStatement,
            "TryStatement" => NodeType::TryStatement,
            "UncheckedBlock" => NodeType::UncheckedBlock,
            "VariableDeclarationStatement" => NodeType::VariableDeclarationStatement,
            "VariableDeclaration" => NodeType::VariableDeclaration,
            "WhileStatement" => NodeType::WhileStatement,
            "YulAssignment" => NodeType::YulAssignment,
            "YulBlock" => NodeType::YulBlock,
            "YulBreak" => NodeType::YulBreak,
            "YulCase" => NodeType::YulCase,
            "YulContinue" => NodeType::YulContinue,
            "YulExpressionStatement" => NodeType::YulExpressionStatement,
            "YulLeave" => NodeType::YulLeave,
            "YulForLoop" => NodeType::YulForLoop,
            "YulFunctionDefinition" => NodeType::YulFunctionDefinition,
            "YulIf" => NodeType::YulIf,
            "YulSwitch" => NodeType::YulSwitch,
            "YulVariableDeclaration" => NodeType::YulVariableDeclaration,
            "YulFunctionCall" => NodeType::YulFunctionCall,
            "YulIdentifier" => NodeType::YulIdentifier,
            "YulLiteral" => NodeType::YulLiteral,
            "YulLiteralValue" => NodeType::YulLiteralValue,
            "YulHexValue" => NodeType::YulHexValue,
            "YulTypedName" => NodeType::YulTypedName,
            "ContractDefinition" => NodeType::ContractDefinition,
            "FunctionDefinition" => NodeType::FunctionDefinition,
            "EventDefinition" => NodeType::EventDefinition,
            "ErrorDefinition" => NodeType::ErrorDefinition,
            "ModifierDefinition" => NodeType::ModifierDefinition,
            "StructDefinition" => NodeType::StructDefinition,
            "EnumDefinition" => NodeType::EnumDefinition,
            "UserDefinedValueTypeDefinition" => NodeType::UserDefinedValueTypeDefinition,
            "PragmaDirective" => NodeType::PragmaDirective,
            "ImportDirective" => NodeType::ImportDirective,
            "UsingForDirective" => NodeType::UsingForDirective,
            "SourceUnit" => NodeType::SourceUnit,
            "InheritanceSpecifier" => NodeType::InheritanceSpecifier,
            "ElementaryTypeName" => NodeType::ElementaryTypeName,
            "FunctionTypeName" => NodeType::FunctionTypeName,
            "ParameterList" => NodeType::ParameterList,
            "TryCatchClause" => NodeType::TryCatchClause,
            "ModifierInvocation" => NodeType::ModifierInvocation,
            _ => NodeType::Other(s),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn can_parse_ast() {
        let ast = include_str!("../../../../../test-data/ast/ast-erc4626.json");
        let _ast: Ast = serde_json::from_str(ast).unwrap();
    }

    #[test]
    fn test_unknown_node_type_deserialization() {
        // Test that unknown node types are properly handled with the Other variant
        let json = r#"{"nodeType": "SomeUnknownNodeType"}"#;
        let parsed: serde_json::Value = serde_json::from_str(json).unwrap();
        let node_type: NodeType = serde_json::from_value(parsed["nodeType"].clone()).unwrap();
        assert_eq!(node_type, NodeType::Other("SomeUnknownNodeType".to_string()));
    }

    #[test]
    fn test_known_node_type_deserialization() {
        // Test that known node types still work properly
        let json = r#"{"nodeType": "Assignment"}"#;
        let parsed: serde_json::Value = serde_json::from_str(json).unwrap();
        let node_type: NodeType = serde_json::from_value(parsed["nodeType"].clone()).unwrap();
        assert_eq!(node_type, NodeType::Assignment);
    }

    #[test]
    fn test_node_type_serialization() {
        // Test that serialization works correctly for both known and unknown node types
        let known = NodeType::Assignment;
        let unknown = NodeType::Other("SomeUnknownNodeType".to_string());

        let known_json = serde_json::to_string(&known).unwrap();
        let unknown_json = serde_json::to_string(&unknown).unwrap();

        assert_eq!(known_json, r#""Assignment""#);
        assert_eq!(unknown_json, r#""SomeUnknownNodeType""#);
    }

    #[test]
    fn test_node_type_roundtrip_serialization() {
        // Test roundtrip serialization for all known node types to ensure nothing is broken
        let test_cases = [
            NodeType::Assignment,
            NodeType::BinaryOperation,
            NodeType::FunctionCall,
            NodeType::ContractDefinition,
            NodeType::YulAssignment,
            NodeType::Other("CustomNodeType".to_string()),
        ];

        for original in test_cases {
            let serialized = serde_json::to_string(&original).unwrap();
            let deserialized: NodeType = serde_json::from_str(&serialized).unwrap();
            assert_eq!(original, deserialized);
        }
    }

    #[test]
    fn test_ast_node_with_unknown_type() {
        // Test that a complete Node with unknown nodeType can be parsed
        let json = r#"{
            "id": 1,
            "nodeType": "NewFancyNodeType", 
            "src": "0:0:0"
        }"#;

        let node: Node = serde_json::from_str(json).unwrap();
        assert_eq!(node.node_type, NodeType::Other("NewFancyNodeType".to_string()));
        assert_eq!(node.id, Some(1));
    }

    #[test]
    fn test_mixed_known_unknown_nodes() {
        // Test parsing a JSON structure with both known and unknown node types
        let json = r#"{
            "absolutePath": "/test/path.sol",
            "id": 0,
            "nodeType": "SourceUnit",
            "src": "0:100:0",
            "nodes": [
                {
                    "id": 1,
                    "nodeType": "Assignment",
                    "src": "10:20:0"
                },
                {
                    "id": 2, 
                    "nodeType": "FutureNodeType",
                    "src": "30:40:0"
                }
            ]
        }"#;

        let ast: Ast = serde_json::from_str(json).unwrap();
        assert_eq!(ast.node_type, NodeType::SourceUnit);
        assert_eq!(ast.nodes.len(), 2);
        assert_eq!(ast.nodes[0].node_type, NodeType::Assignment);
        assert_eq!(ast.nodes[1].node_type, NodeType::Other("FutureNodeType".to_string()));
    }
}

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
            Self::Assignment => serializer.serialize_str("Assignment"),
            Self::BinaryOperation => serializer.serialize_str("BinaryOperation"),
            Self::Conditional => serializer.serialize_str("Conditional"),
            Self::ElementaryTypeNameExpression => {
                serializer.serialize_str("ElementaryTypeNameExpression")
            }
            Self::FunctionCall => serializer.serialize_str("FunctionCall"),
            Self::FunctionCallOptions => serializer.serialize_str("FunctionCallOptions"),
            Self::Identifier => serializer.serialize_str("Identifier"),
            Self::IndexAccess => serializer.serialize_str("IndexAccess"),
            Self::IndexRangeAccess => serializer.serialize_str("IndexRangeAccess"),
            Self::Literal => serializer.serialize_str("Literal"),
            Self::MemberAccess => serializer.serialize_str("MemberAccess"),
            Self::NewExpression => serializer.serialize_str("NewExpression"),
            Self::TupleExpression => serializer.serialize_str("TupleExpression"),
            Self::UnaryOperation => serializer.serialize_str("UnaryOperation"),
            Self::Block => serializer.serialize_str("Block"),
            Self::Break => serializer.serialize_str("Break"),
            Self::Continue => serializer.serialize_str("Continue"),
            Self::DoWhileStatement => serializer.serialize_str("DoWhileStatement"),
            Self::EmitStatement => serializer.serialize_str("EmitStatement"),
            Self::ExpressionStatement => serializer.serialize_str("ExpressionStatement"),
            Self::ForStatement => serializer.serialize_str("ForStatement"),
            Self::IfStatement => serializer.serialize_str("IfStatement"),
            Self::InlineAssembly => serializer.serialize_str("InlineAssembly"),
            Self::PlaceholderStatement => serializer.serialize_str("PlaceholderStatement"),
            Self::Return => serializer.serialize_str("Return"),
            Self::RevertStatement => serializer.serialize_str("RevertStatement"),
            Self::TryStatement => serializer.serialize_str("TryStatement"),
            Self::UncheckedBlock => serializer.serialize_str("UncheckedBlock"),
            Self::VariableDeclarationStatement => {
                serializer.serialize_str("VariableDeclarationStatement")
            }
            Self::VariableDeclaration => serializer.serialize_str("VariableDeclaration"),
            Self::WhileStatement => serializer.serialize_str("WhileStatement"),
            Self::YulAssignment => serializer.serialize_str("YulAssignment"),
            Self::YulBlock => serializer.serialize_str("YulBlock"),
            Self::YulBreak => serializer.serialize_str("YulBreak"),
            Self::YulCase => serializer.serialize_str("YulCase"),
            Self::YulContinue => serializer.serialize_str("YulContinue"),
            Self::YulExpressionStatement => serializer.serialize_str("YulExpressionStatement"),
            Self::YulLeave => serializer.serialize_str("YulLeave"),
            Self::YulForLoop => serializer.serialize_str("YulForLoop"),
            Self::YulFunctionDefinition => serializer.serialize_str("YulFunctionDefinition"),
            Self::YulIf => serializer.serialize_str("YulIf"),
            Self::YulSwitch => serializer.serialize_str("YulSwitch"),
            Self::YulVariableDeclaration => serializer.serialize_str("YulVariableDeclaration"),
            Self::YulFunctionCall => serializer.serialize_str("YulFunctionCall"),
            Self::YulIdentifier => serializer.serialize_str("YulIdentifier"),
            Self::YulLiteral => serializer.serialize_str("YulLiteral"),
            Self::YulLiteralValue => serializer.serialize_str("YulLiteralValue"),
            Self::YulHexValue => serializer.serialize_str("YulHexValue"),
            Self::YulTypedName => serializer.serialize_str("YulTypedName"),
            Self::ContractDefinition => serializer.serialize_str("ContractDefinition"),
            Self::FunctionDefinition => serializer.serialize_str("FunctionDefinition"),
            Self::EventDefinition => serializer.serialize_str("EventDefinition"),
            Self::ErrorDefinition => serializer.serialize_str("ErrorDefinition"),
            Self::ModifierDefinition => serializer.serialize_str("ModifierDefinition"),
            Self::StructDefinition => serializer.serialize_str("StructDefinition"),
            Self::EnumDefinition => serializer.serialize_str("EnumDefinition"),
            Self::UserDefinedValueTypeDefinition => {
                serializer.serialize_str("UserDefinedValueTypeDefinition")
            }
            Self::PragmaDirective => serializer.serialize_str("PragmaDirective"),
            Self::ImportDirective => serializer.serialize_str("ImportDirective"),
            Self::UsingForDirective => serializer.serialize_str("UsingForDirective"),
            Self::SourceUnit => serializer.serialize_str("SourceUnit"),
            Self::InheritanceSpecifier => serializer.serialize_str("InheritanceSpecifier"),
            Self::ElementaryTypeName => serializer.serialize_str("ElementaryTypeName"),
            Self::FunctionTypeName => serializer.serialize_str("FunctionTypeName"),
            Self::ParameterList => serializer.serialize_str("ParameterList"),
            Self::TryCatchClause => serializer.serialize_str("TryCatchClause"),
            Self::ModifierInvocation => serializer.serialize_str("ModifierInvocation"),
            Self::Other(s) => serializer.serialize_str(s),
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
            "Assignment" => Self::Assignment,
            "BinaryOperation" => Self::BinaryOperation,
            "Conditional" => Self::Conditional,
            "ElementaryTypeNameExpression" => Self::ElementaryTypeNameExpression,
            "FunctionCall" => Self::FunctionCall,
            "FunctionCallOptions" => Self::FunctionCallOptions,
            "Identifier" => Self::Identifier,
            "IndexAccess" => Self::IndexAccess,
            "IndexRangeAccess" => Self::IndexRangeAccess,
            "Literal" => Self::Literal,
            "MemberAccess" => Self::MemberAccess,
            "NewExpression" => Self::NewExpression,
            "TupleExpression" => Self::TupleExpression,
            "UnaryOperation" => Self::UnaryOperation,
            "Block" => Self::Block,
            "Break" => Self::Break,
            "Continue" => Self::Continue,
            "DoWhileStatement" => Self::DoWhileStatement,
            "EmitStatement" => Self::EmitStatement,
            "ExpressionStatement" => Self::ExpressionStatement,
            "ForStatement" => Self::ForStatement,
            "IfStatement" => Self::IfStatement,
            "InlineAssembly" => Self::InlineAssembly,
            "PlaceholderStatement" => Self::PlaceholderStatement,
            "Return" => Self::Return,
            "RevertStatement" => Self::RevertStatement,
            "TryStatement" => Self::TryStatement,
            "UncheckedBlock" => Self::UncheckedBlock,
            "VariableDeclarationStatement" => Self::VariableDeclarationStatement,
            "VariableDeclaration" => Self::VariableDeclaration,
            "WhileStatement" => Self::WhileStatement,
            "YulAssignment" => Self::YulAssignment,
            "YulBlock" => Self::YulBlock,
            "YulBreak" => Self::YulBreak,
            "YulCase" => Self::YulCase,
            "YulContinue" => Self::YulContinue,
            "YulExpressionStatement" => Self::YulExpressionStatement,
            "YulLeave" => Self::YulLeave,
            "YulForLoop" => Self::YulForLoop,
            "YulFunctionDefinition" => Self::YulFunctionDefinition,
            "YulIf" => Self::YulIf,
            "YulSwitch" => Self::YulSwitch,
            "YulVariableDeclaration" => Self::YulVariableDeclaration,
            "YulFunctionCall" => Self::YulFunctionCall,
            "YulIdentifier" => Self::YulIdentifier,
            "YulLiteral" => Self::YulLiteral,
            "YulLiteralValue" => Self::YulLiteralValue,
            "YulHexValue" => Self::YulHexValue,
            "YulTypedName" => Self::YulTypedName,
            "ContractDefinition" => Self::ContractDefinition,
            "FunctionDefinition" => Self::FunctionDefinition,
            "EventDefinition" => Self::EventDefinition,
            "ErrorDefinition" => Self::ErrorDefinition,
            "ModifierDefinition" => Self::ModifierDefinition,
            "StructDefinition" => Self::StructDefinition,
            "EnumDefinition" => Self::EnumDefinition,
            "UserDefinedValueTypeDefinition" => Self::UserDefinedValueTypeDefinition,
            "PragmaDirective" => Self::PragmaDirective,
            "ImportDirective" => Self::ImportDirective,
            "UsingForDirective" => Self::UsingForDirective,
            "SourceUnit" => Self::SourceUnit,
            "InheritanceSpecifier" => Self::InheritanceSpecifier,
            "ElementaryTypeName" => Self::ElementaryTypeName,
            "FunctionTypeName" => Self::FunctionTypeName,
            "ParameterList" => Self::ParameterList,
            "TryCatchClause" => Self::TryCatchClause,
            "ModifierInvocation" => Self::ModifierInvocation,
            _ => Self::Other(s),
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

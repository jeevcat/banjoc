use std::collections::HashMap;

use serde::{Deserialize, Deserializer};

use crate::error::BanjoError;

pub type NodeId = String;

#[derive(Deserialize, Debug)]
pub struct Ast {
    #[serde(deserialize_with = "deserialize_nodes")]
    pub nodes: HashMap<NodeId, Node>,
}

impl Ast {
    pub fn get_node(&self, node_id: &str) -> Result<&Node, BanjoError> {
        self.nodes
            .get(node_id)
            .ok_or_else(|| BanjoError::compile(node_id, format!("Unknown node id {node_id}.")))
    }

    pub fn get_definitions(&self) -> impl Iterator<Item = &Node> {
        // TODO perf
        self.nodes.iter().map(|(_, node)| node).filter(|node| {
            matches!(
                node.node_type,
                NodeType::VariableDefinition { .. } | NodeType::FunctionDefinition { .. }
            )
        })
    }
}

#[derive(Deserialize, Debug)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum NodeType {
    Literal {
        value: LiteralType,
    },
    #[serde(alias = "call", rename_all = "camelCase")]
    FunctionCall {
        fn_node_id: NodeId,
        arguments: Vec<NodeId>,
    },
    #[serde(alias = "fn")]
    FunctionDefinition {
        arguments: Vec<NodeId>,
    },
    #[serde(alias = "ref")]
    VariableReference {
        var_node_id: NodeId,
    },
    #[serde(alias = "var")]
    VariableDefinition {
        arguments: Vec<NodeId>,
    },
    Param,
    Return {
        arguments: Vec<NodeId>,
    },
    Unary {
        unary_type: UnaryType,
        arguments: Vec<NodeId>,
    },
    Binary {
        binary_type: BinaryType,
        arguments: Vec<NodeId>,
    },
}

#[derive(Deserialize, Debug)]
#[serde(untagged, rename_all = "lowercase")]
pub enum LiteralType {
    Bool(bool),
    Nil,
    Number(f64),
    String(String),
}

#[derive(Deserialize, Debug)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum UnaryType {
    Negate,
    Not,
}

#[derive(Deserialize, Debug)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum BinaryType {
    #[serde(alias = "-")]
    Subtract,
    #[serde(alias = "/")]
    Divide,
    #[serde(alias = "==")]
    Equals,
    #[serde(alias = ">")]
    Greater,
    #[serde(alias = "<")]
    Less,
    #[serde(alias = "!=")]
    NotEquals,
    #[serde(alias = ">=")]
    GreaterEqual,
    #[serde(alias = "<=")]
    LessEqual,
}

#[derive(Deserialize, Debug)]
pub struct Node {
    pub id: NodeId,
    #[serde(flatten)]
    pub node_type: NodeType,
}

impl Node {
    pub fn arguments(&self) -> impl Iterator<Item = &str> {
        match &self.node_type {
            NodeType::FunctionDefinition { arguments }
            | NodeType::VariableDefinition { arguments }
            | NodeType::Return { arguments }
            | NodeType::Unary { arguments, .. }
            | NodeType::FunctionCall { arguments, .. }
            | NodeType::Binary { arguments, .. } => arguments.as_slice(),
            _ => &[],
        }
        .iter()
        .map(String::as_str)
    }
    pub fn dependencies(&self) -> impl Iterator<Item = &str> {
        match &self.node_type {
            NodeType::VariableReference { var_node_id } => Some(var_node_id.as_str()),
            NodeType::FunctionCall { fn_node_id, .. } => Some(fn_node_id.as_str()),
            _ => None,
        }
        .into_iter()
    }
}

fn deserialize_nodes<'de, D>(deserializer: D) -> Result<HashMap<NodeId, Node>, D::Error>
where
    D: Deserializer<'de>,
{
    let mut map = HashMap::new();
    for item in Vec::<Node>::deserialize(deserializer)? {
        map.insert(item.id.to_owned(), item);
    }
    Ok(map)
}

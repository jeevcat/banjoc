use std::collections::HashMap;

use serde::{Deserialize, Deserializer};

type NodeId = String;

#[derive(Deserialize, Debug)]
pub struct Ast {
    #[serde(deserialize_with = "deserialize_nodes")]
    nodes: HashMap<NodeId, Node>,
}

impl Ast {
    #[must_use]
    pub fn get_node(&self, node_id: &str) -> Option<&Node> {
        self.nodes.get(node_id)
    }

    #[must_use]
    pub fn get_return_node(&self) -> Option<&Node> {
        // TODO perf
        self.nodes
            .iter()
            .map(|(_, node)| node)
            .find(|node| matches!(node.node_type, NodeType::Return { .. }))
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
        value: NodeId,
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

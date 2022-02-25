use std::collections::HashMap;

use serde::{Deserialize, Deserializer};

pub type NodeId<'source> = &'source str;

#[derive(Deserialize, Debug)]
pub struct Ast<'source> {
    #[serde(borrow, deserialize_with = "deserialize_nodes")]
    nodes: HashMap<NodeId<'source>, Node<'source>>,
}

impl<'source> Ast<'source> {
    #[must_use]
    pub fn get_node(&self, node_id: NodeId) -> Option<&Node> {
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
#[serde(tag = "type", rename_all = "lowercase")]
pub enum NodeType<'source> {
    Literal {
        value: LiteralType<'source>,
    },
    #[serde(alias = "call")]
    FunctionCall {
        value: NodeId<'source>,
        arguments: Vec<NodeId<'source>>,
    },
    #[serde(alias = "fn")]
    FunctionDefinition {
        name: &'source str,
        arguments: Vec<NodeId<'source>>,
    },
    #[serde(alias = "ref")]
    VariableReference {
        value: NodeId<'source>,
    },
    #[serde(alias = "var")]
    VariableDefinition {
        name: &'source str,
        arguments: Vec<NodeId<'source>>,
    },
    Param {
        name: &'source str,
    },
    Return {
        arguments: Vec<NodeId<'source>>,
    },
    Unary {
        unary_type: UnaryType,
        arguments: Vec<NodeId<'source>>,
    },
    Binary {
        binary_type: BinaryType,
        arguments: Vec<NodeId<'source>>,
    },
}

#[derive(Deserialize, Debug)]
#[serde(untagged, rename_all = "lowercase")]
pub enum LiteralType<'source> {
    Bool(bool),
    Nil,
    Number(f64),
    String(&'source str),
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
pub struct Node<'source> {
    pub id: NodeId<'source>,
    #[serde(borrow, flatten)]
    pub node_type: NodeType<'source>,
}

fn deserialize_nodes<'de: 'source, 'source, D>(
    deserializer: D,
) -> Result<HashMap<&'source str, Node<'source>>, D::Error>
where
    D: Deserializer<'de>,
{
    let mut map = HashMap::new();
    for item in Vec::<Node>::deserialize(deserializer)? {
        map.insert(item.id, item);
    }
    Ok(map)
}

use std::collections::HashMap;

use serde::{Deserialize, Deserializer};

use crate::error::Error;

pub type NodeId = String;
type Nodes = HashMap<String, Node>;

#[derive(Deserialize, Debug)]
pub struct Source {
    #[serde(deserialize_with = "deserialize_nodes")]
    pub nodes: Nodes,
}

#[derive(Deserialize, Debug)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum NodeType {
    Const {
        value: LiteralType,
    },
    Literal {
        value: LiteralType,
    },
    #[serde(alias = "call", rename_all = "camelCase")]
    FunctionCall {
        fn_node_id: NodeId,
        #[serde(default)]
        args: Vec<NodeId>,
    },
    #[serde(alias = "fn")]
    FunctionDefinition {
        #[serde(default)]
        args: Vec<NodeId>,
    },
    #[serde(alias = "ref", rename_all = "camelCase")]
    VariableReference {
        var_node_id: NodeId,
    },
    #[serde(alias = "var")]
    VariableDefinition {
        #[serde(default)]
        args: Vec<NodeId>,
    },
    Param,
    Unary {
        unary_type: UnaryType,
        #[serde(default)]
        args: Vec<NodeId>,
    },
    Binary {
        binary_type: BinaryType,
        #[serde(default)]
        args: Vec<NodeId>,
    },
}

#[derive(Deserialize, Debug)]
#[serde(untagged, rename_all = "lowercase")]
pub enum LiteralType {
    Bool(bool),
    Nil,
    Number(f64),
    String(String),
    List(Vec<LiteralType>),
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
    pub fn args(&self) -> impl Iterator<Item = &str> {
        match &self.node_type {
            NodeType::FunctionDefinition { args, .. }
            | NodeType::VariableDefinition { args }
            | NodeType::Unary { args, .. }
            | NodeType::FunctionCall { args, .. }
            | NodeType::Binary { args, .. } => args.as_slice(),
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

fn deserialize_nodes<'de, D>(deserializer: D) -> Result<Nodes, D::Error>
where
    D: Deserializer<'de>,
{
    let mut map = HashMap::new();
    for item in Vec::<Node>::deserialize(deserializer)? {
        map.insert(item.id.clone(), item);
    }
    Ok(map)
}

impl Source {}

pub struct Ast<'source> {
    nodes: &'source Nodes,
    arities: HashMap<&'source str, usize>,
    roots: HashMap<&'source str, &'source Node>,
}

impl<'source> Ast<'source> {
    pub fn new(source: &'source Source) -> Self {
        let arities = Self::calculate_arities(&source.nodes);
        let roots = Self::find_roots(&source.nodes);
        Self {
            nodes: &source.nodes,
            arities,
            roots,
        }
    }

    pub fn get_node(&self, node_id: &str) -> Result<&Node, Error> {
        self.nodes
            .get(node_id)
            .ok_or_else(|| Error::node(node_id, format!("Unknown node id {node_id}.")))
    }

    pub fn get_arity(&self, fn_node_id: &str) -> Option<&usize> {
        #[cfg(debug_assertions)]
        {
            if let Ok(node) = self.get_node(fn_node_id) {
                assert!(matches!(
                    node.node_type,
                    NodeType::FunctionDefinition { .. }
                ));
            }
        }

        self.arities.get(fn_node_id)
    }

    pub fn get_roots(&self) -> impl Iterator<Item = &Node> {
        self.roots.values().map(|n| &**n)
    }

    fn find_roots(nodes: &Nodes) -> HashMap<&str, &Node> {
        let mut roots: HashMap<&str, &Node> =
            nodes.iter().map(|(id, n)| (id.as_str(), n)).collect();
        for node in nodes.values() {
            for arg in node.args() {
                roots.remove(arg);
            }
        }
        roots
    }

    fn calculate_arities(nodes: &Nodes) -> HashMap<&str, usize> {
        fn traverse(nodes: &Nodes, node_id: &str, current_arity: &mut usize) {
            if let Some(node) = nodes.get(node_id) {
                if let NodeType::Param = node.node_type {
                    *current_arity += 1;
                }
                for child_id in node.args() {
                    traverse(nodes, child_id, current_arity);
                }
            }
        }

        nodes
            .values()
            .filter_map(|node| {
                if let NodeType::FunctionDefinition { .. } = node.node_type {
                    let mut arity = 0_usize;
                    traverse(nodes, &node.id, &mut arity);
                    return Some((node.id.as_str(), arity));
                }
                None
            })
            .collect()
    }
}

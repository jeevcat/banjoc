use std::collections::HashMap;

use serde::{Deserialize, Deserializer};

pub type NodeId<'source> = &'source str;

#[derive(Deserialize, Debug)]
pub struct Ast<'source> {
    #[serde(borrow, deserialize_with = "deserialize_nodes")]
    nodes: HashMap<NodeId<'source>, Node<'source>>,
}

impl<'source> Ast<'source> {
    pub fn get_node(&self, node_id: NodeId) -> Option<&Node> {
        self.nodes.get(node_id)
    }

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
struct BaseNode<'source> {
    id: &'source str,
    comment: Option<&'source str>,
}

#[derive(Deserialize, Debug)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum NodeType<'source> {
    Literal {
        value: f64,
    },
    #[serde(alias = "call")]
    FunctionCall {
        name: &'source str,
        arguments: Vec<&'source str>,
    },
    #[serde(alias = "fn")]
    FunctionDefinition {
        name: &'source str,
        arguments: Vec<&'source str>,
    },
    #[serde(alias = "ref")]
    VariableReference {
        name: &'source str,
    },
    #[serde(alias = "var")]
    VariableDefinition {
        name: &'source str,
        arguments: Vec<&'source str>,
    },
    Param {
        name: &'source str,
    },
    Return {
        arguments: Vec<&'source str>,
    },
}

#[derive(Deserialize, Debug)]
pub struct Node<'source> {
    #[serde(borrow, flatten)]
    base: BaseNode<'source>,
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
        map.insert(item.base.id, item);
    }
    Ok(map)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_name() {
        let j = r#"
        {
            "nodes": [
              {
                "id": "return",
                "type": "return",
                "arguments": []
              },
              {
                "id": "sum",
                "type": "call",
                "name": "sum",
                "arguments": ["a", "b"]
              },
              {
                "id": "a",
                "type": "literal",
                "value": 1
              },
              {
                "id": "b",
                "type": "literal",
                "value": 2
              }
            ]
          }
        "#;
        println!("{:#?}", serde_json::from_str::<Ast>(j).unwrap());
    }
}

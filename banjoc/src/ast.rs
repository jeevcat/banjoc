use std::collections::HashMap;

use serde::{Deserialize, Deserializer};

#[derive(Deserialize, Debug)]
struct Ast<'source> {
    #[serde(borrow, deserialize_with = "deserialize_nodes")]
    nodes: HashMap<&'source str, Node<'source>>,
}

#[derive(Deserialize, Debug)]
struct BaseNode<'source> {
    id: &'source str,
    comment: Option<&'source str>,
    #[serde(default)]
    arguments: Vec<&'source str>,
}

#[derive(Deserialize, Debug)]
#[serde(tag = "type", rename_all = "lowercase")]
enum NodeType<'source> {
    Literal { value: f64 },
    Call { name: &'source str },
    Return,
}

#[derive(Deserialize, Debug)]
struct Node<'source> {
    #[serde(borrow, flatten)]
    base: BaseNode<'source>,
    #[serde(borrow, flatten)]
    node_type: NodeType<'source>,
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

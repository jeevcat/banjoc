use std::{
    collections::HashMap,
    fs::{read_dir, File},
    io::BufReader,
    path::Path,
};

use banjoc::{ast::NodeId, output::Output, value::Value, vm::Vm};
use serde::{de::DeserializeOwned, Deserialize};

#[test]
fn run_all_tests() {
    let dirs = read_dir("tests").expect("Failed to read directory");
    for maybe_entry in dirs {
        let entry = maybe_entry.expect("Failed to read entry");
        let name = entry.path();
        let name = name.to_str().expect("Failed to convert entry to string");
        if !name.ends_with(".json") || name.ends_with(".output.json") {
            continue;
        }
        let base = name.trim_end_matches(".json");
        dbg!(name);
        let source = read_from_file(name);
        let mut vm = Vm::new();
        let output = vm.interpret(source);
        let expected_output: TestOutput = read_from_file(format!("{base}.output.json"));
        assert_eq!(expected_output, output);
    }
}

fn read_from_file<P: AsRef<Path>, T: DeserializeOwned>(path: P) -> T {
    // Open the file in read-only mode with buffer.
    let file = File::open(&path).expect("Couldn't open file");
    let reader = BufReader::new(file);

    // Read the JSON contents of the file as an instance of `User`.
    serde_json::from_reader(reader).unwrap_or_else(|e| {
        panic!(
            "Couldn't deserialize JSON for {}, {e}",
            path.as_ref().display()
        )
    })
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TestOutput {
    #[serde(default)]
    node_values: HashMap<NodeId, TestValue>,
    #[serde(default)]
    node_errors: HashMap<NodeId, String>,
    #[serde(default)]
    additional_errors: Vec<String>,
}

#[derive(Deserialize, Debug, PartialEq)]
#[serde(untagged)]
pub enum TestValue {
    Bool(bool),
    Nil,
    Number(f64),
    // Following are pointers to garbage collected objects. Value is NOT deep copied.
    String(String),
}

impl PartialEq<Output> for TestOutput {
    fn eq(&self, other: &Output) -> bool {
        node_values_eq(&self.node_values, &other.node_values)
            && self.node_errors == other.errors.node_errors
            && self.additional_errors == other.errors.additional_errors
    }
}

fn node_values_eq(a: &HashMap<NodeId, TestValue>, b: &HashMap<NodeId, Value>) -> bool {
    if a.len() != b.len() {
        return false;
    }
    for (k, v) in b {
        if a.get(k)
            .unwrap_or_else(|| panic!("Expected key '{k}' with value '{v:?}'"))
            != v
        {
            return false;
        }
    }
    true
}

impl PartialEq<Value> for TestValue {
    fn eq(&self, other: &Value) -> bool {
        match self {
            TestValue::Bool(a) => {
                if let Value::Bool(b) = other {
                    a == b
                } else {
                    panic!("Expected bool")
                }
            }
            TestValue::Nil => {
                matches!(other, Value::Nil)
            }
            TestValue::Number(a) => {
                if let Value::Number(b) = other {
                    a == b
                } else {
                    panic!("Expected number")
                }
            }
            TestValue::String(a) => {
                if let Value::String(b) = other {
                    a.as_str() == b.as_str()
                } else {
                    panic!("Expected string")
                }
            }
        }
    }
}

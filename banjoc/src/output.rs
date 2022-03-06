use std::{collections::HashMap, mem};

use serde::Serialize;

use crate::{
    ast::NodeId,
    error::{Error, Result},
    value::Value,
};

type NodeValues = HashMap<NodeId, Value>;

#[derive(Default, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OutputErrors {
    pub node_errors: HashMap<NodeId, String>,
    pub additional_errors: Vec<String>,
}

impl OutputErrors {
    fn add(&mut self, error: Error) {
        match error {
            Error::Compile(s) => self.additional_errors.push(s),
            Error::Runtime(s) => self.additional_errors.push(s),
            Error::Node((n, s)) => {
                self.node_errors.insert(n, s);
            }
        }
    }
}
#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Output {
    pub node_values: NodeValues,
    #[serde(flatten)]
    pub errors: OutputErrors,
}

impl Output {
    pub fn from_single_error(error: Error) -> Self {
        let mut errors = OutputErrors::default();
        errors.add(error);
        Self {
            node_values: NodeValues::default(),
            errors,
        }
    }
}

#[derive(Default)]
pub struct OutputValues {
    /// Output values of nodes in order of execution. Indices correspond with
    /// `Compiler::output_nodes`.
    output_nodes: Vec<NodeId>,
    /// IDs of nodes in order of compilation
    output_values: Vec<Value>,
    errors: OutputErrors,
}

impl OutputValues {
    pub fn add_node(&mut self, node_id: &str) -> Result<u8> {
        if self.output_nodes.len() >= 255 {
            return Error::node_err(node_id, "Can't preview the output of more than 255 nodes");
        }
        self.output_nodes.push(node_id.to_string());
        let output_index = (self.output_nodes.len() - 1) as u8;
        Ok(output_index)
    }

    pub fn add_value(&mut self, output_index: u8, value: Value) {
        let min_len = (output_index + 1) as usize;
        if self.output_values.len() < min_len {
            self.output_values.resize_with(min_len, || Value::Nil);
        }
        self.output_values[output_index as usize] = value;
    }

    pub fn add_error(&mut self, error: Error) {
        self.errors.add(error)
    }

    pub fn take(&mut self) -> Output {
        let output_values = mem::take(&mut self.output_values);
        let output_nodes = mem::take(&mut self.output_nodes);
        debug_assert_eq!(output_nodes.len(), output_values.len());
        let node_values = output_nodes
            .into_iter()
            .zip(output_values.into_iter())
            .collect();

        Output {
            node_values,
            errors: mem::take(&mut self.errors),
        }
    }
}

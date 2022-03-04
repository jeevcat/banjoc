use std::collections::HashMap;

use crate::{
    ast::NodeId,
    error::{BanjoError, Result},
    value::Value,
};

pub type NodeOutputs = HashMap<NodeId, Value>;

#[derive(Default)]
pub struct Output {
    /// Output values of nodes in order of execution. Indices correspond with
    /// `Compiler::output_nodes`.
    output_nodes: Vec<NodeId>,
    /// IDs of nodes in order of compilation
    output_values: Vec<Value>,
}

impl Output {
    pub fn add_node(&mut self, node_id: &str) -> Result<u8> {
        if self.output_nodes.len() >= 255 {
            return BanjoError::compile_err(
                node_id,
                "Can't preview the output of more than 255 nodes",
            );
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

    pub fn take_node_outputs(&mut self) -> NodeOutputs {
        let output_values = std::mem::take(&mut self.output_values);
        let output_nodes = std::mem::take(&mut self.output_nodes);
        debug_assert_eq!(output_nodes.len(), output_values.len());
        let outputs: NodeOutputs = output_nodes
            .into_iter()
            .zip(output_values.into_iter())
            .collect();
        outputs
    }
}

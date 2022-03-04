use std::collections::HashMap;

use crate::ast::NodeId;

pub type Result<T> = std::result::Result<T, BanjoError>;
#[derive(Debug)]
pub enum BanjoError {
    Compile(String),
    /// A compile error with a known node
    Node((NodeId, String)),
    Runtime(String),
}

impl BanjoError {
    pub fn compile<M: Into<String>>(msg: M) -> Self {
        Self::Compile(msg.into())
    }
    pub fn compile_err<T, M: Into<String>>(msg: M) -> Result<T> {
        Err(Self::compile(msg))
    }
    pub fn node<N: Into<NodeId>, M: Into<String>>(node_id: N, msg: M) -> Self {
        Self::Node((node_id.into(), msg.into()))
    }
    pub fn node_err<T, N: Into<NodeId>, M: Into<String>>(node_id: N, msg: M) -> Result<T> {
        Err(Self::node(node_id, msg))
    }
    pub fn runtime<M: Into<String>>(msg: M) -> Self {
        Self::Runtime(msg.into())
    }
    pub fn runtime_err<T, M: Into<String>>(msg: M) -> Result<T> {
        Err(Self::runtime(msg))
    }

    fn node_context(self, node_id: &str) -> BanjoError {
        match self {
            Self::Compile(s) => Self::node(node_id, s),
            _ => unreachable!(),
        }
    }
}

pub trait Context<T> {
    /// Wrap the error value with additional context.
    fn node_context(self, node_id: &str) -> Result<T>;

    /// Wrap the error value with additional context that is evaluated lazily
    /// only once an error does occur.
    fn with_node_context<'node, F>(self, node_id: F) -> Result<T>
    where
        F: FnOnce() -> &'node str;
}

impl<T> Context<T> for Result<T> {
    fn node_context(self, node_id: &str) -> Result<T> {
        self.map_err(|error| error.node_context(node_id))
    }

    fn with_node_context<'node, F>(self, f: F) -> Result<T>
    where
        F: FnOnce() -> &'node str,
    {
        self.map_err(|error| error.node_context(f()))
    }
}

pub type NodeErrors = HashMap<NodeId, BanjoError>;

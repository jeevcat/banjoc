use std::collections::HashMap;

use crate::ast::NodeId;

pub type Result<T> = std::result::Result<T, BanjoError>;
#[derive(Debug)]
pub enum BanjoError {
    Compile(String),
    CompileNode((NodeId, String)),
    Runtime(String),
    CompileErrors(Vec<(NodeId, String)>),
}

impl BanjoError {
    pub fn compile<N: Into<String>, M: Into<String>>(node_id: N, msg: M) -> Self {
        Self::CompileNode((node_id.into(), msg.into()))
    }
    pub fn compile_err<T, N: Into<String>, M: Into<String>>(node_id: N, msg: M) -> Result<T> {
        Err(Self::compile(node_id, msg))
    }
    pub fn runtime<M: Into<String>>(msg: M) -> Self {
        Self::Runtime(msg.into())
    }
    pub fn runtime_err<T, M: Into<String>>(msg: M) -> Result<T> {
        Err(Self::runtime(msg))
    }
    pub fn append(&mut self, other: Self) {
        match self {
            BanjoError::CompileErrors(this) => match other {
                BanjoError::CompileNode(other) => this.push(other),
                BanjoError::CompileErrors(mut other) => this.append(&mut other),
                BanjoError::Runtime(_) | BanjoError::Compile(_) => {}
            },
            BanjoError::CompileNode(_) | BanjoError::Runtime(_) | BanjoError::Compile(_) => {}
        }
    }
    pub fn to_result<T>(self, value: T) -> Result<T> {
        match &self {
            BanjoError::CompileErrors(errors) => {
                if errors.is_empty() {
                    return Ok(value);
                }
                Err(self)
            }
            _ => Err(self),
        }
    }

    fn node_context(self, node_id: &str) -> BanjoError {
        match self {
            Self::Compile(s) => Self::CompileNode((node_id.to_string(), s)),
            _ => unreachable!(),
        }
    }
}

pub fn append<T>(result: &mut Result<T>, other: BanjoError) {
    match result {
        Ok(_) => *result = Err(other),
        Err(e) => e.append(other),
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

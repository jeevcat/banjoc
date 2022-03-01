use std::collections::HashMap;

use crate::ast::NodeId;

pub type Result<T> = std::result::Result<T, BanjoError>;
#[derive(Debug)]
pub enum BanjoError {
    CompileError((String, String)),
    CompileErrors(Vec<(String, String)>),
    RuntimeError(String),
}

impl BanjoError {
    pub fn compile<N: Into<String>, M: Into<String>>(node_id: N, msg: M) -> Self {
        Self::CompileError((node_id.into(), msg.into()))
    }
    pub fn compile_err<T, N: Into<String>, M: Into<String>>(node_id: N, msg: M) -> Result<T> {
        Err(Self::compile(node_id, msg))
    }
    pub fn append(&mut self, other: Self) {
        match self {
            BanjoError::CompileErrors(this) => match other {
                BanjoError::CompileError(other) => this.push(other),
                BanjoError::CompileErrors(mut other) => this.append(&mut other),
                BanjoError::RuntimeError(_) => {}
            },
            BanjoError::CompileError(_) | BanjoError::RuntimeError(_) => {}
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
}

pub fn append<T>(result: &mut Result<T>, other: BanjoError) {
    match result {
        Ok(_) => *result = Err(other),
        Err(e) => e.append(other),
    }
}

pub type NodeErrors = HashMap<NodeId, BanjoError>;

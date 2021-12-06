use std::fmt::Display;

use crate::{
    gc::GcRef,
    obj::{Closure, Function, LoxString, NativeFunction},
};

#[derive(Clone, Copy)]
pub enum Value {
    Bool(bool),
    Nil,
    Number(f64),
    // Following are pointers to garbage collected objects. Value is NOT deep copied.
    String(GcRef<LoxString>),
    Function(GcRef<Function>),
    NativeFunction(GcRef<NativeFunction>),
    Closure(GcRef<Closure>),
}

impl Value {
    pub fn is_falsey(&self) -> bool {
        match self {
            Value::Bool(b) => !b,
            Value::Nil => true,
            _ => false,
        }
    }
}

impl Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Bool(x) => x.fmt(f),
            Value::Nil => f.write_str("nil"),
            Value::Number(x) => x.fmt(f),
            Value::String(x) => x.fmt(f),
            Value::Function(x) => x.fmt(f),
            Value::NativeFunction(x) => x.fmt(f),
            Value::Closure(x) => x.fmt(f),
        }
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Number(a), Value::Number(b)) => a == b,
            (Value::Nil, Value::Nil) => true,
            (Value::String(a), Value::String(b)) => a == b,
            (Value::Function(a), Value::Function(b)) => a == b,
            (Value::NativeFunction(a), Value::NativeFunction(b)) => a == b,
            (Value::Closure(a), Value::Closure(b)) => a == b,
            _ => false,
        }
    }
}

impl Default for Value {
    fn default() -> Self {
        Self::Nil
    }
}

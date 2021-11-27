use std::{fmt::Display, ops::Deref, ptr::NonNull};

use crate::{gc::GcRef, obj::Obj};

#[derive(Clone, Copy)]
pub enum Value {
    Bool(bool),
    Nil,
    Number(f64),
    // Pointer to a garbage collected object. Value is NOT deep copied.
    Obj(GcRef<Obj>),
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
            Value::Obj(x) => x.fmt(f),
        }
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Number(a), Value::Number(b)) => a == b,
            (Value::Nil, Value::Nil) => true,
            _ => false,
        }
    }
}

use std::{
    fmt,
    fmt::{Debug, Display, Formatter},
    ops::Deref,
};

use crate::{
    error::{BanjoError, Result},
    gc::{GarbageCollect, Gc, GcRef},
    obj::{BanjoString, Function, NativeFunction},
    vm::Vm,
};

#[derive(Clone, Copy)]
pub enum Value {
    Bool(bool),
    Nil,
    Number(f64),
    // Following are pointers to garbage collected objects. Value is NOT deep copied.
    String(GcRef<BanjoString>),
    NativeFunction(GcRef<NativeFunction>),
    Function(GcRef<Function>),
}

impl Value {
    pub fn is_falsey(&self) -> bool {
        match self {
            Value::Bool(b) => !b,
            Value::Nil => true,
            _ => false,
        }
    }

    pub fn add(self, rhs: Self, vm: &mut Vm) -> Result<Self> {
        match (self, rhs) {
            (Value::Number(a), Value::Number(b)) => Ok(Value::Number(a + b)),
            (Value::String(a), Value::String(b)) => Ok(Value::String(vm.intern(&format!(
                "{}{}",
                a.as_str(),
                b.as_str()
            )))),
            _ => Err(BanjoError::RuntimeError(
                "Operands must be two numbers or two strings.".to_string(),
            )),
        }
    }

    pub fn binary_op(self, rhs: Self, f: impl Fn(f64, f64) -> Value) -> Result<Self> {
        match (self, rhs) {
            (Value::Number(a), Value::Number(b)) => Ok(f(a, b)),
            _ => Err(BanjoError::RuntimeError(
                "Operands must be numbers.".to_string(),
            )),
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
            (Value::NativeFunction(a), Value::NativeFunction(b)) => a == b,
            (Value::Function(a), Value::Function(b)) => a == b,
            _ => false,
        }
    }
}

impl Display for Value {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Value::Bool(x) => Display::fmt(&x, f),
            Value::Nil => f.write_str("nil"),
            Value::Number(x) => Display::fmt(&x, f),
            Value::String(x) => Display::fmt(x.deref(), f),
            Value::NativeFunction(x) => Display::fmt(x.deref(), f),
            Value::Function(x) => Display::fmt(x.deref(), f),
        }
    }
}

impl Debug for Value {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(self, f)
    }
}

impl Default for Value {
    fn default() -> Self {
        Self::Nil
    }
}

impl GarbageCollect for Value {
    fn mark_gray(&mut self, gc: &mut Gc) {
        match self {
            Value::String(x) => x.mark_gray(gc),
            Value::NativeFunction(x) => x.mark_gray(gc),
            Value::Function(x) => x.mark_gray(gc),
            _ => {}
        }
    }
}

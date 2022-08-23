use std::{
    fmt,
    fmt::{Debug, Formatter},
    iter,
};

use serde::{ser::SerializeSeq, Serialize, Serializer};

use crate::{
    error::{Error, Result},
    gc::{GarbageCollect, Gc, GcRef},
    obj::{BanjoString, Function, List, NativeFunction},
    vm::Vm,
};

#[derive(Clone, Copy)]
pub enum Value {
    Nil,
    Bool(bool),
    Number(f64),
    // Following are pointers to garbage collected objects. Value is NOT deep copied.
    String(GcRef<BanjoString>),
    List(GcRef<List>),
    NativeFunction(GcRef<NativeFunction>),
    Function(GcRef<Function>),
}

impl Value {
    #[must_use]
    pub fn is_falsey(&self) -> bool {
        match self {
            Value::Bool(b) => !b,
            Value::Nil => true,
            _ => false,
        }
    }

    pub fn add(self, rhs: Self, vm: &mut Vm) -> Self {
        // Adding to nil or functions is basically a noop
        if matches!(
            self,
            Value::Nil | Value::Function(_) | Value::NativeFunction(_)
        ) {
            return rhs;
        }
        if matches!(
            rhs,
            Value::Nil | Value::Function(_) | Value::NativeFunction(_)
        ) {
            return self;
        }

        // Lists addition is element-wise
        if let Value::List(a) = self {
            if let Value::List(b) = rhs {
                let values = if a.values.len() > b.values.len() {
                    a.values
                        .iter()
                        .zip(b.values.iter().chain(iter::repeat(&Value::Nil)))
                        .map(|(a, b)| a.add(*b, vm))
                        .collect()
                } else {
                    b.values
                        .iter()
                        .zip(a.values.iter().chain(iter::repeat(&Value::Nil)))
                        .map(|(a, b)| b.add(*a, vm))
                        .collect()
                };

                return Value::List(vm.alloc(List::new(values)));
            } else {
                let values = a.values.iter().map(|v| v.add(rhs, vm)).collect();
                return Value::List(vm.alloc(List::new(values)));
            }
        }
        if let Value::List(b) = rhs {
            let values = b.values.iter().map(|v| self.add(*v, vm)).collect();
            return Value::List(vm.alloc(List::new(values)));
        }

        match self {
            Value::Bool(a) => match rhs {
                Value::Bool(b) => Value::Number(a as i32 as f64 + b as i32 as f64),
                Value::Number(b) => Value::Number(a as i32 as f64 + b),
                Value::String(b) => Value::String(vm.intern(&format!("{}{}", a, b.as_str()))),
                Value::NativeFunction(_) | Value::Function(_) | Value::List(_) | Value::Nil => {
                    unreachable!()
                }
            },
            Value::Number(a) => match rhs {
                Value::Bool(b) => Value::Number(a + b as i32 as f64),
                Value::Number(b) => Value::Number(a + b),
                Value::String(b) => Value::String(vm.intern(&format!("{}{}", a, b.as_str()))),
                Value::NativeFunction(_) | Value::Function(_) | Value::List(_) | Value::Nil => {
                    unreachable!()
                }
            },
            Value::String(a) => match rhs {
                Value::Bool(b) => Value::String(vm.intern(&format!("{}{}", a.as_str(), b))),
                Value::Number(b) => Value::String(vm.intern(&format!("{}{}", a.as_str(), b))),
                Value::String(b) => {
                    Value::String(vm.intern(&format!("{}{}", a.as_str(), b.as_str())))
                }
                Value::NativeFunction(_) | Value::Function(_) | Value::List(_) | Value::Nil => {
                    unreachable!()
                }
            },
            Value::NativeFunction(_) | Value::Function(_) | Value::List(_) | Value::Nil => {
                unreachable!()
            }
        }
    }

    pub fn binary_op(self, rhs: Self, f: impl Fn(f64, f64) -> Value) -> Result<Self> {
        match (self, rhs) {
            (Value::Number(a), Value::Number(b)) => Ok(f(a, b)),
            _ => Error::runtime_err("Operands must be numbers."),
        }
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Nil, Value::Nil) => true,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Number(a), Value::Number(b)) => a == b,
            (Value::String(a), Value::String(b)) => a == b,
            (Value::List(a), Value::List(b)) => a == b,
            (Value::NativeFunction(a), Value::NativeFunction(b)) => a == b,
            (Value::Function(a), Value::Function(b)) => a == b,
            _ => false,
        }
    }
}

impl Debug for Value {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Value::Nil => f.write_str("nil"),
            Value::Bool(x) => Debug::fmt(&x, f),
            Value::Number(x) => Debug::fmt(&x, f),
            Value::String(x) => Debug::fmt(&**x, f),
            Value::List(x) => Debug::fmt(&**x, f),
            Value::NativeFunction(x) => Debug::fmt(&**x, f),
            Value::Function(x) => Debug::fmt(&**x, f),
        }
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

impl Serialize for Value {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Value::Bool(b) => serializer.serialize_bool(*b),
            Value::Nil => serializer.serialize_none(),
            Value::Number(n) => serializer.serialize_f64(*n),
            Value::String(s) => serializer.serialize_str(s.as_str()),
            Value::List(l) => {
                let mut seq = serializer.serialize_seq(Some(l.values.len()))?;
                for element in &l.values {
                    seq.serialize_element(element)?;
                }
                seq.end()
            }
            Value::NativeFunction(_) | Value::Function(_) => {
                serializer.serialize_str(&format!("{self:?}"))
            }
        }
    }
}

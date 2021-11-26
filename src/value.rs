use std::fmt::Display;

#[derive(Clone, Copy)]
pub enum Value {
    Bool(bool),
    Nil,
    Number(f64),
}

impl Value {
    pub fn is_falsey(&self) -> bool {
        match self {
            Value::Bool(b) => !b,
            Value::Nil => true,
            Value::Number(_) => false,
        }
    }
}

impl Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Bool(x) => x.fmt(f),
            Value::Nil => f.write_str("nil"),
            Value::Number(x) => x.fmt(f),
        }
    }
}

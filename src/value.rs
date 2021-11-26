use std::fmt::Display;

#[derive(Clone, Copy)]
pub enum Value {
    Bool(bool),
    Nil,
    Number(f64),
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

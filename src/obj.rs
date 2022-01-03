use std::{
    fmt::{self, Display, Formatter, Write},
    ops::Deref,
};

use crate::{
    chunk::Chunk,
    gc::{GcRef, ObjHeader},
    table::Table,
    value::Value,
    vm::ValueStack,
};

#[derive(Clone, Copy)]
pub enum ObjectType {
    String,
    Graph,
    NativeFunction,
    Closure,
    Upvalue,
}

pub struct LoxString {
    pub header: ObjHeader,
    string: String,
    pub hash: u32,
}

impl LoxString {
    pub fn new(string: String) -> LoxString {
        let hash = hash_string(&string);
        LoxString {
            header: ObjHeader::new(ObjectType::String),
            string,
            hash,
        }
    }

    pub fn as_str(&self) -> &str {
        self.string.as_str()
    }
}

impl Display for LoxString {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.string, f)
    }
}

pub fn hash_string(string: &str) -> u32 {
    // FNV-1a
    let mut hash = 2166136261u32;
    for c in string.bytes() {
        hash ^= c as u32;
        hash = hash.wrapping_mul(16777619u32);
    }
    hash
}

pub struct FunctionUpvalue {
    pub index: u8,
    pub is_local: bool,
}

pub struct Graph {
    pub header: ObjHeader,
    pub arity: usize,
    pub chunk: Chunk,
    pub name: Option<GcRef<LoxString>>,
    pub upvalues: Vec<FunctionUpvalue>,
}

impl Graph {
    pub fn new(name: Option<GcRef<LoxString>>) -> Self {
        Self {
            header: ObjHeader::new(ObjectType::Graph),
            arity: 0,
            chunk: Chunk::new(),
            name,
            upvalues: vec![],
        }
    }
}

impl Display for Graph {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        if let Some(name) = self.name {
            f.write_str("<fn ")?;
            Display::fmt(&name.string, f)?;
            f.write_char('>')?;
        } else {
            f.write_str("<script>")?;
        }
        Ok(())
    }
}

pub type NativeFn = fn(args: &[Value]) -> Value;
pub struct NativeFunction {
    pub header: ObjHeader,
    pub function: NativeFn,
}

impl NativeFunction {
    pub fn new(function: NativeFn) -> Self {
        Self {
            header: ObjHeader::new(ObjectType::NativeFunction),
            function,
        }
    }
}

impl Display for NativeFunction {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str("<native fn>")?;
        Ok(())
    }
}

pub struct Closure {
    pub header: ObjHeader,
    pub function: GcRef<Graph>,
    pub upvalues: Vec<GcRef<Upvalue>>,
}

impl Closure {
    pub fn new(function: GcRef<Graph>) -> Self {
        let upvalues = Vec::with_capacity(function.upvalues.len());
        Self {
            header: ObjHeader::new(ObjectType::Closure),
            upvalues,
            function,
        }
    }
}

impl Display for Closure {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(self.function.deref(), f)
    }
}

pub struct Upvalue {
    pub header: ObjHeader,
    /// Index of the closed-over variable in the locals stack
    pub location: usize,
    pub closed: Option<Value>,
    pub next: Option<GcRef<Upvalue>>,
}

impl Upvalue {
    pub fn new(location: usize, next: Option<GcRef<Upvalue>>) -> Self {
        Self {
            header: ObjHeader::new(ObjectType::Upvalue),
            location,
            closed: None,
            next,
        }
    }

    pub fn read(&self, stack: &ValueStack) -> Value {
        if let Some(closed) = self.closed {
            closed
        } else {
            *stack.read(self.location)
        }
    }

    pub fn write(&mut self, stack: &mut ValueStack) {
        let value = *stack.peek(0);
        if self.closed.is_some() {
            self.closed = Some(value);
        } else {
            stack.write(self.location, value);
        }
    }
}

impl Display for Upvalue {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str("upvalue")
    }
}

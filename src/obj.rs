use std::fmt::{Debug, Display, Formatter, Write};

use crate::{
    chunk::Chunk,
    gc::{GcRef, ObjHeader},
    value::Value,
};

pub struct LoxString {
    pub header: ObjHeader,
    string: String,
    pub hash: u32,
}

impl LoxString {
    pub fn new(string: String) -> LoxString {
        let hash = hash_string(&string);
        LoxString {
            header: ObjHeader::new(),
            string,
            hash,
        }
    }

    pub fn as_str(&self) -> &str {
        self.string.as_str()
    }
}

impl Display for LoxString {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
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

pub struct Function {
    pub header: ObjHeader,
    pub arity: usize,
    pub chunk: Chunk,
    pub name: Option<GcRef<LoxString>>,
    pub upvalue_count: usize,
}

impl Function {
    pub fn new(name: Option<GcRef<LoxString>>) -> Self {
        Self {
            header: ObjHeader::new(),
            arity: 0,
            chunk: Chunk::new(),
            name,
            upvalue_count: 0,
        }
    }
}

impl Display for Function {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
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

impl Debug for Function {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(self, f)
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
            header: ObjHeader::new(),
            function,
        }
    }
}

impl Display for NativeFunction {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("<native fn>")?;
        Ok(())
    }
}

pub struct Closure {
    pub header: ObjHeader,
    pub function: GcRef<Function>,
    pub upvalues: Vec<GcRef<Upvalue>>,
}

impl Closure {
    pub fn new(function: GcRef<Function>) -> Self {
        let upvalues = Vec::with_capacity(function.upvalue_count);
        Self {
            header: ObjHeader::new(),
            upvalues,
            function,
        }
    }
}

impl Display for Closure {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&*self.function, f)
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
    pub fn new(location: usize) -> Self {
        Self {
            header: ObjHeader::new(),
            location,
            closed: None,
            next: None,
        }
    }
}

impl Display for Upvalue {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("upvalue")
    }
}

use std::fmt::{self, Display, Formatter, Write};

use crate::{
    chunk::Chunk,
    error::Result,
    gc::{GcRef, ObjHeader},
    value::Value,
    vm::Vm,
};

#[derive(Clone, Copy)]
pub enum ObjectType {
    String,
    NativeFunction,
    Function,
}

pub struct BanjoString {
    pub header: ObjHeader,
    string: String,
    pub hash: u32,
}

impl BanjoString {
    pub fn new(string: String) -> BanjoString {
        let hash = hash_string(&string);
        BanjoString {
            header: ObjHeader::new(ObjectType::String),
            string,
            hash,
        }
    }

    pub fn as_str(&self) -> &str {
        self.string.as_str()
    }
}

impl Display for BanjoString {
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

pub struct Function {
    pub header: ObjHeader,
    pub arity: usize,
    pub chunk: Chunk,
    pub name: Option<GcRef<BanjoString>>,
}

impl Function {
    pub fn new(name: Option<GcRef<BanjoString>>) -> Self {
        Self {
            header: ObjHeader::new(ObjectType::Function),
            arity: 0,
            chunk: Chunk::new(),
            name,
        }
    }
}

impl Display for Function {
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

pub type NativeFn = fn(args: &[Value], vm: &mut Vm) -> Result<Value>;
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

use std::fmt::{self, Debug, Formatter, Write};

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
    List,
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

impl Debug for BanjoString {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Debug::fmt(&self.string, f)
    }
}

pub fn hash_string(string: &str) -> u32 {
    // FNV-1a
    let mut hash = 2_166_136_261_u32;
    for c in string.bytes() {
        hash ^= u32::from(c);
        hash = hash.wrapping_mul(16_777_619_u32);
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
    pub fn new(name: Option<GcRef<BanjoString>>, arity: usize) -> Self {
        Self {
            header: ObjHeader::new(ObjectType::Function),
            arity,
            chunk: Chunk::new(),
            name,
        }
    }
}

impl Debug for Function {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        if let Some(name) = self.name {
            f.write_str("<fn ")?;
            Debug::fmt(&name.string, f)?;
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

impl Debug for NativeFunction {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str("<native fn>")?;
        Ok(())
    }
}

pub struct List {
    pub header: ObjHeader,
    pub values: Vec<Value>,
}

impl List {
    pub fn new(values: Vec<Value>) -> Self {
        Self {
            header: ObjHeader::new(ObjectType::List),
            values,
        }
    }
}

impl Debug for List {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Debug::fmt(&self.values, f)
    }
}

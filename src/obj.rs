use std::fmt::{Display, Write};

use crate::{
    chunk::Chunk,
    gc::{GcRef, ObjHeader},
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
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.string.fmt(f)
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
    pub arity: i32,
    pub chunk: Chunk,
    pub name: Option<GcRef<LoxString>>,
}

impl Function {
    pub fn new(name: Option<GcRef<LoxString>>) -> Self {
        Self {
            header: ObjHeader::new(),
            arity: 0,
            chunk: Chunk::new(),
            name,
        }
    }
}

impl Display for Function {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(name) = self.name {
            f.write_str("<fn ")?;
            name.string.fmt(f)?;
            f.write_char('>')?;
        } else {
            f.write_str("<script>")?;
        }
        Ok(())
    }
}

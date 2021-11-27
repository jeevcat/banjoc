use std::fmt::Display;

use crate::gc::GcRef;

// Obj is a wrapper which is copied around on the stack, and refers to gc objects on the heap
#[derive(Clone, Copy)]
pub enum Obj {
    String(GcRef<LoxString>),
}

impl Obj {
    // #TODO Could optimize this with punning: mem::transmute
    pub fn header(&mut self) -> &mut ObjHeader {
        match self {
            Obj::String(s) => &mut s.header,
        }
    }
}

impl Display for Obj {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Obj::String(x) => x.fmt(f),
        }
    }
}

pub struct ObjHeader {
    pub next: Option<Obj>,
}

pub struct LoxString {
    pub header: ObjHeader,
    pub string: String,
    pub hash: u32,
}

impl LoxString {
    pub fn new(string: String) -> LoxString {
        let hash = hash_string(&string);
        LoxString {
            header: ObjHeader { next: None },
            string,
            hash,
        }
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

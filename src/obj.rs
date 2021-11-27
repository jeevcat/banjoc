use std::fmt::Display;

use crate::gc::GcRef;

/// Garbage collected object
/// This object is always allocated on the heap by Gc
pub enum Obj {
    String(LoxString),
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
    pub next: Option<GcRef<Obj>>,
}

pub struct LoxString {
    header: ObjHeader,
    pub string: String,
}

impl LoxString {
    pub fn new(string: String) -> LoxString {
        LoxString {
            header: ObjHeader { next: None },
            string,
        }
    }
}

impl Display for LoxString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.string.fmt(f)
    }
}

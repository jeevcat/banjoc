use std::{
    fmt::Display,
    marker::Sized,
    ops::{Deref, DerefMut},
    ptr::NonNull,
};

use crate::{
    obj::{hash_string, Function, LoxString},
    table::Table,
    value::Value,
};

// Basically a NonNull but allows derefing
// Should be passed around by value
pub struct GcRef<T> {
    pub pointer: NonNull<T>,
}

impl<T> GcRef<T> {
    pub fn dangling() -> Self {
        Self {
            pointer: NonNull::dangling(),
        }
    }

    pub fn drop_ptr(self) {
        unsafe { std::ptr::drop_in_place(self.pointer.as_ptr()) }
    }
}

impl<T> Deref for GcRef<T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { self.pointer.as_ref() }
    }
}

impl<T> DerefMut for GcRef<T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { self.pointer.as_mut() }
    }
}

impl<T> Copy for GcRef<T> {}

impl<T> Clone for GcRef<T> {
    fn clone(&self) -> GcRef<T> {
        *self
    }
}

impl<T> PartialEq for GcRef<T> {
    fn eq(&self, other: &Self) -> bool {
        self.pointer == other.pointer
    }
}

pub struct ObjHeader {
    next: Option<Obj>,
}

impl ObjHeader {
    pub fn new() -> Self {
        Self { next: None }
    }
}

/// Obj is a wrapper which is copied around on the stack, and refers to an object tracked by the garbage collector
/// Only used (so far) to make the linked list which the GC uses to keep track of all objects
#[derive(Clone, Copy)]
pub enum Obj {
    String(GcRef<LoxString>),
    Function(GcRef<Function>),
}

impl Obj {
    // #TODO Could optimize this with punning: mem::transmute
    pub fn header(&mut self) -> &mut ObjHeader {
        match self {
            Obj::String(x) => &mut x.header,
            Obj::Function(x) => &mut x.header,
        }
    }

    pub fn drop_inner(self) {
        match self {
            Obj::String(x) => x.drop_ptr(),
            Obj::Function(x) => x.drop_ptr(),
        }
    }

    fn make<T>(gc_ref: GcRef<T>) -> Self
    where
        T: MakeObj,
    {
        MakeObj::make_obj(gc_ref)
    }
}

impl Display for Obj {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Obj::String(x) => x.fmt(f),
            Obj::Function(x) => x.fmt(f),
        }
    }
}

pub trait MakeObj {
    fn make_obj(gc_ref: GcRef<Self>) -> Obj
    where
        Self: Sized;
}

impl MakeObj for LoxString {
    fn make_obj(gc_ref: GcRef<Self>) -> Obj
    where
        Self: Sized,
    {
        Obj::String(gc_ref)
    }
}

impl MakeObj for Function {
    fn make_obj(gc_ref: GcRef<Self>) -> Obj
    where
        Self: Sized,
    {
        Obj::Function(gc_ref)
    }
}

pub struct Gc {
    /// Linked list of all objects tracked by the garbage collector
    first: Option<Obj>,
    /// Table of interned strings
    strings: Table,
}

impl Gc {
    pub fn new() -> Self {
        Self {
            first: None,
            strings: Table::new(),
        }
    }

    pub fn intern(&mut self, string: String) -> GcRef<LoxString> {
        let hash = hash_string(&string);

        if let Some(interned) = self.strings.find_string(&string, hash) {
            interned
        } else {
            let ls = self.alloc(LoxString::new(string));
            self.strings.insert(ls, Value::Nil);
            ls
        }
    }

    /// Move the provided object to the heap and track with the garbage collector
    pub fn alloc<T>(&mut self, object: T) -> GcRef<T>
    where
        T: MakeObj,
    {
        // TODO https://users.rust-lang.org/t/how-to-create-large-objects-directly-in-heap/26405

        // Move the passed in object to new space allocated on the heap
        let boxed = Box::new(object);
        let pointer = unsafe {
            GcRef {
                pointer: NonNull::new_unchecked(
                    // into_raw here prevents the object from be dropped at the end of this scope. Now we are responsible!
                    Box::into_raw(boxed),
                ),
            }
        };

        let mut obj = Obj::make(pointer);

        // Adjust linked list pointers
        obj.header().next = self.first.take();
        self.first = Some(obj);

        pointer
    }
}

impl Drop for Gc {
    fn drop(&mut self) {
        let mut obj = self.first.take();
        while let Some(mut next) = obj {
            println!("Dropping: {}", next);
            let maybe_next = next.header().next;
            next.drop_inner();
            obj = maybe_next;
        }
    }
}

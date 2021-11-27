use std::{
    ops::{Deref, DerefMut},
    ptr::NonNull,
};

use crate::{
    obj::{hash_string, LoxString, Obj},
    table::Table,
    value::Value,
};

// Basically a NonNull but allows derefing
// Should be passed around by value
pub struct GcRef<T> {
    pub pointer: NonNull<T>,
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

pub struct Gc {
    first: Option<Obj>,
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
            let ls = self.alloc(string);
            self.strings.insert(ls, Value::Nil);
            ls
        }
    }

    pub fn alloc(&mut self, string: String) -> GcRef<LoxString> {
        // Allocate a new LoxString Obj on the heap
        let ls = LoxString::new(string);
        let mut boxed = Box::new(ls);

        // Adjust linked list pointers
        boxed.header.next = self.first.take();
        let pointer = unsafe {
            GcRef {
                pointer: NonNull::new_unchecked(
                    // into_raw here prevents the object from be dropped at the end of this scope. Now we are responsible!
                    Box::into_raw(boxed),
                ),
            }
        };
        self.first = Some(Obj::String(pointer));

        pointer
    }
}

impl Drop for Gc {
    fn drop(&mut self) {
        let mut obj = self.first.take();
        while let Some(mut inner) = obj {
            println!("Dropping: {}", inner);
            let next = inner.header().next;
            match inner {
                Obj::String(s) => unsafe { std::ptr::drop_in_place(s.pointer.as_ptr()) },
            }
            obj = next;
        }
    }
}

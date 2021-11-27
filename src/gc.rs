use std::{
    ops::{Deref, DerefMut},
    ptr::NonNull,
};

use crate::obj::{LoxString, Obj};

// Basically a NonNull but allows derefing
pub struct GcRef<T> {
    pointer: NonNull<T>,
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

pub struct Gc {
    first: Option<GcRef<Obj>>,
}

impl Gc {
    pub fn new() -> Self {
        Self { first: None }
    }

    pub fn alloc(&mut self, string: String) -> GcRef<Obj> {
        // Allocate a new LoxString Obj on the heap
        let ls = LoxString::new(string);
        let obj = Obj::String(ls);
        let mut boxed = Box::new(obj);

        // Adjust linked list pointers
        boxed.header().next = self.first.take();
        // into_raw here prevents the object from be dropped at the end of this scope. Now we are responsible!
        let pointer = unsafe {
            GcRef {
                pointer: NonNull::new_unchecked(Box::into_raw(boxed)),
            }
        };
        self.first = Some(pointer);

        pointer
    }
}

impl Drop for Gc {
    fn drop(&mut self) {
        let mut obj = self.first.take();
        while let Some(mut inner) = obj {
            println!("Dropping: {}", inner.deref());
            let next = inner.header().next;
            unsafe { std::ptr::drop_in_place(inner.pointer.as_ptr()) };
            obj = next;
        }
    }
}

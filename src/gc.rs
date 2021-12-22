use std::{
    fmt::{Debug, Display},
    marker::Sized,
    mem,
    ops::{Deref, DerefMut},
    ptr::NonNull,
};

use crate::{
    obj::{hash_string, Closure, Function, LoxString, NativeFunction, Upvalue},
    table::Table,
    value::Value,
};

// TODO Currently we use the following systems which all track the same structs
// - struct GcRef<T>
// - enum Obj
// - trait MakeObj
// The following tracks more than just the Object structs (can stay like this)
// - trait GarbageCollect

// Basically a NonNull but allows derefing
// Should be passed around by value
pub struct GcRef<T> {
    pub pointer: NonNull<T>,
}

impl<T: Display> GcRef<T> {
    pub fn dangling() -> Self {
        Self {
            pointer: NonNull::dangling(),
        }
    }

    pub fn drop_ptr(self) {
        #[cfg(feature = "debug_log_gc")]
        {
            println!("{:?} free {}", self.pointer.as_ptr(), self.deref());
        }
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

impl<T: Display> Debug for GcRef<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.deref().fmt(f)
    }
}

pub trait GarbageCollect {
    fn mark(&mut self, gc: &mut Gc);
}

impl<T> GarbageCollect for GcRef<T>
where
    T: GarbageCollect + MakeObj + Display,
{
    fn mark(&mut self, gc: &mut Gc) {
        if self.is_marked() {
            return;
        }
        #[cfg(feature = "debug_log_gc")]
        {
            // TODO How can we debug information about the outer object
        }
        println!("Marked {}", **self);
        self.deref_mut().mark(gc);
        gc.gray_stack.push(Obj::make(*self));
    }
}

impl GarbageCollect for LoxString {
    fn mark(&mut self, _gc: &mut Gc) {
        self.header.mark()
    }
}

impl GarbageCollect for Function {
    fn mark(&mut self, _gc: &mut Gc) {
        self.header.mark()
    }
}

impl GarbageCollect for Closure {
    fn mark(&mut self, _gc: &mut Gc) {
        self.header.mark()
    }
}

impl GarbageCollect for NativeFunction {
    fn mark(&mut self, _gc: &mut Gc) {
        self.header.mark()
    }
}

pub struct ObjHeader {
    next: Option<Obj>,
    is_marked: bool,
}

impl ObjHeader {
    pub fn new() -> Self {
        Self {
            next: None,
            is_marked: false,
        }
    }

    pub fn mark(&mut self) {
        self.is_marked = true;
    }
}

/// Obj is a wrapper which is copied around on the stack, and refers to an object tracked by the garbage collector
/// Only used (so far) to make the linked list which the GC uses to keep track of all objects
#[derive(Clone, Copy)]
pub enum Obj {
    String(GcRef<LoxString>),
    Function(GcRef<Function>),
    NativeFunction(GcRef<NativeFunction>),
    Closure(GcRef<Closure>),
    Upvalue(GcRef<Upvalue>),
}

impl Obj {
    // #TODO Could optimize this with punning: mem::transmute
    pub fn header(&mut self) -> &mut ObjHeader {
        match self {
            Obj::String(x) => &mut x.header,
            Obj::Function(x) => &mut x.header,
            Obj::NativeFunction(x) => &mut x.header,
            Obj::Closure(x) => &mut x.header,
            Obj::Upvalue(x) => &mut x.header,
        }
    }

    pub fn drop_inner(self) {
        match self {
            Obj::String(x) => x.drop_ptr(),
            Obj::Function(x) => x.drop_ptr(),
            Obj::NativeFunction(x) => x.drop_ptr(),
            Obj::Closure(x) => x.drop_ptr(),
            Obj::Upvalue(x) => x.drop_ptr(),
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
            Obj::NativeFunction(x) => x.fmt(f),
            Obj::Closure(x) => x.fmt(f),
            Obj::Upvalue(x) => x.fmt(f),
        }
    }
}

pub trait MakeObj {
    fn make_obj(gc_ref: GcRef<Self>) -> Obj
    where
        Self: Sized;

    fn is_marked(&self) -> bool;
}

impl MakeObj for LoxString {
    fn make_obj(gc_ref: GcRef<Self>) -> Obj
    where
        Self: Sized,
    {
        Obj::String(gc_ref)
    }

    fn is_marked(&self) -> bool {
        self.header.is_marked
    }
}

impl MakeObj for Function {
    fn make_obj(gc_ref: GcRef<Self>) -> Obj
    where
        Self: Sized,
    {
        Obj::Function(gc_ref)
    }

    fn is_marked(&self) -> bool {
        self.header.is_marked
    }
}

impl MakeObj for NativeFunction {
    fn make_obj(gc_ref: GcRef<Self>) -> Obj
    where
        Self: Sized,
    {
        Obj::NativeFunction(gc_ref)
    }

    fn is_marked(&self) -> bool {
        self.header.is_marked
    }
}

impl MakeObj for Closure {
    fn make_obj(gc_ref: GcRef<Self>) -> Obj
    where
        Self: Sized,
    {
        Obj::Closure(gc_ref)
    }

    fn is_marked(&self) -> bool {
        self.header.is_marked
    }
}

impl MakeObj for Upvalue {
    fn make_obj(gc_ref: GcRef<Self>) -> Obj
    where
        Self: Sized,
    {
        Obj::Upvalue(gc_ref)
    }

    fn is_marked(&self) -> bool {
        self.header.is_marked
    }
}

pub struct Gc {
    /// Linked list of all objects tracked by the garbage collector
    first: Option<Obj>,
    /// Table of interned strings
    strings: Table,
    gray_stack: Vec<Obj>,
    bytes_allocated: usize,
    next_gc: usize,
}

impl Gc {
    const HEAP_GROW_FACTOR: usize = 2;

    pub fn new() -> Self {
        Self {
            first: None,
            strings: Table::new(),
            gray_stack: Vec::new(),
            bytes_allocated: 0,
            next_gc: 1024 * 1024,
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
        T: MakeObj + Display,
    {
        self.bytes_allocated += mem::size_of_val(&object);
        if self.bytes_allocated > self.next_gc {
            self.collect_garbage();
        }
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

        #[cfg(feature = "debug_log_gc")]
        {
            println!(
                "{:?} allocate {} for {}",
                pointer.pointer.as_ptr(),
                mem::size_of_val(pointer.deref()),
                pointer.deref()
            );
        }

        pointer
    }

    pub fn collect_garbage(&mut self) {
        #[cfg(feature = "debug_log_gc")]
        let before = self.bytes_allocated;
        #[cfg(feature = "debug_log_gc")]
        println!("-- gc begin");

        self.trace_references();
        self.strings.remove_white();
        self.sweep();

        self.next_gc = self.bytes_allocated * Self::HEAP_GROW_FACTOR;

        #[cfg(feature = "debug_log_gc")]
        {
            println!("-- gc end");
            println!(
                "   collected {} bytes (from {} to {}) next at {}",
                before - self.bytes_allocated,
                before,
                self.bytes_allocated,
                self.next_gc
            );
        }
    }

    fn trace_references(&mut self) {
        while let Some(obj) = self.gray_stack.pop() {
            self.blacken_object(obj);
        }
    }

    fn blacken_object(&mut self, obj: Obj) {
        #[cfg(feature = "debug_log_gc")]
        {
            println!("blacken {}", obj);
        }

        // Mark all outgoing references
        match obj {
            Obj::String(_) => {
                // No outgoing references
            }
            Obj::NativeFunction(_) => {
                // No outgoing references
            }
            Obj::Upvalue(upvalue) => {
                if let Some(mut closed) = upvalue.closed {
                    closed.mark(self);
                }
            }
            Obj::Function(mut function) => {
                if let Some(mut name) = function.name {
                    name.mark(self);
                }
                for constant in &mut function.chunk.constants {
                    constant.mark(self);
                }
            }
            Obj::Closure(mut closure) => {
                closure.function.mark(self);
                for i in 0..closure.upvalues.len() {
                    closure.upvalues[i].mark(self);
                }
            }
        }
    }

    fn sweep(&mut self) {
        let mut prev = None;
        let mut maybe_obj = self.first;
        // Walk through the linked list of every object in the heap, checking if marked
        while let Some(mut obj) = maybe_obj {
            if obj.header().is_marked {
                // Skip marked (black) objects, but unmark for next run
                obj.header().is_marked = false;
                prev = maybe_obj;
                maybe_obj = obj.header().next;
                println!("Not dropping {}", obj);
            } else {
                // Unlink and free unmarked (white) objects
                let unreached = obj;
                maybe_obj = obj.header().next;
                if let Some(mut prev) = prev {
                    prev.header().next = maybe_obj;
                } else {
                    self.first = maybe_obj;
                }

                println!("Dropping {}", obj);
                self.bytes_allocated -= mem::size_of_val(&unreached);
                unreached.drop_inner();
            }
        }
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

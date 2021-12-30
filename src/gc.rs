use std::{
    fmt::Display,
    mem,
    ops::{Deref, DerefMut},
    ptr::NonNull,
};

use crate::{
    obj::{
        hash_string, BoundMethod, Class, Closure, Function, Instance, LoxString, NativeFunction,
        ObjectType, Upvalue,
    },
    table::Table,
    value::Value,
};

struct HeaderPtr(NonNull<ObjHeader>);
impl HeaderPtr {
    fn size_of_val(&self) -> usize {
        match self.obj_type {
            ObjectType::String => mem::size_of::<LoxString>(),
            ObjectType::Function => mem::size_of::<Function>(),
            ObjectType::NativeFunction => mem::size_of::<NativeFunction>(),
            ObjectType::Closure => mem::size_of::<Closure>(),
            ObjectType::Upvalue => mem::size_of::<Upvalue>(),
            ObjectType::Class => mem::size_of::<Class>(),
            ObjectType::Instance => mem::size_of::<Instance>(),
            ObjectType::BoundMethod => mem::size_of::<BoundMethod>(),
        }
    }

    fn transmute<T>(&self) -> GcRef<T> {
        unsafe { mem::transmute(self.0.as_ref()) }
    }

    fn drop_ptr(&mut self) {
        // Must transmute to drop the full object, not just the header
        match self.obj_type {
            ObjectType::String => self.transmute::<LoxString>().drop_ptr(),
            ObjectType::Function => self.transmute::<Function>().drop_ptr(),
            ObjectType::NativeFunction => self.transmute::<NativeFunction>().drop_ptr(),
            ObjectType::Closure => self.transmute::<Closure>().drop_ptr(),
            ObjectType::Upvalue => self.transmute::<Upvalue>().drop_ptr(),
            ObjectType::Class => self.transmute::<Class>().drop_ptr(),
            ObjectType::Instance => self.transmute::<Instance>().drop_ptr(),
            ObjectType::BoundMethod => self.transmute::<BoundMethod>().drop_ptr(),
        }
    }
}

impl Copy for HeaderPtr {}

impl Clone for HeaderPtr {
    fn clone(&self) -> HeaderPtr {
        *self
    }
}

impl Deref for HeaderPtr {
    type Target = ObjHeader;

    fn deref(&self) -> &Self::Target {
        unsafe { self.0.as_ref() }
    }
}

impl DerefMut for HeaderPtr {
    fn deref_mut(&mut self) -> &mut ObjHeader {
        unsafe { self.0.as_mut() }
    }
}

impl Display for HeaderPtr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.obj_type {
            ObjectType::String => self.transmute::<LoxString>().fmt(f),
            ObjectType::Function => self.transmute::<Function>().fmt(f),
            ObjectType::NativeFunction => self.transmute::<NativeFunction>().fmt(f),
            ObjectType::Closure => self.transmute::<Closure>().fmt(f),
            ObjectType::Upvalue => self.transmute::<Upvalue>().fmt(f),
            ObjectType::Class => self.transmute::<Class>().fmt(f),
            ObjectType::Instance => self.transmute::<Instance>().fmt(f),
            ObjectType::BoundMethod => self.transmute::<BoundMethod>().fmt(f),
        }
    }
}

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

    pub fn is_marked(&self) -> bool {
        self.header().is_marked
    }

    fn drop_ptr(self) {
        #[cfg(feature = "debug_log_gc")]
        {
            println!("{:?} free {}", self.pointer.as_ptr(), self.deref());
        }
        unsafe { std::ptr::drop_in_place(self.pointer.as_ptr()) }
    }

    fn header(&self) -> HeaderPtr {
        unsafe { mem::transmute(self.deref()) }
    }

    fn size_of_val(&self) -> usize {
        mem::size_of_val(self.deref())
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

pub trait GarbageCollect {
    fn mark_gray(&mut self, gc: &mut Gc);
}

impl<T> GarbageCollect for GcRef<T>
where
    T: Display,
{
    fn mark_gray(&mut self, gc: &mut Gc) {
        if self.is_marked() {
            return;
        }
        #[cfg(feature = "debug_log_gc")]
        println!("Marked {}", **self);

        self.header().mark();
        gc.gray_stack.push(self.header());
    }
}

pub struct ObjHeader {
    obj_type: ObjectType,
    next: Option<HeaderPtr>,
    is_marked: bool,
}

impl ObjHeader {
    pub fn new(obj_type: ObjectType) -> Self {
        Self {
            obj_type,
            next: None,
            is_marked: false,
        }
    }

    pub fn mark(&mut self) {
        self.is_marked = true;
    }
}

pub struct Gc {
    /// Linked list of all objects tracked by the garbage collector
    first: Option<HeaderPtr>,
    /// Table of interned strings
    strings: Table,
    gray_stack: Vec<HeaderPtr>,
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

    pub fn intern(&mut self, string: &str) -> GcRef<LoxString> {
        let hash = hash_string(string);

        if let Some(interned) = self.strings.find_string(string, hash) {
            interned
        } else {
            let ls = self.alloc(LoxString::new(string.to_string()));
            self.strings.insert(ls, Value::Nil);
            ls
        }
    }

    /// Move the provided object to the heap and track with the garbage collector
    pub fn alloc<T>(&mut self, object: T) -> GcRef<T>
    where
        T: Display,
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

        let mut obj = pointer.header();

        // Adjust linked list pointers
        obj.next = self.first.take();
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

        self.bytes_allocated += pointer.size_of_val();

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

        if self.bytes_allocated > 0 {
            self.next_gc = self.bytes_allocated * Self::HEAP_GROW_FACTOR;
        }

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

    fn blacken_object(&mut self, obj: HeaderPtr) {
        // A black object is any object who is marked and is no longer in the gray stack
        #[cfg(feature = "debug_log_gc")]
        println!("Blacken {}", obj);

        // Mark all outgoing references
        match obj.obj_type {
            ObjectType::String => {
                // No outgoing references
            }
            ObjectType::NativeFunction => {
                // No outgoing references
            }
            ObjectType::Upvalue => {
                let upvalue = obj.transmute::<Upvalue>();
                // Only closed over values which are no longer on the stack need to be garbage collected
                if let Some(mut closed) = upvalue.closed {
                    closed.mark_gray(self);
                }
            }
            ObjectType::Function => {
                let mut function = obj.transmute::<Function>();
                if let Some(mut name) = function.name {
                    name.mark_gray(self);
                }
                for constant in &mut function.chunk.constants {
                    constant.mark_gray(self);
                }
            }
            ObjectType::Closure => {
                let mut closure = obj.transmute::<Closure>();
                closure.function.mark_gray(self);
                for i in 0..closure.upvalues.len() {
                    closure.upvalues[i].mark_gray(self);
                }
            }
            ObjectType::Class => {
                let mut class = obj.transmute::<Class>();
                class.name.mark_gray(self);
                class.methods.mark_gray(self);
            }
            ObjectType::Instance => {
                let mut instance = obj.transmute::<Instance>();
                instance.class.mark_gray(self);
                instance.fields.mark_gray(self);
            }
            ObjectType::BoundMethod => {
                let mut bound = obj.transmute::<BoundMethod>();
                bound.receiver.mark_gray(self);
                bound.method.mark_gray(self);
            }
        }
    }

    fn sweep(&mut self) {
        let mut prev = None;
        let mut maybe_obj = self.first;
        // Walk through the linked list of every object in the heap, checking if marked
        while let Some(mut obj) = maybe_obj {
            if obj.is_marked {
                // Skip marked (black) objects, but unmark for next run
                obj.is_marked = false;
                prev = maybe_obj;
                maybe_obj = obj.next;

                #[cfg(feature = "debug_log_gc")]
                println!("Not dropping {}", obj);
            } else {
                // Unlink and free unmarked (white) objects
                let mut unreached = obj;
                maybe_obj = obj.next;
                if let Some(mut prev) = prev {
                    prev.next = maybe_obj;
                } else {
                    self.first = maybe_obj;
                }

                #[cfg(feature = "debug_log_gc")]
                println!("Dropping {}", obj);

                self.bytes_allocated -= obj.size_of_val();
                unreached.drop_ptr();
            }
        }
    }

    #[cfg(feature = "debug_stress_gc")]
    pub fn should_gc(&self) -> bool {
        true
    }
    #[cfg(not(feature = "debug_stress_gc"))]
    pub fn should_gc(&self) -> bool {
        self.bytes_allocated > self.next_gc
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn as_obj_transmute() {
        let mut gc = Gc::new();

        let ls1 = LoxString::new("first".to_string());
        let ls1 = gc.alloc(ls1);
        let obj = ls1.header();
        let ls2 = obj.transmute::<LoxString>();
        assert_eq!((&ls1.header as *const _), (&ls2.header as *const _));
        assert_eq!((&ls1.hash as *const _), (&ls2.hash as *const _));
        assert_eq!(ls1.hash, ls2.hash);
        assert_eq!(ls1.as_str(), ls2.as_str());
    }

    #[test]
    fn string_header() {
        let ls = LoxString::new("what up".to_string());
        assert!(matches!(ls.header.obj_type, ObjectType::String));
    }

    #[test]
    fn function_header() {
        let mut ls = LoxString::new("func".to_string());
        let pointer = unsafe { NonNull::new_unchecked(&mut ls) };
        let gcref = GcRef { pointer };
        let ls = Function::new(Some(gcref));
        assert!(matches!(ls.header.obj_type, ObjectType::Function));
    }

    #[test]
    fn alloc() {
        let mut gc = Gc::new();
        let obj1 = {
            let ls = LoxString::new("first".to_string());
            let gcref = gc.alloc(ls);
            gcref.header()
        };
        assert_eq!(gc.first.unwrap().0, obj1.0);
        let obj2 = {
            let ls = LoxString::new("second".to_string());
            let gcref = gc.alloc(ls);
            gcref.header()
        };
        assert_eq!(gc.first.unwrap().0, obj2.0);
        assert_eq!(gc.first.unwrap().next.unwrap().0, obj1.0);
    }

    #[test]
    fn intern_transmute() {
        let mut gc = Gc::new();
        gc.intern("aaa");
        gc.intern("bbb");
        gc.intern("ccc");
        let c = gc.first.unwrap().transmute::<LoxString>();
        assert_eq!(c.as_str(), "ccc");
        let b = c.header.next.unwrap().transmute::<LoxString>();
        assert_eq!(b.as_str(), "bbb");
        let a = b.header.next.unwrap().transmute::<LoxString>();
        assert_eq!(a.as_str(), "aaa");
    }

    #[test]
    fn size_of() {
        let mut gc = Gc::new();
        let ls = LoxString::new("first".to_string());
        let size = std::mem::size_of_val(&ls);
        gc.alloc(ls);
        assert_eq!(gc.first.unwrap().size_of_val(), size);
    }
}

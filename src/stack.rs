use std::{
    fmt::{Debug, Write},
    ptr::null_mut,
};

use crate::value::Value;

pub struct Stack {
    data: [Value; Stack::STACK_SIZE],
    /// Points just past the last used element of the stack
    top: *mut Value,
}

impl Stack {
    const STACK_SIZE: usize = 256;
    pub fn new() -> Stack {
        Stack {
            data: [Value::Nil; Stack::STACK_SIZE],
            top: null_mut(),
        }
    }

    pub fn initialize(&mut self) {
        self.top = self.data.as_mut_ptr();
    }

    pub fn push(&mut self, value: Value) {
        unsafe {
            *self.top = value;
            self.top = self.top.offset(1);
        }
    }

    pub fn pop(&mut self) -> Value {
        unsafe {
            self.top = self.top.offset(-1);
            *self.top
        }
    }

    pub fn peek(&self, distance: isize) -> Value {
        unsafe { *self.top.offset(-1 - distance) }
    }

    pub fn read(&self, index: usize) -> Value {
        self.data[index]
    }

    pub fn write(&mut self, index: usize, value: Value) {
        self.data[index] = value;
    }
}

impl Debug for Stack {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut slot = self.data.as_ptr();
        while slot < self.top {
            f.write_str(&format!("[ {} ]", unsafe { *slot }))?;
            unsafe {
                slot = slot.offset(1);
            }
        }
        f.write_char('\n')?;
        Ok(())
    }
}

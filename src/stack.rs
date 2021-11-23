use std::fmt::{Debug, Write};
use std::ptr::null_mut;

use crate::value::Value;

pub struct Stack {
    stack: [Value; Stack::STACK_SIZE],
    /// Points just past the last used element of the stack
    stack_top: *mut Value,
}

impl Stack {
    const STACK_SIZE: usize = 256;
    pub fn new() -> Stack {
        let mut stack = Stack {
            stack: [0.; Stack::STACK_SIZE],
            stack_top: null_mut(),
        };
        stack.stack_top = stack.stack.as_mut_ptr();
        stack
    }

    pub fn push(&mut self, value: Value) {
        unsafe {
            *self.stack_top = value;
            self.stack_top = self.stack_top.offset(1);
        }
    }

    pub fn pop(&mut self) -> Value {
        unsafe {
            self.stack_top = self.stack_top.offset(-1);
            *self.stack_top
        }
    }
}

impl Debug for Stack {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut slot = self.stack.as_ptr();
        while slot < self.stack_top {
            f.write_str(&format!("[ {} ]", unsafe { *slot }))?;
            unsafe {
                slot = slot.offset(1);
            }
        }
        f.write_char('\n')?;
        Ok(())
    }
}

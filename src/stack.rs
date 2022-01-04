use std::{
    fmt::{Debug, Display, Write},
    mem::MaybeUninit,
};

use crate::gc::{GarbageCollect, Gc};

pub struct Stack<T, const N: usize> {
    data: [MaybeUninit<T>; N],
    /// Points just past the last used element of the stack
    /// TODO: Use pointer instead of index?
    index: usize,
}

impl<T, const N: usize> Stack<T, N>
where
    T: Default,
{
    const INIT: MaybeUninit<T> = MaybeUninit::uninit();
    pub fn new() -> Self {
        Stack {
            data: [Self::INIT; N],
            index: 0,
        }
    }

    pub fn push(&mut self, value: T) {
        debug_assert!(self.index < N);
        unsafe {
            *self.data.get_unchecked_mut(self.index) = MaybeUninit::new(value);
            self.index += 1;
        }
    }

    pub fn pop(&mut self) -> T {
        debug_assert!(self.index > 0);
        unsafe {
            self.index -= 1;
            (self.data.get_unchecked_mut(self.index).as_ptr()).read()
        }
    }

    pub fn pop_n(&mut self, num: usize) -> &[T] {
        debug_assert!(self.index >= num);
        unsafe {
            self.index -= num;
            std::slice::from_raw_parts(self.data.get_unchecked_mut(self.index).as_ptr(), num)
        }
    }

    /// Pop all of the values until stack is given length
    /// e.g. stack: 0,1,2,3
    /// stack.truncate(2) -> stack: 0,1
    pub fn truncate(&mut self, length: usize) {
        debug_assert!(length <= N);
        debug_assert!(length <= self.index);
        self.index = length;
    }

    pub fn peek(&self, distance: usize) -> &T {
        debug_assert!(distance < self.index);
        let index = (self.index - distance - 1) as usize;
        unsafe { self.data.get_unchecked(index).assume_init_ref() }
    }

    pub fn read(&self, index: usize) -> &T {
        debug_assert!(index < self.index);
        unsafe { self.data.get_unchecked(index).assume_init_ref() }
    }

    pub fn top(&mut self) -> &mut T {
        debug_assert!(self.index > 0);
        unsafe {
            self.data
                .get_unchecked_mut(self.index - 1)
                .assume_init_mut()
        }
    }

    pub fn len(&self) -> usize {
        self.index
    }

    pub fn get_offset(&self) -> usize {
        debug_assert!(self.index > 0);
        self.index - 1
    }
}

impl<T, const N: usize> Debug for Stack<T, N>
where
    T: Default + Display,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for index in 0..self.index {
            f.write_str(&format!("[ {} ]", self.read(index)))?;
        }
        f.write_char('\n')?;
        Ok(())
    }
}

impl<T, const N: usize> GarbageCollect for Stack<T, N>
where
    T: GarbageCollect,
{
    fn mark_gray(&mut self, gc: &mut Gc) {
        for index in 0..self.index {
            let item = unsafe { self.data.get_unchecked_mut(index).assume_init_mut() };
            item.mark_gray(gc);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stack() {
        const MAX: usize = 1000;
        let mut stack = Stack::<usize, MAX>::new();
        for i in 0..MAX {
            stack.push(i);
            assert_eq!(stack.peek(0), &i);
            for j in 0..i {
                assert_eq!(stack.read(j as usize), &j);
            }
        }

        for i in (0..MAX).rev() {
            let popped = stack.pop();
            assert_eq!(popped, i);
        }
    }
}

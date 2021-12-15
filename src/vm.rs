use std::{
    ptr::null,
    time::{SystemTime, UNIX_EPOCH},
};

use crate::{
    error::{LoxError, Result},
    gc::{Gc, GcRef},
    obj::{Closure, NativeFn, NativeFunction, Upvalue},
    parser,
    stack::Stack,
    table::Table,
};

use crate::{op_code::OpCode, value::Value};

pub struct Vm {
    stack: Stack<Value, { Vm::STACK_MAX }>,
    frames: Stack<CallFrame, { Vm::FRAMES_MAX }>,
    globals: Table,
    gc: Gc,
    open_upvalues: Option<GcRef<Upvalue>>,
}

impl Vm {
    const FRAMES_MAX: usize = 64;
    const STACK_MAX: usize = Self::FRAMES_MAX * 8;

    pub fn new() -> Vm {
        let mut vm = Vm {
            stack: Stack::new(),
            frames: Stack::new(),
            globals: Table::new(),
            gc: Gc::new(),
            open_upvalues: None,
        };

        vm.define_native("clock", |_| {
            Value::Number(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs_f64(),
            )
        });

        vm
    }

    pub fn interpret(&mut self, source: &str) -> Result<()> {
        // All this stack pushing and popping is to cool the carbage collector happy?
        let function = parser::compile(source, &mut self.gc)?;
        self.stack.push(Value::Function(function));
        let closure = Closure::new(function);
        self.stack.pop();
        let closure = self.gc.alloc(closure);
        self.stack.push(Value::Closure(closure));

        self.call(closure, 0)?;

        self.run()
    }

    // Returning an error from this function (including ?) halts execution
    fn run(&mut self) -> Result<()> {
        loop {
            #[cfg(feature = "debug_trace_execution")]
            {
                print!("        ");
                println!("{:?}", self.stack);
                let frame = self.current_frame();
                crate::disassembler::disassemble_instruction_ptr(
                    &frame.closure.function.chunk,
                    frame.ip,
                );
            }
            let instruction = self.current_frame().read_byte();
            if let Ok(instruction) = instruction.try_into() {
                match instruction {
                    OpCode::Add => {
                        let b = *self.stack.peek(0);
                        let a = *self.stack.peek(1);
                        match (a, b) {
                            (Value::Number(a), Value::Number(b)) => {
                                self.stack.pop();
                                self.stack.pop();
                                let result = Value::Number(a + b);
                                self.stack.push(result);
                            }
                            (Value::String(a), Value::String(b)) => {
                                self.stack.pop();
                                self.stack.pop();
                                let result =
                                    self.gc.intern(format!("{}{}", a.as_str(), b.as_str()));
                                self.stack.push(Value::String(result));
                            }
                            _ => {
                                return self
                                    .runtime_error("Operands must be two numbers or two strings.")
                            }
                        }
                    }
                    OpCode::Constant => {
                        let constant = self.current_frame().read_constant();
                        self.stack.push(constant);
                    }
                    OpCode::Divide => self.binary_op(|a, b| Value::Number(a / b))?,
                    OpCode::Multiply => self.binary_op(|a, b| Value::Number(a * b))?,
                    OpCode::Negate => {
                        if let Value::Number(value) = *self.stack.peek(0) {
                            self.stack.pop();
                            self.stack.push(Value::Number(-value));
                        } else {
                            return self.runtime_error("Operand must be a number.");
                        }
                    }
                    OpCode::Return => {
                        let result = self.stack.pop();
                        let fun_stack_start = self.frames.pop().slot;
                        self.close_upvalues(fun_stack_start);
                        if self.frames.len() == 0 {
                            // Exit interpreter
                            return Ok(());
                        }
                        self.stack.truncate(fun_stack_start);
                        self.stack.push(result);
                    }
                    OpCode::Subtract => self.binary_op(|a, b| Value::Number(a - b))?,
                    OpCode::Nil => self.stack.push(Value::Nil),
                    OpCode::True => self.stack.push(Value::Bool(true)),
                    OpCode::False => self.stack.push(Value::Bool(false)),
                    OpCode::Not => {
                        let value = self.stack.pop();
                        self.stack.push(Value::Bool(value.is_falsey()));
                    }
                    OpCode::Equal => {
                        let a = self.stack.pop();
                        let b = self.stack.pop();
                        self.stack.push(Value::Bool(a == b))
                    }
                    OpCode::Greater => self.binary_op(|a, b| Value::Bool(a > b))?,
                    OpCode::Less => self.binary_op(|a, b| Value::Bool(a < b))?,
                    OpCode::Print => println!("{}", self.stack.pop()),
                    OpCode::Pop => {
                        self.stack.pop();
                    }
                    OpCode::DefineGlobal => {
                        let value = self.current_frame().read_constant();
                        match value {
                            Value::String(name) => {
                                self.globals.insert(name, *self.stack.peek(0));
                                self.stack.pop();
                            }
                            // The compiler never emits and instruct that refers to a non-string constant
                            _ => unreachable!(),
                        }
                    }
                    OpCode::GetGlobal => {
                        let value = self.current_frame().read_constant();
                        match value {
                            Value::String(name) => {
                                if let Some(value) = self.globals.get(name) {
                                    self.stack.push(value);
                                } else {
                                    return self.runtime_error(&format!(
                                        "Undefined variable '{}'.",
                                        name.as_str()
                                    ));
                                }
                            }
                            // The compiler never emits and instruct that refers to a non-string constant
                            _ => unreachable!(),
                        }
                    }
                    OpCode::SetGlobal => {
                        let value = self.current_frame().read_constant();
                        match value {
                            Value::String(name) => {
                                if self.globals.insert(name, *self.stack.peek(0)) {
                                    self.globals.remove(name);
                                    return self.runtime_error(&format!(
                                        "Undefined variable '{}'.",
                                        name.as_str()
                                    ));
                                }
                            }
                            // The compiler never emits and instruct that refers to a non-string constant
                            _ => unreachable!(),
                        }
                    }
                    OpCode::GetLocal => {
                        let offset = self.current_frame().read_local_offset();
                        self.stack.push(*self.stack.read(offset));
                    }
                    OpCode::SetLocal => {
                        let offset = self.current_frame().read_local_offset();
                        self.stack.write(offset, *self.stack.peek(0));
                    }
                    OpCode::JumpIfFalse => {
                        let offset = self.current_frame().read_short();
                        if self.stack.peek(0).is_falsey() {
                            self.current_frame().apply_offset(offset as isize);
                        }
                    }
                    OpCode::Jump => {
                        let frame = self.current_frame();
                        let offset = frame.read_short();
                        frame.apply_offset(offset as isize);
                    }
                    OpCode::Loop => {
                        let frame = self.current_frame();
                        let offset = frame.read_short();
                        let offset = -(offset as isize);
                        frame.apply_offset(offset);
                    }
                    OpCode::Call => {
                        let arg_count = self.current_frame().read_byte() as usize;
                        self.call_value(*self.stack.peek(arg_count), arg_count)?;
                    }
                    OpCode::Closure => {
                        // Load the compiled function from the constant table
                        let function = self.current_frame().read_constant();
                        if let Value::Function(function) = function {
                            // Wrap that function in a new closure object and push it onto the stack
                            let closure = Closure::new(function);
                            let mut closure = self.gc.alloc(closure);
                            self.stack.push(Value::Closure(closure));

                            // Iterate over each upvalue the closure expects
                            for _ in 0..function.upvalue_count {
                                let is_local = self.current_frame().read_byte() != 0;
                                let index = self.current_frame().read_byte() as usize;
                                let upvalue = if is_local {
                                    // If the upvalue closes over a local variable in the immediately enclosing function, we can directly capture it
                                    let location = self.current_frame().slot + index;
                                    self.capture_upvalue(location)
                                } else {
                                    // Otherwise we capture the *upvalue* from the immediately enclosing function
                                    self.current_frame().closure.upvalues[index]
                                };
                                closure.upvalues.push(upvalue);
                            }
                        } else {
                            unreachable!()
                        }
                    }
                    OpCode::GetUpvalue => {
                        let slot = self.current_frame().read_byte() as usize;
                        let upvalue = self.current_frame().closure.upvalues[slot];
                        let value = if let Some(closed) = upvalue.closed {
                            closed
                        } else {
                            *self.stack.read(upvalue.location)
                        };
                        self.stack.push(value);
                    }
                    OpCode::SetUpvalue => {
                        let slot = self.current_frame().read_byte() as usize;
                        let mut upvalue = self.current_frame().closure.upvalues[slot];
                        let value = *self.stack.peek(0);
                        if upvalue.closed.is_some() {
                            upvalue.closed = Some(value);
                        } else {
                            self.stack.write(upvalue.location, value);
                        }
                    }
                    OpCode::CloseUpvalue => {
                        self.close_upvalues(self.stack.get_offset());
                        self.stack.pop();
                    }
                }
            }
        }
    }

    fn current_frame(&mut self) -> &mut CallFrame {
        self.frames.top()
    }

    fn binary_op(&mut self, f: impl Fn(f64, f64) -> Value) -> Result<()> {
        let b = *self.stack.peek(0);
        let a = *self.stack.peek(1);
        match (a, b) {
            (Value::Number(a), Value::Number(b)) => {
                self.stack.pop();
                self.stack.pop();
                let result = f(a, b);
                self.stack.push(result);
                Ok(())
            }
            _ => self.runtime_error("Operands must be numbers."),
        }
    }

    fn call_value(&mut self, callee: Value, arg_count: usize) -> Result<()> {
        match callee {
            Value::NativeFunction(callee) => {
                let args = self.stack.pop_n(arg_count);
                let result = (callee.function)(args);
                self.stack.pop();
                self.stack.push(result);
                Ok(())
            }
            Value::Closure(callee) => self.call(callee, arg_count),
            _ => self.runtime_error(&format!(
                "Can only call functions and classes, not on '{}'.",
                callee
            )),
        }
    }

    fn call(&mut self, callee: GcRef<Closure>, arg_count: usize) -> Result<()> {
        if arg_count != callee.function.arity {
            return self.runtime_error(&format!(
                "Expected {} arguments but got {}.",
                callee.function.arity, arg_count
            ));
        }

        if self.frames.len() == Self::FRAMES_MAX {
            return self.runtime_error("Stack overflow.");
        }

        let slot = if self.frames.len() == 0 {
            // This value will never be used
            usize::MAX
        } else {
            self.stack.get_offset() - arg_count
        };
        self.frames.push(CallFrame::new(callee, slot));
        Ok(())
    }

    fn capture_upvalue(&mut self, local: usize) -> GcRef<Upvalue> {
        let mut prev_upvalue = None;
        let mut upvalue = self.open_upvalues;
        while let Some(inner) = upvalue {
            if inner.location <= local {
                break;
            }
            prev_upvalue = upvalue;
            upvalue = inner.next;
        }

        // We found an existing upvalue capturing the variable, so we reuse that upvalue
        if let Some(upvalue) = upvalue {
            if upvalue.location == local {
                return upvalue;
            }
        }

        let created_upvalue = Upvalue::new(local);
        let created_upvalue = self.gc.alloc(created_upvalue);

        // Insert new upvalue between 'prev_upvalue' and 'upvalue'
        if let Some(mut prev_upvalue) = prev_upvalue {
            prev_upvalue.next = Some(created_upvalue);
        } else {
            self.open_upvalues = Some(created_upvalue);
        }

        created_upvalue
    }

    fn close_upvalues(&mut self, last: usize) {
        while let Some(mut upvalue) = self.open_upvalues {
            if upvalue.location < last {
                break;
            }
            upvalue.closed = Some(*self.stack.read(upvalue.location));
            self.open_upvalues = upvalue.next;
        }
    }

    fn runtime_error(&self, message: &str) -> Result<()> {
        eprintln!("{}", message);

        // Print callstack
        for i in (0..self.frames.len()).rev() {
            let frame = self.frames.read(i);
            let closure = frame.closure;
            let instruction =
                unsafe { frame.ip.offset_from(closure.function.chunk.code.as_ptr()) - 1 } as usize;
            let line = closure.function.chunk.lines[instruction];
            eprintln!("[line {}] in {}", line, *closure);
        }

        Err(LoxError::RuntimeError)
    }

    fn define_native(&mut self, name: &str, function: NativeFn) {
        // Pushing and popping to and from stack is only to ensure no GC occurs
        // #TODO probably can manually mark things as roots instead?
        let ls = self.gc.intern(name.to_string());
        self.stack.push(Value::String(ls));
        let native = self.gc.alloc(NativeFunction::new(function));
        self.stack.push(Value::NativeFunction(native));
        self.globals.insert(ls, *self.stack.peek(0));
        self.stack.pop();
        self.stack.pop();
    }
}

/// Represents a single ongoing function call
struct CallFrame {
    closure: GcRef<Closure>,
    /// The instruction pointer of this function. Returning from this function will resume from here.
    // #TODO NonNull?
    ip: *const u8,
    /// The first slot in the VM's value stack that this function can use
    slot: usize,
}

impl Default for CallFrame {
    fn default() -> Self {
        Self {
            ip: null(),
            slot: 0,
            closure: GcRef::dangling(),
        }
    }
}

impl CallFrame {
    fn new(closure: GcRef<Closure>, slot: usize) -> Self {
        Self {
            closure,
            ip: closure.function.chunk.code.as_ptr(),
            slot,
        }
    }
    fn read_byte(&mut self) -> u8 {
        let byte = unsafe { *self.ip };
        self.ip = unsafe { self.ip.offset(1) };
        byte
    }

    fn read_short(&mut self) -> u16 {
        let byte1 = self.read_byte();
        let byte2 = self.read_byte();
        (byte1 as u16) << 8 | (byte2 as u16)
    }

    fn read_constant(&mut self) -> Value {
        let index: usize = self.read_byte().try_into().unwrap();
        self.closure.function.chunk.constants[index]
    }

    fn read_local_offset(&mut self) -> usize {
        let offset = self.read_byte() as usize;
        self.slot + offset
    }

    fn apply_offset(&mut self, offset: isize) {
        self.ip = unsafe { self.ip.offset(offset) };
    }
}

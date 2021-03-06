use std::{fmt, fmt::Write, ptr::null};

use crate::{
    ast::{Ast, Source},
    compiler::Compiler,
    error::{Error, Result},
    gc::{GarbageCollect, Gc, GcRef},
    native_functions::{clock, product, sum},
    obj::{BanjoString, Function, NativeFn, NativeFunction},
    op_code::{Constant, LocalIndex, OpCode},
    output::{Output, OutputValues},
    stack::Stack,
    table::Table,
    value::Value,
};

pub type ValueStack = Stack<Value, { Vm::STACK_MAX }>;
pub struct Vm {
    gc: Gc,
    output: OutputValues,
    stack: ValueStack,
    frames: Stack<CallFrame, { Vm::FRAMES_MAX }>,
    globals: Table,
}

impl Vm {
    const FRAMES_MAX: usize = 64;
    const STACK_MAX: usize = Self::FRAMES_MAX * (u8::MAX as usize + 1);

    #[must_use]
    pub fn new() -> Vm {
        let gc = Gc::new();

        let mut vm = Vm {
            gc,
            stack: Stack::new(),
            frames: Stack::new(),
            globals: Table::new(),
            output: OutputValues::default(),
        };

        vm.define_native("clock", clock);
        vm.define_native("sum", sum);
        vm.define_native("product", product);

        vm
    }

    /// Compile then execute the given AST using this VM.
    ///
    /// # Errors
    ///
    /// This function can return both compile and runtime errors.
    pub fn interpret(&mut self, source: Source) -> Output {
        let ast = Ast::new(&source);
        let mut compiler: Compiler<'_> = Compiler::new(&ast, &mut self.gc, &mut self.output);
        let function = compiler.compile();

        // Leave the <script> function on the stack forever so it's not GC'd
        self.stack.push(Value::Function(function));

        self.call(function, 0)
            .unwrap_or_else(|e| self.output.add_error(e));

        self.run().unwrap_or_else(|e| self.output.add_error(e));

        self.output.take()
    }

    // Returning an error from this function (including ?) halts execution
    fn run(&mut self) -> Result<()> {
        loop {
            #[cfg(feature = "debug_trace_execution")]
            {
                print!("        ");
                println!("{:?}", self.stack);
                let frame = self.current_frame();
                crate::disassembler::disassemble_instruction_ptr(&frame.function.chunk, frame.ip);
            }
            let instruction = unsafe { *self.current_frame().ip };
            self.current_frame().ip = unsafe { self.current_frame().ip.offset(1) };

            match instruction {
                OpCode::Add => {
                    let b = *self.stack.peek(0);
                    let a = *self.stack.peek(1);
                    let result = a.add(b, self);
                    self.stack.push(result);
                }
                // Load constant/function onto the stack
                OpCode::Constant(constant) | OpCode::Function(constant) => {
                    let constant = self.current_frame().read_constant(constant);
                    self.stack.push(constant);
                }
                OpCode::Divide => self.binary_op(|a, b| Value::Number(a / b))?,
                OpCode::Multiply => self.binary_op(|a, b| Value::Number(a * b))?,
                OpCode::Negate => {
                    if let Value::Number(value) = *self.stack.peek(0) {
                        self.stack.pop();
                        self.stack.push(Value::Number(-value));
                    } else {
                        self.runtime_error("Operand must be a number.")?;
                    }
                }
                OpCode::Return => {
                    let result = self.stack.pop();
                    let fun_stack_start = self.frames.pop().slot;
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
                    self.stack.push(Value::Bool(a == b));
                }
                OpCode::Greater => self.binary_op(|a, b| Value::Bool(a > b))?,
                OpCode::Less => self.binary_op(|a, b| Value::Bool(a < b))?,
                OpCode::Pop => {
                    self.stack.pop();
                }
                OpCode::DefineGlobal(constant) => {
                    let name = self.read_string(constant);
                    self.globals.insert(name, *self.stack.peek(0));
                    self.stack.pop();
                }
                OpCode::GetGlobal(constant) => {
                    let name = self.read_string(constant);
                    if let Some(value) = self.globals.get(name) {
                        self.stack.push(value);
                    } else {
                        self.runtime_error(format!("Undefined variable '{}'.", name.as_str()))?;
                    }
                }
                OpCode::GetLocal(offset) => {
                    let offset = self.current_frame().read_local_offset(offset);
                    self.stack.push(*self.stack.read(offset));
                }
                OpCode::Call { arg_count } => {
                    let arg_count = arg_count as usize;
                    self.call_value(*self.stack.peek(arg_count), arg_count)?;
                }
                OpCode::Output { output_index } => {
                    self.output.add_value(output_index, *self.stack.peek(0))
                }
            }
        }
    }

    fn current_frame(&mut self) -> &mut CallFrame {
        self.frames.top()
    }

    fn read_string(&mut self, constant: Constant) -> GcRef<BanjoString> {
        match self.current_frame().read_constant(constant) {
            Value::String(name) => name,
            _ => unreachable!(),
        }
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
                let result = (callee.function)(args, self).map_err(|e| self.add_stacktrace(e))?;
                self.stack.pop();
                self.stack.push(result);
                Ok(())
            }
            Value::Function(callee) => self.call(callee, arg_count),
            _ => self.runtime_error("Can only call functions."),
        }
    }

    fn call(&mut self, callee: GcRef<Function>, arg_count: usize) -> Result<()> {
        if arg_count != callee.arity {
            return self.runtime_error(format!(
                "Expected {} arguments but got {}.",
                callee.arity, arg_count
            ));
        }

        if self.frames.len() == Self::FRAMES_MAX {
            return self.runtime_error("Stack overflow.");
        }

        let slot = self.stack.get_offset() - arg_count;
        self.frames.push(CallFrame::new(callee, slot));
        Ok(())
    }

    fn make_stacktrace<M: Into<String>>(&self, message: M) -> String {
        // Print callstack
        let mut error_str = message.into();
        for i in (0..self.frames.len()).rev() {
            let frame = self.frames.read(i);
            let closure = frame.function;
            write!(error_str, "\nin {:?}", *closure).unwrap();
        }
        error_str
    }

    fn runtime_error<M: Into<String>>(&self, message: M) -> Result<()> {
        Error::runtime_err(self.make_stacktrace(message))
    }

    fn add_stacktrace(&self, error: Error) -> Error {
        match error {
            Error::Runtime(message) => Error::Runtime(self.make_stacktrace(message)),
            _ => error,
        }
    }

    fn define_native(&mut self, name: &str, function: NativeFn) {
        let ls = self.intern(name);
        // Pushing and popping to and from stack is only to ensure no GC occurs on call
        // to alloc
        self.stack.push(Value::String(ls));
        let native = self.alloc(NativeFunction::new(function));
        self.globals.insert(ls, Value::NativeFunction(native));
        self.stack.pop();
    }

    pub fn intern(&mut self, string: &str) -> GcRef<BanjoString> {
        self.mark_and_collect_garbage();
        self.gc.intern(string)
    }

    /// Move the provided object to the heap and track with the garbage
    /// collector
    pub fn alloc<T>(&mut self, object: T) -> GcRef<T>
    where
        T: fmt::Debug,
    {
        self.mark_and_collect_garbage();
        self.gc.alloc(object)
    }

    fn mark_and_collect_garbage(&mut self) {
        if self.gc.should_gc() {
            self.mark_roots();
            self.gc.collect_garbage();
        }
    }

    fn mark_roots(&mut self) {
        // Stack
        self.stack.mark_gray(&mut self.gc);

        // Call frame closures
        self.frames.mark_gray(&mut self.gc);

        // Globals
        self.globals.mark_gray(&mut self.gc);
    }
}

impl Default for Vm {
    fn default() -> Self {
        Self::new()
    }
}

/// Represents a single ongoing function call
struct CallFrame {
    function: GcRef<Function>,
    /// The instruction pointer of this function. Returning from this function
    /// will resume from here.
    ip: *const OpCode,
    /// The first slot in the VM's value stack that this function can use
    slot: usize,
}

impl Default for CallFrame {
    fn default() -> Self {
        Self {
            ip: null(),
            slot: 0,
            function: GcRef::dangling(),
        }
    }
}

impl CallFrame {
    fn new(function: GcRef<Function>, slot: usize) -> Self {
        Self {
            function,
            ip: function.chunk.code.as_ptr(),
            slot,
        }
    }

    fn read_constant(&self, constant: Constant) -> Value {
        self.function.chunk.constants[constant.slot as usize]
    }

    fn read_local_offset(&mut self, local: LocalIndex) -> usize {
        self.slot + (local as usize)
    }
}

impl GarbageCollect for CallFrame {
    fn mark_gray(&mut self, gc: &mut Gc) {
        self.function.mark_gray(gc);
    }
}

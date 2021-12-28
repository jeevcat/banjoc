use std::{
    fmt::Display,
    ptr::null,
    time::{SystemTime, UNIX_EPOCH},
};

use crate::{
    error::{LoxError, Result},
    gc::{GarbageCollect, Gc, GcRef},
    obj::{
        BoundMethod, Class, Closure, FunctionUpvalue, Instance, LoxString, NativeFn,
        NativeFunction, Upvalue,
    },
    parser,
    stack::Stack,
    table::Table,
};

use crate::{op_code::OpCode, value::Value};

pub type ValueStack = Stack<Value, { Vm::STACK_MAX }>;
pub struct Vm {
    pub gc: Gc,
    stack: ValueStack,
    frames: Stack<CallFrame, { Vm::FRAMES_MAX }>,
    globals: Table,
    open_upvalues: Option<GcRef<Upvalue>>,
    init_string: GcRef<LoxString>,
}

impl Vm {
    const FRAMES_MAX: usize = 64;
    const STACK_MAX: usize = Self::FRAMES_MAX * (u8::MAX as usize + 1);

    pub fn new() -> Vm {
        let mut gc = Gc::new();
        let init_string = gc.intern("init".to_string());

        let mut vm = Vm {
            gc,
            stack: Stack::new(),
            frames: Stack::new(),
            globals: Table::new(),
            open_upvalues: None,
            init_string,
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
        let function = parser::compile(source, &mut self.gc)?;
        // Leave the <script> function on the stack forever so it's not GC'd
        self.stack.push(Value::Function(function));
        let closure = Closure::new(function);
        let closure = self.alloc(closure);

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
            let instruction = unsafe { *self.current_frame().ip };
            self.current_frame().ip = unsafe { self.current_frame().ip.offset(1) };

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
                            let result = self.intern(format!("{}{}", a.as_str(), b.as_str()));
                            self.stack.push(Value::String(result));
                        }
                        _ => {
                            return self
                                .runtime_error("Operands must be two numbers or two strings.")
                        }
                    }
                }
                OpCode::Constant(constant) => {
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
                OpCode::DefineGlobal(constant) => {
                    let value = self.current_frame().read_constant(constant);
                    match value {
                        Value::String(name) => {
                            self.globals.insert(name, *self.stack.peek(0));
                            self.stack.pop();
                        }
                        // The compiler never emits any instructions that refer to a non-string constant
                        _ => unreachable!(),
                    }
                }
                OpCode::GetGlobal(constant) => {
                    let value = self.current_frame().read_constant(constant);
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
                        // The compiler never emits any instructions that refer to a non-string constant
                        _ => unreachable!(),
                    }
                }
                OpCode::SetGlobal(constant) => {
                    let value = self.current_frame().read_constant(constant);
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
                        // The compiler never emits any instructions that refer to a non-string constant
                        _ => unreachable!(),
                    }
                }
                OpCode::GetLocal(offset) => {
                    let offset = self.current_frame().read_local_offset(offset);
                    self.stack.push(*self.stack.read(offset));
                }
                OpCode::SetLocal(offset) => {
                    let offset = self.current_frame().read_local_offset(offset);
                    self.stack.write(offset, *self.stack.peek(0));
                }
                OpCode::JumpIfFalse(offset) => {
                    if self.stack.peek(0).is_falsey() {
                        self.current_frame().apply_offset(offset as isize);
                    }
                }
                OpCode::Jump(offset) => {
                    let frame = self.current_frame();
                    frame.apply_offset(offset as isize);
                }
                OpCode::Loop(offset) => {
                    let frame = self.current_frame();
                    let offset = -1 - (offset as isize);
                    frame.apply_offset(offset);
                }
                OpCode::Call(arg_count) => {
                    let arg_count = arg_count as usize;
                    self.call_value(*self.stack.peek(arg_count), arg_count)?;
                }
                OpCode::Closure(constant) => {
                    // Load the compiled function from the constant table
                    let function = self.current_frame().read_constant(constant);
                    if let Value::Function(function) = function {
                        // Wrap that function in a new closure object and push it onto the stack
                        let mut closure = Closure::new(function);

                        // Iterate over each upvalue the closure expects
                        for FunctionUpvalue { is_local, index } in function.upvalues.iter() {
                            let index = *index as usize;
                            let upvalue = if *is_local {
                                // If the upvalue closes over a local variable in the immediately enclosing function, we can directly capture it
                                let location = self.current_frame().slot + index;
                                self.capture_upvalue(location)
                            } else {
                                // Otherwise we capture the *upvalue* from the immediately enclosing function
                                self.current_frame().closure.upvalues[index]
                            };
                            closure.upvalues.push(upvalue);
                        }
                        let closure = self.alloc(closure);
                        self.stack.push(Value::Closure(closure));
                    } else {
                        unreachable!()
                    }
                }
                OpCode::GetUpvalue(slot) => {
                    let upvalue = self.current_frame().closure.upvalues[slot as usize];
                    let value = upvalue.read(&self.stack);
                    self.stack.push(value);
                }
                OpCode::SetUpvalue(slot) => {
                    let mut upvalue = self.current_frame().closure.upvalues[slot as usize];
                    upvalue.write(&mut self.stack);
                }
                OpCode::CloseUpvalue => {
                    self.close_upvalues(self.stack.get_offset());
                    self.stack.pop();
                }
                OpCode::Class(constant) => {
                    let value = self.current_frame().read_constant(constant);
                    match value {
                        Value::String(name) => {
                            let class = self.alloc(Class::new(name));
                            self.stack.push(Value::Class(class));
                        }
                        _ => unreachable!(),
                    }
                }
                OpCode::GetProperty(constant) => {
                    let instance = match *self.stack.peek(0) {
                        Value::Instance(instance) => instance,
                        _ => return self.runtime_error("Only instances have properties."),
                    };
                    let name = self.read_string(constant);
                    if let Some(value) = instance.fields.get(name) {
                        self.stack.pop(); // Instance
                        self.stack.push(value);
                    } else {
                        self.bind_method(instance.class, name)?;
                    }
                }
                OpCode::SetProperty(constant) => {
                    let instance = *self.stack.peek(1);
                    let mut instance = match instance {
                        Value::Instance(instance) => instance,
                        _ => return self.runtime_error("Only instances have fields."),
                    };
                    let name = self.read_string(constant);
                    let value = *self.stack.peek(0);
                    instance.fields.insert(name, value);

                    // Remove 2nd element from the stack (the instance)
                    let value = self.stack.pop();
                    self.stack.pop();
                    self.stack.push(value);
                }
                OpCode::Method(constant) => {
                    let name = self.read_string(constant);
                    self.define_method(name);
                }
                OpCode::Invoke((constant, arg_count)) => {
                    let method = self.read_string(constant);
                    self.invoke(method, arg_count as usize)?;
                }
                OpCode::Inherit => {
                    let superclass = match self.stack.peek(1) {
                        Value::Class(class) => class,
                        _ => return self.runtime_error("Superclass must be a class."),
                    };
                    match self.stack.peek(0) {
                        Value::Class(mut subclass) => subclass.methods.append(&superclass.methods),
                        _ => unreachable!(),
                    };
                }
                OpCode::GetSuper(constant) => {
                    let name = self.read_string(constant);
                    let class = match self.stack.pop() {
                        Value::Class(class) => class,
                        _ => unreachable!(),
                    };

                    self.bind_method(class, name)?;
                }
                OpCode::SuperInvoke((constant, arg_count)) => {
                    let method = self.read_string(constant);
                    let class = match self.stack.pop() {
                        Value::Class(class) => class,
                        _ => unreachable!(),
                    };
                    self.invoke_from_class(class, method, arg_count as usize)?;
                }
            }
        }
    }

    fn current_frame(&mut self) -> &mut CallFrame {
        self.frames.top()
    }

    fn read_string(&mut self, constant: u8) -> GcRef<LoxString> {
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
                let result = (callee.function)(args);
                self.stack.pop();
                self.stack.push(result);
                Ok(())
            }
            Value::Closure(callee) => self.call(callee, arg_count),
            Value::Class(class) => {
                let instance = self.alloc(Instance::new(class));
                self.stack.write(
                    self.stack.get_offset() - arg_count,
                    Value::Instance(instance),
                );
                if let Some(initializer) = class.methods.get(self.init_string) {
                    match initializer {
                        Value::Closure(initializer) => return self.call(initializer, arg_count),
                        _ => unreachable!(),
                    }
                } else if arg_count != 0 {
                    return self
                        .runtime_error(&format!("Expected 0 arguments but got {}.", arg_count));
                }
                Ok(())
            }
            Value::BoundMethod(bound) => {
                self.stack
                    .write(self.stack.get_offset() - arg_count, bound.receiver);
                self.call(bound.method, arg_count)
            }

            _ => self.runtime_error("Can only call functions and classes."),
        }
    }

    fn invoke_from_class(
        &mut self,
        class: GcRef<Class>,
        name: GcRef<LoxString>,
        arg_count: usize,
    ) -> Result<()> {
        if let Some(method) = class.methods.get(name) {
            match method {
                Value::Closure(closure) => self.call(closure, arg_count),
                _ => unreachable!(),
            }
        } else {
            self.runtime_error(&format!("Undefined property '{}'.", name.as_str()))
        }
    }

    fn invoke(&mut self, name: GcRef<LoxString>, arg_count: usize) -> Result<()> {
        let receiver = *self.stack.peek(arg_count);
        let receiver = match receiver {
            Value::Instance(instance) => instance,
            _ => return self.runtime_error("Only instances have methods."),
        };

        if let Some(value) = receiver.fields.get(name) {
            self.stack.write(self.stack.get_offset() - arg_count, value);
            return self.call_value(value, arg_count);
        }

        self.invoke_from_class(receiver.class, name, arg_count)
    }

    fn bind_method(&mut self, class: GcRef<Class>, name: GcRef<LoxString>) -> Result<()> {
        let method = match class.methods.get(name) {
            Some(value) => value,
            None => return self.runtime_error(&format!("Undefined property '{}'.", name.as_str())),
        };

        let closure = match method {
            Value::Closure(closure) => closure,
            _ => unreachable!(),
        };

        let bound = self.alloc(BoundMethod::new(*self.stack.peek(0), closure));
        let bound = Value::BoundMethod(bound);

        self.stack.pop();
        self.stack.push(bound);

        Ok(())
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

        let slot = self.stack.get_offset() - arg_count;
        self.frames.push(CallFrame::new(callee, slot));
        Ok(())
    }

    fn capture_upvalue(&mut self, local: usize) -> GcRef<Upvalue> {
        let mut prev_upvalue = None;
        let mut maybe_upvalue = self.open_upvalues;
        while let Some(upvalue) = maybe_upvalue {
            if upvalue.location <= local {
                break;
            }
            prev_upvalue = maybe_upvalue;
            maybe_upvalue = upvalue.next;
        }

        // We found an existing upvalue capturing the variable, so we reuse that upvalue
        if let Some(upvalue) = maybe_upvalue {
            if upvalue.location == local {
                return upvalue;
            }
        }

        let created_upvalue = Upvalue::new(local, maybe_upvalue);
        let created_upvalue = self.alloc(created_upvalue);

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

    fn define_method(&mut self, name: GcRef<LoxString>) {
        let method = *self.stack.peek(0);
        let mut class = match self.stack.peek(1) {
            Value::Class(class) => *class,
            _ => unreachable!(),
        };
        class.methods.insert(name, method);
        self.stack.pop();
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
        let ls = self.intern(name.to_string());
        // Pushing and popping to and from stack is only to ensure no GC occurs on call to alloc
        self.stack.push(Value::String(ls));
        let native = self.alloc(NativeFunction::new(function));
        self.globals.insert(ls, Value::NativeFunction(native));
        self.stack.pop();
    }

    pub fn intern(&mut self, string: String) -> GcRef<LoxString> {
        self.mark_and_collect_garbage();
        self.gc.intern(string)
    }

    /// Move the provided object to the heap and track with the garbage collector
    pub fn alloc<T>(&mut self, object: T) -> GcRef<T>
    where
        T: Display,
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

        // Open upvalue list
        let mut next = self.open_upvalues;
        while let Some(upvalue) = next {
            upvalue.read(&self.stack).mark_gray(&mut self.gc);
            next = upvalue.next;
        }

        // Globals
        self.globals.mark_gray(&mut self.gc);
    }
}

/// Represents a single ongoing function call
struct CallFrame {
    closure: GcRef<Closure>,
    /// The instruction pointer of this function. Returning from this function will resume from here.
    // #TODO NonNull?
    ip: *const OpCode,
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

    fn read_constant(&self, index: u8) -> Value {
        self.closure.function.chunk.constants[index as usize]
    }

    fn read_local_offset(&mut self, offset: u8) -> usize {
        self.slot + (offset as usize)
    }

    fn apply_offset(&mut self, offset: isize) {
        self.ip = unsafe { self.ip.offset(offset) };
    }
}

impl GarbageCollect for CallFrame {
    fn mark_gray(&mut self, gc: &mut Gc) {
        self.closure.mark_gray(gc)
    }
}

use std::ptr::null;

use crate::{
    error::{LoxError, Result},
    gc::Gc,
    parser,
    stack::Stack,
    table::Table,
};

use crate::{chunk::Chunk, op_code::OpCode, value::Value};

pub struct Vm {
    chunk: Option<Chunk>,
    ip: *const u8,
    stack: Stack,
    globals: Table,
    gc: Gc,
}

impl Vm {
    pub fn new() -> Vm {
        let mut vm = Vm {
            ip: null(),
            chunk: None,
            stack: Stack::new(),
            globals: Table::new(),
            gc: Gc::new(),
        };
        vm.stack.initialize();
        vm
    }

    pub fn interpret(&mut self, source: &str) -> Result<()> {
        let chunk = parser::compile(source, &mut self.gc)?;

        self.ip = chunk.code.as_ptr();
        self.chunk = Some(chunk);

        self.run()
    }

    // Returning an error from this function (including ?) halts execution
    fn run(&mut self) -> Result<()> {
        loop {
            #[cfg(feature = "debug_trace_execution")]
            {
                print!("        ");
                println!("{:?}", self.stack);
                match &self.chunk {
                    Some(chunk) => crate::disassembler::disassemble_instruction_ptr(chunk, self.ip),
                    None => return Err(LoxError::RuntimeError),
                };
            }
            let instruction = self.read_byte();
            if let Ok(instruction) = instruction.try_into() {
                match instruction {
                    OpCode::Add => {
                        let b = self.stack.peek(0);
                        let a = self.stack.peek(1);
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
                                let result = self.gc.alloc(format!("{}{}", &a.string, &b.string));
                                self.stack.push(Value::String(result));
                            }
                            _ => {
                                return self
                                    .runtime_error("Operands must be two numbers or two strings.")
                            }
                        }
                    }
                    OpCode::Constant => {
                        let constant = self.read_constant();
                        self.stack.push(constant);
                    }
                    OpCode::Divide => self.binary_op(|a, b| Value::Number(a / b))?,
                    OpCode::Multiply => self.binary_op(|a, b| Value::Number(a * b))?,
                    OpCode::Negate => {
                        if let Value::Number(value) = self.stack.peek(0) {
                            self.stack.pop();
                            self.stack.push(Value::Number(-value));
                        } else {
                            return self.runtime_error("Operand must be a number.");
                        }
                    }
                    OpCode::Return => {
                        // Exit interpreter
                        return Ok(());
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
                        let value = self.read_constant();
                        match value {
                            Value::String(name) => {
                                self.globals.insert(name, self.stack.peek(0));
                                self.stack.pop();
                            }
                            // The compiler never emits and instruct that refers to a non-string constant
                            _ => unreachable!(),
                        }
                    }
                    OpCode::GetGlobal => {
                        let value = self.read_constant();
                        match value {
                            Value::String(name) => {
                                if let Some(value) = self.globals.get(name) {
                                    self.stack.push(value);
                                } else {
                                    return self.runtime_error(&format!(
                                        "Undefined variable '{}'.",
                                        name.string
                                    ));
                                }
                            }
                            // The compiler never emits and instruct that refers to a non-string constant
                            _ => unreachable!(),
                        }
                    }
                    OpCode::SetGlobal => {
                        let value = self.read_constant();
                        match value {
                            Value::String(name) => {
                                if self.globals.insert(name, self.stack.peek(0)) {
                                    self.globals.remove(name);
                                    return self.runtime_error(&format!(
                                        "Undefined variable '{}'.",
                                        name.string
                                    ));
                                }
                            }
                            // The compiler never emits and instruct that refers to a non-string constant
                            _ => unreachable!(),
                        }
                    }
                    OpCode::GetLocal => {
                        let slot = self.read_byte() as usize;
                        self.stack.push(self.stack.read(slot));
                    }
                    OpCode::SetLocal => {
                        let slot = self.read_byte() as usize;
                        self.stack.write(slot, self.stack.peek(0));
                    }
                    OpCode::JumpIfFalse => {
                        let offset = self.read_short();
                        if self.stack.peek(0).is_falsey() {
                            self.ip = unsafe { self.ip.offset(offset as isize) };
                        }
                    }
                    OpCode::Jump => {
                        let offset = self.read_short();
                        self.ip = unsafe { self.ip.offset(offset as isize) };
                    }
                    OpCode::Loop => {
                        let offset = self.read_short();
                        let offset = -(offset as isize);
                        self.ip = unsafe { self.ip.offset(offset) };
                    }
                }
            }
        }
    }

    fn binary_op(&mut self, f: impl Fn(f64, f64) -> Value) -> Result<()> {
        let b = self.stack.peek(0);
        let a = self.stack.peek(1);
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
        // Only called when we know chunk is Some
        self.chunk.as_ref().unwrap().constants[index]
    }

    fn runtime_error(&self, message: &str) -> Result<()> {
        let chunk = self.chunk.as_ref().unwrap();
        let instruction = unsafe { self.ip.offset_from(chunk.code.as_ptr()) - 1 } as usize;
        let line = chunk.lines[instruction];
        eprintln!("{}", message);
        eprintln!("[line {}] in script", line);
        Err(LoxError::RuntimeError)
    }
}

use crate::{error::Result, stack::Stack};

use crate::{chunk::Chunk, op_code::OpCode, value::Value};

pub struct Vm {
    chunk: Chunk,
    ip: *const u8,
    stack: Stack,
}

impl Vm {
    pub fn interpret(chunk: Chunk) -> Result {
        let mut vm = Vm::new(chunk);
        vm.run()
    }

    fn new(chunk: Chunk) -> Vm {
        let mut vm = Vm {
            ip: chunk.code.as_ptr(),
            chunk,
            stack: Stack::new(),
        };
        vm.stack.initialize();
        vm
    }

    fn run(&mut self) -> Result {
        loop {
            #[cfg(feature = "debug_trace_execution")]
            {
                print!("        ");
                println!("{:?}", self.stack);
                crate::disassembler::disassemble_instruction_ptr(&self.chunk, self.ip);
            }
            let instruction = self.read_byte();
            if let Ok(instruction) = instruction.try_into() {
                match instruction {
                    OpCode::Add => self.binary_op(|a, b| a + b),
                    OpCode::Constant => {
                        let constant = self.read_constant();
                        self.stack.push(constant);
                    }
                    OpCode::Divide => self.binary_op(|a, b| a / b),
                    OpCode::Multiply => self.binary_op(|a, b| a * b),
                    OpCode::Negate => {
                        let value = -self.stack.pop();
                        self.stack.push(value);
                    }
                    OpCode::Return => {
                        println!("{}", self.stack.pop());
                        return Ok(());
                    }
                    OpCode::Subtract => self.binary_op(|a, b| a - b),
                }
            }
        }
    }

    fn binary_op(&mut self, f: impl Fn(f64, f64) -> f64) {
        let b = self.stack.pop();
        let a = self.stack.pop();
        let result = f(a, b);
        self.stack.push(result);
    }

    fn read_byte(&mut self) -> u8 {
        let byte = unsafe { *self.ip };
        self.ip = unsafe { self.ip.offset(1) };
        byte
    }

    fn read_constant(&mut self) -> Value {
        let index: usize = self.read_byte().try_into().unwrap();
        self.chunk.constants[index]
    }
}

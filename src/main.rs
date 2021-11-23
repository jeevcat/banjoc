use std::process;

use error::LoxError;
use vm::Vm;

use crate::{chunk::Chunk, op_code::OpCode};

mod chunk;
#[cfg(feature = "debug_trace_execution")]
mod disassembler;
mod error;
mod op_code;
mod stack;
mod value;
mod vm;

fn main() {
    let mut chunk = Chunk::new();
    let constant = chunk.add_constant(1.2);
    chunk.write(OpCode::Constant.into(), 123);
    chunk.write(constant.try_into().unwrap(), 123);

    let constant = chunk.add_constant(3.4);
    chunk.write(OpCode::Constant.into(), 123);
    chunk.write(constant.try_into().unwrap(), 123);

    chunk.write(OpCode::Add.into(), 123);

    let constant = chunk.add_constant(5.6);
    chunk.write(OpCode::Constant.into(), 123);
    chunk.write(constant.try_into().unwrap(), 123);

    chunk.write(OpCode::Divide.into(), 123);
    chunk.write(OpCode::Negate.into(), 123);
    chunk.write(OpCode::Return.into(), 123);

    if let Err(error) = Vm::interpret(chunk) {
        match error {
            LoxError::CompileError => process::exit(65),
            LoxError::RuntimeError => process::exit(70),
        }
    }
}

#![deny(rust_2018_idioms)]
//#![warn(clippy::pedantic)]

mod chunk;
mod compiler;
#[cfg(any(feature = "debug_trace_execution", feature = "debug_print_code"))]
mod disassembler;
mod func_compiler;
mod gc;
mod native_functions;
mod obj;
mod op_code;
mod stack;
mod table;

pub mod ast;
pub mod error;
pub mod output;
pub mod value;
pub mod vm;

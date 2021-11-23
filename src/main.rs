use crate::{chunk::Chunk, op_code::OpCode};

mod chunk;
mod op_code;
mod value;

fn main() {
    let mut chunk = Chunk::new();
    let constant = chunk.add_constant(1.2);
    chunk.write(OpCode::Constant.into(), 123);
    // TODO We'll only support 256 constants. Is this a problem?
    chunk.write(constant.try_into().unwrap(), 123);
    chunk.write(OpCode::Return.into(), 123);
    chunk.disassemble("test");
    chunk.free();
}

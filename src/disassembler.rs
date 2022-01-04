use crate::{
    chunk::Chunk,
    op_code::{Constant, Jump, OpCode},
};

#[cfg(feature = "debug_print_code")]
pub fn disassemble(chunk: &Chunk, name: &str) {
    println!("== {} ==", name);
    let mut offset = 0;
    while offset < chunk.code.len() {
        offset = disassemble_instruction(chunk, offset);
    }
}

#[cfg(feature = "debug_trace_execution")]
pub fn disassemble_instruction_ptr(chunk: &Chunk, ip: *const OpCode) -> usize {
    let offset = unsafe { ip.offset_from(chunk.code.as_ptr()) as usize };
    disassemble_instruction(chunk, offset)
}

pub fn disassemble_instruction(chunk: &Chunk, offset: usize) -> usize {
    print!("{:04} ", offset);

    let instruction = chunk.code[offset];
    match instruction {
        OpCode::Constant(constant) => constant_instruction("OP_CONSTANT", chunk, offset, constant),
        OpCode::Negate => simple_instruction("OP_NEGATE", offset),
        OpCode::Return => simple_instruction("OP_RETURN", offset),
        OpCode::Add => simple_instruction("OP_ADD", offset),
        OpCode::Subtract => simple_instruction("OP_SUBTRACT", offset),
        OpCode::Multiply => simple_instruction("OP_MULTIPLY", offset),
        OpCode::Divide => simple_instruction("OP_DIVIDE", offset),
        OpCode::Nil => simple_instruction("OP_NIL", offset),
        OpCode::True => simple_instruction("OP_TRUE", offset),
        OpCode::False => simple_instruction("OP_FALSE", offset),
        OpCode::Not => simple_instruction("OP_NOT", offset),
        OpCode::Equal => simple_instruction("OP_EQUAL", offset),
        OpCode::Greater => simple_instruction("OP_GREATER", offset),
        OpCode::Less => simple_instruction("OP_LESS", offset),
        OpCode::Print => simple_instruction("OP_PRINT", offset),
        OpCode::Pop => simple_instruction("OP_POP", offset),
        OpCode::DefineGlobal(constant) => {
            constant_instruction("OP_DEFINE_GLOBAL", chunk, offset, constant)
        }
        OpCode::GetGlobal(constant) => {
            constant_instruction("OP_GET_GLOBAL", chunk, offset, constant)
        }
        OpCode::GetLocal(index) => byte_instruction("OP_GET_LOCAL", offset, index),
        OpCode::JumpIfFalse(jump) => jump_instruction("OP_JUMP_IF_FALSE", 1, offset, jump),
        OpCode::Jump(jump) => jump_instruction("OP_JUMP", 1, offset, jump),
        OpCode::Loop(jump) => jump_instruction("OP_LOOP", -1, offset, jump),
        OpCode::Call { arg_count } => byte_instruction("OP_CALL", offset, arg_count),
        OpCode::Function(constant) => constant_instruction("OP_FUNCTION", chunk, offset, constant),
    }
}

fn simple_instruction(name: &str, offset: usize) -> usize {
    println!("{}", name);
    offset + 1
}

fn constant_instruction(name: &str, chunk: &Chunk, offset: usize, constant: Constant) -> usize {
    println!(
        "{:-16} {:4} '{}'",
        name, constant.slot, chunk.constants[constant.slot as usize]
    );
    offset + 1
}

fn byte_instruction(name: &str, offset: usize, slot: u8) -> usize {
    println!("{:-16} {:4}", name, slot);
    offset + 1
}

fn jump_instruction(name: &str, sign: isize, offset: usize, jump: Jump) -> usize {
    println!(
        "{:-16} {:4} -> {}",
        name,
        offset,
        offset as isize + 3 + sign * jump.offset as isize
    );
    offset + 1
}

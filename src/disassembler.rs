use crate::{
    chunk::Chunk,
    op_code::{Constant, Invoke, OpCode},
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

    if offset > 0 && chunk.lines[offset] == chunk.lines[offset - 1] {
        print!("   | ")
    } else {
        print!("{:4} ", chunk.lines[offset])
    }

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
        OpCode::SetGlobal(constant) => {
            constant_instruction("OP_SET_GLOBAL", chunk, offset, constant)
        }
        OpCode::GetLocal(slot) => byte_instruction("OP_GET_LOCAL", offset, slot),
        OpCode::SetLocal(slot) => byte_instruction("OP_SET_LOCAL", offset, slot),
        OpCode::JumpIfFalse(jump) => jump_instruction("OP_JUMP_IF_FALSE", 1, offset, jump),
        OpCode::Jump(jump) => jump_instruction("OP_JUMP", 1, offset, jump),
        OpCode::Loop(jump) => jump_instruction("OP_LOOP", -1, offset, jump),
        OpCode::Call(slot) => byte_instruction("OP_CALL", offset, slot),
        OpCode::Closure(constant) => constant_instruction("OP_CLOSURE", chunk, offset, constant),
        OpCode::GetUpvalue(slot) => byte_instruction("OP_GET_UPVALUE", offset, slot),
        OpCode::SetUpvalue(slot) => byte_instruction("OP_SET_UPVALUE", offset, slot),
        OpCode::CloseUpvalue => simple_instruction("OP_CLOSE_UPVALUE", offset),
        OpCode::Class(constant) => constant_instruction("OP_CLASS", chunk, offset, constant),
        OpCode::GetProperty(constant) => {
            constant_instruction("OP_GET_PROPERTY", chunk, offset, constant)
        }
        OpCode::SetProperty(constant) => {
            constant_instruction("OP_SET_PROPERTY", chunk, offset, constant)
        }
        OpCode::Method(constant) => constant_instruction("OP_METHOD", chunk, offset, constant),
        OpCode::Invoke(invoke) => invoke_instruction("OP_INVOKE", chunk, offset, invoke),
        OpCode::Inherit => simple_instruction("OP_INHERIT", offset),
        OpCode::GetSuper(constant) => constant_instruction("OP_GET_SUPER", chunk, offset, constant),
        OpCode::SuperInvoke(invoke) => invoke_instruction("OP_SUPER_INVOKE", chunk, offset, invoke),
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

fn invoke_instruction(name: &str, chunk: &Chunk, offset: usize, invoke: Invoke) -> usize {
    println!(
        "{:-16} {:4} '{}' ({} args)",
        name, invoke.name.slot, chunk.constants[invoke.name.slot as usize], invoke.arg_count,
    );
    offset + 1
}

fn byte_instruction(name: &str, offset: usize, slot: u8) -> usize {
    println!("{:-16} {:4}", name, slot);
    offset + 1
}

fn jump_instruction(name: &str, sign: isize, offset: usize, jump: u16) -> usize {
    println!(
        "{:-16} {:4} -> {}",
        name,
        offset,
        offset as isize + 3 + sign * jump as isize
    );
    offset + 1
}

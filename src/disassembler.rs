use crate::{chunk::Chunk, op_code::OpCode};

pub fn disassemble(chunk: &Chunk, name: &str) {
    println!("== {} ==", name);
    let mut offset = 0;
    while offset < chunk.code.len() {
        offset = disassemble_instruction(chunk, offset);
    }
}

pub fn disassemble_instruction_ptr(chunk: &Chunk, ip: *const u8) -> usize {
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

    let byte = chunk.code[offset];
    match OpCode::try_from(byte) {
        Ok(instruction) => match instruction {
            OpCode::Constant => constant_instruction("OP_CONSTANT", chunk, offset),
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
            OpCode::DefineGlobal => constant_instruction("OP_DEFINE_GLOBAL", chunk, offset),
            OpCode::GetGlobal => constant_instruction("OP_GET_GLOBAL", chunk, offset),
            OpCode::SetGlobal => constant_instruction("OP_SET_GLOBAL", chunk, offset),
            OpCode::GetLocal => byte_instruction("OP_GET_LOCAL", chunk, offset),
            OpCode::SetLocal => byte_instruction("OP_SET_LOCAL", chunk, offset),
            OpCode::JumpIfFalse => jump_instruction("OP_JUMP_IF_FALSE", 1, chunk, offset),
            OpCode::Jump => jump_instruction("OP_JUMP", 1, chunk, offset),
            OpCode::Loop => jump_instruction("OP_WHILE", -1, chunk, offset),
            OpCode::Call => byte_instruction("OP_CALL", chunk, offset),
        },
        Err(_) => {
            println!("Unknown opcode {}", byte);
            offset + 1
        }
    }
}

fn simple_instruction(name: &str, offset: usize) -> usize {
    println!("{}", name);
    offset + 1
}

fn constant_instruction(name: &str, chunk: &Chunk, offset: usize) -> usize {
    let constant = chunk.code[offset + 1] as usize;
    println!(
        "{:-16} {:4} '{}'",
        name, constant, chunk.constants[constant]
    );
    offset + 2
}

fn byte_instruction(name: &str, chunk: &Chunk, offset: usize) -> usize {
    let slot = chunk.code[offset + 1] as usize;
    println!("{:-16} {:4}", name, slot);
    offset + 2
}

fn jump_instruction(name: &str, sign: isize, chunk: &Chunk, offset: usize) -> usize {
    let byte1 = chunk.code[offset + 1];
    let byte2 = chunk.code[offset + 2];
    let jump = (byte1 as u16) << 8 | (byte2 as u16);

    println!(
        "{:-16} {:4} -> {}",
        name,
        offset,
        offset as isize + 3 + sign * jump as isize
    );
    offset + 3
}

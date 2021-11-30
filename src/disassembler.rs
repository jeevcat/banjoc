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
            OpCode::Constant => constant_instruction(chunk, "OP_CONSTANT", offset),
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
            OpCode::Less => simple_instruction("Less", offset),
            OpCode::Print => simple_instruction("print", offset),
            OpCode::Pop => simple_instruction("pop", offset),
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

fn constant_instruction(chunk: &Chunk, name: &str, offset: usize) -> usize {
    let constant = chunk.code[offset + 1] as usize;
    println!(
        "{:-16} {:4} '{}'",
        name, constant, chunk.constants[constant]
    );
    offset + 2
}

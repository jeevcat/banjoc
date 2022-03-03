use crate::{
    ast::{BinaryType, LiteralType, UnaryType},
    error::{BanjoError, Result},
    gc::Gc,
    op_code::{Constant, OpCode},
    value::Value,
};

pub struct Chunk {
    pub code: Vec<OpCode>,
    pub constants: Vec<Value>,
}

impl Chunk {
    pub fn new() -> Chunk {
        Chunk {
            code: vec![],
            constants: vec![],
        }
    }

    /// Write the given op code to the chunk
    pub fn emit(&mut self, opcode: OpCode) {
        self.code.push(opcode);
    }

    pub fn emit_unary(&mut self, unary_type: &UnaryType) {
        match unary_type {
            UnaryType::Negate => self.emit(OpCode::Negate),
            UnaryType::Not => self.emit(OpCode::Not),
        }
    }

    pub fn emit_binary(&mut self, binary_type: &BinaryType) {
        // Compile the operator
        match binary_type {
            BinaryType::Subtract => self.emit(OpCode::Subtract),
            BinaryType::Divide => self.emit(OpCode::Divide),
            BinaryType::Equals => self.emit(OpCode::Equal),
            BinaryType::Greater => self.emit(OpCode::Greater),
            BinaryType::Less => self.emit(OpCode::Less),
            BinaryType::NotEquals => {
                self.emit(OpCode::Equal);
                self.emit(OpCode::Not);
            }
            BinaryType::GreaterEqual => {
                self.emit(OpCode::Less);
                self.emit(OpCode::Not);
            }
            BinaryType::LessEqual => {
                self.emit(OpCode::Greater);
                self.emit(OpCode::Not);
            }
        }
    }

    pub fn literal(&mut self, gc: &mut Gc, value: &LiteralType) -> Result<()> {
        match value {
            LiteralType::Bool(b) => self.emit(if *b { OpCode::True } else { OpCode::False }),
            LiteralType::Nil => self.emit(OpCode::Nil),
            LiteralType::Number(n) => self.emit_constant(Value::Number(*n))?,
            LiteralType::String(s) => {
                let value = Value::String(gc.intern(s));
                self.emit_constant(value)?;
            }
        }
        Ok(())
    }

    pub fn make_constant(&mut self, value: Value) -> Result<Constant> {
        let constant = self.add_constant(value);
        if constant > u8::MAX.into() {
            // TODO we'd want to add another instruction like OpCode::Constant16 which
            // stores the index as a two-byte operand when this limit is hit
            return Err(BanjoError::Compile(
                "Too many constants in one chunk.".to_string(),
            ));
        }
        Ok(Constant {
            slot: constant.try_into().unwrap(),
        })
    }

    fn emit_constant(&mut self, value: Value) -> Result<()> {
        let slot = self.make_constant(value)?;
        self.emit(OpCode::Constant(slot));
        Ok(())
    }

    fn add_constant(&mut self, value: Value) -> usize {
        self.constants.push(value);
        self.constants.len() - 1
    }
}

impl Default for Chunk {
    fn default() -> Self {
        Self::new()
    }
}

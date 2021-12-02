use num_enum::{IntoPrimitive, TryFromPrimitive};

#[derive(IntoPrimitive, TryFromPrimitive)]
#[repr(u8)]
pub enum OpCode {
    /// Load constant for use
    Constant,

    Not,
    Negate,

    Add,
    Subtract,
    Multiply,
    Divide,

    Return,

    // Literals stored directly as instructions
    Nil,
    True,
    False,

    // Comparison
    Equal,
    Greater,
    Less,

    Print,
    Pop,

    DefineGlobal,
    GetGlobal,
    SetGlobal,

    GetLocal,
    SetLocal,

    JumpIfFalse,
    Jump,
    Loop,
}

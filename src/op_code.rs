use num_enum::{IntoPrimitive, TryFromPrimitive};

#[derive(IntoPrimitive, TryFromPrimitive)]
#[repr(u8)]
pub enum OpCode {
    /// Load constant for use
    Constant,
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
}

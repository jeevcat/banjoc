use num_enum::{IntoPrimitive, UnsafeFromPrimitive};

#[derive(IntoPrimitive, UnsafeFromPrimitive)]
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

    GetUpvalue,
    SetUpvalue,

    JumpIfFalse,
    Jump,
    Loop,

    Call,
    Closure,
    CloseUpvalue,

    Class,
    GetProperty,
    SetProperty,
    Method,
    Invoke,
    Inherit,
    GetSuper,
    SuperInvoke,
}

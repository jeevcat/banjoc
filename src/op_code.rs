#[derive(Clone, Copy)]
pub enum OpCode {
    /// Load constant for use
    Constant(u8),

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

    DefineGlobal(u8),
    GetGlobal(u8),
    SetGlobal(u8),

    GetLocal(u8),
    SetLocal(u8),

    GetUpvalue(u8),
    SetUpvalue(u8),

    JumpIfFalse(u16),
    Jump(u16),
    Loop(u16),

    Call(u8),
    Closure(u8),
    CloseUpvalue,

    Class(u8),
    GetProperty(u8),
    SetProperty(u8),
    Method(u8),
    Invoke((u8, u8)),
    Inherit,
    GetSuper(u8),
    SuperInvoke((u8, u8)),
}

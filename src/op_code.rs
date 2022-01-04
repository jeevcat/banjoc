#[derive(Clone, Copy)]
pub struct Constant {
    pub slot: u8,
}

impl Constant {
    pub const fn none() -> Self {
        Self { slot: 0 }
    }
}

#[derive(Clone, Copy)]
pub struct Jump {
    pub offset: u16,
}

impl Jump {
    pub const fn none() -> Self {
        Self { offset: 0xffff }
    }
}

pub type LocalIndex = u8;

#[derive(Clone, Copy)]
pub enum OpCode {
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

    /// Load constant for use to top of stack
    Constant(Constant),
    DefineGlobal(Constant),
    GetGlobal(Constant),

    GetLocal(LocalIndex),

    JumpIfFalse(Jump),
    Jump(Jump),
    Loop(Jump),

    Call {
        arg_count: u8,
    },
    Function(Constant),
}

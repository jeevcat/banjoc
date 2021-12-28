#[derive(Clone, Copy)]
pub struct Constant {
    pub slot: u8,
}

impl Constant {
    pub fn none() -> Self {
        Self { slot: 0 }
    }
}

#[derive(Clone, Copy)]
pub struct Invoke {
    pub name: Constant,
    pub arg_count: u8,
}

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
    SetGlobal(Constant),

    GetLocal(u8),
    SetLocal(u8),

    GetUpvalue(u8),
    SetUpvalue(u8),

    JumpIfFalse(u16),
    Jump(u16),
    Loop(u16),

    Call(u8),
    Closure(Constant),
    CloseUpvalue,

    Class(Constant),
    GetProperty(Constant),
    SetProperty(Constant),
    Method(Constant),
    Invoke(Invoke),
    Inherit,
    GetSuper(Constant),
    SuperInvoke(Invoke),
}

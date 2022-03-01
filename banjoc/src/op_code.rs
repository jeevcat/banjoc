#[derive(Clone, Copy)]
pub struct Constant {
    pub slot: u8,
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
    Equal,
    Greater,
    Less,

    Return,

    // Literals stored directly as instructions
    Nil,
    True,
    False,

    Pop,

    /// Load constant for use to top of stack
    Constant(Constant),
    DefineGlobal(Constant),
    GetGlobal(Constant),
    GetLocal(LocalIndex),

    Call {
        arg_count: u8,
    },
    Function(Constant),
    // Write top of stack to output
    Output {
        output_index: u8,
    },
}

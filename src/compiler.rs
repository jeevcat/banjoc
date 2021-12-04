use crate::{
    error::{LoxError, Result},
    obj::Function,
    scanner::Token,
};

#[derive(Clone, Copy)]
pub enum FunctionType {
    Script,
    Function,
}

pub struct Compiler<'source> {
    locals: [Local<'source>; Compiler::MAX_LOCAL_COUNT],
    pub function: Function,
    function_type: FunctionType,
    /// How many locals are currently in scope
    local_count: usize,
    /// The number of blocks surrounding the current bit of code
    scope_depth: i32,
}

impl<'source> Compiler<'source> {
    const MAX_LOCAL_COUNT: usize = u8::MAX as usize + 1;

    pub fn new(function_type: FunctionType) -> Self {
        const INIT: Local = Local {
            name: Token::none(),
            depth: 0,
        };
        // Claim stack slot zero for the VM's own internal use
        let local_count = 1;

        Self {
            locals: [INIT; Compiler::MAX_LOCAL_COUNT],
            function: Function::new(),
            function_type,
            local_count,
            scope_depth: 0,
        }
    }

    pub fn begin_scope(&mut self) {
        self.scope_depth += 1;
    }

    pub fn end_scope(&mut self) {
        self.scope_depth -= 1;
    }

    pub fn add_local(&mut self, name: Token<'source>) -> Result<()> {
        if self.local_count == Self::MAX_LOCAL_COUNT {
            return Err(LoxError::CompileError);
        }

        let local = &mut self.locals[self.local_count];
        local.name = name;
        // Only "declare" for now, by assigning sentinel value
        local.depth = -1;

        self.local_count += 1;

        Ok(())
    }

    pub fn mark_local_initialized(&mut self) {
        // Now "define"
        self.locals[self.local_count - 1].depth = self.scope_depth;
    }

    pub fn remove_local(&mut self) {
        self.local_count -= 1;
    }

    pub fn resolve_local(&mut self, name: Token) -> Option<(usize, bool)> {
        for i in (0..self.local_count).rev() {
            let local = &self.locals[i];
            if name.lexeme == local.name.lexeme {
                let err = local.depth == -1;
                return Some((i, err));
            }
        }
        None
    }

    /// Is the current scope a non-global scope?
    pub fn is_local_scope(&self) -> bool {
        self.scope_depth > 0
    }

    /// Are there locals stored in the current scope?
    pub fn has_local_in_scope(&self) -> bool {
        self.local_count > 0 && self.locals[self.local_count - 1].depth >= self.scope_depth
    }

    pub fn is_local_already_in_scope(&self, name: Token) -> bool {
        // Search for a variable with the same name in the current scope
        for i in (0..self.local_count).rev() {
            let local = &self.locals[i];
            if local.depth != -1 && local.depth < self.scope_depth {
                break;
            }

            if name.lexeme == local.name.lexeme {
                return true;
            }
        }
        false
    }
}

struct Local<'source> {
    name: Token<'source>,
    /// The scope depth of the block where the local variable was declared
    depth: i32,
}

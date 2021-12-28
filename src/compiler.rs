use crate::{
    error::{LoxError, Result},
    gc::GcRef,
    obj::{Function, FunctionUpvalue, LoxString},
    op_code::{LocalIndex, UpvalueIndex},
    scanner::{Token, TokenType},
};

#[derive(Clone, Copy)]
pub enum FunctionType {
    Script,
    Function,
    Method,
    Initializer,
}

/// A compiler for a function, including the implicit top-level function, <script>
pub struct Compiler<'source> {
    // TODO can this be improved without using the heap?
    pub enclosing: Option<Box<Compiler<'source>>>,
    pub function: Function,
    pub function_type: FunctionType,
    /// Keeps track of which stack slots are associated with which local variables or temporaries
    // TODO this can be a Stack
    locals: [Local<'source>; Compiler::MAX_LOCAL_COUNT],
    /// How many locals are currently in scope
    local_count: usize,
    /// The number of blocks surrounding the current bit of code
    scope_depth: u32,
}

impl<'source> Compiler<'source> {
    const MAX_LOCAL_COUNT: usize = u8::MAX as usize + 1;

    pub fn new(function_type: FunctionType, function_name: Option<GcRef<LoxString>>) -> Self {
        const INIT_LOCAL: Local = Local {
            name: Token::none(),
            depth: None,
            is_captured: false,
        };

        let mut locals = [INIT_LOCAL; Compiler::MAX_LOCAL_COUNT];
        // Claim stack slot zero for the VM's own internal use
        locals[0].depth = Some(0);
        if !matches!(function_type, FunctionType::Function) {
            locals[0].name.token_type = TokenType::This;
            locals[0].name.lexeme = "this";
        }

        let local_count = 1;

        Self {
            enclosing: None,
            locals,
            function: Function::new(function_name),
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
            return Err(LoxError::CompileError(
                "Too many local variables in function.",
            ));
        }

        let local = &mut self.locals[self.local_count];
        local.name = name;
        // Only "declare" for now, by assigning sentinel value
        local.depth = None;
        local.is_captured = false;

        self.local_count += 1;

        Ok(())
    }

    /// Returns the upvalue index
    fn add_upvalue(&mut self, index: u8, is_local: bool) -> Result<UpvalueIndex> {
        // Search for the upvalue first, for cases where closure references variable in surounding function multiple times
        let count = self.function.upvalues.len();
        for i in 0..count {
            let upvalue = &self.function.upvalues[i];
            if upvalue.index == index && upvalue.is_local == is_local {
                return Ok(i as UpvalueIndex);
            }
        }

        if count == Self::MAX_LOCAL_COUNT {
            return Err(LoxError::CompileError(
                "Too many closure variables in function.",
            ));
        }

        let upvalue = FunctionUpvalue { index, is_local };
        self.function.upvalues.push(upvalue);
        Ok(count as UpvalueIndex)
    }

    pub fn mark_var_initialized(&mut self) {
        if !self.is_local_scope() {
            return;
        }

        // Now "define"
        self.locals[self.local_count - 1].depth = Some(self.scope_depth);
    }

    pub fn remove_local(&mut self) {
        self.local_count -= 1;
    }

    pub fn resolve_local(&mut self, name: Token) -> Result<Option<LocalIndex>> {
        for i in (0..self.local_count).rev() {
            let local = &self.locals[i];
            if name.lexeme == local.name.lexeme {
                return if local.depth.is_none() {
                    Err(LoxError::CompileError(
                        "Can't read local variable in its own initializer.",
                    ))
                } else {
                    Ok(Some(i as u8))
                };
            }
        }
        Ok(None)
    }

    pub fn resolve_upvalue(&mut self, name: Token) -> Result<Option<UpvalueIndex>> {
        Ok(if let Some(enclosing) = self.enclosing.as_mut() {
            if let Some(index) = enclosing.resolve_local(name)? {
                enclosing.locals[index as usize].is_captured = true;
                Some(self.add_upvalue(index, true)?)
            } else if let Some(upvalue) = enclosing.resolve_upvalue(name)? {
                Some(self.add_upvalue(upvalue as u8, false)?)
            } else {
                None
            }
        } else {
            None
        })
    }

    pub fn get_local(&self) -> &Local {
        &self.locals[self.local_count - 1]
    }

    /// Is the current scope a non-global scope?
    pub fn is_local_scope(&self) -> bool {
        self.scope_depth > 0
    }

    /// Are there locals stored in the current scope?
    pub fn has_local_in_scope(&self) -> bool {
        if self.local_count == 0 {
            return false;
        }
        if let Some(depth) = self.get_local().depth {
            depth >= self.scope_depth
        } else {
            false
        }
    }

    pub fn is_local_already_in_scope(&self, name: Token) -> bool {
        // Search for a variable with the same name in the current scope
        for i in (0..self.local_count).rev() {
            let local = &self.locals[i];
            if let Some(depth) = local.depth {
                if depth < self.scope_depth {
                    break;
                }
            }

            if name.lexeme == local.name.lexeme {
                return true;
            }
        }
        false
    }
}

pub struct ClassCompiler {
    pub enclosing: Option<Box<ClassCompiler>>,
    pub has_superclass: bool,
}

impl ClassCompiler {
    pub fn new(enclosing: Option<Box<ClassCompiler>>) -> Self {
        Self {
            enclosing,
            has_superclass: false,
        }
    }
}

pub struct Local<'source> {
    name: Token<'source>,
    /// The scope depth of the block where the local variable was declared
    /// None means declared but not defined
    depth: Option<u32>,
    pub is_captured: bool,
}

#[derive(Debug)]
pub struct Upvalue {
    pub index: u8,
    pub is_local: bool,
}

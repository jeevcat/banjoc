use crate::{
    error::{BanjoError, Result},
    gc::GcRef,
    obj::{BanjoString, Function},
    op_code::LocalIndex,
    scanner::Token,
};

/// A compiler for a graph, including the implicit top-level function, <script>
pub struct FuncCompiler<'source> {
    // TODO can this be improved without using the heap?
    pub enclosing: Option<Box<FuncCompiler<'source>>>,
    pub function: Function,
    /// Keeps track of which stack slots are associated with which local
    /// variables or temporaries
    locals: Vec<Local<'source>>,
    /// The number of blocks surrounding the current bit of code
    scope_depth: u32,
}

impl<'source> FuncCompiler<'source> {
    const MAX_LOCAL_COUNT: usize = u8::MAX as usize + 1;

    pub fn new(function_name: Option<GcRef<BanjoString>>) -> Self {
        let mut locals = Vec::with_capacity(Self::MAX_LOCAL_COUNT);
        // Claim stack slot zero for the VM's own internal use
        let name = Token::none();
        locals.push(Local {
            name,
            depth: Some(0),
        });

        Self {
            enclosing: None,
            locals,
            function: Function::new(function_name),
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
        if self.locals.len() == Self::MAX_LOCAL_COUNT {
            return Err(BanjoError::CompileError(
                "Too many local variables in function.",
            ));
        }

        // Only "declare" for now, by assigning sentinel value
        self.locals.push(Local { name, depth: None });

        Ok(())
    }

    pub fn mark_var_initialized(&mut self) {
        debug_assert!(self.is_local_scope());

        // Now "define"
        self.locals
            .last_mut()
            .unwrap()
            .mark_initialized(self.scope_depth);
    }

    pub fn remove_local(&mut self) {
        self.locals.pop();
    }

    pub fn resolve_local(&mut self, name: Token) -> Result<Option<LocalIndex>> {
        for (i, local) in self.locals.iter().enumerate().rev() {
            if name.lexeme == local.name.lexeme {
                return if local.is_initialized() {
                    Ok(Some(i as u8))
                } else {
                    Err(BanjoError::CompileError(
                        "Can't read local variable in its own initializer.",
                    ))
                };
            }
        }
        Ok(None)
    }

    /// Is the current scope a non-global scope?
    pub fn is_local_scope(&self) -> bool {
        self.scope_depth > 0
    }

    /// Are there locals stored in the current scope?
    pub fn has_local_in_scope(&self) -> bool {
        if let Some(depth) = self.locals.last().and_then(|x| x.depth) {
            depth >= self.scope_depth
        } else {
            false
        }
    }

    pub fn is_local_already_in_scope(&self, name: Token) -> bool {
        // Search for a variable with the same name in the current scope
        for local in self.locals.iter().rev() {
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

    pub fn increment_arity(&mut self) -> Result<()> {
        self.function.arity += 1;

        if self.function.arity > 255 {
            Err(BanjoError::CompileError(
                "Can't have more than 255 parameters.",
            ))
        } else {
            Ok(())
        }
    }
}

pub struct Local<'source> {
    name: Token<'source>,
    /// The scope depth of the block where the local variable was declared
    /// None means declared but not defined
    depth: Option<u32>,
}

impl<'source> Local<'source> {
    fn is_initialized(&self) -> bool {
        self.depth.is_some()
    }

    fn mark_initialized(&mut self, depth: u32) {
        self.depth = Some(depth)
    }
}

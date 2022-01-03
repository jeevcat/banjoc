use crate::{
    error::{LoxError, Result},
    gc::GcRef,
    obj::{FunctionUpvalue, Graph, LoxString},
    op_code::{LocalIndex, UpvalueIndex},
    scanner::Token,
};

/// A compiler for a graph, including the implicit top-level function, <script>
pub struct GraphCompiler<'source> {
    // TODO can this be improved without using the heap?
    pub enclosing: Option<Box<GraphCompiler<'source>>>,
    pub graph: Graph,
    /// Keeps track of which stack slots are associated with which local variables or temporaries
    locals: Vec<Local<'source>>,
    /// The number of blocks surrounding the current bit of code
    scope_depth: u32,
}

impl<'source> GraphCompiler<'source> {
    const MAX_LOCAL_COUNT: usize = u8::MAX as usize + 1;

    pub fn new(graph_name: Option<GcRef<LoxString>>) -> Self {
        let mut locals = Vec::with_capacity(Self::MAX_LOCAL_COUNT);
        // Claim stack slot zero for the VM's own internal use
        let name = Token::none();
        locals.push(Local {
            name,
            depth: Some(0),
            is_captured: false,
        });

        Self {
            enclosing: None,
            locals,
            graph: Graph::new(graph_name),
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
            return Err(LoxError::CompileError(
                "Too many local variables in function.",
            ));
        }

        // Only "declare" for now, by assigning sentinel value
        self.locals.push(Local {
            name,
            depth: None,
            is_captured: false,
        });

        Ok(())
    }

    /// Returns the upvalue index
    fn add_upvalue(&mut self, index: u8, is_local: bool) -> Result<UpvalueIndex> {
        // Search for the upvalue first, for cases where closure references variable in surounding function multiple times
        let count = self.graph.upvalues.len();
        for i in 0..count {
            let upvalue = &self.graph.upvalues[i];
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
        self.graph.upvalues.push(upvalue);
        Ok(count as UpvalueIndex)
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
                    Err(LoxError::CompileError(
                        "Can't read local variable in its own initializer.",
                    ))
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

    /// Is the current scope a non-global scope?
    pub fn is_local_scope(&self) -> bool {
        self.scope_depth > 0
    }

    pub fn get_local(&self) -> &Local {
        self.locals.last().unwrap()
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
}

pub struct Local<'source> {
    name: Token<'source>,
    /// The scope depth of the block where the local variable was declared
    /// None means declared but not defined
    depth: Option<u32>,
    pub is_captured: bool,
}

impl<'source> Local<'source> {
    fn is_initialized(&self) -> bool {
        self.depth.is_some()
    }

    fn mark_initialized(&mut self, depth: u32) {
        self.depth = Some(depth)
    }
}

#[derive(Debug)]
pub struct Upvalue {
    pub index: u8,
    pub is_local: bool,
}

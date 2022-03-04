use crate::{
    error::{Error, Result},
    gc::GcRef,
    obj::{BanjoString, Function},
    op_code::LocalIndex,
};

/// A compiler for a graph, including the implicit top-level function, <script>
pub struct FuncCompiler<'ast> {
    // TODO can this be improved without using the heap?
    pub enclosing: Option<Box<FuncCompiler<'ast>>>,
    pub function: Function,
    /// Keeps track of which stack slots are associated with which local
    /// variables or temporaries
    locals: Vec<Local<'ast>>,
    /// The number of blocks surrounding the current bit of code
    scope_depth: u32,
}

impl<'ast> FuncCompiler<'ast> {
    const MAX_LOCAL_COUNT: usize = u8::MAX as usize + 1;

    pub fn new(function_name: Option<GcRef<BanjoString>>, arity: usize) -> Self {
        let mut locals = Vec::with_capacity(Self::MAX_LOCAL_COUNT);
        // Claim stack slot zero for the VM's own internal use
        locals.push(Local {
            id: "",
            depth: Some(0),
        });

        Self {
            enclosing: None,
            locals,
            function: Function::new(function_name, arity),
            scope_depth: 0,
        }
    }

    pub fn begin_scope(&mut self) {
        self.scope_depth += 1;
    }

    pub fn add_local(&mut self, node_id: &'ast str) -> Result<()> {
        if self.locals.len() == Self::MAX_LOCAL_COUNT {
            return Error::node_err(node_id, "Too many local variables in function.");
        }

        // Only "declare" for now, by assigning sentinel value
        self.locals.push(Local {
            id: node_id,
            depth: None,
        });

        Ok(())
    }

    pub fn mark_var_initialized(&mut self) {
        if !self.is_local_scope() {
            return;
        }

        // Now "define"
        self.locals
            .last_mut()
            .unwrap()
            .mark_initialized(self.scope_depth);
    }

    pub fn resolve_local(&mut self, node_id: &str) -> Result<Option<LocalIndex>> {
        for (i, local) in self.locals.iter().enumerate().rev() {
            if node_id == local.id {
                return if local.is_initialized() {
                    Ok(Some(i as u8))
                } else {
                    Error::node_err(node_id, "Can't read local variable in its own initializer.")
                };
            }
        }
        Ok(None)
    }

    /// Is the current scope a non-global scope?
    pub fn is_local_scope(&self) -> bool {
        self.scope_depth > 0
    }

    pub fn is_local_already_in_scope(&self, node_id: &str) -> bool {
        // Search for a variable with the same name in the current scope
        for local in self.locals.iter().rev() {
            if let Some(depth) = local.depth {
                if depth < self.scope_depth {
                    break;
                }
            }

            if node_id == local.id {
                return true;
            }
        }
        false
    }
}

pub struct Local<'ast> {
    id: &'ast str,
    /// The scope depth of the block where the local variable was declared
    /// None means declared but not defined
    depth: Option<u32>,
}

impl<'ast> Local<'ast> {
    fn is_initialized(&self) -> bool {
        self.depth.is_some()
    }

    fn mark_initialized(&mut self, depth: u32) {
        self.depth = Some(depth);
    }
}

use std::{collections::HashSet, mem};

use crate::{
    ast::{Ast, LiteralType, Node, NodeType},
    error::{Context, Error, Result},
    func_compiler::FuncCompiler,
    gc::{Gc, GcRef},
    obj::Function,
    op_code::{Constant, OpCode},
    output::OutputValues,
    value::Value,
};

pub struct Compiler<'ast> {
    /// The abstract syntax tree to compile
    ast: &'ast Ast<'ast>,
    /// Needed so we can allocate functions and interned strings
    gc: &'ast mut Gc,
    /// Needed so we can inform VM of nodes that expect output values
    output: &'ast mut OutputValues,
    // TODO: this should be an option
    compiler: Box<FuncCompiler<'ast>>,
}

macro_rules! current_chunk {
    ($self:ident) => {
        $self.compiler.function.chunk
    };
}

impl<'ast> Compiler<'ast> {
    pub fn new(
        ast: &'ast Ast<'ast>,
        gc: &'ast mut Gc,
        output: &'ast mut OutputValues,
    ) -> Compiler<'ast> {
        Self {
            compiler: Box::new(FuncCompiler::new(None, 0)),
            gc,
            ast,
            output,
        }
    }

    pub fn compile(&mut self) -> GcRef<Function> {
        // Topological sort
        fn visit<'ast>(
            this: &mut Compiler<'ast>,
            in_branch: &mut HashSet<&'ast str>,
            visited: &mut HashSet<&'ast str>,
            node: &'ast Node,
        ) -> Result<()> {
            if visited.contains(node.id.as_str()) {
                return Ok(());
            }
            if in_branch.contains(node.id.as_str()) {
                return Error::node_err(&node.id, "Detected cycle");
            }

            in_branch.insert(node.id.as_str());

            for child in node.dependencies().chain(node.args()) {
                // We shoud ignore missing nodes as they could reference native functions
                // Besides, the error will surface later if a non-native function is incorrectly
                // referenced
                if let Ok(child_node) = this.ast.get_node(child) {
                    visit(this, in_branch, visited, child_node)
                        .unwrap_or_else(|e| this.output.add_error(e));
                }
            }

            in_branch.remove(node.id.as_str());
            visited.insert(node.id.as_str());

            match &node.node_type {
                NodeType::FunctionDefinition { args, .. } => {
                    if args.len() != 1 {
                        return Error::node_err(
                            &node.id,
                            "Function definition requires exactly 1 input.",
                        );
                    }

                    let arity = *this.ast.get_arity(&node.id).unwrap_or(&256);
                    if arity > 0 {
                        this.node_function_definition(&node.id, args, arity)
                    } else {
                        // Treat a function defn with no parameters as a variable defn, effectively
                        // memoizing it
                        this.node_variable_definition(&node.id, args)
                    }
                }
                NodeType::VariableDefinition { args } => {
                    if args.len() != 1 {
                        return Error::node_err(
                            &node.id,
                            "Variable definition requires exactly 1 input.",
                        );
                    }

                    this.node_variable_definition(&node.id, args)
                }
                NodeType::Const { value } => this.node_const_declaration(value, &node.id),
                _ => Ok(()),
            }
            .unwrap_or_else(|e| this.output.add_error(e));
            Ok(())
        }

        // Node is in the current topological sort branch.
        // If true and this node is visited during compilation, then graph is cyclic
        let mut in_branch = HashSet::<&str>::new();
        // Node has already been processed during topological sort
        let mut visited = HashSet::<&str>::new();

        // Compile var/fn definitions
        for node in self.ast.get_roots() {
            match node.node_type {
                NodeType::VariableDefinition { .. }
                | NodeType::FunctionDefinition { .. }
                | NodeType::Const { .. } => {
                    visit(self, &mut in_branch, &mut visited, node)
                        .unwrap_or_else(|e| self.output.add_error(e));
                }
                _ => {}
            }
        }
        // Also compile disconnected roots AFTER definitions
        for node in self.ast.get_roots() {
            match node.node_type {
                NodeType::VariableDefinition { .. }
                | NodeType::FunctionDefinition { .. }
                | NodeType::Const { .. } => {}
                _ => self.node(node).unwrap_or_else(|e| self.output.add_error(e)),
            }
        }

        let function = self.pop_func_compiler().function;

        self.gc.alloc(function)
    }

    fn node(&mut self, node: &'ast Node) -> Result<()> {
        match &node.node_type {
            NodeType::Literal { value } => current_chunk!(self)
                .literal(self.gc, value)
                .node_context(&node.id)?,
            NodeType::Param => {
                if !self.compiler.is_local_scope() {
                    return Error::node_err(
                        &node.id,
                        "Can only use param in function declaration.",
                    );
                }
                // Only declare the param once, but allow same param to be input many times
                if !self.compiler.is_local_already_in_scope(&node.id) {
                    self.declare_local_variable(&node.id)?;
                    self.compiler.mark_var_initialized();
                }
                self.named_variable(&node.id)?;
            }
            NodeType::VariableReference { var_node_id } => {
                self.named_variable(var_node_id)?;
                self.output(&node.id)?;
            }
            NodeType::FunctionCall { args, fn_node_id } => {
                self.named_variable(fn_node_id)?;
                // Functions are compiled as variables if they have no parameters, so skip
                // calling them if arity == 0
                let arity = self.ast.get_arity(fn_node_id);
                if let Some(arity) = arity {
                    if *arity != args.len() {
                        return Error::node_err(
                            &node.id,
                            format!("Expected {} arguments but got {}.", arity, args.len()),
                        );
                    }
                }
                if *arity.unwrap_or(&256) > 0 {
                    self.call(args)?;
                }
                self.output(&node.id)?;
            }
            NodeType::Unary { args, unary_type } => {
                if args.len() != 1 {
                    return Error::node_err(&node.id, "Unary has invalid input.");
                }
                let argument = self.ast.get_node(&args[0])?;
                self.node(argument)?;
                current_chunk!(self).emit_unary(unary_type);
            }
            NodeType::Binary { args, binary_type } => {
                if args.len() != 2 {
                    return Error::node_err(&node.id, "Binary has invalid input.");
                }
                for term in args {
                    let term = self.ast.get_node(term)?;
                    self.node(term)?;
                }
                current_chunk!(self).emit_binary(binary_type);
            }
            NodeType::FunctionDefinition { .. }
            | NodeType::VariableDefinition { .. }
            | NodeType::Const { .. } => {
                unreachable!("Should only be called via topological sort in Self::compile()");
            }
        }
        Ok(())
    }

    fn node_function_definition(
        &mut self,
        node_id: &'ast str,
        args: &[String],
        arity: usize,
    ) -> Result<()> {
        if arity > 255 {
            return Error::node_err(node_id, "Can't have more than 255 parameters.");
        }
        let body_node = self.ast.get_node(&args[0])?;
        self.fun_declaration(body_node, node_id, arity)?;
        Ok(())
    }

    fn node_variable_definition(&mut self, node_id: &'ast str, args: &[String]) -> Result<()> {
        let body_node = self.ast.get_node(&args[0])?;
        self.var_declaration(body_node, node_id)?;
        Ok(())
    }

    fn named_variable(&mut self, node_id: &'ast str) -> Result<()> {
        let opcode = {
            if let Some(index) = self.compiler.resolve_local(node_id)? {
                OpCode::GetLocal(index)
            } else {
                let constant = self.identifier_constant(node_id)?;
                OpCode::GetGlobal(constant)
            }
        };

        current_chunk!(self).emit(opcode);
        Ok(())
    }

    fn fun_declaration(
        &mut self,
        body_node: &'ast Node,
        node_id: &'ast str,
        arity: usize,
    ) -> Result<()> {
        let global = self.declare_variable(node_id);
        self.compiler.mark_var_initialized();
        self.function(body_node, node_id, arity)?;
        self.define_variable(global);
        Ok(())
    }

    fn function(&mut self, body_node: &'ast Node, node_id: &str, arity: usize) -> Result<()> {
        self.push_func_compiler(node_id, arity);
        self.compiler.begin_scope();

        self.node(body_node)?;

        // Because we end the compiler completely, there’s no need to close the
        // lingering outermost scope with end_scope().
        let FuncCompiler { function, .. } = self.pop_func_compiler();
        let value = Value::Function(self.gc.alloc(function));

        let constant = current_chunk!(self)
            .make_constant(value)
            .node_context(node_id)?;
        current_chunk!(self).emit(OpCode::Function(constant));
        Ok(())
    }

    fn call<T: AsRef<str>>(&mut self, arg_node_ids: &[T]) -> Result<()> {
        for arg in arg_node_ids {
            let arg = self.ast.get_node(arg.as_ref()).unwrap();
            self.node(arg)?;
        }
        current_chunk!(self).emit(OpCode::Call {
            arg_count: arg_node_ids.len() as u8,
        });
        Ok(())
    }

    /// A shortcut node for literal + var declaration
    fn node_const_declaration(&mut self, value: &LiteralType, node_id: &'ast str) -> Result<()> {
        let global = self.declare_variable(node_id);

        current_chunk!(self)
            .literal(self.gc, value)
            .node_context(node_id)?;

        self.output(node_id)?;

        self.define_variable(global);
        Ok(())
    }

    fn var_declaration(&mut self, body_node: &'ast Node, node_id: &'ast str) -> Result<()> {
        let global = self.declare_variable(node_id);

        self.node(body_node)?;
        self.output(node_id)?;

        self.define_variable(global);
        Ok(())
    }

    /// Declare existence of local or global variable, not yet assigning a value
    fn declare_variable(&mut self, node_id: &'ast str) -> Option<Constant> {
        // At runtime, locals aren’t looked up by name.
        // There’s no need to stuff the variable’s name into the constant table, so if
        // the declaration is inside a local scope, we return None instead.
        if self.compiler.is_local_scope() {
            self.declare_local_variable(node_id).ok()?;
            None
        } else {
            Some(self.identifier_constant(node_id).ok()?)
        }
    }

    fn declare_local_variable(&mut self, node_id: &'ast str) -> Result<()> {
        debug_assert!(self.compiler.is_local_scope());

        if self.compiler.is_local_already_in_scope(node_id) {
            return Error::node_err(node_id, "Already a variable with this name in this scope.");
        }

        self.compiler.add_local(node_id)
    }

    fn define_variable(&mut self, global: Option<Constant>) {
        if let Some(global) = global {
            current_chunk!(self).emit(OpCode::DefineGlobal(global));
        } else {
            // For local variables, we just save references to values on the stack. No need
            // to store them somewhere else like globals do.
            debug_assert!(self.compiler.is_local_scope());
            self.compiler.mark_var_initialized();
        }
    }

    fn identifier_constant(&mut self, node_id: &str) -> Result<Constant> {
        let value = Value::String(self.gc.intern(node_id));
        current_chunk!(self)
            .make_constant(value)
            .node_context(node_id)
    }

    fn push_func_compiler(&mut self, func_id: &str, arity: usize) {
        let graph_name = self.gc.intern(func_id);
        let new_compiler = Box::new(FuncCompiler::new(Some(graph_name), arity));
        let old_compiler = mem::replace(&mut self.compiler, new_compiler);
        self.compiler.enclosing = Some(old_compiler);
    }

    fn pop_func_compiler(&mut self) -> FuncCompiler<'_> {
        current_chunk!(self).emit(OpCode::Return);

        #[cfg(feature = "debug_print_code")]
        {
            let name = self
                .compiler
                .function
                .name
                .map_or_else(|| "<script>".to_string(), |ls| ls.as_str().to_string());

            crate::disassembler::disassemble(&self.compiler.function.chunk, &name);
        }

        if let Some(enclosing) = self.compiler.enclosing.take() {
            let compiler = mem::replace(&mut self.compiler, enclosing);
            *compiler
        } else {
            // TODO no need to put a random object into self.compiler
            let compiler = mem::replace(&mut self.compiler, Box::new(FuncCompiler::new(None, 0)));
            *compiler
        }
    }

    fn output(&mut self, node_id: &'ast str) -> Result<()> {
        // We can preview the result only if we're in a function which isn't
        // parameterized
        if self.compiler.function.arity == 0 {
            let output_index = self.output.add_node(node_id)?;
            current_chunk!(self).emit(OpCode::Output { output_index });
        }

        Ok(())
    }
}

use std::{
    collections::{HashMap, HashSet},
    mem::{self},
};

use crate::{
    ast::{Ast, BinaryType, LiteralType, Node, NodeType, UnaryType},
    chunk::Chunk,
    error::{BanjoError, Result},
    func_compiler::FuncCompiler,
    gc::{Gc, GcRef},
    obj::Function,
    op_code::{Constant, OpCode},
    value::Value,
};

pub struct Compiler<'ast> {
    // TODO: this should be an option
    compiler: Box<FuncCompiler<'ast>>,
    gc: &'ast mut Gc,
    ast: &'ast Ast,
    arities: HashMap<&'ast str, usize>,
    /// IDs of nodes in order of compilation
    pub output_nodes: Vec<&'ast str>,
}

impl<'ast> Compiler<'ast> {
    pub fn new(ast: &'ast Ast, gc: &'ast mut Gc) -> Compiler<'ast> {
        Self {
            compiler: Box::new(FuncCompiler::new(None, 0)),
            gc,
            ast,
            arities: ast.calculate_arities(),
            output_nodes: vec![],
        }
    }

    pub fn compile(&mut self) -> Result<GcRef<Function>> {
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
                return BanjoError::compile_err(&node.id, "Detected cycle");
            }

            in_branch.insert(node.id.as_str());

            let mut errors = BanjoError::CompileErrors(vec![]);

            for child in node.dependencies().chain(node.arguments()) {
                // We shoud ignore missing nodes as they could reference native functions
                // Besides, the error will surface later if a non-native function is incorrectly
                // referenced
                if let Ok(child_node) = this.ast.get_node(child) {
                    visit(this, in_branch, visited, child_node)
                        .unwrap_or_else(|e| errors.append(e));
                }
            }

            in_branch.remove(node.id.as_str());
            visited.insert(node.id.as_str());

            match &node.node_type {
                NodeType::FunctionDefinition { arguments, .. } => {
                    let arity = this.get_arity(&node.id);
                    if arity > 0 {
                        this.node_function_definition(&node.id, arguments, arity)
                    } else {
                        // Treat a function defn with no parameters as a variable defn, effectively
                        // memoizing it
                        this.node_variable_definition(&node.id, arguments)
                    }
                }
                NodeType::VariableDefinition { arguments } => {
                    this.node_variable_definition(&node.id, arguments)
                }
                _ => Ok(()),
            }
            .unwrap_or_else(|e| errors.append(e));
            errors.to_result(())
        }

        let mut errors = BanjoError::CompileErrors(vec![]);
        // Node is in the current topological sort branch.
        // If true and this node is visited during compilation, then graph is cyclic
        let mut in_branch = HashSet::<&str>::new();
        // Node has already been processed during topological sort
        let mut visited = HashSet::<&str>::new();

        // Compile var/fn definitions
        let roots = self.ast.find_roots();
        for node in roots.values() {
            match node.node_type {
                NodeType::VariableDefinition { .. } | NodeType::FunctionDefinition { .. } => {
                    visit(self, &mut in_branch, &mut visited, node)
                        .unwrap_or_else(|e| errors.append(e))
                }
                _ => {}
            }
        }
        // Also compile disconnected roots AFTER definitions
        for node in roots.values() {
            match node.node_type {
                NodeType::VariableDefinition { .. } | NodeType::FunctionDefinition { .. } => {}
                _ => self.node(node).unwrap_or_else(|e| errors.append(e)),
            }
        }

        let function = self.pop_func_compiler().function;

        errors.to_result(self.gc.alloc(function))
    }

    fn node(&mut self, node: &'ast Node) -> Result<()> {
        match &node.node_type {
            NodeType::Literal { value } => self.literal(&node.id, value)?,
            NodeType::Param => {
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
            NodeType::FunctionCall {
                arguments,
                fn_node_id,
            } => {
                self.named_variable(fn_node_id)?;
                // Functions are compiled as variables if they have no parameters, so skip
                // calling them if arity == 0
                if self.get_arity(fn_node_id) > 0 {
                    self.call(arguments)?;
                }
                self.output(&node.id)?;
            }
            NodeType::Unary {
                arguments,
                unary_type,
            } => {
                if arguments.len() != 1 {
                    return BanjoError::compile_err(&node.id, "Unary has invalid input.");
                }
                let argument = self.ast.get_node(&arguments[0])?;
                self.node(argument)?;
                self.emit_unary(unary_type);
            }
            NodeType::Binary {
                arguments,
                binary_type,
            } => {
                if arguments.len() != 2 {
                    return BanjoError::compile_err(&node.id, "Binary has invalid input.");
                }
                for term in arguments {
                    let term = self.ast.get_node(term)?;
                    self.node(term)?;
                }
                self.emit_binary(binary_type);
            }
            NodeType::FunctionDefinition { .. } | NodeType::VariableDefinition { .. } => {
                // Should only be called via topological sort in Self::compile()
                unreachable!("Should only be called via topological sort in Self::compile()");
            }
        }
        Ok(())
    }

    fn node_function_definition(
        &mut self,
        node_id: &'ast str,
        arguments: &[String],
        arity: usize,
    ) -> Result<()> {
        if arguments.len() != 1 {
            return BanjoError::compile_err(node_id, "Function definition has invalid input.");
        }
        if arity > 255 {
            return BanjoError::compile_err(node_id, "Can't have more than 255 parameters.");
        }
        let body_node = self.ast.get_node(&arguments[0])?;
        self.fun_declaration(body_node, node_id, arity)?;
        Ok(())
    }

    fn node_variable_definition(&mut self, node_id: &'ast str, arguments: &[String]) -> Result<()> {
        if arguments.len() != 1 {
            return BanjoError::compile_err(node_id, "Variable definition has invalid input.");
        }

        let body_node = self.ast.get_node(&arguments[0])?;
        self.var_declaration(body_node, node_id)?;
        Ok(())
    }

    fn emit_unary(&mut self, unary_type: &UnaryType) {
        // Emit the operator instruction.
        match unary_type {
            UnaryType::Negate => self.emit(OpCode::Negate),
            UnaryType::Not => self.emit(OpCode::Not),
        }
    }

    fn emit_binary(&mut self, binary_type: &BinaryType) {
        // Compile the operator
        match binary_type {
            BinaryType::Subtract => self.emit(OpCode::Subtract),
            BinaryType::Divide => self.emit(OpCode::Divide),
            BinaryType::Equals => self.emit(OpCode::Equal),
            BinaryType::Greater => self.emit(OpCode::Greater),
            BinaryType::Less => self.emit(OpCode::Less),
            BinaryType::NotEquals => {
                self.emit(OpCode::Equal);
                self.emit(OpCode::Not);
            }
            BinaryType::GreaterEqual => {
                self.emit(OpCode::Less);
                self.emit(OpCode::Not);
            }
            BinaryType::LessEqual => {
                self.emit(OpCode::Greater);
                self.emit(OpCode::Not);
            }
        }
    }

    fn literal(&mut self, node_id: &str, value: &LiteralType) -> Result<()> {
        match value {
            LiteralType::Bool(b) => self.emit(if *b { OpCode::True } else { OpCode::False }),
            LiteralType::Nil => self.emit(OpCode::Nil),
            LiteralType::Number(n) => self.emit_constant(node_id, Value::Number(*n))?,
            LiteralType::String(s) => {
                let value = Value::String(self.gc.intern(s));
                self.emit_constant(node_id, value)?;
            }
        }
        Ok(())
    }

    fn current_chunk(&mut self) -> &mut Chunk {
        &mut self.compiler.function.chunk
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

        self.emit(opcode);
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
        self.begin_scope();

        self.node(body_node)?;

        // Because we end the compiler completely, there’s no need to close the
        // lingering outermost scope with end_scope().
        let FuncCompiler { function, .. } = self.pop_func_compiler();
        let value = Value::Function(self.gc.alloc(function));

        let constant = self.make_constant(node_id, value)?;
        self.emit(OpCode::Function(constant));
        Ok(())
    }

    fn call<T: AsRef<str>>(&mut self, arg_node_ids: &[T]) -> Result<()> {
        for arg in arg_node_ids {
            let arg = self.ast.get_node(arg.as_ref()).unwrap();
            self.node(arg)?;
        }
        self.emit(OpCode::Call {
            arg_count: arg_node_ids.len() as u8,
        });
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
            return BanjoError::compile_err(
                node_id,
                "Already a variable with this name in this scope.",
            );
        }

        self.compiler.add_local(node_id)
    }

    fn define_variable(&mut self, global: Option<Constant>) {
        if let Some(global) = global {
            self.emit(OpCode::DefineGlobal(global));
        } else {
            // For local variables, we just save references to values on the stack. No need
            // to store them somewhere else like globals do.
            debug_assert!(self.compiler.is_local_scope());
            self.compiler.mark_var_initialized();
        }
    }

    fn identifier_constant(&mut self, node_id: &str) -> Result<Constant> {
        let value = Value::String(self.gc.intern(node_id));
        self.make_constant(node_id, value)
    }

    fn push_func_compiler(&mut self, func_id: &str, arity: usize) {
        let graph_name = self.gc.intern(func_id);
        let new_compiler = Box::new(FuncCompiler::new(Some(graph_name), arity));
        let old_compiler = mem::replace(&mut self.compiler, new_compiler);
        self.compiler.enclosing = Some(old_compiler);
    }

    fn pop_func_compiler(&mut self) -> FuncCompiler {
        // #TODO can we include the return in the OpCode::Call?
        self.emit(OpCode::Return);

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

    fn begin_scope(&mut self) {
        self.compiler.begin_scope();
    }

    fn emit(&mut self, opcode: OpCode) {
        self.current_chunk().write(opcode);
    }

    fn emit_constant(&mut self, node_id: &str, value: Value) -> Result<()> {
        let slot = self.make_constant(node_id, value)?;
        self.emit(OpCode::Constant(slot));
        Ok(())
    }

    fn make_constant(&mut self, node_id: &str, value: Value) -> Result<Constant> {
        let constant = self.current_chunk().add_constant(value);
        if constant > u8::MAX.into() {
            // TODO we'd want to add another instruction like OpCode::Constant16 which
            // stores the index as a two-byte operand when this limit is hit
            return BanjoError::compile_err(node_id, "Too many constants in one chunk.");
        }
        Ok(Constant {
            slot: constant.try_into().unwrap(),
        })
    }

    fn output(&mut self, node_id: &'ast str) -> Result<()> {
        // We can preview the result only if we're in a function which isn't
        // parameterized
        if self.compiler.function.arity == 0 {
            if self.output_nodes.len() >= 255 {
                return BanjoError::compile_err(
                    node_id,
                    "Can't preview the output of more than 255 nodes",
                );
            }
            self.output_nodes.push(node_id);
            let output_index = (self.output_nodes.len() - 1) as u8;
            self.emit(OpCode::Output { output_index });
        }

        Ok(())
    }

    fn get_arity(&self, fn_node_id: &'ast str) -> usize {
        #[cfg(debug_assertions)]
        {
            if let Ok(node) = self.ast.get_node(fn_node_id) {
                assert!(matches!(
                    node.node_type,
                    NodeType::FunctionDefinition { .. }
                ));
            }
        }

        match self.arities.get(fn_node_id) {
            Some(arity) => *arity,
            None => u8::MAX as usize + 1,
        }
    }
}

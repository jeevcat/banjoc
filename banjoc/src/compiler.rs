use std::mem::{self};

use crate::{
    ast::{Ast, BinaryType, LiteralType, Node, NodeId, NodeType, UnaryType},
    chunk::Chunk,
    error::{append, BanjoError, Result},
    func_compiler::FuncCompiler,
    gc::{Gc, GcRef},
    obj::Function,
    op_code::{Constant, OpCode},
    value::Value,
};

pub fn compile(ast: &Ast, gc: &mut Gc) -> Result<GcRef<Function>> {
    let mut compiler = Compiler::new(ast, gc);

    compiler.compile()?;

    let function = compiler.pop_func_compiler().function;

    Ok(gc.alloc(function))
}

struct Compiler<'source> {
    // TODO: this should be an option
    compiler: Box<FuncCompiler<'source>>,
    gc: &'source mut Gc,
    ast: &'source Ast<'source>,
}

impl<'source> Compiler<'source> {
    fn new(ast: &'source Ast, gc: &'source mut Gc) -> Compiler<'source> {
        Self {
            compiler: Box::new(FuncCompiler::new(None)),
            gc,
            ast,
        }
    }

    fn compile(&mut self) -> Result<()> {
        let mut result = Ok(());
        self.begin_scope();
        for node in self.ast.get_definitions() {
            if let Err(e) = self.node(node) {
                append(&mut result, e);
            }
        }

        let return_node = self
            .ast
            .get_return_node()
            .ok_or_else(|| BanjoError::compile("No return node."))?;
        if let Err(e) = self.node(return_node) {
            append(&mut result, e);
        }
        self.end_scope();

        result
    }

    fn node(&mut self, node: &'source Node<'source>) -> Result<()> {
        match &node.node_type {
            NodeType::Literal { value } => self.literal(value)?,
            NodeType::FunctionDefinition { arguments, .. } => {
                if arguments.len() != 1 {
                    return BanjoError::compile_err("Function definition has invalid input.");
                }
                if let Some(body_node) = self.ast.get_node(arguments[0]) {
                    self.fun_declaration(body_node, node.id)?
                } else {
                    return BanjoError::compile_err("Function definition has no input.");
                }
            }
            NodeType::VariableDefinition { arguments, name } => {
                if arguments.len() != 1 {
                    return BanjoError::compile_err("Variable definition has invalid input.");
                }
                if let Some(body_node) = self.ast.get_node(arguments[0]) {
                    self.var_declaration(body_node, name)?
                } else {
                    return BanjoError::compile_err("Variable definition has no input.");
                }
            }
            NodeType::Param { name } => {
                // Only declare the param once, but allow same param to be input many times
                if !self.compiler.is_local_already_in_scope(name) {
                    self.compiler.increment_arity()?;
                    self.declare_local_variable(name)?;
                    self.compiler.mark_var_initialized();
                }
                self.named_variable(name)?;
            }
            NodeType::VariableReference { value } => self.named_variable(value)?,
            NodeType::FunctionCall { arguments, value } => {
                self.named_variable(value)?;
                self.call(arguments)?;
            }
            NodeType::Return { arguments } => {
                if arguments.len() != 1 {
                    return BanjoError::compile_err("Return has invalid input.");
                }
                if let Some(argument) = self.ast.get_node(arguments[0]) {
                    self.node(argument)?;
                } else {
                    self.emit(OpCode::Nil);
                }
                self.emit(OpCode::Return);
            }
            NodeType::Unary {
                arguments,
                unary_type,
            } => {
                if arguments.len() != 1 {
                    return BanjoError::compile_err("Unary has invalid input.");
                }
                if let Some(argument) = self.ast.get_node(arguments[0]) {
                    self.node(argument)?;
                    self.emit_unary(unary_type);
                } else {
                    return BanjoError::compile_err("Unary has no input.");
                }
            }
            NodeType::Binary {
                arguments,
                binary_type,
            } => {
                if arguments.len() != 2 {
                    return BanjoError::compile_err("Binary has invalid input.");
                }
                for term in arguments {
                    if let Some(term) = self.ast.get_node(term) {
                        self.node(term)?;
                    } else {
                        return BanjoError::compile_err("Binary is missing an input.");
                    }
                }
                self.emit_binary(binary_type)
            }
        }
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

    fn literal(&mut self, value: &LiteralType) -> Result<()> {
        match *value {
            LiteralType::Bool(b) => self.emit(if b { OpCode::True } else { OpCode::False }),
            LiteralType::Nil => self.emit(OpCode::Nil),
            LiteralType::Number(n) => self.emit_constant(Value::Number(n))?,
            LiteralType::String(s) => {
                let value = Value::String(self.gc.intern(s));
                self.emit_constant(value)?
            }
        }
        Ok(())
    }

    fn current_chunk(&mut self) -> &mut Chunk {
        &mut self.compiler.function.chunk
    }

    fn named_variable(&mut self, id: NodeId) -> Result<()> {
        let get_opcode = {
            if let Some(index) = self.compiler.resolve_local(id)? {
                OpCode::GetLocal(index)
            } else {
                let constant = self.identifier_constant(id)?;
                OpCode::GetGlobal(constant)
            }
        };

        self.emit(get_opcode);
        Ok(())
    }

    fn fun_declaration(
        &mut self,
        body_node: &'source Node<'source>,
        id: NodeId<'source>,
    ) -> Result<()> {
        let global = self.declare_variable(id);
        self.compiler.mark_var_initialized();
        self.function(body_node, id)?;
        self.define_variable(global);
        Ok(())
    }

    fn function(&mut self, body_node: &'source Node<'source>, id: NodeId) -> Result<()> {
        self.push_func_compiler(id);
        self.begin_scope();

        self.node(body_node)?;

        // Because we end the compiler completely, there’s no need to close the
        // lingering outermost scope with end_scope().
        let FuncCompiler { function, .. } = self.pop_func_compiler();
        let value = Value::Function(self.gc.alloc(function));

        let constant = self.make_constant(value)?;
        self.emit(OpCode::Function(constant));
        Ok(())
    }

    fn call(&mut self, arguments: &[NodeId<'source>]) -> Result<()> {
        for arg in arguments {
            let arg = self.ast.get_node(arg).unwrap();
            self.node(arg)?;
        }
        self.emit(OpCode::Call {
            arg_count: arguments.len() as u8,
        });
        Ok(())
    }

    fn var_declaration(
        &mut self,
        body_node: &'source Node<'source>,
        id: NodeId<'source>,
    ) -> Result<()> {
        let global = self.declare_variable(id);

        self.node(body_node)?;

        self.define_variable(global);
        Ok(())
    }

    /// Declare existence of local or global variable, not yet assigning a value
    fn declare_variable(&mut self, id: NodeId<'source>) -> Option<Constant> {
        // At runtime, locals aren’t looked up by name.
        // There’s no need to stuff the variable’s name into the constant table, so if
        // the declaration is inside a local scope, we return None instead.
        if self.compiler.is_local_scope() {
            self.declare_local_variable(id).ok()?;
            None
        } else {
            Some(self.identifier_constant(id).ok()?)
        }
    }

    fn declare_local_variable(&mut self, id: NodeId<'source>) -> Result<()> {
        debug_assert!(self.compiler.is_local_scope());

        if self.compiler.is_local_already_in_scope(id) {
            return BanjoError::compile_err("Already a variable with this name in this scope.");
        }

        self.compiler.add_local(id)
    }

    fn define_variable(&mut self, global: Option<Constant>) {
        if let Some(global) = global {
            self.emit(OpCode::DefineGlobal(global))
        } else {
            // For local variables, we just save references to values on the stack. No need
            // to store them somewhere else like globals do.
            debug_assert!(self.compiler.is_local_scope());
            self.compiler.mark_var_initialized();
        }
    }

    fn identifier_constant(&mut self, id: NodeId) -> Result<Constant> {
        let value = Value::String(self.gc.intern(id));
        self.make_constant(value)
    }

    fn push_func_compiler(&mut self, func_id: &str) {
        let graph_name = self.gc.intern(func_id);
        let new_compiler = Box::new(FuncCompiler::new(Some(graph_name)));
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
                .map(|ls| ls.as_str().to_string())
                .unwrap_or_else(|| "<script>".to_string());

            crate::disassembler::disassemble(&self.compiler.function.chunk, &name);
        }

        if let Some(enclosing) = self.compiler.enclosing.take() {
            let compiler = mem::replace(&mut self.compiler, enclosing);
            *compiler
        } else {
            // TODO no need to put a random object into self.compiler
            let compiler = mem::replace(&mut self.compiler, Box::new(FuncCompiler::new(None)));
            *compiler
        }
    }

    fn begin_scope(&mut self) {
        self.compiler.begin_scope();
    }

    fn end_scope(&mut self) {
        // Discard locally declared variables
        while self.compiler.has_local_in_scope() {
            self.emit(OpCode::Pop);
            self.compiler.remove_local();
        }
        self.compiler.end_scope();
    }

    fn emit(&mut self, opcode: OpCode) {
        self.current_chunk().write(opcode)
    }

    fn emit_constant(&mut self, value: Value) -> Result<()> {
        let slot = self.make_constant(value)?;
        self.emit(OpCode::Constant(slot));
        Ok(())
    }

    fn make_constant(&mut self, value: Value) -> Result<Constant> {
        let constant = self.current_chunk().add_constant(value);
        if constant > u8::MAX.into() {
            // TODO we'd want to add another instruction like OpCode::Constant16 which
            // stores the index as a two-byte operand when this limit is hit
            return BanjoError::compile_err("Too many constants in one chunk.");
        }
        Ok(Constant {
            slot: constant.try_into().unwrap(),
        })
    }
}

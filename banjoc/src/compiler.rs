use std::mem::{self};

use crate::{
    chunk::Chunk,
    error::{append, LoxError, Result},
    func_compiler::FuncCompiler,
    gc::{Gc, GcRef},
    obj::Function,
    op_code::{Constant, OpCode},
    parser::{Ast, Node, NodeId, NodeType, Parser},
    scanner::{Token, TokenType},
    value::Value,
};

pub fn compile(source: &str, vm: &mut Gc) -> Result<GcRef<Function>> {
    let parser = Parser::new(source);
    let ast = parser.parse()?;
    let mut compiler = Compiler::new(vm);

    compiler.compile(&ast)?;

    let function = compiler.pop_func_compiler().function;

    Ok(vm.alloc(function))
}

struct Compiler<'source> {
    // TODO: this should be an option
    compiler: Box<FuncCompiler<'source>>,
    gc: &'source mut Gc,
}

impl<'source> Compiler<'source> {
    fn new(gc: &'source mut Gc) -> Compiler<'source> {
        Self {
            compiler: Box::new(FuncCompiler::new(None)),
            gc,
        }
    }

    fn compile(&mut self, ast: &'source Ast<'source>) -> Result<()> {
        let mut result = Ok(());
        self.begin_scope();
        for node in ast.get_definitions() {
            if let Err(e) = self.node(ast, node) {
                append(&mut result, e);
            }
        }

        let return_node = ast.get_return_node();
        if let Err(e) = self.node(ast, return_node) {
            append(&mut result, e);
        }
        self.end_scope();

        result
    }

    fn node(&mut self, ast: &'source Ast<'source>, node: &'source Node<'source>) -> Result<()> {
        // TODO unwraps below
        match &node.node_type {
            NodeType::Literal => self.literal(node.get_name())?,
            NodeType::FunctionDefinition { body, .. } => {
                let body_node = ast.get_node(body.unwrap()).unwrap();
                self.fun_declaration(ast, body_node, node.get_name())?
            }
            NodeType::VariableDefinition { body } => {
                let body =
                    body.ok_or(LoxError::CompileError("Variable definition has no input."))?;
                let body_node = ast.get_node(body).unwrap();
                self.var_declaration(ast, body_node, node.get_name())?
            }
            NodeType::Param => {
                let name = node.get_name();
                // Only declare the param once, but allow same param to be input many times
                if !self.compiler.is_local_already_in_scope(name) {
                    self.compiler.increment_arity()?;
                    self.declare_local_variable(name)?;
                    self.compiler.mark_var_initialized();
                }
                self.named_variable(name)?;
            }
            NodeType::VariableReference => self.named_variable(node.get_name())?,
            NodeType::FunctionCall { arguments } => self.call(ast, arguments)?,
            NodeType::Return { argument } => {
                let argument = ast.get_node(argument.unwrap()).unwrap();
                self.node(ast, argument)?;
                self.emit_return();
            }
            NodeType::Unary { argument } => {
                let argument = ast.get_node(argument.unwrap()).unwrap();
                self.node(ast, argument)?;
                self.emit_unary(node.node_id.token_type);
            }
            NodeType::Binary { term_a, term_b } => {
                for term in [term_a, term_b] {
                    let term = ast.get_node(term.unwrap()).unwrap();
                    self.node(ast, term)?;
                }
                self.emit_binary(node.node_id.token_type)
            }
            NodeType::Variadic { terms } => {
                if terms.is_empty() {
                    self.emit(OpCode::Nil);
                } else {
                    // For single term, don't apply an operator, just evaluate
                    let first_term = ast.get_node(terms[0]).unwrap();
                    self.node(ast, first_term)?;

                    if terms.len() > 1 {
                        for term in terms.iter().skip(1) {
                            let term = ast.get_node(term).unwrap();
                            self.node(ast, term)?;
                            self.emit_binary(node.node_id.token_type)
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn emit_unary(&mut self, operator_type: TokenType) {
        // Emit the operator instruction.
        match operator_type {
            TokenType::Negate => self.emit(OpCode::Negate),
            TokenType::Not => self.emit(OpCode::Not),
            _ => unreachable!(),
        }
    }

    fn emit_binary(&mut self, operator_type: TokenType) {
        // Compile the operator
        match operator_type {
            TokenType::Subtract => self.emit(OpCode::Subtract),
            TokenType::Divide => self.emit(OpCode::Divide),
            TokenType::Equals => self.emit(OpCode::Equal),
            TokenType::Greater => self.emit(OpCode::Greater),
            TokenType::Less => self.emit(OpCode::Less),
            TokenType::Sum => self.emit(OpCode::Add),
            TokenType::Product => self.emit(OpCode::Multiply),
            TokenType::NotEquals => {
                self.emit(OpCode::Equal);
                self.emit(OpCode::Not);
            }
            TokenType::GreaterEqual => {
                self.emit(OpCode::Less);
                self.emit(OpCode::Not);
            }
            TokenType::LessEqual => {
                self.emit(OpCode::Greater);
                self.emit(OpCode::Not);
            }
            _ => unreachable!(),
        }
    }

    fn literal(&mut self, token: Token) -> Result<()> {
        match token.token_type {
            TokenType::False => self.emit(OpCode::False),
            TokenType::Nil => self.emit(OpCode::Nil),
            TokenType::True => self.emit(OpCode::True),
            TokenType::Number => self.number(token)?,
            TokenType::String => self.string(token)?,
            _ => unreachable!(),
        }
        Ok(())
    }

    fn number(&mut self, token: Token) -> Result<()> {
        let value: f64 = token.lexeme.parse().unwrap();
        self.emit_constant(Value::Number(value))
    }

    fn string(&mut self, token: Token) -> Result<()> {
        let string = &token.lexeme[1..token.lexeme.len() - 1];
        let value = Value::String(self.gc.intern(string));
        self.emit_constant(value)
    }

    fn current_chunk(&mut self) -> &mut Chunk {
        &mut self.compiler.function.chunk
    }

    fn named_variable(&mut self, name: Token) -> Result<()> {
        let get_opcode = {
            if let Some(index) = self.compiler.resolve_local(name)? {
                OpCode::GetLocal(index)
            } else {
                let constant = self.identifier_constant(name)?;
                OpCode::GetGlobal(constant)
            }
        };

        self.emit(get_opcode);
        Ok(())
    }

    fn fun_declaration(
        &mut self,
        ast: &'source Ast<'source>,
        body_node: &'source Node<'source>,
        name: Token<'source>,
    ) -> Result<()> {
        let global = self.declare_variable(name);
        self.compiler.mark_var_initialized();
        self.function(ast, body_node, name)?;
        self.define_variable(global);
        Ok(())
    }

    fn function(
        &mut self,
        ast: &'source Ast<'source>,
        body_node: &'source Node<'source>,
        name: Token<'source>,
    ) -> Result<()> {
        self.push_func_compiler(name.lexeme);
        self.begin_scope();

        self.node(ast, body_node)?;

        // Because we end the compiler completely, there’s no need to close the
        // lingering outermost scope with end_scope().
        let FuncCompiler { function, .. } = self.pop_func_compiler();
        let value = Value::Function(self.gc.alloc(function));

        let constant = self.make_constant(value)?;
        self.emit(OpCode::Function(constant));
        Ok(())
    }

    fn call(&mut self, ast: &'source Ast, arguments: &[NodeId<'source>]) -> Result<()> {
        for arg in arguments {
            let arg = ast.get_node(arg).unwrap();
            self.node(ast, arg)?;
        }
        self.emit(OpCode::Call {
            arg_count: arguments.len() as u8,
        });
        Ok(())
    }

    fn var_declaration(
        &mut self,
        ast: &'source Ast<'source>,
        body_node: &'source Node<'source>,
        name: Token<'source>,
    ) -> Result<()> {
        let global = self.declare_variable(name);

        self.node(ast, body_node)?;

        self.define_variable(global);
        Ok(())
    }

    /// Declare existance of local or global variable, not yet assigning a value
    fn declare_variable(&mut self, name: Token<'source>) -> Option<Constant> {
        // At runtime, locals aren’t looked up by name.
        // There’s no need to stuff the variable’s name into the constant table, so if
        // the declaration is inside a local scope, we return None instead.
        if self.compiler.is_local_scope() {
            self.declare_local_variable(name).ok()?;
            None
        } else {
            Some(self.identifier_constant(name).ok()?)
        }
    }

    fn declare_local_variable(&mut self, name: Token<'source>) -> Result<()> {
        debug_assert!(self.compiler.is_local_scope());

        if self.compiler.is_local_already_in_scope(name) {
            return Err(LoxError::CompileError(
                "Already a variable with this name in this scope.",
            ));
        }

        self.compiler.add_local(name)
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

    fn identifier_constant(&mut self, name: Token) -> Result<Constant> {
        let value = Value::String(self.gc.intern(name.lexeme));
        self.make_constant(value)
    }

    fn push_func_compiler(&mut self, func_name: &str) {
        let graph_name = self.gc.intern(func_name);
        let new_compiler = Box::new(FuncCompiler::new(Some(graph_name)));
        let old_compiler = mem::replace(&mut self.compiler, new_compiler);
        self.compiler.enclosing = Some(old_compiler);
    }

    fn pop_func_compiler(&mut self) -> FuncCompiler {
        // #TODO can we include the return in the OpCode::Call?
        self.emit_return();

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

    fn emit_return(&mut self) {
        self.emit(OpCode::Return);
    }

    fn make_constant(&mut self, value: Value) -> Result<Constant> {
        let constant = self.current_chunk().add_constant(value);
        if constant > u8::MAX.into() {
            // TODO we'd want to add another instruction like OpCode::Constant16 which
            // stores the index as a two-byte operand when this limit is hit
            return Err(LoxError::CompileError("Too many constants in one chunk."));
        }
        Ok(Constant {
            slot: constant.try_into().unwrap(),
        })
    }
}

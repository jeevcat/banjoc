use std::mem::{self};

use crate::{
    chunk::Chunk,
    error::{LoxError, Result},
    func_compiler::FuncCompiler,
    gc::{Gc, GcRef},
    obj::Function,
    op_code::{Constant, Jump, OpCode},
    parser::{Ast, Node, NodeId, NodeType, Parser},
    scanner::{Token, TokenType},
    value::Value,
};

pub fn compile(source: &str, vm: &mut Gc) -> Result<GcRef<Function>> {
    let parser = Parser::new(source);
    let ast = parser.parse()?;
    let mut compiler = Compiler::new(vm);

    compiler.compile(&ast);

    let function = compiler.pop_func_compiler().function;

    if compiler.had_error {
        Err(LoxError::CompileError("Parser error."))
    } else {
        Ok(vm.alloc(function))
    }
}

struct Compiler<'source> {
    // TODO: this should be an option
    compiler: Box<FuncCompiler<'source>>,
    gc: &'source mut Gc,
    had_error: bool,
    panic_mode: bool,
}

impl<'source> Compiler<'source> {
    fn new(gc: &'source mut Gc) -> Compiler<'source> {
        Self {
            compiler: Box::new(FuncCompiler::new(None)),
            gc,
            had_error: false,
            panic_mode: false,
        }
    }

    fn compile(&mut self, ast: &'source Ast<'source>) {
        self.begin_scope();
        for node in ast.get_definitions() {
            self.compile_node(ast, node);
        }

        let return_node = ast.get_return_node();
        self.compile_node(ast, return_node);
        self.end_scope();
    }

    fn compile_node(&mut self, ast: &'source Ast<'source>, node: &'source Node<'source>) {
        // If a node fails to compile, surface the error but continue compilation
        if let Err(error) = self.node(ast, node) {
            self.error(error);
        }
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
                let body_node = ast.get_node(body.unwrap()).unwrap();
                self.var_declaration(ast, body_node, node.get_name())?
            }
            NodeType::Param => {
                self.compiler.increment_arity()?;
                self.declare_local_variable(node.get_name())?;
                self.compiler.mark_var_initialized();
            }
            NodeType::VariableReference => self.named_variable(node.get_name())?,
            NodeType::FunctionCall { arguments } => self.call(ast, arguments)?,
            NodeType::Return { argument } => {
                let node = ast.get_node(argument.unwrap()).unwrap();
                self.node(ast, node)?;
                self.emit_return();
            }
        }
        Ok(())
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
        println!("Declaring function named {}", name.lexeme);
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

        // Because we end the compiler completely, there’s no need to close the lingering outermost scope with end_scope().
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
        // There’s no need to stuff the variable’s name into the constant table, so if the declaration is inside a local scope, we return None instead.
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
            // For local variables, we just save references to values on the stack. No need to store them somewhere else like globals do.
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
            if !self.had_error {
                let name = self
                    .compiler
                    .function
                    .name
                    .map(|ls| ls.as_str().to_string())
                    .unwrap_or_else(|| "<script>".to_string());

                crate::disassembler::disassemble(&self.compiler.function.chunk, &name);
            }
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
            // TODO we'd want to add another instruction like OpCode::Constant16 which stores the index as a two-byte operand when this limit is hit
            return Err(LoxError::CompileError("Too many constants in one chunk."));
        }
        Ok(Constant {
            slot: constant.try_into().unwrap(),
        })
    }

    fn emit_jump(&mut self, opcode: OpCode) -> usize {
        self.emit(opcode);
        self.current_chunk().code.len() - 1
    }

    fn patch_jump(&mut self, pos: usize) {
        let offset = self.current_chunk().code.len() - 1 - pos;
        let offset = match u16::try_from(offset) {
            Ok(offset) => Jump { offset },
            Err(_) => {
                self.error_str("Too much code to jump over.");
                Jump::none()
            }
        };

        match self.current_chunk().code[pos] {
            OpCode::JumpIfFalse(ref mut o) => *o = offset,
            OpCode::Jump(ref mut o) => *o = offset,
            _ => unreachable!(),
        }
    }

    fn emit_loop(&mut self, start_pos: usize) {
        let offset = self.current_chunk().code.len() - start_pos;
        let offset = match u16::try_from(offset) {
            Ok(o) => Jump { offset: o },
            Err(_) => {
                self.error_str("Loop body too large.");
                Jump::none()
            }
        };
        self.emit(OpCode::Loop(offset));
    }

    fn error_str(&mut self, message: &str) {
        self.error_at(message);
    }

    fn error(&mut self, error: LoxError) {
        if let LoxError::CompileError(message) = error {
            self.error_at(message)
        }
    }

    fn error_at(&mut self, message: &str) {
        if self.panic_mode {
            return;
        }
        self.panic_mode = true;
        eprint!("Error: {}", message);
        self.had_error = true;
    }
}

use std::mem::{self};

use crate::{
    chunk::Chunk,
    error::{LoxError, Result},
    gc::{Gc, GcRef},
    graph_compiler::GraphCompiler,
    obj::Graph,
    op_code::{Constant, Jump, OpCode},
    parser::{Ast, Node, NodeType, Parser},
    scanner::{Token, TokenType},
    value::Value,
};

pub fn compile(source: &str, vm: &mut Gc) -> Result<GcRef<Graph>> {
    let parser = Parser::new(source);
    let ast = parser.parse()?;
    let mut compiler = Compiler::new(vm);

    compiler.compile(&ast);

    let graph = compiler.pop_graph_compiler().graph;

    if compiler.had_error {
        Err(LoxError::CompileError("Parser error."))
    } else {
        Ok(vm.alloc(graph))
    }
}

struct Compiler<'source> {
    // TODO: this should be an option
    compiler: Box<GraphCompiler<'source>>,
    gc: &'source mut Gc,
    had_error: bool,
    panic_mode: bool,
}

impl<'source> Compiler<'source> {
    fn new(gc: &'source mut Gc) -> Compiler<'source> {
        Self {
            compiler: Box::new(GraphCompiler::new(None)),
            gc,
            had_error: false,
            panic_mode: false,
        }
    }

    fn compile(&mut self, ast: &'source Ast<'source>) {
        self.begin_scope();
        for node in ast.get_definitions() {
            dbg!(node);
            self.node(ast, node);
        }

        let return_node = ast.get_return_node();
        self.node(ast, return_node);
        self.end_scope();
    }

    fn node(&mut self, ast: &'source Ast<'source>, node: &'source Node<'source>) {
        match &node.node_type {
            NodeType::Literal => self.literal(node.get_name()),
            NodeType::Definition { body, arity } => {
                let body_node = ast.get_node(body.unwrap()).unwrap();
                if *arity > 0 {
                    todo!();
                } else {
                    self.var_declaration(ast, body_node, node.get_name());
                }
            }
            NodeType::Param => todo!(),
            NodeType::Var => self.variable(node.get_name()),
            NodeType::Fn { arguments } => todo!(),
            NodeType::Return { argument } => {
                let node = ast.get_node(argument.unwrap()).unwrap();
                self.node(ast, node);
                self.emit_return();
            }
        }
    }

    fn literal(&mut self, token: Token) {
        match token.token_type {
            TokenType::False => self.emit(OpCode::False),
            TokenType::Nil => self.emit(OpCode::Nil),
            TokenType::True => self.emit(OpCode::True),
            TokenType::Number => self.number(token),
            TokenType::String => self.string(token),
            _ => unreachable!(),
        }
    }

    fn number(&mut self, token: Token) {
        let value: f64 = token.lexeme.parse().unwrap();
        self.emit_constant(Value::Number(value))
    }

    fn string(&mut self, token: Token) {
        let string = &token.lexeme[1..token.lexeme.len() - 1];
        let value = Value::String(self.gc.intern(string));
        self.emit_constant(value);
    }

    fn current_chunk(&mut self) -> &mut Chunk {
        &mut self.compiler.graph.chunk
    }
    
    fn variable(&mut self, name: Token) {
        if let Err(err) = self.named_variable(name) {
            self.error(err)
        }
    }
    
    fn named_variable(&mut self, name: Token) -> Result<()> {
        let get_opcode  = {
            if let Some(index) = self.compiler.resolve_local(name)? {
                OpCode::GetLocal(index)
            } else if let Some(index) = self.compiler.resolve_upvalue(name)? {
                OpCode::GetUpvalue(index)
            } else {
                let constant = self.identifier_constant(name);
                OpCode::GetGlobal(constant)
            }
        };

        self.emit(get_opcode);
        Ok(())
    }

    fn var_declaration(
        &mut self,
        ast: &'source Ast<'source>,
        body_node: &'source Node<'source>,
        name: Token<'source>,
    ) {
        let global = self.parse_variable(name);

        self.node(ast, body_node);

        self.define_variable(global);
    }

    fn parse_variable(&mut self, name: Token<'source>) -> Option<Constant> {
        // At runtime, locals aren’t looked up by name.
        // There’s no need to stuff the variable’s name into the constant table, so if the declaration is inside a local scope, we return None instead.
        if self.compiler.is_local_scope() {
            self.declare_local_variable(name);
            None
        } else {
            Some(self.identifier_constant(name))
        }
    }

    fn declare_local_variable(&mut self, name: Token<'source>) {
        debug_assert!(self.compiler.is_local_scope());

        if self.compiler.is_local_already_in_scope(name) {
            self.error_str("Already a variable with this name in this scope.");
        }

        self.add_local(name);
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

    fn add_local(&mut self, name: Token<'source>) {
        if let Err(err) = self.compiler.add_local(name) {
            self.error(err)
        }
    }

    fn identifier_constant(&mut self, name: Token) -> Constant {
        let value = Value::String(self.gc.intern(name.lexeme));
        self.make_constant(value)
    }

    fn push_graph_compiler(&mut self, graph_name: &str) {
        let graph_name = self.gc.intern(graph_name);
        let new_compiler = Box::new(GraphCompiler::new(Some(graph_name)));
        let old_compiler = mem::replace(&mut self.compiler, new_compiler);
        self.compiler.enclosing = Some(old_compiler);
    }

    fn pop_graph_compiler(&mut self) -> GraphCompiler {
        #[cfg(feature = "debug_print_code")]
        {
            if !self.had_error {
                let name = self
                    .compiler
                    .graph
                    .name
                    .map(|ls| ls.as_str().to_string())
                    .unwrap_or_else(|| "<script>".to_string());

                crate::disassembler::disassemble(&self.compiler.graph.chunk, &name);
            }
        }

        if let Some(enclosing) = self.compiler.enclosing.take() {
            let compiler = mem::replace(&mut self.compiler, enclosing);
            *compiler
        } else {
            // TODO no need to put a random object into self.compiler
            let compiler = mem::replace(&mut self.compiler, Box::new(GraphCompiler::new(None)));
            *compiler
        }
    }

    fn begin_scope(&mut self) {
        self.compiler.begin_scope();
    }

    fn end_scope(&mut self) {
        // Discard locally declared variables
        while self.compiler.has_local_in_scope() {
            if self.compiler.get_local().is_captured {
                self.emit(OpCode::CloseUpvalue);
            } else {
                self.emit(OpCode::Pop);
            }
            self.compiler.remove_local();
        }
        self.compiler.end_scope();
    }

    fn emit(&mut self, opcode: OpCode) {
        self.current_chunk().write(opcode)
    }

    fn emit_constant(&mut self, value: Value) {
        let slot = self.make_constant(value);
        self.emit(OpCode::Constant(slot));
    }

    fn emit_return(&mut self) {
        self.emit(OpCode::Return);
    }

    fn make_constant(&mut self, value: Value) -> Constant {
        let constant = self.current_chunk().add_constant(value);
        if constant > u8::MAX.into() {
            // TODO we'd want to add another instruction like OpCode::Constant16 which stores the index as a two-byte operand when this limit is hit
            self.error_str("Too many constants in one chunk.");
            return Constant::none();
        }
        Constant {
            slot: constant.try_into().unwrap(),
        }
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

    fn error_at_current(&mut self, message: &str) {
        self.error_at(message)
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

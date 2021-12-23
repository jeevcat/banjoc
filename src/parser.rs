use std::{
    mem::{self, MaybeUninit},
    ops::Index,
};

use strum::{EnumCount, IntoEnumIterator};

use crate::{
    chunk::Chunk,
    compiler::{Compiler, FunctionType},
    error::{LoxError, Result},
    gc::GcRef,
    obj::Function,
    op_code::OpCode,
    scanner::{Scanner, Token, TokenType},
    value::Value,
    vm::Vm,
};

pub fn compile(source: &str, vm: &mut Vm) -> Result<GcRef<Function>> {
    let scanner = Scanner::new(source);
    let mut parser = Parser::new(scanner, vm);

    parser.advance();
    while !parser.advance_matching(TokenType::Eof) {
        parser.declaration();
    }

    let function = parser.pop_compiler().function;

    if parser.had_error {
        Err(LoxError::CompileError("Parser error."))
    } else {
        println!("Alloc {}", function);
        Ok(vm.alloc(function))
    }
}

struct Parser<'source> {
    scanner: Scanner<'source>,
    compiler: Box<Compiler<'source>>,
    current: Token<'source>,
    previous: Token<'source>,
    vm: &'source mut Vm,
    had_error: bool,
    panic_mode: bool,
    rules: ParseRuleTable<'source>,
}

impl<'source> Parser<'source> {
    fn new(scanner: Scanner<'source>, vm: &'source mut Vm) -> Parser<'source> {
        let rules = ParseRuleTable::new();

        Self {
            scanner,
            compiler: Box::new(Compiler::new(FunctionType::Script, None)),
            current: Token::none(),
            previous: Token::none(),
            vm,
            had_error: false,
            panic_mode: false,
            rules,
        }
    }

    fn advance(&mut self) {
        self.previous = self.current;

        loop {
            self.current = self.scanner.scan_token();
            if self.current.token_type != TokenType::Error {
                break;
            }

            self.error_at_current(self.current.lexeme);
        }
    }

    fn consume(&mut self, token_type: TokenType, message: &str) {
        if self.current.token_type == token_type {
            self.advance();
            return;
        }

        self.error_at_current(message);
    }

    fn advance_matching(&mut self, token_type: TokenType) -> bool {
        if !self.check(token_type) {
            return false;
        }
        self.advance();
        true
    }

    fn check(&self, token_type: TokenType) -> bool {
        self.current.token_type == token_type
    }

    fn current_chunk(&mut self) -> &mut Chunk {
        &mut self.compiler.function.chunk
    }

    fn expression(&mut self) {
        self.parse_precedence(Precedence::Assignment)
    }

    fn block(&mut self) {
        while !self.check(TokenType::RightBrace) && !self.check(TokenType::Eof) {
            self.declaration();
        }

        self.consume(TokenType::RightBrace, "Expect '}' after block.");
    }

    fn function(&mut self, function_type: FunctionType) {
        self.push_compiler(function_type);
        self.begin_scope();

        self.consume(TokenType::LeftParen, "Expect '(' after function name.");
        if !self.check(TokenType::RightParen) {
            // Parse function parameters
            loop {
                self.compiler.function.arity += 1;

                if self.compiler.function.arity > 255 {
                    self.error_at_current("Can't have more than 255 parameters");
                }
                let constant = self.parse_variable("Expect parameter name");
                self.define_variable(constant);

                if !self.advance_matching(TokenType::Comma) {
                    break;
                }
            }
        }
        self.consume(TokenType::RightParen, "Expect ')' after parameters.");
        self.consume(TokenType::LeftBrace, "Expect '{' before function body.");
        self.block();

        let Compiler {
            function, upvalues, ..
        } = self.pop_compiler();
        let upvalue_count = function.upvalue_count;
        let value = Value::Function(self.vm.alloc(function));

        let constant = self.make_constant(value);
        self.emit_instruction(OpCode::Closure, constant);

        for upvalue in upvalues.iter().take(upvalue_count) {
            self.emit_byte(upvalue.is_local as u8);
            self.emit_byte(upvalue.index);
        }
    }

    fn fun_declaration(&mut self) {
        let global = self.parse_variable("Expect function name.");
        self.compiler.mark_var_initialized();
        self.function(FunctionType::Function);
        self.define_variable(global);
    }

    fn var_declaration(&mut self) {
        let global = self.parse_variable("Expect variable name.");

        if self.advance_matching(TokenType::Equal) {
            self.expression();
        } else {
            self.emit_opcode(OpCode::Nil);
        }
        self.consume(
            TokenType::Semicolon,
            "Expect ';' after variable declaration",
        );

        self.define_variable(global);
    }

    fn declaration(&mut self) {
        if self.advance_matching(TokenType::Fun) {
            self.fun_declaration();
        } else if self.advance_matching(TokenType::Var) {
            self.var_declaration();
        } else {
            self.statement();
        }

        if self.panic_mode {
            self.synchronize();
        }
    }

    fn statement(&mut self) {
        if self.advance_matching(TokenType::Print) {
            self.print_statement();
        } else if self.advance_matching(TokenType::LeftBrace) {
            self.begin_scope();
            self.block();
            self.end_scope();
        } else if self.advance_matching(TokenType::If) {
            self.if_statement();
        } else if self.advance_matching(TokenType::While) {
            self.while_statement();
        } else if self.advance_matching(TokenType::For) {
            self.for_statement();
        } else if self.advance_matching(TokenType::Return) {
            self.return_statement();
        } else {
            self.expression_statement();
        }
    }

    fn if_statement(&mut self) {
        self.consume(TokenType::LeftParen, "Expect '(' after 'if'.");
        self.expression();
        self.consume(TokenType::RightParen, "Expect ')' after condition.");

        let then_jump = self.emit_jump(OpCode::JumpIfFalse);
        // If we didn't jump ('if' expression was true), then pop the result of the expression before executing 'if' body
        self.emit_opcode(OpCode::Pop);

        self.statement();

        let else_jump = self.emit_jump(OpCode::Jump);

        self.patch_jump(then_jump);
        // If we did jump above ('if' expression was false), then pop the result of the expression before executing 'else' body (even if the else body is empty)
        self.emit_opcode(OpCode::Pop);

        if self.advance_matching(TokenType::Else) {
            self.statement();
        }
        self.patch_jump(else_jump);
    }

    fn while_statement(&mut self) {
        let loop_start = self.current_chunk().code.len();
        self.consume(TokenType::LeftParen, "Expect '(' after 'while'.");
        self.expression();
        self.consume(TokenType::RightParen, "Expect ')' after condition.");

        let exit_jump = self.emit_jump(OpCode::JumpIfFalse);
        // If we didn't jump ('while' expression was true), then pop the result of the expression before executing 'if' body
        self.emit_opcode(OpCode::Pop);

        self.statement();
        self.emit_loop(loop_start);

        self.patch_jump(exit_jump);
        // If we did jump above ('while' expression was false), then pop the result of the expression before executing 'else' body (even if the else body is empty)
        self.emit_opcode(OpCode::Pop);
    }

    fn for_statement(&mut self) {
        self.begin_scope();

        self.consume(TokenType::LeftParen, "Expect '(' after 'for'.");

        // Initializer clause
        if self.advance_matching(TokenType::Semicolon) {
            // No initializer
        } else if self.advance_matching(TokenType::Var) {
            self.var_declaration();
        } else {
            self.expression_statement();
        }

        let mut loop_start = self.current_chunk().code.len();

        // Compile the for clause, if present
        let exit_jump = {
            if !self.advance_matching(TokenType::Semicolon) {
                self.expression();
                self.consume(TokenType::Semicolon, "Expect ';' after loop condition.");

                // Jump out of the loop if the condition is false
                let offset = self.emit_jump(OpCode::JumpIfFalse);
                self.emit_opcode(OpCode::Pop); // Condition
                Some(offset)
            } else {
                None
            }
        };

        // Increment clause
        if !self.advance_matching(TokenType::RightParen) {
            // Jump to body of the loop
            let body_jump = self.emit_jump(OpCode::Jump);
            let increment_start = self.current_chunk().code.len();
            self.expression();
            self.emit_opcode(OpCode::Pop);
            self.consume(TokenType::RightParen, "Expect ')' after for clauses.");

            //
            self.emit_loop(loop_start);
            loop_start = increment_start;
            self.patch_jump(body_jump);
        }

        self.statement();

        self.emit_loop(loop_start);

        // Patch the for clause jump, if it was present
        if let Some(exit_jump) = exit_jump {
            self.patch_jump(exit_jump);
            self.emit_opcode(OpCode::Pop); // Condition
        }

        self.end_scope();
    }

    fn expression_statement(&mut self) {
        self.expression();
        self.consume(TokenType::Semicolon, "Expect ';' after expression.");
        self.emit_opcode(OpCode::Pop)
    }

    fn print_statement(&mut self) {
        self.expression();
        self.consume(TokenType::Semicolon, "Expect ';' after value.");
        self.emit_opcode(OpCode::Print);
    }

    fn return_statement(&mut self) {
        if matches!(self.compiler.function_type, FunctionType::Script) {
            self.error_str("Can't return from top-level code.");
        }

        if self.advance_matching(TokenType::Semicolon) {
            self.emit_return();
        } else {
            self.expression();
            self.consume(TokenType::Semicolon, "Expect ';' after return value.");
            self.emit_opcode(OpCode::Return);
        }
    }

    fn synchronize(&mut self) {
        self.panic_mode = false;

        // Skip all tokens intil we reach something that looks like a statement boundary
        while !matches!(self.current.token_type, TokenType::Eof) {
            if matches!(self.previous.token_type, TokenType::Semicolon) {
                return;
            }
            match self.current.token_type {
                TokenType::Class
                | TokenType::Fun
                | TokenType::Var
                | TokenType::For
                | TokenType::If
                | TokenType::While
                | TokenType::Print
                | TokenType::Return => return,
                _ => {
                    // Do nothing
                }
            }

            self.advance();
        }
    }

    fn number(&mut self, _can_assign: bool) {
        let value: f64 = self.previous.lexeme.parse().unwrap();
        self.emit_constant(Value::Number(value))
    }

    fn grouping(&mut self, _can_assign: bool) {
        self.expression();
        self.consume(TokenType::RightParen, "Expect ')' after expression.");
    }

    fn call(&mut self, _can_assign: bool) {
        let arg_count = self.argument_list();
        self.emit_instruction(OpCode::Call, arg_count);
    }

    fn unary(&mut self, _can_assign: bool) {
        let operator_type = self.previous.token_type;

        // Compile the operand
        self.parse_precedence(Precedence::Unary);

        // Emit the operator instruction.
        match operator_type {
            TokenType::Minus => self.emit_opcode(OpCode::Negate),
            TokenType::Bang => self.emit_opcode(OpCode::Not),
            _ => unreachable!(),
        }
    }

    fn binary(&mut self, _can_assign: bool) {
        // By the time we get here we've already compiled the left operand
        let operator_type = self.previous.token_type;

        // Compile the right operand
        // Each binary operator's right-hand operand precedence is one level higher than its own
        self.parse_precedence(self.get_rule(operator_type).precedence.next());

        // Compile the operator
        match operator_type {
            TokenType::Plus => self.emit_opcode(OpCode::Add),
            TokenType::Minus => self.emit_opcode(OpCode::Subtract),
            TokenType::Star => self.emit_opcode(OpCode::Multiply),
            TokenType::Slash => self.emit_opcode(OpCode::Divide),
            TokenType::EqualEqual => self.emit_opcode(OpCode::Equal),
            TokenType::Greater => self.emit_opcode(OpCode::Greater),
            TokenType::Less => self.emit_opcode(OpCode::Less),
            TokenType::BangEqual => {
                self.emit_opcode(OpCode::Equal);
                self.emit_opcode(OpCode::Not);
            }
            TokenType::GreaterEqual => {
                self.emit_opcode(OpCode::Less);
                self.emit_opcode(OpCode::Not);
            }
            TokenType::LessEqual => {
                self.emit_opcode(OpCode::Greater);
                self.emit_opcode(OpCode::Not);
            }
            _ => unreachable!(),
        }
    }

    fn and(&mut self, _can_assign: bool) {
        // Here, the left operand expression has already been compiled
        let end_jump = self.emit_jump(OpCode::JumpIfFalse);

        self.emit_opcode(OpCode::Pop);
        // Compile the right operand expression
        self.parse_precedence(Precedence::And);

        self.patch_jump(end_jump);
    }

    fn or(&mut self, _can_assign: bool) {
        // Here, the left operand expression has already been compiled

        // Simulate JumpIfTrue with two jumps #TODO would be more efficient with a dedicated opcode
        let else_jump = self.emit_jump(OpCode::JumpIfFalse);
        let end_jump = self.emit_jump(OpCode::Jump);

        self.patch_jump(else_jump);
        self.emit_opcode(OpCode::Pop);

        // Compile the right operand expression
        self.parse_precedence(Precedence::Or);
        self.patch_jump(end_jump);
    }

    fn literal(&mut self, _can_assign: bool) {
        match self.previous.token_type {
            TokenType::False => self.emit_opcode(OpCode::False),
            TokenType::Nil => self.emit_opcode(OpCode::Nil),
            TokenType::True => self.emit_opcode(OpCode::True),
            _ => unreachable!(),
        }
    }

    fn string(&mut self, _can_assign: bool) {
        let string = self.previous.lexeme[1..self.previous.lexeme.len() - 1].to_string();
        let value = Value::String(self.vm.intern(string));
        self.emit_constant(value);
    }

    fn variable(&mut self, can_assign: bool) {
        if let Err(err) = self.named_variable(self.previous, can_assign) {
            self.error(err)
        }
    }

    fn named_variable(&mut self, name: Token, can_assign: bool) -> Result<()> {
        let (operand, get_opcode, set_opcode) = {
            if let Some(arg) = self.compiler.resolve_local(name)? {
                (arg as u8, OpCode::GetLocal, OpCode::SetLocal)
            } else if let Some(arg) = self.compiler.resolve_upvalue(name)? {
                (arg as u8, OpCode::GetUpvalue, OpCode::SetUpvalue)
            } else {
                let arg = self.identifier_constant(name);
                (arg, OpCode::GetGlobal, OpCode::SetGlobal)
            }
        };

        if can_assign && self.advance_matching(TokenType::Equal) {
            self.expression();
            self.emit_instruction(set_opcode, operand);
        } else {
            self.emit_instruction(get_opcode, operand);
        }
        Ok(())
    }

    /// Starts at the current token and parses any expression at the given precedence or higher
    fn parse_precedence(&mut self, precedence: Precedence) {
        self.advance();
        let can_assign = precedence <= Precedence::Assignment;
        let prefix_rule = self.get_rule(self.previous.token_type).prefix;
        match prefix_rule {
            None => {
                return self.error_str("Expect expression.");
            }
            Some(prefix_rule) => prefix_rule(self, can_assign),
        }

        while precedence <= self.get_rule(self.current.token_type).precedence {
            self.advance();
            // Can unwrap as
            let infix_rule = self.get_rule(self.previous.token_type).infix.unwrap();
            infix_rule(self, can_assign);
        }

        if can_assign && self.advance_matching(TokenType::Equal) {
            self.error_str("Invalid assignment target.")
        }
    }

    fn parse_variable(&mut self, error_message: &str) -> u8 {
        self.consume(TokenType::Identifier, error_message);

        self.declare_variable();
        if self.compiler.is_local_scope() {
            return 0;
        }

        self.identifier_constant(self.previous)
    }

    fn define_variable(&mut self, global: u8) {
        if self.compiler.is_local_scope() {
            self.compiler.mark_var_initialized();
            // For local variables, we just save references to values on the stack. No need to store them somewhere else like globals do.
            return;
        }

        self.emit_instruction(OpCode::DefineGlobal, global)
    }

    fn declare_variable(&mut self) {
        if !self.compiler.is_local_scope() {
            return;
        }

        let name = self.previous;

        if self.compiler.is_local_already_in_scope(name) {
            self.error_str("Already a variable with this name in this scope.");
        }

        self.add_local(name);
    }

    fn argument_list(&mut self) -> u8 {
        let mut arg_count = 0;

        if !self.check(TokenType::RightParen) {
            loop {
                self.expression();
                if arg_count == u8::MAX {
                    self.error_str("Can't have more than 255 arguments.");
                }
                arg_count += 1;

                if !self.advance_matching(TokenType::Comma) {
                    break;
                }
            }
        }
        self.consume(TokenType::RightParen, "Expect ')' after arguments.");
        arg_count
    }

    fn add_local(&mut self, name: Token<'source>) {
        if let Err(err) = self.compiler.add_local(name) {
            self.error(err)
        }
    }

    fn identifier_constant(&mut self, name: Token) -> u8 {
        let value = Value::String(self.vm.intern(name.lexeme.to_string()));
        self.make_constant(value)
    }

    fn get_rule(&self, token_type: TokenType) -> &ParseRule<'source> {
        &self.rules[token_type]
    }

    fn push_compiler(&mut self, function_type: FunctionType) {
        let function_name = self.vm.intern(self.previous.lexeme.to_owned());
        let new_compiler = Box::new(Compiler::new(function_type, Some(function_name)));
        let old_compiler = mem::replace(&mut self.compiler, new_compiler);
        self.compiler.enclosing = Some(old_compiler);
    }

    fn pop_compiler(&mut self) -> Compiler {
        self.emit_return();

        #[cfg(feature = "debug_trace_execution")]
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
            let compiler = mem::replace(
                &mut self.compiler,
                Box::new(Compiler::new(FunctionType::Script, None)),
            );
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
                self.emit_opcode(OpCode::CloseUpvalue);
            } else {
                self.emit_opcode(OpCode::Pop);
            }
            self.compiler.remove_local();
        }
        self.compiler.end_scope();
    }

    fn emit_byte(&mut self, byte: u8) {
        let line = self.previous.line;
        self.current_chunk().write(byte, line)
    }

    fn emit_instruction(&mut self, opcode: OpCode, operand: u8) {
        self.emit_opcode(opcode);
        self.emit_byte(operand)
    }

    fn emit_opcode(&mut self, opcode: OpCode) {
        self.emit_byte(opcode.into())
    }

    fn emit_constant(&mut self, value: Value) {
        let operand = self.make_constant(value);
        self.emit_instruction(OpCode::Constant, operand);
    }

    fn emit_return(&mut self) {
        self.emit_opcode(OpCode::Nil);
        self.emit_opcode(OpCode::Return);
    }

    fn make_constant(&mut self, value: Value) -> u8 {
        let constant = self.current_chunk().add_constant(value);
        if constant > u8::MAX.into() {
            // TODO we'd want to add another instruction like OpCode::Constant16 which stores the index as a two-byte operand when this limit is hit
            self.error_str("Too many constants in one chunk.");
            return 0;
        }
        constant.try_into().unwrap()
    }

    fn emit_jump(&mut self, opcode: OpCode) -> usize {
        self.emit_opcode(opcode);
        self.emit_byte(0xff);
        self.emit_byte(0xff);
        self.current_chunk().code.len() - 2
    }

    fn patch_jump(&mut self, offset: usize) {
        // -2 to adjust for the bytecode for the jump offset itself
        let jump = self.current_chunk().code.len() - offset - 2;

        if jump > u16::MAX as usize {
            self.error_str("Too much code to jump over.");
        }

        let (byte1, byte2) = to_bytes(jump as u16);
        self.current_chunk().code[offset] = byte1 as u8;
        self.current_chunk().code[offset + 1] = byte2 as u8;
    }

    fn emit_loop(&mut self, loop_start: usize) {
        self.emit_opcode(OpCode::Loop);

        // +2: take into account the size of the 2-byte Loop operand
        let offset = self.current_chunk().code.len() - loop_start + 2;
        if offset > u16::MAX as usize {
            self.error_str("Loop body too large");
        }

        let (byte1, byte2) = to_bytes(offset as u16);
        self.emit_byte(byte1);
        self.emit_byte(byte2);
    }

    fn error_at_current(&mut self, message: &str) {
        self.error_at(self.current, message)
    }

    fn error_str(&mut self, message: &str) {
        self.error_at(self.previous, message);
    }

    fn error(&mut self, error: LoxError) {
        if let LoxError::CompileError(message) = error {
            self.error_at(self.previous, message)
        }
    }

    fn error_at(&mut self, token: Token, message: &str) {
        self.panic_mode = true;
        eprint!("[line {}] Error", token.line);

        match token.token_type {
            TokenType::Eof => eprint!(" at end"),
            TokenType::Error => {
                // Nothing
            }
            _ => eprint!(" at '{}'", token.lexeme),
        }

        eprintln!(": {}", message);
        self.had_error = true;
    }
}

#[derive(PartialEq, PartialOrd)]
enum Precedence {
    None,
    Assignment,
    Or,
    And,
    Equality,
    Comparison,
    Term,
    Factor,
    Unary,
    Call,
    Primary,
}

impl Precedence {
    fn next(&self) -> Precedence {
        match self {
            Precedence::None => Precedence::Assignment,
            Precedence::Assignment => Precedence::Or,
            Precedence::Or => Precedence::And,
            Precedence::And => Precedence::Equality,
            Precedence::Equality => Precedence::Comparison,
            Precedence::Comparison => Precedence::Term,
            Precedence::Term => Precedence::Factor,
            Precedence::Factor => Precedence::Unary,
            Precedence::Unary => Precedence::Call,
            Precedence::Call => Precedence::Primary,
            Precedence::Primary => Precedence::None,
        }
    }
}

type ParseFn<'a> = fn(&mut Parser<'a>, can_assign: bool) -> ();
struct ParseRule<'source> {
    prefix: Option<ParseFn<'source>>,
    infix: Option<ParseFn<'source>>,
    precedence: Precedence,
}

impl<'source> ParseRule<'source> {
    fn new(
        prefix: Option<ParseFn<'source>>,
        infix: Option<ParseFn<'source>>,
        precedence: Precedence,
    ) -> Self {
        Self {
            prefix,
            infix,
            precedence,
        }
    }
}

struct ParseRuleTable<'source>([ParseRule<'source>; TokenType::COUNT]);

impl<'source> ParseRuleTable<'source> {
    fn new() -> Self {
        let table = {
            // Create an array of uninitialized values.
            let mut array: [MaybeUninit<ParseRule>; TokenType::COUNT] =
                unsafe { MaybeUninit::uninit().assume_init() };

            for token_type in TokenType::iter() {
                let rule = ParseRuleTable::make_rule(token_type);
                let index: u8 = token_type.into();
                array[index as usize] = MaybeUninit::new(rule);
            }

            unsafe { std::mem::transmute::<_, [ParseRule; TokenType::COUNT]>(array) }
        };
        Self(table)
    }

    #[rustfmt::skip]
    fn make_rule(token_type: TokenType) -> ParseRule<'source> {
        use Precedence as P;
        use TokenType::*;
        match token_type {
            LeftParen =>    ParseRule::new(Some(Parser::grouping), Some(Parser::call),   P::Call),
            RightParen =>   ParseRule::new(None,                   None,                 P::None),
            LeftBrace =>    ParseRule::new(None,                   None,                 P::None),
            RightBrace =>   ParseRule::new(None,                   None,                 P::None),
            Comma =>        ParseRule::new(None,                   None,                 P::None),
            Dot =>          ParseRule::new(None,                   None,                 P::None),
            Minus =>        ParseRule::new(Some(Parser::unary),    Some(Parser::binary), P::Term),
            Plus =>         ParseRule::new(None,                   Some(Parser::binary), P::Term),
            Semicolon =>    ParseRule::new(None,                   None,                 P::None),
            Slash =>        ParseRule::new(None,                   Some(Parser::binary), P::Factor),
            Star =>         ParseRule::new(None,                   Some(Parser::binary), P::Factor),
            Bang =>         ParseRule::new(Some(Parser::unary),    None,                 P::None),
            BangEqual =>    ParseRule::new(None,                   Some(Parser::binary), P::Equality),
            Equal =>        ParseRule::new(None,                   None,                 P::None),
            EqualEqual =>   ParseRule::new(None,                   Some(Parser::binary), P::Equality),
            Greater =>      ParseRule::new(None,                   Some(Parser::binary), P::Comparison),
            GreaterEqual => ParseRule::new(None,                   Some(Parser::binary), P::Comparison),
            Less =>         ParseRule::new(None,                   Some(Parser::binary), P::Comparison),
            LessEqual =>    ParseRule::new(None,                   Some(Parser::binary), P::Comparison),
            Identifier =>   ParseRule::new(Some(Parser::variable), None,                 P::None),
            String =>       ParseRule::new(Some(Parser::string),   None,                 P::None),
            Number =>       ParseRule::new(Some(Parser::number),   None,                 P::None),
            And =>          ParseRule::new(None,                   Some(Parser::and),    P::And),
            Class =>        ParseRule::new(None,                   None,                 P::None),
            Else =>         ParseRule::new(None,                   None,                 P::None),
            False =>        ParseRule::new(Some(Parser::literal),  None,                 P::None),
            For =>          ParseRule::new(None,                   None,                 P::None),
            Fun =>          ParseRule::new(None,                   None,                 P::None),
            If =>           ParseRule::new(None,                   None,                 P::None),
            Nil =>          ParseRule::new(Some(Parser::literal),  None,                 P::None),
            Or =>           ParseRule::new(None,                   Some(Parser::or),     P::Or),
            Print =>        ParseRule::new(None,                   None,                 P::None),
            Return =>       ParseRule::new(None,                   None,                 P::None),
            Super =>        ParseRule::new(None,                   None,                 P::None),
            This =>         ParseRule::new(None,                   None,                 P::None),
            True =>         ParseRule::new(Some(Parser::literal),  None,                 P::None),
            Var =>          ParseRule::new(None,                   None,                 P::None),
            While =>        ParseRule::new(None,                   None,                 P::None),
            Error =>        ParseRule::new(None,                   None,                 P::None),
            Eof =>          ParseRule::new(None,                   None,                 P::None),
        }
    }
}

impl<'source> Index<TokenType> for ParseRuleTable<'source> {
    type Output = ParseRule<'source>;

    fn index(&self, index: TokenType) -> &Self::Output {
        let index: u8 = index.into();
        &self.0[index as usize]
    }
}

fn to_bytes(short: u16) -> (u8, u8) {
    let byte1 = (short >> 8) & 0xff;
    let byte2 = short & 0xff;
    (byte1 as u8, byte2 as u8)
}

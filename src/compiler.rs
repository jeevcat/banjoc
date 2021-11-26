use std::{mem::MaybeUninit, ops::Index};

use strum::{EnumCount, IntoEnumIterator};

use crate::{
    chunk::Chunk,
    error::{LoxError, Result},
    op_code::OpCode,
    scanner::{Scanner, Token, TokenType},
    value::Value,
};

pub fn compile(source: &str) -> Result<Chunk> {
    let scanner = Scanner::new(source);

    let mut parser = Parser::new(scanner);
    parser.advance();
    parser.expression();
    parser.consume(TokenType::Eof, "Expect end of expression.");
    parser.end_compiler();
    match parser.had_error {
        true => Err(LoxError::CompileError),
        false => Ok(Chunk::new()),
    }
}

struct Parser<'source> {
    scanner: Scanner<'source>,
    current: Token<'source>,
    previous: Token<'source>,
    current_chunk: Chunk,
    rules: ParseRuleTable<'source>,
    had_error: bool,
    panic_mode: bool,
}

impl<'source> Parser<'source> {
    fn new(scanner: Scanner) -> Parser {
        let rules = ParseRuleTable::new();

        Parser {
            scanner,
            current: todo!(),
            previous: todo!(),
            current_chunk: Chunk::new(),
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

    fn expression(&mut self) {
        self.parse_precedence(Precedence::Assignment)
    }

    fn number(&mut self) {
        let value: f64 = self.previous.lexeme.parse().unwrap();
        self.emit_constant(value)
    }

    fn grouping(&mut self) {
        self.expression();
        self.consume(TokenType::RightParen, "Expect ')' after expression.");
    }

    fn unary(&mut self) {
        let operator_type = self.previous.token_type;

        // Compile the operand
        self.parse_precedence(Precedence::Unary);

        // Emit the operator instruction.
        match operator_type {
            TokenType::Minus => self.emit_opcode(OpCode::Negate),
            _ => unreachable!(),
        }
    }

    fn binary(&mut self) {
        // By the time we get here we've already compiled the left operand
        let operator_type = self.previous.token_type;
        let rule = self.get_rule(operator_type);

        // Compile the right operand
        // Each binary operator's right-hand operand precedence is one level higher than its own
        self.parse_precedence(rule.precedence.next());

        // Compile the operator
        match operator_type {
            TokenType::Plus => self.emit_opcode(OpCode::Add),
            TokenType::Minus => self.emit_opcode(OpCode::Subtract),
            TokenType::Star => self.emit_opcode(OpCode::Multiply),
            TokenType::Slash => self.emit_opcode(OpCode::Divide),
            _ => unreachable!(),
        }
    }

    /// Starts at the current token and parses any expression at the given precedence or higher
    fn parse_precedence(&mut self, precedence: Precedence) {
        self.advance();
        let prefix_rule = self.get_rule(self.previous.token_type).prefix;
        match prefix_rule {
            None => {
                self.error("Expect expression.");
                return;
            },
            Some(prefix_rule) => prefix_rule(self),
        }

        while precedence <= self.get_rule(self.current.token_type).precedence { 
            self.advance();
            // Can unwrap as 
            let infix_rule = self.get_rule(self.previous.token_type).infix.unwrap();
            infix_rule(self);
        }
    }

    fn get_rule(&self, token_type: TokenType) -> &ParseRule<'source> {
        &self.rules[token_type]
    }

    fn end_compiler(&mut self) {
        self.emit_return();
        #[cfg(feature = "debug_trace_execution")]
        {
            if !self.had_error {
                crate::disassembler::disassemble(&self.current_chunk, "code");
            }
        }
    }

    fn emit_byte(&mut self, byte: u8) {
        self.current_chunk.write(byte, self.previous.line)
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
        self.emit_opcode(OpCode::Return)
    }

    fn make_constant(&mut self, value: Value) -> u8 {
        let constant = self.current_chunk.add_constant(value);
        if constant > u8::MAX.into() {
            // TODO we'd want to add another instruction like OpCode::Constant16 which stores the index as a two-byte operand when this limit is hit
            self.error("To many constants in one chunk.");
            return 0;
        }
        constant.try_into().unwrap()
    }

    fn error_at_current(&mut self, message: &str) {
        self.error_at(self.current, message)
    }

    fn error(&mut self, message: &str) {
        self.error_at(self.previous, message);
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

type ParseFn<'source> = fn(&mut Parser<'source>) -> ();
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
        match token_type {
            TokenType::LeftParen =>    ParseRule::new(Some(Parser::grouping), None,                 Precedence::None),
            TokenType::RightParen =>   ParseRule::new(None,                   None,                 Precedence::None),
            TokenType::LeftBrace =>    ParseRule::new(None,                   None,                 Precedence::None),
            TokenType::RightBrace =>   ParseRule::new(None,                   None,                 Precedence::None),
            TokenType::Comma =>        ParseRule::new(None,                   None,                 Precedence::None),
            TokenType::Dot =>          ParseRule::new(None,                   None,                 Precedence::None),
            TokenType::Minus =>        ParseRule::new(Some(Parser::unary),    Some(Parser::binary), Precedence::Term),
            TokenType::Plus =>         ParseRule::new(None,                   Some(Parser::binary), Precedence::Term),
            TokenType::Semicolon =>    ParseRule::new(None,                   Some(Parser::binary), Precedence::Factor),
            TokenType::Slash =>        ParseRule::new(None,                   Some(Parser::binary), Precedence::Factor),
            TokenType::Star =>         ParseRule::new(None,                   None,                 Precedence::None),
            TokenType::Bang =>         ParseRule::new(None,                   None,                 Precedence::None),
            TokenType::BangEqual =>    ParseRule::new(None,                   None,                 Precedence::None),
            TokenType::Equal =>        ParseRule::new(None,                   None,                 Precedence::None),
            TokenType::EqualEqual =>   ParseRule::new(None,                   None,                 Precedence::None),
            TokenType::Greater =>      ParseRule::new(None,                   None,                 Precedence::None),
            TokenType::GreaterEqual => ParseRule::new(None,                   None,                 Precedence::None),
            TokenType::Less =>         ParseRule::new(None,                   None,                 Precedence::None),
            TokenType::LessEqual =>    ParseRule::new(None,                   None,                 Precedence::None),
            TokenType::Identifier =>   ParseRule::new(None,                   None,                 Precedence::None),
            TokenType::String =>       ParseRule::new(None,                   None,                 Precedence::None),
            TokenType::Number =>       ParseRule::new(Some(Parser::number),   None,                 Precedence::None),
            TokenType::And =>          ParseRule::new(None,                   None,                 Precedence::None),
            TokenType::Class =>        ParseRule::new(None,                   None,                 Precedence::None),
            TokenType::Else =>         ParseRule::new(None,                   None,                 Precedence::None),
            TokenType::False =>        ParseRule::new(None,                   None,                 Precedence::None),
            TokenType::For =>          ParseRule::new(None,                   None,                 Precedence::None),
            TokenType::Fun =>          ParseRule::new(None,                   None,                 Precedence::None),
            TokenType::If =>           ParseRule::new(None,                   None,                 Precedence::None),
            TokenType::Nil =>          ParseRule::new(None,                   None,                 Precedence::None),
            TokenType::Or =>           ParseRule::new(None,                   None,                 Precedence::None),
            TokenType::Print =>        ParseRule::new(None,                   None,                 Precedence::None),
            TokenType::Return =>       ParseRule::new(None,                   None,                 Precedence::None),
            TokenType::Super =>        ParseRule::new(None,                   None,                 Precedence::None),
            TokenType::This =>         ParseRule::new(None,                   None,                 Precedence::None),
            TokenType::True =>         ParseRule::new(None,                   None,                 Precedence::None),
            TokenType::Var =>          ParseRule::new(None,                   None,                 Precedence::None),
            TokenType::While =>        ParseRule::new(None,                   None,                 Precedence::None),
            TokenType::Error =>        ParseRule::new(None,                   None,                 Precedence::None),
            TokenType::Eof =>          ParseRule::new(None,                   None,                 Precedence::None),
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

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
        false => Ok(parser.current_chunk),
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
            current: Token::none(),
            previous: Token::none(),
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
        self.emit_constant(Value::Number(value))
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

        // Compile the right operand
        // Each binary operator's right-hand operand precedence is one level higher than its own
        self.parse_precedence(self.get_rule(operator_type).precedence.next());

        // Compile the operator
        match operator_type {
            TokenType::Plus => self.emit_opcode(OpCode::Add),
            TokenType::Minus => self.emit_opcode(OpCode::Subtract),
            TokenType::Star => self.emit_opcode(OpCode::Multiply),
            TokenType::Slash => self.emit_opcode(OpCode::Divide),
            _ => unreachable!(),
        }
    }
    fn literal(&mut self) {
        match self.previous.token_type {
            TokenType::False => self.emit_opcode(OpCode::False),
            TokenType::Nil => self.emit_opcode(OpCode::Nil),
            TokenType::True => self.emit_opcode(OpCode::True),
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
            }
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
        self.emit_opcode(OpCode::Return);
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

type ParseFn<'a> = fn(&mut Parser<'a>) -> ();
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
            LeftParen =>    ParseRule::new(Some(Parser::grouping), None,                 P::None),
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
            Bang =>         ParseRule::new(None,                   None,                 P::None),
            BangEqual =>    ParseRule::new(None,                   None,                 P::None),
            Equal =>        ParseRule::new(None,                   None,                 P::None),
            EqualEqual =>   ParseRule::new(None,                   None,                 P::None),
            Greater =>      ParseRule::new(None,                   None,                 P::None),
            GreaterEqual => ParseRule::new(None,                   None,                 P::None),
            Less =>         ParseRule::new(None,                   None,                 P::None),
            LessEqual =>    ParseRule::new(None,                   None,                 P::None),
            Identifier =>   ParseRule::new(None,                   None,                 P::None),
            String =>       ParseRule::new(None,                   None,                 P::None),
            Number =>       ParseRule::new(Some(Parser::number),   None,                 P::None),
            And =>          ParseRule::new(None,                   None,                 P::None),
            Class =>        ParseRule::new(None,                   None,                 P::None),
            Else =>         ParseRule::new(None,                   None,                 P::None),
            False =>        ParseRule::new(Some(Parser::literal),  None,                 P::None),
            For =>          ParseRule::new(None,                   None,                 P::None),
            Fun =>          ParseRule::new(None,                   None,                 P::None),
            If =>           ParseRule::new(None,                   None,                 P::None),
            Nil =>          ParseRule::new(Some(Parser::literal),  None,                 P::None),
            Or =>           ParseRule::new(None,                   None,                 P::None),
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

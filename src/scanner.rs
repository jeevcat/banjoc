use num_enum::IntoPrimitive;
use strum::{EnumCount, EnumIter};

pub struct Scanner<'source> {
    source: &'source str,
    start: usize,
    current: usize,
    line: u32,
}

impl<'source> Scanner<'source> {
    pub fn new(source: &'source str) -> Self {
        Self {
            source,
            start: 0,
            current: 0,
            line: 1,
        }
    }

    pub fn scan_token(&mut self) -> Token<'source> {
        self.skip_whitespace();
        self.start = self.current;

        if self.is_at_end() {
            return self.make_token(TokenType::Eof);
        }

        match self.advance() {
            b'(' => self.make_token(TokenType::LeftParen),
            b')' => self.make_token(TokenType::RightParen),
            b'{' => self.make_token(TokenType::LeftBrace),
            b'}' => self.make_token(TokenType::RightBrace),
            b'[' => self.make_token(TokenType::LeftBracket),
            b']' => self.make_token(TokenType::RightBracket),
            b';' => self.make_token(TokenType::Semicolon),
            b',' => self.make_token(TokenType::Comma),
            b'.' => self.make_token(TokenType::Dot),
            b'-' if self.match_advance(b'>') => self.make_token(TokenType::Arrow),
            b'-' => self.make_token(TokenType::Minus),
            b'+' => self.make_token(TokenType::Plus),
            b'/' => self.make_token(TokenType::Slash),
            b'*' => self.make_token(TokenType::Star),
            b'!' if self.match_advance(b'=') => self.make_token(TokenType::BangEqual),
            b'!' => self.make_token(TokenType::Bang),
            b'=' if self.match_advance(b'=') => self.make_token(TokenType::EqualEqual),
            b'=' => self.make_token(TokenType::Equal),
            b'<' if self.match_advance(b'=') => self.make_token(TokenType::LessEqual),
            b'<' => self.make_token(TokenType::Less),
            b'>' if self.match_advance(b'=') => self.make_token(TokenType::GreaterEqual),
            b'>' => self.make_token(TokenType::Greater),
            b'"' => self.string(),
            c if c.is_ascii_digit() => self.number(),
            c if c.is_ascii_alphabetic() || c == b'_' => self.identifier(),
            _ => self.error_token("Unexpected character."),
        }
    }

    fn match_advance(&mut self, expected: u8) -> bool {
        if self.is_at_end() {
            return false;
        }
        if self.peek() != expected {
            return false;
        }
        self.current += 1;
        true
    }

    fn advance(&mut self) -> u8 {
        self.current += 1;
        self.source.as_bytes()[self.current - 1]
    }

    fn skip_whitespace(&mut self) {
        while !self.is_at_end() {
            let c = self.peek();
            match c {
                // Same line whitespace
                b' ' | b'\r' | b'\t' => {
                    self.advance();
                }
                // Newlines
                b'\n' => {
                    self.line += 1;
                    self.advance();
                }
                // Comments
                b'/' => {
                    if self.peek_next() == b'/' {
                        // A comment goes until the end of the line
                        while !self.is_at_end() && self.peek() != b'\n' {
                            self.advance();
                        }
                    } else {
                        // This slash is actually a token
                        return;
                    }
                }
                _ => {
                    return;
                }
            }
        }
    }

    fn string(&mut self) -> Token<'source> {
        while !self.is_at_end() && self.peek() != b'"' {
            if self.peek() == b'\n' {
                self.line += 1;
            }
            self.advance();
        }

        if self.is_at_end() {
            return self.error_token("Unterminated string.");
        }

        // The closing quote
        self.advance();
        self.make_token(TokenType::String)
    }

    fn number(&mut self) -> Token<'source> {
        while !self.is_at_end() && self.peek().is_ascii_digit() {
            self.advance();
        }

        // Look for a fractional part
        if !self.is_at_end() && self.peek() == b'.' && self.peek_next().is_ascii_digit() {
            // Consume the '.'
            self.advance();

            // Consume the rest of the numbers
            while self.peek().is_ascii_digit() {
                self.advance();
            }
        }

        self.make_token(TokenType::Number)
    }

    fn identifier(&mut self) -> Token<'source> {
        while self.peek().is_ascii_alphanumeric() || self.peek() == b'_' {
            self.advance();
        }
        self.make_token(self.identifier_type())
    }

    fn char_n(&self, n: usize) -> u8 {
        self.source.as_bytes()[self.start + n]
    }

    fn len(&self) -> usize {
        self.current - self.start
    }

    fn identifier_type(&self) -> TokenType {
        match self.char_n(0) {
            b'a' => self.check_keyword(1, "nd", TokenType::And),
            b'c' => self.check_keyword(1, "all", TokenType::Call),
            b'd' => self.check_keyword(1, "igraph", TokenType::Digraph),
            b'e' => self.check_keyword(1, "lse", TokenType::Else),
            b'i' => self.check_keyword(1, "f", TokenType::If),
            b'n' => self.check_keyword(1, "il", TokenType::Nil),
            b'o' => self.check_keyword(1, "r", TokenType::Or),
            b'p' => self.check_keyword(1, "aram", TokenType::Param),
            b't' => self.check_keyword(1, "rue", TokenType::True),
            b'v' => self.check_keyword(1, "ar", TokenType::Var),
            b'f' if self.len() > 1 => match self.char_n(1) {
                b'n' => self.check_keyword(2, "", TokenType::Fn),
                b'a' => self.check_keyword(2, "lse", TokenType::False),
                _ => TokenType::Identifier,
            },
            b'r' if self.char_n(1) == b'e' && self.len() > 2 => match self.char_n(2) {
                b'f' => self.check_keyword(3, "", TokenType::Ref),
                b't' => self.check_keyword(3, "urn", TokenType::Return),
                _ => TokenType::Identifier,
            },
            _ => TokenType::Identifier,
        }
    }

    fn check_keyword(&self, start: usize, rest: &str, token_type: TokenType) -> TokenType {
        // Same length
        if self.len() == start + rest.len() {
            let start_index = self.start + start;
            let end_index = start_index + rest.len();
            // Same bytes
            if &self.source.as_bytes()[start_index..end_index] == rest.as_bytes() {
                return token_type;
            }
        }
        TokenType::Identifier
    }

    fn peek(&self) -> u8 {
        self.source.as_bytes()[self.current]
    }

    fn peek_next(&self) -> u8 {
        if self.current + 1 >= self.source.len() {
            b'\0'
        } else {
            self.source.as_bytes()[self.current + 1]
        }
    }

    fn is_at_end(&self) -> bool {
        self.current == self.source.len()
    }

    /// Use start+current pointers in source to create a token
    fn make_token(&self, token_type: TokenType) -> Token<'source> {
        Token {
            token_type,
            lexeme: &self.source[self.start..self.current],
            line: self.line,
        }
    }

    fn error_token(&self, message: &'static str) -> Token<'source> {
        Token {
            token_type: TokenType::Error,
            lexeme: message,
            line: self.line,
        }
    }
}

// Tokens are pretty small, so we'll pass them around by value
#[derive(Clone, Copy, Debug)]
pub struct Token<'source> {
    pub token_type: TokenType,
    pub lexeme: &'source str,
    pub line: u32,
}

impl<'source> Token<'source> {
    pub const fn none() -> Token<'source> {
        Token {
            token_type: TokenType::Error,
            lexeme: "",
            line: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, IntoPrimitive, EnumIter, EnumCount)]
#[repr(u8)]
pub enum TokenType {
    LeftParen,
    RightParen,
    LeftBrace,
    RightBrace,
    LeftBracket,
    RightBracket,
    Comma,
    Dot,
    Minus,
    Plus,
    Semicolon,
    Slash,
    Star,

    // One or two character tokens.
    Bang,
    BangEqual,
    Equal,
    EqualEqual,
    Greater,
    GreaterEqual,
    Less,
    LessEqual,

    // Literals.
    Identifier,
    String,
    Number,

    // Keywords.
    If,
    Else,
    And,
    Or,
    False,
    True,
    Nil,

    // Define function
    Fn,
    // Call function
    Call,
    // Define variable
    Var,
    // Reference variable
    Ref,
    Param,
    Return,
    Digraph,
    Arrow,

    Error,
    Eof,
}

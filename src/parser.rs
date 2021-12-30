use std::collections::HashMap;

use crate::{
    error::{LoxError, Result},
    gc::Gc,
    scanner::{Scanner, Token, TokenType},
    value::Value,
};

pub struct Graph<'source> {
    roots: Vec<Node<'source>>,
}

pub struct Node<'source> {
    id: Token<'source>,
    node_type: NodeType<'source>,
}

pub enum NodeType<'source> {
    Literal { value: Value },
    NativeFunction { arguments: Vec<Node<'source>> },
}

pub struct Parser<'source> {
    scanner: Scanner<'source>,
    current: Token<'source>,
    previous: Token<'source>,
    all_nodes: HashMap<&'source str, Node<'source>>,
    gc: &'source mut Gc,
    had_error: bool,
    panic_mode: bool,
}

impl<'source> Parser<'source> {
    pub fn new(source: &'source str, gc: &'source mut Gc) -> Self {
        Self {
            scanner: Scanner::new(source),
            current: Token::none(),
            previous: Token::none(),
            all_nodes: HashMap::new(),
            gc,
            had_error: false,
            panic_mode: false,
        }
    }

    pub fn parse(&mut self) -> Result<Graph> {
        self.advance();
        self.digraph();
        while !self.advance_matching(TokenType::Eof) {
            // Skip rest of file
        }
        Ok(Graph { roots: todo!() })
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

    fn digraph(&mut self) {
        if self.advance_matching(TokenType::Digraph) {
            // Graph names are allowed, but ignored
            if self.check(TokenType::Identifier) {
                self.advance();
            }
            self.consume(TokenType::LeftBrace, "Expect '{' before digraph body.");
            self.block();
        } else {
            self.error_str("Expect 'digraph'");
        }
    }

    fn block(&mut self) {
        while !self.check(TokenType::RightBrace) && !self.check(TokenType::Eof) {
            self.declaration();
        }

        self.consume(TokenType::RightBrace, "Expect '}' after block.");
    }

    fn declaration(&mut self) {
        let node_id = self.current;
        self.advance();

        // Only edge and node statements supported from dot spec
        if self.advance_matching(TokenType::Arrow) {
            self.edge_statement(node_id)
        } else {
            self.node_statement()
        }

        if self.panic_mode {
            self.synchronize();
        }
    }

    fn edge_statement(&mut self, node_id: Token) {
        dbg!(node_id.lexeme);
        self.advance();
    }

    fn node_statement(&mut self) {
        dbg!(self.previous.lexeme);
        if let Some(mut node) = self.all_nodes.get_mut(self.previous.lexeme) {
        } else {
            let node_type = match self.previous.token_type {
                TokenType::Number => NodeType::Literal {
                    value: self.number(),
                },
                TokenType::String => NodeType::Literal {
                    value: self.string(),
                },
                TokenType::Identifier => NodeType::NativeFunction { arguments: vec![] },
                _ => return self.error_at_current("Unrecognized node statement."),
            };
            let node = Node {
                id: self.previous,
                node_type,
            };
            self.all_nodes.insert(self.previous.lexeme, node);
        }
        self.attribute_list();
    }

    fn number(&mut self) -> Value {
        let value: f64 = self.previous.lexeme.parse().unwrap();
        Value::Number(value)
    }

    fn string(&mut self) -> Value {
        let string = &self.previous.lexeme[1..self.previous.lexeme.len() - 1];
        Value::String(self.gc.intern(string))
    }

    fn attribute_list(&mut self) {}

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
        if self.panic_mode {
            return;
        }
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

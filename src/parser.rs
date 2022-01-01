use std::collections::HashMap;

use crate::{
    error::{LoxError, Result},
    gc::Gc,
    scanner::{Scanner, Token, TokenType},
    value::Value,
};

pub struct Graph<'source> {
    all_nodes: HashMap<NodeId<'source>, Node<'source>>,
}

impl<'source> Graph<'source> {
    pub fn new() -> Self {
        Self {
            all_nodes: HashMap::new(),
        }
    }

    pub fn get_node(&self, node_id: NodeId) -> Option<&Node> {
        self.all_nodes.get(node_id)
    }

    pub fn get_return_node(&self) -> &Node {
        self.get_node("return").unwrap()
    }
}

type NodeId<'source> = &'source str;

#[derive(Debug)]
pub struct Node<'source> {
    node_id: Token<'source>,
    node_type: NodeType<'source>,
}

#[derive(Debug)]
pub enum NodeType<'source> {
    Literal { value: Value },
    Symbol { symbol_type: SymbolType<'source> },
    Return { argument: NodeId<'source> },
    Error,
}

impl<'source> NodeType<'source> {
    fn add_input(&mut self, input: NodeId<'source>) {
        // TODO errors
        match self {
            NodeType::Symbol { symbol_type } => match symbol_type {
                SymbolType::NativeFunction { arguments } => arguments.push(input),
                SymbolType::Variable => {
                    // An input means we now know this symbol is callable
                    *symbol_type = SymbolType::NativeFunction {
                        arguments: vec![input],
                    }
                }
            },
            NodeType::Return { argument } => *argument = input,
            _ => {}
        }
    }
}

#[derive(Debug)]
pub enum SymbolType<'source> {
    Variable,
    NativeFunction { arguments: Vec<NodeId<'source>> },
}

pub struct Parser<'source> {
    scanner: Scanner<'source>,
    current: Token<'source>,
    previous: Token<'source>,
    graph: Graph<'source>,
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
            graph: Graph::new(),
            gc,
            had_error: false,
            panic_mode: false,
        }
    }

    pub fn parse(mut self) -> Result<Graph<'source>> {
        self.advance();
        self.digraph();
        while !self.advance_matching(TokenType::Eof) {
            // Skip rest of file
        }
        Ok(self.graph)
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
    }

    fn edge_statement(&mut self, node_id: Token<'source>) {
        self.ensure_node(node_id);
        let target_token = self.current;
        let target = self.ensure_node(target_token);
        target.node_type.add_input(node_id.lexeme);
        self.advance();
        if self.advance_matching(TokenType::Arrow) {
            self.edge_statement(target_token);
        }
    }

    fn node_statement(&mut self) {
        self.ensure_node(self.previous);
        self.attribute_list();
    }

    fn ensure_node(&mut self, node_id: Token<'source>) -> &mut Node<'source> {
        // TODO why doesn't the borrow checker let me skip the extra get_mut
        if self.graph.all_nodes.contains_key(node_id.lexeme) {
            return self.graph.all_nodes.get_mut(node_id.lexeme).unwrap();
        }
        let node_type = match node_id.token_type {
            TokenType::Number => NodeType::Literal {
                value: number(node_id),
            },
            TokenType::String => NodeType::Literal {
                value: self.string(node_id),
            },
            TokenType::Identifier => NodeType::Symbol {
                symbol_type: SymbolType::Variable,
            },
            TokenType::Return => NodeType::Return { argument: "" },
            _ => {
                self.error_at(node_id, "Unrecognized node statement.");
                NodeType::Error
            }
        };
        let node = Node { node_id, node_type };
        self.graph.all_nodes.insert(node_id.lexeme, node);
        self.graph.all_nodes.get_mut(node_id.lexeme).unwrap()
    }

    fn string(&mut self, token: Token) -> Value {
        let string = &token.lexeme[1..token.lexeme.len() - 1];
        Value::String(self.gc.intern(string))
    }

    fn attribute_list(&mut self) {}

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

fn number(token: Token) -> Value {
    let value: f64 = token.lexeme.parse().unwrap();
    Value::Number(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edges() {
        let source = "digraph { 10 -> b -> return }";
        let mut gc = Gc::new();
        let parser = Parser::new(source, &mut gc);
        let graph = parser.parse().unwrap();
        let return_node = graph.get_return_node();
        match return_node.node_type {
            NodeType::Return { argument } => {
                let b = graph.get_node(argument).unwrap();
                match &b.node_type {
                    NodeType::Symbol {
                        symbol_type: SymbolType::NativeFunction { arguments },
                    } => {
                        let literal = graph.get_node(arguments[0]).unwrap();
                        match literal.node_type {
                            NodeType::Literal {
                                value: Value::Number(number),
                            } => assert!(f64::abs(10. - number) < 0.00001),
                            _ => panic!(),
                        }
                    }
                    _ => panic!(),
                }
            }
            _ => panic!(),
        }
    }

    #[test]
    fn nodes() {
        let source = "digraph { a b c }";
        let mut gc = Gc::new();
        let parser = Parser::new(source, &mut gc);
        let graph = parser.parse().unwrap();
        for node_id in ["a", "b", "c"] {
            let node = graph.get_node(node_id).unwrap();
            assert_eq!(node_id, node.node_id.lexeme);
            match node.node_type {
                NodeType::Symbol {
                    symbol_type: SymbolType::Variable,
                } => {}
                _ => panic!(),
            }
        }
    }
}

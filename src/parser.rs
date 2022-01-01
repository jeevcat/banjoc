use std::collections::HashMap;

use crate::{
    error::{LoxError, Result},
    scanner::{Scanner, Token, TokenType},
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

    fn ensure_node(
        &mut self,
        node_id: Token<'source>,
        attributes: Option<Attributes<'source>>,
    ) -> &mut Node<'source> {
        // TODO why doesn't the borrow checker let me skip the extra get_mut
        if self.all_nodes.contains_key(node_id.lexeme) {
            let node = self.all_nodes.get_mut(node_id.lexeme).unwrap();
            if let Some(attributes) = attributes {
                node.attributes.merge(attributes);
            }
            return node;
        }

        let node_type = match node_id.token_type {
            TokenType::Number => NodeType::Literal,
            TokenType::String => NodeType::Literal,
            TokenType::Identifier => NodeType::Symbol {
                symbol_type: SymbolType::Variable,
            },
            TokenType::Return => NodeType::Return { argument: "" },
            _ => NodeType::Error,
        };
        let node = Node {
            node_id,
            node_type,
            attributes: attributes.unwrap_or_default(),
        };
        self.all_nodes.insert(node_id.lexeme, node);
        self.all_nodes.get_mut(node_id.lexeme).unwrap()
    }
}

type NodeId<'source> = &'source str;

#[derive(Debug)]
pub struct Node<'source> {
    node_id: Token<'source>,
    node_type: NodeType<'source>,
    attributes: Attributes<'source>,
}

#[derive(Debug)]
pub enum NodeType<'source> {
    Literal,
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

#[derive(Debug, Default)]
struct Attributes<'source> {
    comment: Option<Token<'source>>,
    pos: Option<Token<'source>>,
    label: Option<Token<'source>>,
}

impl<'source> Attributes<'source> {
    fn merge(&mut self, other: Self) {
        if other.comment.is_some() {
            self.comment = other.comment;
        }
        if other.pos.is_some() {
            self.pos = other.pos;
        }
        if other.label.is_some() {
            self.label = other.label;
        }
    }
}

struct Tokens<'source> {
    scanner: Scanner<'source>,
    current: Token<'source>,
    previous: Token<'source>,
    had_error: bool,
    panic_mode: bool,
}

impl<'source> Tokens<'source> {
    fn new(source: &'source str) -> Self {
        Self {
            scanner: Scanner::new(source),
            current: Token::none(),
            previous: Token::none(),
            had_error: false,
            panic_mode: false,
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
        if self.check(token_type) {
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

pub struct Parser<'source> {
    tokens: Tokens<'source>,
    graph: Graph<'source>,
}

impl<'source> Parser<'source> {
    pub fn new(source: &'source str) -> Self {
        Self {
            tokens: Tokens::new(source),
            graph: Graph::new(),
        }
    }

    pub fn parse(mut self) -> Result<Graph<'source>> {
        self.tokens.advance();
        self.digraph();
        while !self.tokens.advance_matching(TokenType::Eof) {
            // Skip rest of file
        }
        Ok(self.graph)
    }

    fn digraph(&mut self) {
        if self.tokens.advance_matching(TokenType::Digraph) {
            // Graph names are allowed, but ignored
            if self.tokens.check(TokenType::Identifier) {
                self.tokens.advance();
            }
            self.tokens
                .consume(TokenType::LeftBrace, "Expect '{' before digraph body.");
            self.block();
        } else {
            self.tokens.error_str("Expect 'digraph'");
        }
    }

    fn block(&mut self) {
        while !self.tokens.check(TokenType::RightBrace) && !self.tokens.check(TokenType::Eof) {
            self.declaration();
        }

        self.tokens
            .consume(TokenType::RightBrace, "Expect '}' after block.");
    }

    fn declaration(&mut self) {
        let node_id = self.tokens.current;
        self.tokens.advance();

        // Only edge and node statements supported from dot spec
        if self.tokens.advance_matching(TokenType::Arrow) {
            self.edge_statement(node_id)
        } else {
            self.node_statement()
        }
    }

    fn edge_statement(&mut self, node_id: Token<'source>) {
        self.graph.ensure_node(node_id, None);
        let target_token = self.tokens.current;
        let target = self.graph.ensure_node(target_token, None);
        target.node_type.add_input(node_id.lexeme);
        self.tokens.advance();
        if self.tokens.advance_matching(TokenType::Arrow) {
            self.edge_statement(target_token);
        }
    }

    fn node_statement(&mut self) {
        let node_id = self.tokens.previous;
        let attributes = Self::attribute_list(&mut self.tokens);
        self.graph.ensure_node(node_id, attributes);
    }

    fn attribute_list(tokens: &mut Tokens<'source>) -> Option<Attributes<'source>> {
        if tokens.advance_matching(TokenType::LeftBracket) {
            let mut attributes = Attributes {
                comment: None,
                pos: None,
                label: None,
            };

            if !tokens.check(TokenType::RightBracket) {
                loop {
                    tokens.consume(
                        TokenType::Identifier,
                        "Expected attribute name in attribute list.",
                    );
                    let name = tokens.previous;
                    tokens.consume(TokenType::Equal, "Expected '=' after attribute name.");
                    match tokens.current.token_type {
                        TokenType::Number | TokenType::String => match name.lexeme {
                            "comment" => attributes.comment = Some(tokens.current),
                            "pos" => attributes.pos = Some(tokens.current),
                            "label" => attributes.label = Some(tokens.current),
                            _ => tokens
                                .error_str(&format!("Unexpected attribute name {}", name.lexeme)),
                        },
                        _ => tokens.error_str("Expected attribute value in attribute list."),
                    }
                    tokens.advance();

                    if !tokens.advance_matching(TokenType::Comma) {
                        break;
                    }
                }
            }
            tokens.consume(
                TokenType::RightBracket,
                "Expected ']' after attribute list.",
            );
            return Some(attributes);
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edges() {
        let source = "digraph { 10 -> b -> return }";
        let parser = Parser::new(source);
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
                        assert_eq!(literal.node_id.lexeme, "10");
                        match literal.node_type {
                            NodeType::Literal => {}
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
        let parser = Parser::new(source);
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

    #[test]
    fn node_attr() {
        let source = "digraph { a [comment=\"hi\"] }";
        let parser = Parser::new(source);
        let graph = parser.parse().unwrap();
        let node = graph.get_node("a").unwrap();
        assert_eq!(node.node_id.lexeme, "a");
        assert!(matches!(node.node_type, NodeType::Symbol {symbol_type: SymbolType::Variable}));
        assert_eq!(node.attributes.comment.unwrap().lexeme, "\"hi\"");
    }

    #[test]
    fn node_attribs() {
        let source = "digraph { b [pos=\"1,2\"]; a -> b; a [label=2.5] }";
        let parser = Parser::new(source);
        let graph = parser.parse().unwrap();
        let a = graph.get_node("a").unwrap();
        assert_eq!(a.node_id.lexeme, "a");
        assert!(matches!(a.node_type, NodeType::Symbol {symbol_type: SymbolType::Variable}));
        assert_eq!(a.attributes.label.unwrap().lexeme, "2.5");
        let b = graph.get_node("b").unwrap();
        assert_eq!(b.node_id.lexeme, "b");
        match &b.node_type {
            NodeType::Symbol {symbol_type: SymbolType::NativeFunction { arguments }} => assert_eq!(arguments[0], "a"),
            _ => panic!()
        };
        assert_eq!(b.attributes.pos.unwrap().lexeme, "\"1,2\"");
    }
}

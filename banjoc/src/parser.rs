use std::collections::HashMap;

use crate::{
    error::{BanjoError, Result},
    scanner::{Scanner, Token, TokenType},
};

pub struct Ast<'source> {
    pub all_nodes: HashMap<NodeId<'source>, Node<'source>>,
    pub return_node: Option<NodeId<'source>>,
}

impl<'source> Ast<'source> {
    pub fn new() -> Self {
        Self {
            all_nodes: HashMap::new(),
            return_node: None,
        }
    }

    pub fn get_node(&self, node_id: NodeId) -> Option<&Node> {
        self.all_nodes.get(node_id)
    }

    pub fn get_return_node(&self) -> Option<&Node> {
        self.get_node(self.return_node?)
    }

    pub fn get_definitions(&self) -> impl Iterator<Item = &Node> {
        // TODO perf
        self.all_nodes.iter().map(|(_, node)| node).filter(|node| {
            matches!(
                node.node_type,
                NodeType::VariableDefinition { .. } | NodeType::FunctionDefinition { .. }
            )
        })
    }

    pub fn insert_node(&mut self, node: Node<'source>) -> &mut Node<'source> {
        if let NodeType::Return { .. } = node.node_type {
            self.return_node = Some(node.node_id.lexeme)
        }

        self.all_nodes.entry(node.node_id.lexeme).or_insert(node)
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

                if let Some(type_attr) = node.attributes.node_type {
                    let (needs_convert, body) = match node.node_type {
                        NodeType::VariableReference => (true, None),
                        NodeType::VariableDefinition { body } => (true, body),
                        _ => (false, None),
                    };
                    if needs_convert {
                        match type_attr.token_type {
                            TokenType::Not | TokenType::Negate => {
                                node.node_type = NodeType::Unary { argument: body }
                            }
                            TokenType::Subtract
                            | TokenType::Divide
                            | TokenType::Equals
                            | TokenType::Greater
                            | TokenType::GreaterEqual
                            | TokenType::Less
                            | TokenType::LessEqual => {
                                node.node_type = NodeType::Binary {
                                    term_a: body,
                                    term_b: None,
                                }
                            }
                            TokenType::Fn => {
                                node.node_type = NodeType::FunctionDefinition { body, arity: 1 }
                            }
                            TokenType::Call => {
                                node.node_type = NodeType::FunctionCall {
                                    arguments: body.into_iter().collect(),
                                }
                            }
                            TokenType::Ref => node.node_type = NodeType::VariableReference,
                            TokenType::Param => node.node_type = NodeType::Param,
                            TokenType::Return => {
                                node.node_type = NodeType::Return { argument: body }
                            }
                            TokenType::Literal => node.node_type = NodeType::Literal,
                            _ => {}
                        }
                    }
                }
            }
            node
        } else {
            self.insert_node(Node {
                node_id,
                node_type: NodeType::new(node_id, attributes.as_ref()),
                attributes: attributes.unwrap_or_default(),
            })
        }
    }
}

impl<'source> Default for Ast<'source> {
    fn default() -> Self {
        Self::new()
    }
}

pub type NodeId<'source> = &'source str;

#[derive(Debug)]
pub struct Node<'source> {
    pub node_id: Token<'source>,
    pub node_type: NodeType<'source>,
    attributes: Attributes<'source>,
}

impl<'source> Node<'source> {
    /// The label if available, otherwise the node_id
    pub fn get_name(&self) -> Token {
        self.attributes.label.unwrap_or(self.node_id)
    }
}

#[derive(Debug)]
pub enum NodeType<'source> {
    Literal,
    VariableDefinition {
        body: Option<NodeId<'source>>,
    },
    FunctionDefinition {
        body: Option<NodeId<'source>>,
        arity: u8,
    },
    Param,
    VariableReference,
    /// A reference to a function
    FunctionCall {
        arguments: Vec<NodeId<'source>>,
    },
    Return {
        argument: Option<NodeId<'source>>,
    },
    Unary {
        argument: Option<NodeId<'source>>,
    },
    Binary {
        term_a: Option<NodeId<'source>>,
        term_b: Option<NodeId<'source>>,
    },
}

impl<'source> NodeType<'source> {
    fn unary() -> Self {
        NodeType::Unary { argument: None }
    }
    fn binary() -> Self {
        NodeType::Binary {
            term_a: None,
            term_b: None,
        }
    }
}

impl<'source> NodeType<'source> {
    fn new(node_id: Token<'source>, attributes: Option<&Attributes<'source>>) -> NodeType<'source> {
        if let Some(node) = Self::from_type_attribute(attributes) {
            return node;
        }

        if let Some(node) = Self::from_name(node_id, attributes) {
            return node;
        }
        NodeType::VariableDefinition { body: None }
    }

    fn from_type_attribute<'a>(attributes: Option<&Attributes<'a>>) -> Option<NodeType<'a>> {
        Some(match attributes?.node_type?.token_type {
            TokenType::Not => NodeType::unary(),
            TokenType::Negate => NodeType::unary(),
            TokenType::Subtract => NodeType::binary(),
            TokenType::Divide => NodeType::binary(),
            TokenType::Equals => NodeType::binary(),
            TokenType::Greater => NodeType::binary(),
            TokenType::GreaterEqual => NodeType::binary(),
            TokenType::Less => NodeType::binary(),
            TokenType::LessEqual => NodeType::binary(),
            TokenType::Literal => NodeType::Literal,
            TokenType::Fn => NodeType::FunctionDefinition {
                body: None,
                arity: 0,
            },
            TokenType::Call => NodeType::FunctionCall { arguments: vec![] },
            TokenType::Var => NodeType::VariableDefinition { body: None },
            TokenType::Ref => NodeType::VariableReference,
            TokenType::Param => NodeType::Param,
            TokenType::Return => NodeType::Return { argument: None },
            _ => return None,
        })
    }

    /// Deduce the node type using the node_id or label
    fn from_name<'a>(
        token: Token<'a>,
        attributes: Option<&Attributes<'a>>,
    ) -> Option<NodeType<'a>> {
        match token.token_type {
            TokenType::Not => Some(NodeType::unary()),
            TokenType::Negate => Some(NodeType::unary()),
            TokenType::Subtract => Some(NodeType::binary()),
            TokenType::Divide => Some(NodeType::binary()),
            TokenType::Equals => Some(NodeType::binary()),
            TokenType::Greater => Some(NodeType::binary()),
            TokenType::GreaterEqual => Some(NodeType::binary()),
            TokenType::Less => Some(NodeType::binary()),
            TokenType::LessEqual => Some(NodeType::binary()),
            TokenType::Number
            | TokenType::String
            | TokenType::Nil
            | TokenType::True
            | TokenType::False => Some(NodeType::Literal),
            TokenType::Identifier => Self::from_name(attributes?.label?, None), /* try again with label */
            TokenType::Return => Some(NodeType::Return { argument: None }),
            _ => None,
        }
    }

    fn set_argument(argument: &mut Option<NodeId<'source>>, input: NodeId<'source>) -> Result<()> {
        match argument {
            Some(_) => return BanjoError::compile_err("Node can only have 1 input."),
            None => *argument = Some(input),
        }
        Ok(())
    }

    fn add_input(&mut self, input: NodeId<'source>) -> Result<()> {
        match self {
            NodeType::VariableReference => {
                *self = NodeType::FunctionCall {
                    arguments: vec![input],
                }
            }
            NodeType::VariableDefinition { body, .. } => Self::set_argument(body, input)?,
            NodeType::FunctionCall { arguments } => arguments.push(input),
            NodeType::FunctionDefinition { body, .. } => Self::set_argument(body, input)?,
            NodeType::Return { argument } => Self::set_argument(argument, input)?,
            NodeType::Literal => return BanjoError::compile_err("A literal cannot have an input."),
            NodeType::Param => return BanjoError::compile_err("A parameter cannot have an input."),
            NodeType::Unary { argument } => Self::set_argument(argument, input)?,
            NodeType::Binary { term_a, term_b } => match term_a {
                Some(_) => match term_b {
                    Some(_) => return BanjoError::compile_err("Node can only have 2 inputs."),
                    None => *term_b = Some(input),
                },
                None => *term_a = Some(input),
            },
        };
        Ok(())
    }

    fn add_output(&mut self) -> Result<()> {
        match self {
            NodeType::VariableDefinition { body, .. } => {
                *self = match body {
                    Some(body) => NodeType::FunctionCall {
                        arguments: vec![body],
                    },
                    None => NodeType::VariableReference,
                }
            }
            NodeType::Return { .. } => {
                return BanjoError::compile_err("A return cannot have an output.")
            }
            _ => {}
        }
        Ok(())
    }
}

#[derive(Debug, Default)]
struct Attributes<'source> {
    comment: Option<Token<'source>>,
    pos: Option<Token<'source>>,
    label: Option<Token<'source>>,
    node_type: Option<Token<'source>>,
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
        if other.node_type.is_some() {
            self.node_type = other.node_type;
        }
    }
}

struct Tokens<'source> {
    scanner: Scanner<'source>,
    current: Token<'source>,
    previous: Token<'source>,
    had_error: bool,
}

impl<'source> Tokens<'source> {
    fn new(source: &'source str) -> Self {
        Self {
            scanner: Scanner::new(source),
            current: Token::none(),
            previous: Token::none(),
            had_error: false,
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

    fn error(&mut self, error: BanjoError) {
        if let BanjoError::CompileError(message) = error {
            self.error_at(self.previous, &message)
        }
    }

    fn error_at(&mut self, token: Token, message: &str) {
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
    graph: Ast<'source>,
}

impl<'source> Parser<'source> {
    pub fn new(source: &'source str) -> Self {
        Self {
            tokens: Tokens::new(source),
            graph: Ast::new(),
        }
    }

    pub fn parse(mut self) -> Result<Ast<'source>> {
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
            self.declaration().unwrap_or_else(|e| self.tokens.error(e));
        }

        self.tokens
            .consume(TokenType::RightBrace, "Expect '}' after block.");
    }

    fn declaration(&mut self) -> Result<()> {
        let node_id = self.tokens.current;
        self.tokens.advance();

        // Only edge and node statements supported from dot spec
        if self.tokens.advance_matching(TokenType::Arrow) {
            self.edge_statement(node_id)?
        } else {
            self.node_statement()
        }
        Ok(())
    }

    fn edge_statement(&mut self, node_id: Token<'source>) -> Result<()> {
        let source = self.graph.ensure_node(node_id, None);
        source.node_type.add_output()?;

        let target_token = self.tokens.current;
        let target = self.graph.ensure_node(target_token, None);
        target.node_type.add_input(node_id.lexeme)?;

        self.tokens.advance();
        if self.tokens.advance_matching(TokenType::Arrow) {
            self.edge_statement(target_token)?;
        }
        Ok(())
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
                node_type: None,
            };

            if !tokens.check(TokenType::RightBracket) {
                loop {
                    tokens.consume(
                        TokenType::Identifier,
                        "Expected attribute name in attribute list.",
                    );
                    let name = tokens.previous;
                    tokens.consume(TokenType::Equal, "Expected '=' after attribute name.");
                    match name.lexeme {
                        "comment" => attributes.comment = Some(tokens.current),
                        "pos" => attributes.pos = Some(tokens.current),
                        "label" => attributes.label = Some(tokens.current),
                        "type" => attributes.node_type = Some(tokens.current),
                        _ => tokens
                            .error_str(&format!("Unexpected attribute name '{}'", name.lexeme)),
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
        let return_node = graph.get_return_node().unwrap();
        match return_node.node_type {
            NodeType::Return {
                argument: Some(argument),
            } => {
                let b = graph.get_node(argument).unwrap();
                match &b.node_type {
                    NodeType::FunctionCall { arguments } => {
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
                NodeType::VariableDefinition { body } => {
                    assert!(body.is_none());
                }
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
        assert!(matches!(
            node.node_type,
            NodeType::VariableDefinition { .. }
        ));
        assert_eq!(node.attributes.comment.unwrap().lexeme, "\"hi\"");
    }

    #[test]
    fn node_attribs() {
        let source = "digraph { b [pos=\"1,2\"]; a -> b; a [label=2.5] }";
        let parser = Parser::new(source);
        let graph = parser.parse().unwrap();

        let a = graph.get_node("a").unwrap();
        assert_eq!(a.node_id.lexeme, "a");
        assert!(matches!(a.node_type, NodeType::VariableReference));
        assert_eq!(a.attributes.label.unwrap().lexeme, "2.5");

        let b = graph.get_node("b").unwrap();
        assert_eq!(b.node_id.lexeme, "b");
        match b.node_type {
            NodeType::VariableDefinition { body: Some(body) } => {
                assert_eq!(body, "a");
            }
            _ => panic!(),
        };
        assert_eq!(b.attributes.pos.unwrap().lexeme, "\"1,2\"");
    }

    #[test]
    fn deduce_literal_types() {
        let source = r#"
            digraph {
                3.14
                "hi"
                num1 [label=1]
                string2 [label="stringy"]
            }
        "#;
        let parser = Parser::new(source);
        let graph = parser.parse().unwrap();
        assert!(matches!(
            graph.get_node("3.14").unwrap().node_type,
            NodeType::Literal
        ));
        assert!(matches!(
            graph.get_node("\"hi\"").unwrap().node_type,
            NodeType::Literal
        ));
        assert!(matches!(
            graph.get_node("num1").unwrap().node_type,
            NodeType::Literal
        ));
        assert!(matches!(
            graph.get_node("string2").unwrap().node_type,
            NodeType::Literal
        ));
    }

    #[test]
    fn deduce_types() {
        let source = r#"
            digraph {
                return
                var1
                ref1
                fn1
                ret1 [label=return]
                1 -> var1
                ref1 -> ret1
                fn1 -> fn1
            }
        "#;
        let parser = Parser::new(source);
        let graph = parser.parse().unwrap();
        assert!(matches!(
            graph.get_node("return").unwrap().node_type,
            NodeType::Return { .. }
        ));
        assert!(matches!(
            graph.get_node("ret1").unwrap().node_type,
            NodeType::Return { .. }
        ));
        assert!(matches!(
            graph.get_node("var1").unwrap().node_type,
            NodeType::VariableDefinition { .. }
        ));
        assert!(matches!(
            graph.get_node("ref1").unwrap().node_type,
            NodeType::VariableReference
        ));
        assert!(matches!(
            graph.get_node("fn1").unwrap().node_type,
            NodeType::FunctionCall { .. }
        ));
    }

    #[test]
    fn explicit_types() {
        let source = r#"
            digraph {
                a [type=fn]
                b [type=var]
                c [type=call]
                d [type=ref]
                e [type=return]
                f [type=param]
                g [type=not]
                h [type=subtract]
                j [type=gt]
                k [type=gte]
                l [type=literal]
                m [type=lt]
                n [type=lte]
            }
        "#;
        let parser = Parser::new(source);
        let graph = parser.parse().unwrap();
        assert!(matches!(
            graph.get_node("a").unwrap().node_type,
            NodeType::FunctionDefinition { .. }
        ));
        assert!(matches!(
            graph.get_node("b").unwrap().node_type,
            NodeType::VariableDefinition { .. }
        ));
        assert!(matches!(
            graph.get_node("c").unwrap().node_type,
            NodeType::FunctionCall { .. }
        ));
        assert!(matches!(
            graph.get_node("d").unwrap().node_type,
            NodeType::VariableReference { .. }
        ));
        assert!(matches!(
            graph.get_node("e").unwrap().node_type,
            NodeType::Return { .. }
        ));
        assert!(matches!(
            graph.get_node("f").unwrap().node_type,
            NodeType::Param
        ));
        assert!(matches!(
            graph.get_node("g").unwrap().node_type,
            NodeType::Unary { .. }
        ));
        assert!(matches!(
            graph.get_node("h").unwrap().node_type,
            NodeType::Binary { .. }
        ));
        assert!(matches!(
            graph.get_node("j").unwrap().node_type,
            NodeType::Binary { .. }
        ));
        assert!(matches!(
            graph.get_node("k").unwrap().node_type,
            NodeType::Binary { .. }
        ));
        assert!(matches!(
            graph.get_node("l").unwrap().node_type,
            NodeType::Literal
        ));
        assert!(matches!(
            graph.get_node("m").unwrap().node_type,
            NodeType::Binary { .. }
        ));
        assert!(matches!(
            graph.get_node("n").unwrap().node_type,
            NodeType::Binary { .. }
        ));
    }
}

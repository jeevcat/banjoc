use crate::scanner::{Scanner, TokenType};

pub fn compile(source: &str) {
    let mut scanner = Scanner::new(source);

    let mut line = u32::MAX;
    loop {
        let token = scanner.scan_token();
        
        if token.line != line {
            print!("{:4} ", token.line);
            line = token.line;
        }
        else {
            print!("   | ");
        }
        println!("{:12} '{}'", format!("{:?}", token.token_type), token.lexeme);

        if token.token_type == TokenType::Eof {
            break;
        }
    }
}

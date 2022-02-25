use std::{
    env, fs,
    io::{self, Write},
    process,
};

use banjoc::{
    ast::Ast,
    error::{BanjoError, Result},
    value::Value,
    vm::Vm,
};

fn repl(vm: &mut Vm) {
    loop {
        print!("> ");
        io::stdout().flush().unwrap();
        let mut line = String::new();
        io::stdin()
            .read_line(&mut line)
            .expect("Unable to read line from the REPL");
        if line.is_empty() {
            break;
        }
        if let Ok(result) = interpret(vm, &line) {
            println!("{}", result);
        }
    }
}

fn run_file(vm: &mut Vm, path: &str) {
    let source = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(error) => {
            eprint!("Unable to read file {}: {}", path, error);
            process::exit(74);
        }
    };
    match interpret(vm, &source) {
        Ok(result) => println!("{}", result),
        Err(error) => match error {
            BanjoError::CompileError(e) => {
                eprint!("{e}");
                process::exit(65);
            }
            BanjoError::CompileErrors(errors) => {
                for e in errors {
                    eprintln!("{e}");
                }
                process::exit(65);
            }
            BanjoError::RuntimeError(e) => {
                eprintln!("{e}");
                process::exit(70);
            }
        },
    }
}

fn interpret(vm: &mut Vm, source: &str) -> Result<Value> {
    let ast = Ast::new(source)?;
    vm.interpret(&ast)
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let mut vm = Vm::new();
    match args.len() {
        1 => repl(&mut vm),
        2 => run_file(&mut vm, &args[1]),
        _ => {
            eprintln!("Usage: banjo [path]");
            process::exit(64);
        }
    }
}

use std::{
    env, fs,
    io::{self, Write},
    process,
};

use banjoc::{error::LoxError, vm::Vm};

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
        if let Ok(result) = vm.interpret(&line) {
            println!("{}", result);
        }
    }
}

fn run_file(vm: &mut Vm, path: &str) {
    let code = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(error) => {
            eprint!("Unable to read file {}: {}", path, error);
            process::exit(74);
        }
    };
    match vm.interpret(&code) {
        Ok(result) => println!("{}", result),
        Err(error) => match error {
            LoxError::CompileError(_) => {
                process::exit(65);
            }
            LoxError::RuntimeError => {
                eprintln!("Runtime error.");
                process::exit(70);
            }
        },
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let mut vm = Vm::new();
    match args.len() {
        1 => repl(&mut vm),
        2 => run_file(&mut vm, &args[1]),
        _ => {
            eprintln!("Usage: clox [path]");
            process::exit(64);
        }
    }
}

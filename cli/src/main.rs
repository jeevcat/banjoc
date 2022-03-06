use std::{
    env, fs,
    io::{self, Write},
    process,
    time::Instant,
};

use banjoc::{ast::Source, error::Error, output::Output, vm::Vm};
use serde_json::from_str;

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
        let result = interpret(vm, &line);
        println!("{}", serde_json::to_string_pretty(&result).unwrap());
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
    let output = interpret(vm, &source);
    println!("{}", serde_json::to_string_pretty(&output).unwrap());
}

fn interpret(vm: &mut Vm, source: &str) -> Output {
    let now = Instant::now();
    let source: Source = match from_str(source) {
        Ok(source) => source,
        Err(e) => {
            return Output::from_single_error(Error::Compile(format!("JSON parsing error: {e}")))
        }
    };
    println!("Parsing took {:.0?}", now.elapsed());
    vm.interpret(source)
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

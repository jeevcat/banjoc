use std::{
    env, fs,
    io::{self, Write},
    process,
    time::Instant,
};

use banjoc::{
    ast::Source,
    error::{BanjoError, Result},
    output::NodeOutputs,
    vm::Vm,
};
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
        if let Ok(result) = interpret(vm, &line) {
            println!("{:#?}", result);
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
        Ok(result) => println!("{:#?}", result),
        Err(error) => match error {
            BanjoError::CompileNode((node_id, e)) => {
                eprint!("{node_id}: {e}");
                process::exit(65);
            }
            BanjoError::CompileErrors(errors) => {
                for (node_id, e) in errors {
                    eprint!("{node_id}: {e}");
                }
                process::exit(65);
            }
            BanjoError::Runtime(e) => {
                eprintln!("{e}");
                process::exit(70);
            }
            BanjoError::Compile(e) => panic!("Compile error without node information {e}"),
        },
    }
}

fn interpret(vm: &mut Vm, source: &str) -> Result<NodeOutputs> {
    let now = Instant::now();
    let source: Source =
        from_str(source).map_err(|e| BanjoError::Compile(format!("JSON parsing error: {e}")))?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_name() {
        let j = r#"
        {
            "nodes": [
              {
                "id": "return",
                "type": "return",
                "arguments": []
              },
              {
                "id": "sum",
                "type": "call",
                "name": "sum",
                "arguments": ["a", "b"]
              },
              {
                "id": "a",
                "type": "literal",
                "value": 1
              },
              {
                "id": "b",
                "type": "literal",
                "value": 2
              }
            ]
          }
        "#;
        println!("{:#?}", serde_json::from_str::<Source>(j).unwrap());
    }
}

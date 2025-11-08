use std::env;
use std::fs;
use std::io;
use std::io::BufRead;
use std::io::BufReader;
use std::io::Read;
use std::process;

use anyhow::Context;
use env_logger::Env;
use grep_starter_rust::Pattern;
use log::error;

// Usage: echo <input_text> | your_grep.sh -E <pattern>
fn main() -> anyhow::Result<()> {
    let mut args = env::args();
    args.next();

    env_logger::init_from_env(Env::new().default_filter_or("trace"));
    if args.next() != Some("-E".to_string()) {
        error!("Expected first argument to be '-E'");
        process::exit(1);
    }

    let pattern = args.next().unwrap();
    let pattern = Pattern::new(&pattern)?;

    let mut inputs: Vec<(String, Box<dyn Read>)> = vec![];

    while let Some(file) = args.next() {
        if file == "-" {
            inputs.push(("-".to_string(), Box::new(io::stdin())));
        } else {
            inputs.push((
                file.clone(),
                Box::new(
                    fs::File::open(&file)
                        .with_context(|| format!("Failed to open file: {file}"))?,
                ),
            ));
        }
    }

    if inputs.is_empty() {
        inputs.push(("-".to_string(), Box::new(io::stdin())));
    }

    let mut input_line = String::new();
    let mut matched = false;
    let has_multiple_inputs = inputs.len() > 1;

    for (name, input) in inputs {
        let mut input_reader = BufReader::new(input);
        while let Ok(len) = input_reader.read_line(&mut input_line)
            && len > 0
        {
            if pattern.test(&input_line) {
                matched = true;
                if has_multiple_inputs {
                    print!("{name}:");
                }
                print!("{input_line}");
            }
            input_line.clear();
        }
    }

    if matched {
        process::exit(0)
    } else {
        process::exit(1)
    }
}

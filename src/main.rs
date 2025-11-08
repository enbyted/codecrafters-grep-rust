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
    env_logger::init_from_env(Env::new().default_filter_or("trace"));
    if env::args().nth(1).unwrap() != "-E" {
        error!("Expected first argument to be '-E'");
        process::exit(1);
    }

    let pattern = env::args().nth(2).unwrap();
    let pattern = Pattern::new(&pattern)?;

    let input: Box<dyn Read> = if let Some(file) = env::args().nth(3) {
        Box::new(fs::File::open(&file).with_context(|| format!("Failed to open file: {file}"))?)
    } else {
        Box::new(io::stdin())
    };

    let mut input_reader = BufReader::new(input);
    let mut input_line = String::new();
    let mut matched = false;
    while let Ok(len) = input_reader.read_line(&mut input_line)
        && len > 0
    {
        if pattern.test(&input_line) {
            matched = true;
            print!("{input_line}");
        }
        input_line.clear();
    }

    if matched {
        process::exit(0)
    } else {
        process::exit(1)
    }
}

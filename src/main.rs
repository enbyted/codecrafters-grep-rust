use std::env;
use std::io;
use std::process;

use grep_starter_rust::Pattern;

// Usage: echo <input_text> | your_grep.sh -E <pattern>
fn main() -> anyhow::Result<()> {
    if env::args().nth(1).unwrap() != "-E" {
        eprintln!("Expected first argument to be '-E'");
        process::exit(1);
    }

    let pattern = env::args().nth(2).unwrap();
    let pattern = Pattern::new(&pattern)?;
    let mut input_line = String::new();

    io::stdin().read_line(&mut input_line).unwrap();

    if pattern.test(&input_line) {
        process::exit(0)
    } else {
        process::exit(1)
    }
}

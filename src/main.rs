use std::fs;
use std::io;
use std::io::BufRead;
use std::io::BufReader;
use std::io::Read;
use std::path::PathBuf;
use std::process;

use anyhow::Context;
use clap::Parser;
use env_logger::Env;
use grep_starter_rust::Pattern;
use log::info;

#[derive(Debug, Clone, PartialEq, Eq, Parser)]
struct ProgramArgs {
    #[arg(short = 'E')]
    extended: bool,
    #[arg(short = 'r')]
    recursive: bool,

    pattern: String,

    paths: Vec<PathBuf>,
}

// Usage: echo <input_text> | your_grep.sh -E <pattern>
fn main() -> anyhow::Result<()> {
    let args = ProgramArgs::parse();

    env_logger::init_from_env(Env::new().default_filter_or("info"));

    let pattern = Pattern::new(&args.pattern)?;

    let mut inputs: Vec<(String, Box<dyn Read>)> = vec![];

    for file in args.paths {
        if file.to_str() == Some("-") {
            inputs.push(("-".to_string(), Box::new(io::stdin())));
        } else if file.is_dir() && args.recursive {
            let base = PathBuf::from(&file)
                .canonicalize()
                .with_context(|| format!("Failed to canonicalize path: {file:?}"))?;
            let mut dir_inputs = vec![base.clone()];
            let base = base.parent().unwrap_or(&base);
            while let Some(file) = dir_inputs.pop() {
                for entry in fs::read_dir(&file)
                    .with_context(|| format!("Failed to read directory: {file:?}"))?
                {
                    let entry = entry
                        .with_context(|| format!("Failed to get entry for file in {file:?}"))?;

                    if entry
                        .file_type()
                        .with_context(|| format!("Failed to get file type for entry {file:?}"))?
                        .is_dir()
                    {
                        dir_inputs.push(entry.path());
                    } else {
                        inputs.push((
                            entry
                                .path()
                                .strip_prefix(&base)
                                .with_context(|| {
                                    format!("Failed to strip prefix from path: {:?}", entry.path())
                                })?
                                .to_string_lossy()
                                .to_string(),
                            Box::new(fs::File::open(&entry.path()).with_context(|| {
                                format!("Failed to open file: {:?}", entry.path())
                            })?),
                        ));
                    }
                }
            }
        } else {
            inputs.push((
                file.to_string_lossy().to_string(),
                Box::new(
                    fs::File::open(&file)
                        .with_context(|| format!("Failed to open file: {file:?}"))?,
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
        info!("Processing {name:?}");
        let mut input_reader = BufReader::new(input);
        while let Ok(len) = input_reader.read_line(&mut input_line)
            && len > 0
        {
            if pattern.test(&input_line) {
                matched = true;
                if has_multiple_inputs {
                    print!("{name}:");
                }
                println!("{}", input_line.trim_end_matches('\n'));
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

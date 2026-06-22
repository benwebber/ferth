use std::env;
use std::io::{IsTerminal, Read};
use std::process::exit;

use ferth::io::Io;
use ferth::{Config, Fe};

fn main() {
    let Args {
        config,
        mem,
        command,
        file,
    } = parse_args();
    let io = make_io();
    let mut fe =
        Fe::with_config(vec![0u8; mem], io, config).expect("failed to initialize interpreter");

    if let Some(code) = command {
        if let Err(e) = fe.evaluate(code) {
            eprintln!("{e}");
            exit(1);
        }
        return;
    }

    if let Some(path) = file {
        if path == "-" {
            if !std::io::stdin().is_terminal() {
                if let Err(e) = fe.evaluate(read_stdin()) {
                    eprintln!("{e}");
                    exit(1);
                }
                return;
            }
        } else {
            if let Err(e) = fe.evaluate(read_file(&path)) {
                eprintln!("{e}");
                exit(1);
            }
            return;
        }
    }

    let is_terminal = std::io::stdin().is_terminal();
    loop {
        match fe.quit() {
            Ok(()) => break, // end of input
            Err(e) => {
                eprintln!("{e}");
                if !is_terminal {
                    std::process::exit(1);
                }
            }
        }
    }
}

struct Args {
    config: Config,
    mem: usize,
    command: Option<String>,
    file: Option<String>,
}

fn parse_args() -> Args {
    let mut mem = 65536usize;
    let mut config = Config::default();
    let mut command = None;
    let mut file = None;
    let mut args = env::args();
    let basename = args.next().unwrap_or_else(|| "fe".into());
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                usage(&basename);
                exit(0);
            }
            "-v" | "--version" => {
                version();
                exit(0);
            }
            "-c" => command = Some(string(&basename, "-c", args.next())),
            "-m" => mem = int(&basename, "-m", args.next()),
            "-s" => config.stack_cells = int(&basename, "-s", args.next()),
            "-r" => config.return_stack_cells = int(&basename, "-s", args.next()),
            other => {
                if file.is_some() {
                    eprintln!("{basename}: unexpected argument: {other}");
                    exit(1);
                }
                file = Some(other.to_string());
            }
        }
    }
    if command.is_some() && file.is_some() {
        eprintln!("{basename}: -c and FILE are mutually exclusive");
        exit(1);
    }
    Args {
        config,
        mem,
        command,
        file,
    }
}

fn usage(basename: &str) {
    println!(
        "usage: {basename} [-c CODE] [-m MEMORY] [-s STACK_CELLS] [-r RETURN_STACK_CELLS] [-h] [-v] [FILE]"
    )
}

fn version() {
    println!("fe {}", env!("CARGO_PKG_VERSION"))
}

fn string(basename: &str, flag: &str, value: Option<String>) -> String {
    value.unwrap_or_else(|| {
        eprintln!("{basename}: {flag}: expected string");
        exit(1);
    })
}

fn int(basename: &str, flag: &str, value: Option<String>) -> usize {
    let raw = value.unwrap_or_else(|| {
        eprintln!("{basename}: {flag}: expected integer");
        exit(1);
    });
    raw.parse().unwrap_or_else(|_| {
        eprintln!("{basename}: {flag}: expected integer, got {raw}");
        exit(1);
    })
}

fn read_file(path: &str) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| {
        eprintln!("{path}: {e}");
        exit(1);
    })
}

fn read_stdin() -> String {
    let mut buf = String::new();
    if let Err(e) = std::io::stdin().read_to_string(&mut buf) {
        eprintln!("stdin: {e}");
        exit(1);
    }
    buf
}

#[cfg(not(feature = "repl"))]
fn make_io() -> impl Io {
    ferth::io::StdIo
}

#[cfg(feature = "repl")]
fn make_io() -> impl Io {
    use ferth::io::repl::ReplIo;
    use rustyline::DefaultEditor;
    ReplIo::new(DefaultEditor::new().expect("failed to initialize editor"))
}

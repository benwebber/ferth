use std::env;
use std::io::IsTerminal;
use std::process::exit;

use ferth::io::Io;
use ferth::{Config, Fe};

fn main() {
    let (config, mem, command) = parse_args();
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

fn parse_args() -> (Config, usize, Option<String>) {
    let mut mem = 65536usize;
    let mut config = Config::default();
    let mut command = None;
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
                eprintln!("{basename}: unexpected argument: {other}");
                exit(1);
            }
        }
    }
    (config, mem, command)
}

fn usage(basename: &str) {
    println!(
        "usage: {basename} [-c CODE] [-m MEMORY] [-s STACK_CELLS] [-r RETURN_STACK_CELLS] [-h] [-v]"
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

use std::env;
use std::io::{IsTerminal, Read};
use std::process::exit;

use ferth::host::{Clock, Io};
use ferth::{Config, Ferth};

fn main() {
    let Args {
        config,
        mem,
        evaluate,
        file,
    } = parse_args();
    let host = make_host();
    let mut fe = match Ferth::with_config(vec![0u8; mem], host, config) {
        Ok(fe) => fe,
        Err(e) => {
            eprintln!("{e}");
            exit(1);
        }
    };

    if let Some(code) = evaluate {
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
    evaluate: Option<String>,
    file: Option<String>,
}

fn parse_args() -> Args {
    let mut mem = 65536usize;
    let mut config = Config::default();
    let mut evaluate = None;
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
            "-d" => config.stack_cells = int(&basename, "-d", args.next()),
            "-e" => evaluate = Some(string(&basename, "-e", args.next())),
            "-m" => mem = int(&basename, "-m", args.next()),
            "-r" => config.return_stack_cells = int(&basename, "-r", args.next()),
            other => {
                if file.is_some() {
                    eprintln!("{basename}: unexpected argument: {other}");
                    exit(1);
                }
                file = Some(other.to_string());
            }
        }
    }
    if evaluate.is_some() && file.is_some() {
        eprintln!("{basename}: -e and FILE are mutually exclusive");
        exit(1);
    }
    Args {
        config,
        mem,
        evaluate,
        file,
    }
}

fn usage(basename: &str) {
    println!(
        "usage: {basename} [-e CODE] [-m MEMORY] [-s STACK_CELLS] [-r RETURN_STACK_CELLS] [-h] [-v] [FILE]"
    )
}

fn version() {
    println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"))
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
fn make_host() -> impl Io + Clock {
    ferth::host::StdHost
}

#[cfg(feature = "repl")]
fn make_host() -> impl Io + Clock {
    use ferth::host::repl::ReplHost;
    use rustyline::DefaultEditor;
    ReplHost::new(DefaultEditor::new().expect("failed to initialize editor"))
}

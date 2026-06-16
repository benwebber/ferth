use ferth::Fe;
use ferth::io::StdIo;

#[cfg(feature = "repl")]
fn main() {
    use rustyline::DefaultEditor;
    use rustyline::error::ReadlineError;
    use std::io::{self, Write};

    let mut fe = Fe::new([0u8; 65536], StdIo).expect("failed to initialize interpreter");
    let mut rl = DefaultEditor::new().expect("failed to initialize editor");
    loop {
        match rl.readline("") {
            Ok(line) => {
                rl.add_history_entry(&line).ok();
                print!("\x1b[1A\r{line} ");
                io::stdout().flush().ok();
                match fe.evaluate(line.as_bytes()) {
                    Ok(()) => {
                        println!(" ok");
                        io::stdout().flush().ok();
                    }
                    Err(e) => {
                        io::stdout().flush().ok();
                        eprintln!("\n{e}");
                    }
                }
            }
            Err(ReadlineError::Eof | ReadlineError::Interrupted) => break,
            Err(e) => eprintln!("{e}"),
        }
    }
}

#[cfg(not(feature = "repl"))]
fn main() {
    let mut fe = Fe::new([0u8; 65536], StdIo).expect("failed to initialize interpreter");
    if let Err(e) = fe.quit() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}

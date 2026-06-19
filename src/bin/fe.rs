use std::io::IsTerminal;

use ferth::Fe;
use ferth::io::Io;

fn main() {
    let is_terminal = std::io::stdin().is_terminal();
    let io = make_io();
    let mut fe = Fe::new([0u8; 65536], io).expect("failed to initialize interpreter");
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

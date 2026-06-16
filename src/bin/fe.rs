use ferth::Fe;
use ferth::io::Io;

fn main() {
    let io = make_io();
    let mut fe = Fe::new([0u8; 65536], io).expect("failed to initialize interpreter");
    if let Err(e) = fe.quit() {
        eprintln!("{e}");
        std::process::exit(1);
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

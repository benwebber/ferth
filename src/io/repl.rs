use std::io::{self, Read, Write};

use rustyline::{DefaultEditor, error::ReadlineError};

use crate::io::Io;
use crate::{Error, Result};

/// A line editor.
///
/// This trait exists to make `ReplIo` generic over any rustyline `Editor`. rustyline does not
/// expose a trait like this.
pub trait LineEditor {
    /// Yield one edited line (without trailing newline), or `None` at EOF.
    fn read_line(&mut self, prompt: &str) -> Result<Option<String>>;
}

impl LineEditor for DefaultEditor {
    fn read_line(&mut self, prompt: &str) -> Result<Option<String>> {
        match self.readline(prompt) {
            Ok(line) => {
                self.add_history_entry(&line).ok();
                Ok(Some(line))
            }
            Err(ReadlineError::Eof | ReadlineError::Interrupted) => Ok(None),
            Err(_) => Err(Error::Io),
        }
    }
}

pub struct ReplIo {
    editor: Box<dyn LineEditor>,
}

impl ReplIo {
    pub fn new(editor: impl LineEditor + 'static) -> Self {
        Self {
            editor: Box::new(editor),
        }
    }
}

impl Io for ReplIo {
    fn key(&mut self) -> Result<Option<u8>> {
        let mut buf = [0u8; 1];
        match io::stdin().read_exact(&mut buf) {
            Ok(()) => Ok(Some(buf[0])),
            Err(e) if matches!(e.kind(), io::ErrorKind::UnexpectedEof) => Ok(None),
            _ => Err(Error::Io),
        }
    }

    fn emit(&mut self, u: u8) -> Result<()> {
        let mut stdout = io::stdout();
        stdout.write_all(&[u]).map_err(|_| Error::Io)?;
        if u == b'\n' {
            stdout.flush().map_err(|_| Error::Io)?;
        }
        Ok(())
    }

    fn refill(&mut self, buf: &mut [u8]) -> Result<Option<usize>> {
        match self.editor.read_line("")? {
            Some(line) => {
                let bytes = line.as_bytes();
                let len = bytes.len().min(buf.len().saturating_sub(1));
                buf[..len].copy_from_slice(&bytes[..len]);
                // rustyline strips `\n`. `refill` and `parse` currently expect it.
                buf[len] = b'\n';
                Ok(Some(len + 1))
            }
            None => Ok(None),
        }
    }
}

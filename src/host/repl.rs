use std::io::{self, Read, Write};

use rustyline::{DefaultEditor, error::ReadlineError};

use crate::double::Double;
use crate::time::{DateTime, sleep_ms, utime};
use crate::{Error, Result};

use super::{Clock, Io};

/// A line editor.
///
/// This trait exists to make `ReplHost` generic over any rustyline `Editor`. rustyline does not
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

/// A host implementation that uses [`rustyline`].
pub struct ReplHost {
    editor: Box<dyn LineEditor>,
}

impl ReplHost {
    pub fn new(editor: impl LineEditor + 'static) -> Self {
        Self {
            editor: Box::new(editor),
        }
    }
}

impl Io for ReplHost {
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
                let len = bytes.len().min(buf.len());
                buf[..len].copy_from_slice(&bytes[..len]);
                // rustyline already strips end of line characters.
                Ok(Some(len))
            }
            None => Ok(None),
        }
    }
}

impl Clock for ReplHost {
    fn utime(&self) -> Double {
        utime()
    }

    fn sleep_ms(&self, ms: usize) {
        sleep_ms(ms)
    }

    fn time_and_date(&self) -> DateTime {
        DateTime::now()
    }
}

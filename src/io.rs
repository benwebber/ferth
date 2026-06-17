//! System I/O.
use crate::{Error, Result};

#[cfg(feature = "repl")]
pub mod repl;

/// System I/O.
pub trait Io {
    /// Read a single character (byte) from the input source.
    ///
    /// Returns `Ok(None)` if there is no more data to read.
    fn key(&mut self) -> Result<Option<u8>>;

    /// Output a single character (byte) to the output device.
    fn emit(&mut self, u: u8) -> Result<()>;

    /// Read a line of input into `buf`.
    ///
    /// Returns `Ok(None)` if there is no more data to read.
    fn refill(&mut self, buf: &mut [u8]) -> Result<Option<usize>>;
}

/// An [`Io`] implementation that returns an error for any I/O operation.
///
/// Suitable for `no_std`.
pub struct NoIo;

impl Io for NoIo {
    fn key(&mut self) -> Result<Option<u8>> {
        // TODO: more descriptive error variant for "no I/O available"
        Err(Error::Io)
    }

    fn emit(&mut self, _u: u8) -> Result<()> {
        Err(Error::Io)
    }

    fn refill(&mut self, _buf: &mut [u8]) -> Result<Option<usize>> {
        Err(Error::Io)
    }
}

/// An [`Io`] implementation that reads from, and writes to, slice references.
pub struct BufIo<'a> {
    input: &'a [u8],
    input_pos: usize,
    output: &'a mut [u8],
    output_pos: usize,
}

impl<'a> BufIo<'a> {
    pub fn new(input: &'a [u8], output: &'a mut [u8]) -> Self {
        Self {
            input,
            input_pos: 0,
            output,
            output_pos: 0,
        }
    }
}

impl Io for BufIo<'_> {
    fn key(&mut self) -> Result<Option<u8>> {
        match self.input.get(self.input_pos) {
            Some(c) => {
                self.input_pos += 1;
                Ok(Some(*c))
            }
            None => Ok(None),
        }
    }

    fn emit(&mut self, u: u8) -> Result<()> {
        *self.output.get_mut(self.output_pos).ok_or(Error::Io)? = u;
        self.output_pos += 1;
        Ok(())
    }

    fn refill(&mut self, buf: &mut [u8]) -> Result<Option<usize>> {
        if self.input_pos >= self.input.len() {
            return Ok(None);
        }
        let mut len = 0;
        while self.input_pos < self.input.len() && len < buf.len() {
            let c = self.input[self.input_pos];
            buf[len] = c;
            len += 1;
            self.input_pos += 1;
            if c == b'\n' {
                break;
            }
        }
        Ok(Some(len))
    }
}

#[cfg(feature = "std")]
/// An [`Io`] implementation that uses standard input and output.
pub struct StdIo;

#[cfg(feature = "std")]
impl Io for StdIo {
    fn key(&mut self) -> Result<Option<u8>> {
        use std::io::ErrorKind;
        use std::io::Read;
        let mut buf = [0u8; 1];
        match std::io::stdin().read_exact(&mut buf) {
            Ok(()) => Ok(Some(buf[0])),
            Err(e) if matches!(e.kind(), ErrorKind::UnexpectedEof) => Ok(None),
            _ => Err(Error::Io),
        }
    }

    fn emit(&mut self, u: u8) -> Result<()> {
        use std::io::Write;
        let mut stdout = std::io::stdout();
        stdout.write_all(&[u]).map_err(|_| Error::Io)?;
        if u == b'\n' {
            stdout.flush().map_err(|_| Error::Io)?;
        }
        Ok(())
    }

    fn refill(&mut self, buf: &mut [u8]) -> Result<Option<usize>> {
        // Flush pending output before waiting on the prompt.
        use std::io::{BufRead, Write};
        std::io::stdout().flush().map_err(|_| Error::Io)?;
        let mut stdin = std::io::stdin().lock();
        let mut line = Vec::new();
        let n = stdin.read_until(b'\n', &mut line).map_err(|_| Error::Io)?;
        if n == 0 {
            return Ok(None);
        }
        let len = n.min(buf.len());
        buf[..len].copy_from_slice(&line[..len]);
        Ok(Some(len))
    }
}

use crate::double::Double;
use crate::time::{DateTime, sleep_ms, utime};
use crate::{Error, Result};

use super::{Clock, Io};

/// A host implementation that uses standard input and output, and the system clock.
pub struct StdHost;

impl Io for StdHost {
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
        // Strip end of line characters.
        if line.last() == Some(&b'\n') {
            line.pop();
            if line.last() == Some(&b'\r') {
                line.pop();
            }
        }
        let len = line.len().min(buf.len());
        buf[..len].copy_from_slice(&line[..len]);
        Ok(Some(len))
    }
}

impl Clock for StdHost {
    fn utime(&self) -> Double {
        utime()
    }

    fn sleep_ms(&self, ms: usize) {
        sleep_ms(ms)
    }

    #[cfg(feature = "time")]
    fn time_and_date(&self) -> DateTime {
        DateTime::now()
    }

    #[cfg(not(feature = "time"))]
    fn time_and_date(&self) -> DateTime {
        DateTime::default()
    }
}

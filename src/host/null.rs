use crate::double::Double;
use crate::time::DateTime;
use crate::{Error, Result};

use super::{Clock, Io};

/// A host implementation that returns an error for any I/O operation does not keep time.
///
/// Suitable for `no_std`.
pub struct NullHost;

impl Io for NullHost {
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

impl Clock for NullHost {
    fn utime(&self) -> Double {
        Double::default()
    }

    fn time_and_date(&self) -> DateTime {
        DateTime::default()
    }

    fn sleep_ms(&self, _ms: usize) {}
}

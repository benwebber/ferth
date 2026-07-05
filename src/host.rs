//! Host implementations.
use crate::Result;
use crate::double::Double;
use crate::time::DateTime;

mod null;
#[cfg(feature = "repl")]
pub mod repl;
#[cfg(feature = "std")]
pub mod std;

pub use null::NullHost;

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

/// A system clock.
pub trait Clock {
    /// The monotonic clock, in microseconds.
    fn utime(&self) -> Double;

    /// The wall clock, in UTC.
    fn time_and_date(&self) -> DateTime;

    /// Sleep for *ms* milliseconds.
    fn sleep_ms(&self, ms: usize);
}

/// A proxy for `Clock` that requires it only when the build includes clock builtins.
///
/// `Kernel` and `Fe` can reference this trait instead of `Clock`. This avoids having to duplicate
/// functions for each combination of traits (`H: Io`, `H: Io + Clock`).
///
/// Do not implement this trait directly. It is implemented automatically for any type that
/// implements `Clock`.
#[cfg(not(feature = "std"))]
pub trait MaybeClock {}
#[cfg(not(feature = "std"))]
impl<T> MaybeClock for T {}

#[cfg(feature = "std")]
pub trait MaybeClock: Clock {}
#[cfg(feature = "std")]
impl<T: Clock> MaybeClock for T {}

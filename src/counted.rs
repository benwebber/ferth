//! Forth counted strings.
use crate::{Error, Result};

/// A Forth [counted string](https://forth-standard.org/standard/usage#subsubsection.3.1.3.4).
///
/// Counted strings encode the length of the string as one byte, then the characters of the string
/// as subsequent bytes. Therefore counted strings may only contain up to 255 bytes.
#[derive(Copy, Clone, Debug, PartialEq)]
#[repr(C)]
pub struct CountedStr<const N: usize> {
    len: u8,
    data: [u8; N],
}

/// A counted string that can hold up to 31 bytes, for a total size of 32 bytes.
pub type CountedStr31 = CountedStr<31>;

impl<const N: usize> CountedStr<N> {
    pub const MAX_LEN: usize = N;

    pub fn len(&self) -> usize {
        self.len as usize
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.data[..self.len as usize]
    }
}

impl<const N: usize> Default for CountedStr<N> {
    fn default() -> Self {
        Self {
            len: 0,
            data: [0; N],
        }
    }
}

impl<const N: usize> core::fmt::Display for CountedStr<N> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match core::str::from_utf8(self.as_bytes()) {
            Ok(s) => write!(f, "{s}"),
            Err(_) => {
                for b in self.as_bytes() {
                    write!(f, "\\x{b:02x}")?;
                }
                Ok(())
            }
        }
    }
}

impl<const N: usize> TryFrom<&str> for CountedStr<N> {
    type Error = Error;

    fn try_from(s: &str) -> Result<Self> {
        s.as_bytes().try_into()
    }
}

impl<const N: usize> TryFrom<&[u8]> for CountedStr<N> {
    type Error = Error;

    fn try_from(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > N {
            return Err(Error::CountedStrTooLong(bytes.len()));
        }
        let mut data = [0u8; N];
        data[..bytes.len()].copy_from_slice(bytes);
        Ok(Self {
            len: bytes.len() as u8,
            data,
        })
    }
}

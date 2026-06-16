//! Error and result types.
use crate::counted::CountedStr31;
use crate::vm::VmError;

/// The result type of this crate.
pub type Result<T> = core::result::Result<T, Error>;

pub const UNDEFINED_WORD: isize = -13;

/// Errors returned by this crate.
#[derive(Debug, PartialEq)]
pub enum Error {
    /// An error raised by the inner interpreter.
    Vm(VmError),
    /// The length of a string is too long for the Forth counted string type (255 bytes).
    CountedStrTooLong(usize),
    /// The name does not match a known word in the dictionary.
    UndefinedWord(CountedStr31),
    /// A string is not valid UTF-8.
    InvalidUtf8(core::str::Utf8Error),
    /// A generic error for I/O errors.
    Io,
    /// No builtin exists with the wrapped index.
    InvalidBuiltin(u8),
    /// The builtins table is full.
    BuiltinTableFull,
    /// The line length (in bytes) exceeds the size of the terminal input buffer.
    LineTooLong,
    /// The stacks are too small.
    ///
    /// The data space must start above the opcode range in order to distinguish between opcodes
    /// and defined words.
    StacksTooSmall,
    Throw(isize),
}

impl From<VmError> for Error {
    fn from(e: VmError) -> Self {
        Self::Vm(e)
    }
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Vm(e) => write!(f, "{e}"),
            Self::CountedStrTooLong(len) => write!(f, "counted string too long: {len}"),
            Self::UndefinedWord(name) => write!(f, "undefined word: {name}"),
            Self::InvalidUtf8(e) => write!(f, "invalid UTF-8: {e}"),
            Self::Io => write!(f, "I/O error"),
            Self::InvalidBuiltin(idx) => write!(f, "invalid builtin: 0x{idx:02x}"),
            Self::BuiltinTableFull => write!(f, "builtin table full"),
            Self::LineTooLong => write!(f, "line too long"),
            Self::StacksTooSmall => write!(f, "stacks too small"),
            Self::Throw(n) => write!(f, "error: {n}"),
        }
    }
}

impl core::error::Error for Error {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Self::Vm(e) => Some(e),
            Self::InvalidUtf8(e) => Some(e),
            _ => None,
        }
    }
}

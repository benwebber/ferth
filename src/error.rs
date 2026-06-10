//! Error and result types.
use crate::counted::CountedStr31;

/// The result type of this crate.
pub type Result<T> = core::result::Result<T, Error>;

/// The result type of the inner interpreter.
pub type VmResult<T> = core::result::Result<T, VmError>;

/// Errors raised by the inner interpreter.
#[derive(Debug, PartialEq)]
pub enum VmError {
    /// The data stack overflowed.
    StackOverflow,
    /// The data stack underflowed.
    StackUnderflow,
    /// The return stack overflowed.
    ReturnStackOverflow,
    /// The return stack underflowed.
    ReturnStackUnderflow,
    /// The address is out of range.
    AddressOutOfRange(usize),
    /// The address is not aligned to the cell size.
    AddressMisaligned(usize),
    /// The opcode is not valid.
    InvalidOpCode(u8),
    /// Attempted to divide by zero.
    DivisionByZero,
}

impl core::fmt::Display for VmError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let w = core::mem::size_of::<usize>() * 2;
        match self {
            Self::StackOverflow => write!(f, "stack overflow"),
            Self::StackUnderflow => write!(f, "stack underflow"),
            Self::ReturnStackOverflow => write!(f, "return stack overflow"),
            Self::ReturnStackUnderflow => write!(f, "return stack underflow"),
            Self::AddressOutOfRange(addr) => {
                write!(f, "address out of range: 0x{:0width$x}", addr, width = w)
            }
            Self::AddressMisaligned(addr) => {
                write!(f, "address misaligned: 0x{:0width$x}", addr, width = w)
            }
            Self::InvalidOpCode(op) => write!(f, "invalid opcode: 0x{op:02x}"),
            Self::DivisionByZero => write!(f, "division by zero"),
        }
    }
}

impl core::error::Error for VmError {}

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

//! Error and result types.
use crate::counted::CountedStr31;
use crate::vm::VmError;

/// The result type of this crate.
pub type Result<T> = core::result::Result<T, Error>;

/// An error code.
#[derive(Debug, Clone, Copy)]
pub struct Ior(pub isize);

impl Ior {
    pub const STACK_OVERFLOW: isize = -3;
    pub const STACK_UNDERFLOW: isize = -4;
    pub const RETURN_STACK_OVERFLOW: isize = -5;
    pub const RETURN_STACK_UNDERFLOW: isize = -6;
    pub const INVALID_MEMORY_ADDRESS: isize = -9;
    pub const DIVISION_BY_ZERO: isize = -10;
    pub const UNDEFINED_WORD: isize = -13;
    pub const PARSED_STRING_OVERFLOW: isize = -18;
    pub const DEFINITION_NAME_TOO_LONG: isize = -19;
    pub const UNSUPPORTED_OPERATION: isize = -21;
}

impl From<Ior> for isize {
    fn from(ior: Ior) -> Self {
        ior.0
    }
}

impl TryFrom<Error> for Ior {
    type Error = Error;

    fn try_from(e: Error) -> std::result::Result<Self, Self::Error> {
        Ok(match e {
            Error::Vm(v) => Ior::from(v),
            Error::Throw(n) => Ior(n),
            Error::UndefinedWord(_) => Ior(Ior::UNDEFINED_WORD),
            Error::CountedStrTooLong(_) => Ior(Ior::DEFINITION_NAME_TOO_LONG),
            Error::LineTooLong => Ior(Ior::PARSED_STRING_OVERFLOW),
            // All others fall through as normal.
            e @ (Error::Io
            | Error::InvalidUtf8(_)
            | Error::InvalidBuiltin(_)
            | Error::BuiltinTableFull
            | Error::StacksTooSmall) => return Err(e),
        })
    }
}

impl From<VmError> for Ior {
    fn from(e: VmError) -> Self {
        Self(match e {
            VmError::StackOverflow => Self::STACK_OVERFLOW,
            VmError::StackUnderflow => Self::STACK_UNDERFLOW,
            VmError::ReturnStackOverflow => Self::RETURN_STACK_OVERFLOW,
            VmError::ReturnStackUnderflow => Self::RETURN_STACK_UNDERFLOW,
            VmError::AddressOutOfRange(_) | VmError::AddressMisaligned(_) => {
                Self::INVALID_MEMORY_ADDRESS
            }
            VmError::DivisionByZero => Self::DIVISION_BY_ZERO,
            VmError::InvalidOpCode(_) => Self::UNSUPPORTED_OPERATION,
        })
    }
}

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

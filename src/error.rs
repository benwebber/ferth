//! Error and result types.
pub use crate::vm::VmError;

macro_rules! impl_ior {
    ($($(#[$attr:meta])* $name:ident = $val:literal),+ $(,)?) => {
        impl Ior {
            $($(#[$attr])* pub const $name: isize = $val;)+
        }
    }
}

/// The result type of this crate.
pub type Result<T> = core::result::Result<T, Error>;

/// A Forth error code (*ior*).
#[derive(Debug, Clone, Copy)]
pub struct Ior(pub isize);

impl_ior!(
    STACK_OVERFLOW = -3,
    STACK_UNDERFLOW = -4,
    RETURN_STACK_OVERFLOW = -5,
    RETURN_STACK_UNDERFLOW = -6,
    INVALID_MEMORY_ADDRESS = -9,
    DIVISION_BY_ZERO = -10,
    UNDEFINED_WORD = -13,
    PARSED_STRING_OVERFLOW = -18,
    DEFINITION_NAME_TOO_LONG = -19,
);

impl From<Ior> for isize {
    fn from(ior: Ior) -> Self {
        ior.0
    }
}

impl TryFrom<Error> for Ior {
    type Error = Error;

    fn try_from(e: Error) -> core::result::Result<Self, Self::Error> {
        Ok(match e {
            Error::Vm(v) => Ior::try_from(v).map_err(Error::Vm)?,
            Error::Throw(n) => Ior(n),
            // All others fall through as normal.
            e @ (Error::Io | Error::Fault(_)) => return Err(e),
        })
    }
}

impl TryFrom<VmError> for Ior {
    type Error = VmError;
    fn try_from(e: VmError) -> core::result::Result<Self, VmError> {
        Ok(Self(match e {
            VmError::StackOverflow => Self::STACK_OVERFLOW,
            VmError::StackUnderflow => Self::STACK_UNDERFLOW,
            VmError::ReturnStackOverflow => Self::RETURN_STACK_OVERFLOW,
            VmError::ReturnStackUnderflow => Self::RETURN_STACK_UNDERFLOW,
            VmError::AddressOutOfRange(_) | VmError::AddressMisaligned(_) => {
                Self::INVALID_MEMORY_ADDRESS
            }
            VmError::DivisionByZero => Self::DIVISION_BY_ZERO,
            VmError::InvalidOpCode(_) => return Err(e),
        }))
    }
}

/// An error returned by this crate.
#[derive(Debug, PartialEq)]
pub enum Error {
    /// An error raised by the inner interpreter.
    Vm(VmError),
    /// A generic error for I/O errors.
    Io,
    /// A Forth exception.
    Throw(isize),
    /// An irrecoverable error.
    Fault(Fault),
}

/// An irrecoverable error.
#[derive(Debug, PartialEq)]
pub enum Fault {
    /// No builtin exists with the wrapped index.
    InvalidBuiltin(u8),
    /// The builtins table is full.
    BuiltinTableFull,
}

impl From<Fault> for Error {
    fn from(e: Fault) -> Self {
        Self::Fault(e)
    }
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
            Self::Io => write!(f, "I/O error"),
            Self::Throw(n) => write!(f, "error: {n}"),
            Self::Fault(fault) => write!(f, "fatal error: {}", fault),
        }
    }
}

impl core::fmt::Display for Fault {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidBuiltin(idx) => write!(f, "invalid builtin: 0x{idx:02x}"),
            Self::BuiltinTableFull => write!(f, "builtin table full"),
        }
    }
}

impl core::error::Error for Error {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Self::Vm(e) => Some(e),
            _ => None,
        }
    }
}

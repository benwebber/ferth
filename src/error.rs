//! Error and result types.
pub use crate::vm::VmError;

/// The result type of this crate.
pub type Result<T> = core::result::Result<T, Error>;

/// A Forth error code (*ior*).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ior(pub isize);

macro_rules! impl_ior {
    ($($(#[$attr:meta])* $name:ident = $val:literal),+ $(,)?) => {
        impl Ior {
            $($(#[$attr])* pub const $name: Self = Self($val);)+
        }
    }
}

// Standard throw codes.
impl_ior!(
    STACK_OVERFLOW = -3,
    STACK_UNDERFLOW = -4,
    RETURN_STACK_OVERFLOW = -5,
    RETURN_STACK_UNDERFLOW = -6,
    INVALID_MEMORY_ADDRESS = -9,
    DIVISION_BY_ZERO = -10,
    UNDEFINED_WORD = -13,
    ATTEMPT_TO_USE_ZERO_LENGTH_STRING_AS_NAME = -16,
    PARSED_STRING_OVERFLOW = -18,
    DEFINITION_NAME_TOO_LONG = -19,
    ADDRESS_ALIGNMENT_EXCEPTION = -23,
);

// Custom throw codes.
impl_ior!(INVALID_ESCAPE = -256,);

impl core::fmt::Display for Ior {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<isize> for Ior {
    fn from(i: isize) -> Self {
        Self(i)
    }
}

impl From<Ior> for isize {
    fn from(ior: Ior) -> Self {
        ior.0
    }
}

impl From<Ior> for usize {
    fn from(ior: Ior) -> Self {
        ior.0 as usize
    }
}

/// The recoverability of an [`Error`].
#[derive(Debug, PartialEq, Eq)]
pub enum Severity {
    /// A catchable Forth exception.
    ///
    /// The kernel places the error code on the stack for `throw`.
    Throw(Ior),
    /// An unrecoverable fault.
    ///
    /// The kernel resets and aborts to the host.
    Abort,
}

/// An error returned by this crate.
#[derive(Debug, PartialEq)]
pub enum Error {
    /// An error raised by the inner interpreter.
    Vm(VmError),
    /// A generic error for I/O errors.
    Io,
    /// A Forth exception.
    Throw(Ior),
    /// A kernel error.
    Kernel(KernelError),
}

impl Error {
    pub fn severity(&self) -> Severity {
        use Severity::{Abort, Throw};
        match self {
            Error::Throw(n) => Throw(*n),
            Error::Vm(v) => match v {
                // Stack overflows are irrecoverable because the stack is too full to set up
                // `throw`.
                VmError::StackOverflow
                | VmError::ReturnStackOverflow
                // A malformed word should terminate the program.
                | VmError::InvalidOpCode(_)
                | VmError::MemoryTooSmall(_) => Abort,
                VmError::StackUnderflow => Throw(Ior::STACK_UNDERFLOW),
                VmError::ReturnStackUnderflow => Throw(Ior::RETURN_STACK_UNDERFLOW),
                VmError::DivisionByZero => Throw(Ior::DIVISION_BY_ZERO),
                VmError::AddressOutOfRange(_) => Throw(Ior::INVALID_MEMORY_ADDRESS),
                VmError::AddressMisaligned(_) => Throw(Ior::ADDRESS_ALIGNMENT_EXCEPTION),
                VmError::ParsedStringOverflow => Throw(Ior::PARSED_STRING_OVERFLOW),
                VmError::InvalidEscape(_) => Throw(Ior::INVALID_ESCAPE),
            },
            Error::Io | Error::Kernel(_) => Abort,
        }
    }
}

/// A kernel error.
#[derive(Debug, PartialEq)]
pub enum KernelError {
    /// No builtin exists with the wrapped index.
    InvalidBuiltin(u8),
    /// The builtins table is full.
    BuiltinTableFull,
    /// A runtime entry point (e.g. `quit`) does not exist.
    MissingEntryPoint(&'static str),
    /// An *xt* does not fit in the bytes reserved for it in a packed instruction cell.
    XtTooLarge(usize),
    /// The data space is too small to boot the system.
    ///
    /// Contains the minimum size of the data space in bytes.
    DataSpaceTooSmall(usize),
}

impl From<KernelError> for Error {
    fn from(e: KernelError) -> Self {
        Self::Kernel(e)
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
            Self::Kernel(fault) => write!(f, "fatal error: {}", fault),
        }
    }
}

impl core::fmt::Display for KernelError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidBuiltin(idx) => write!(f, "invalid builtin: 0x{idx:02x}"),
            Self::BuiltinTableFull => write!(f, "builtin table full"),
            Self::MissingEntryPoint(name) => write!(f, "missing entry point: {name}"),
            Self::XtTooLarge(xt) => write!(f, "xt too large to pack: {xt:#x}"),
            Self::DataSpaceTooSmall(n) => write!(f, "data space must be at least {n} bytes"),
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

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

impl core::error::Error for VmError {}

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

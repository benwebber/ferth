/// The result type of the inner interpreter.
pub type VmResult<T> = core::result::Result<T, VmError>;

/// An error raised by the inner interpreter.
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
    /// Irrecoverable. The opcode is not valid.
    InvalidOpCode(u8),
    /// Attempted to divide by zero.
    DivisionByZero,
    ParsedStringOverflow,
    InvalidEscape(u8),
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
            Self::ParsedStringOverflow => write!(f, "parsed string overflow"),
            Self::InvalidEscape(c) => write!(f, "invalid escape: 0x{c:02x}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn width() -> usize {
        core::mem::size_of::<usize>() * 2
    }

    #[test]
    fn display_stack_overflow() {
        assert_eq!(VmError::StackOverflow.to_string(), "stack overflow");
    }

    #[test]
    fn display_stack_underflow() {
        assert_eq!(VmError::StackUnderflow.to_string(), "stack underflow");
    }

    #[test]
    fn display_return_stack_overflow() {
        assert_eq!(
            VmError::ReturnStackOverflow.to_string(),
            "return stack overflow"
        );
    }

    #[test]
    fn display_return_stack_underflow() {
        assert_eq!(
            VmError::ReturnStackUnderflow.to_string(),
            "return stack underflow"
        );
    }

    #[test]
    fn display_address_out_of_range() {
        let addr = 0xcafe;
        assert_eq!(
            VmError::AddressOutOfRange(addr).to_string(),
            format!("address out of range: 0x{:0width$x}", addr, width = width())
        );
    }

    #[test]
    fn display_address_misaligned() {
        let addr = 0x0003;
        assert_eq!(
            VmError::AddressMisaligned(addr).to_string(),
            format!("address misaligned: 0x{:0width$x}", addr, width = width())
        );
    }

    #[test]
    fn display_invalid_opcode() {
        assert_eq!(
            VmError::InvalidOpCode(0xab).to_string(),
            "invalid opcode: 0xab"
        );
    }

    #[test]
    fn display_division_by_zero() {
        assert_eq!(VmError::DivisionByZero.to_string(), "division by zero");
    }
}

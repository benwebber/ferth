//! Forth data types.
use crate::{VmError, VmResult};

/// A value on the stack, and a word in memory (*x*).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct Cell(pub usize);

impl Cell {
    pub const SIZE: usize = size_of::<Cell>();
    pub const ZERO: Self = Self(0);

    #[inline]
    pub fn to_isize(self) -> isize {
        self.0 as isize
    }
}

impl From<usize> for Cell {
    #[inline]
    fn from(u: usize) -> Self {
        Self(u)
    }
}

impl From<Cell> for usize {
    #[inline]
    fn from(c: Cell) -> Self {
        c.0
    }
}

/// A cell-aligned address (*a-addr*).
///
/// `AAddr` does not implement `From<usize>`. A misaligned value cannot become an `AAddr`. Use
/// [`AAddr::try_from`] to check alignment.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct AAddr(pub usize);

impl TryFrom<usize> for AAddr {
    type Error = VmError;

    #[inline]
    fn try_from(u: usize) -> VmResult<Self> {
        if u.is_multiple_of(Cell::SIZE) {
            Ok(Self(u))
        } else {
            Err(VmError::AddressMisaligned(u))
        }
    }
}

impl TryFrom<Cell> for AAddr {
    type Error = VmError;

    #[inline]
    fn try_from(c: Cell) -> VmResult<Self> {
        Self::try_from(c.0)
    }
}

impl From<AAddr> for Cell {
    #[inline]
    fn from(a: AAddr) -> Self {
        Cell(a.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cell_usize_roundtrip() {
        assert_eq!(Cell::from(42), Cell(42));
        assert_eq!(usize::from(Cell(42)), 42);
    }

    #[test]
    fn aaddr_aligned() {
        let a = AAddr::try_from(2 * Cell::SIZE).unwrap();
        assert_eq!(Cell::from(a), Cell(2 * Cell::SIZE));
    }

    #[test]
    fn aaddr_misaligned() {
        let bad = Cell::SIZE + 1;
        assert_eq!(AAddr::try_from(bad), Err(VmError::AddressMisaligned(bad)));
    }

    #[test]
    fn aaddr_from_cell() {
        assert!(AAddr::try_from(Cell(Cell::SIZE)).is_ok());
        assert_eq!(AAddr::try_from(Cell(1)), Err(VmError::AddressMisaligned(1)));
    }
}

//! Forth data types.
use crate::vm::{VmError, VmResult};

#[cfg(target_pointer_width = "64")]
type DoubleInner = u128;

#[cfg(target_pointer_width = "32")]
type DoubleInner = u64;

#[cfg(target_pointer_width = "16")]
type DoubleInner = u32;

/// A double-cell value (*ud*).
///
/// |Word size (bits)|Type|
/// |---|---|
/// |16|`u32`|
/// |32|`u64`|
/// |64|`u128`|
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct Double(pub DoubleInner);

impl Double {
    pub const MAX: Self = Self(DoubleInner::MAX);
}

impl From<usize> for Double {
    #[inline]
    fn from(u: usize) -> Self {
        Self(u as DoubleInner)
    }
}

impl From<u32> for Double {
    #[inline]
    fn from(u: u32) -> Self {
        Self(u as DoubleInner)
    }
}

impl From<(usize, usize)> for Double {
    #[inline]
    fn from((lo, hi): (usize, usize)) -> Self {
        Self((hi as DoubleInner) << usize::BITS | lo as DoubleInner)
    }
}

impl From<Double> for (usize, usize) {
    #[inline]
    fn from(d: Double) -> Self {
        (d.0 as usize, (d.0 >> usize::BITS) as usize)
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
        if u.is_multiple_of(size_of::<usize>()) {
            Ok(Self(u))
        } else {
            Err(VmError::AddressMisaligned(u))
        }
    }
}

impl From<AAddr> for usize {
    #[inline]
    fn from(a: AAddr) -> Self {
        a.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aaddr_aligned() {
        let a = AAddr::try_from(2 * size_of::<usize>()).unwrap();
        assert_eq!(usize::from(a), 2 * size_of::<usize>());
    }

    #[test]
    fn aaddr_misaligned() {
        let bad = size_of::<usize>() + 1;
        assert_eq!(AAddr::try_from(bad), Err(VmError::AddressMisaligned(bad)));
    }

    #[test]
    fn aaddr_from_usize() {
        assert!(AAddr::try_from(size_of::<usize>()).is_ok());
        assert_eq!(AAddr::try_from(1), Err(VmError::AddressMisaligned(1)));
    }
}

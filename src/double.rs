//! Forth double types, sized for the target system.

#[cfg(target_pointer_width = "64")]
type DoubleInner = u128;

#[cfg(target_pointer_width = "32")]
type DoubleInner = u64;

#[cfg(target_pointer_width = "16")]
type DoubleInner = u32;

#[cfg(target_pointer_width = "64")]
type SignedDoubleInner = i128;

#[cfg(target_pointer_width = "32")]
type SignedDoubleInner = i64;

#[cfg(target_pointer_width = "16")]
type SignedDoubleInner = i32;

/// A signed double-cell value (*d*).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct SignedDouble(pub SignedDoubleInner);

impl SignedDouble {
    pub const MAX: Self = Self(SignedDoubleInner::MAX);
}

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

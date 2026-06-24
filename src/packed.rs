use crate::error::KernelError;
use crate::vm::Op;
use crate::{Result, SIZE};

/// A packed instruction cell.
///
/// The least significant byte (`0`) holds the [`Op`]. The remaining bytes hold the payload, which depends on the op:
///
/// * An [`Op::Yield`] instruction contains the builtin index in the byte `1`, and the *xt* in the higher bytes (`2..`).
/// * All other ops contain the *xt* in the bytes after the least significant byte `1..`.
#[derive(Debug, Clone, Copy)]
pub struct PackedInstr(usize);

impl PackedInstr {
    pub const OP_MASK: usize = 0xff;

    pub fn new(op: Op, xt: usize, index: usize) -> Result<Self> {
        if xt >> (8 * Self::xt_bytes(op)) != 0 {
            return Err(KernelError::XtTooLarge(xt).into());
        }
        let u = match op {
            Op::Yield => (Op::Yield as usize) | (index << 8) | (xt << Self::xt_shift(op)),
            _ => (op as usize) | (xt << Self::xt_shift(op)),
        };
        Ok(Self(u))
    }

    /// The number of bytes the *xt* may occupy.
    const fn xt_bytes(op: Op) -> usize {
        match op {
            Op::Yield => SIZE - 2,
            _ => SIZE - 1,
        }
    }

    /// The bit offset of the *xt* within the cell.
    const fn xt_shift(op: Op) -> usize {
        match op {
            Op::Yield => 16,
            _ => 8,
        }
    }
}

impl From<PackedInstr> for usize {
    fn from(instr: PackedInstr) -> Self {
        instr.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::Error;

    const XT: usize = 0xab;
    const INDEX: usize = 0x07;

    #[test]
    fn new_primitive_packs_op_and_xt() {
        let instr = PackedInstr::new(Op::Add, XT, 0).unwrap();
        assert_eq!(usize::from(instr), (Op::Add as usize) | (XT << 8));
    }

    #[test]
    fn new_yield_packs_op_index_and_xt() {
        let instr = PackedInstr::new(Op::Yield, XT, INDEX).unwrap();
        assert_eq!(
            usize::from(instr),
            (Op::Yield as usize) | (INDEX << 8) | (XT << 16)
        );
    }

    #[test]
    fn new_rejects_oversized_primitive_xt() {
        let too_big = 1usize << (8 * (SIZE - 1));
        assert!(matches!(
            PackedInstr::new(Op::Add, too_big, 0),
            Err(Error::Kernel(KernelError::XtTooLarge(xt))) if xt == too_big
        ));
    }

    #[test]
    fn new_rejects_oversized_yield_xt() {
        // Yield reserves one fewer byte for the xt than a primitive does.
        let too_big = 1usize << (8 * (SIZE - 2));
        assert!(matches!(
            PackedInstr::new(Op::Yield, too_big, 0),
            Err(Error::Kernel(KernelError::XtTooLarge(xt))) if xt == too_big
        ));
    }
}

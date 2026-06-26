use crate::error::KernelError;
use crate::vm::{Op, VmResult};
use crate::{Result, SIZE};

/// A packed instruction cell.
///
/// The least significant byte (`0`) holds the [`Op`]. The remaining bytes hold the payload, which depends on the op:
///
/// * An [`Op::Yield`] instruction contains the builtin index in byte `1`, and the *xt* in the higher bytes (`2..`).
/// * All other ops contain the *xt* in the bytes after the least significant byte (`1..`).
///
/// The packed representation ensures all instructions with operands have the same width, and that
/// words like `see` can recover primitives' code addresses without a lookup table. This scheme
/// restricts `Op` primitives to the first 2<sup>24</sup> addresses (16777216), and builtins to the
/// first 2<sup>16</sup> (65566), minus the system's reserved addresses.
///
/// # Performance
///
/// The system only encodes instructions once, when they are defined. However it must decode them
/// in the innermost loop of the VM ([`Vm::run`](crate::vm::Vm::run)).
///
/// An earlier revision decoded `op`, `xt`, and `operand` into struct fields. This negatively
/// impacted performance for two reasons. First, most instructions do not need the packed data at
/// execution time, so decoding the entire instruction cost extra work. More importantly, it
/// changed how the compiler passed the instruction to `Vm::step`. As a `usize` newtype, it likely
/// ends up in a register. As decoded struct fields, the generated code passed the instruction to
/// [`Vm::step`](crate::vm::Vm::step) as a pointer to a set of values on the stack.
///
/// `Vm::step` decodes instructions lazily instead. Nearly every operation only requires the
/// opcode byte. [`Op::Yield`] decodes the packed index in its own branch.
#[derive(Debug, Clone, Copy)]
pub struct PackedInstr(usize);

impl PackedInstr {
    const OP_MASK: usize = 0xff;

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

    // Inline. The hottest path of the system decodes opcodes.
    #[inline(always)]
    // TODO: The `VmResult` return type smells. All the methods should use the same error type.
    pub fn op(&self) -> VmResult<Op> {
        Op::try_from(self.0 & Self::OP_MASK)
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

impl From<usize> for PackedInstr {
    fn from(u: usize) -> Self {
        PackedInstr(u)
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

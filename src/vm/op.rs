use super::{VmError, VmResult};

macro_rules! ops {
    ($($(#[$attr:meta])* $name:ident = $val:literal),+ $(,)?) => {
        /// An instruction.
        #[repr(usize)]
        #[derive(Debug, Copy, Clone, PartialEq, Eq)]
        pub enum Op {
            $($(#[$attr])* $name = $val,)+
        }

        impl TryFrom<usize> for Op {
            type Error = VmError;

            fn try_from(u: usize) -> VmResult<Self> {
                match u {
                    $($val => Ok(Op::$name),)+
                    _ => Err(VmError::InvalidOpCode(u as u8)),
                }
            }
        }
    }
}

ops! {
    // Control flow
    /// Halt execution.
    Halt = 0x00,

    /// Yield execution to the host.
    Yield = 0x01,

    /// Exit from the current nested definition.
    ///
    /// ```text
    /// exit ( -- ) ( R: nest-sys -- )
    /// ```
    Exit = 0x02,

    // Literals
    /// Place a literal value on the data stack.
    ///
    /// ```text
    /// (lit) ( -- x )
    /// ```
    Lit = 0x04,

    /// Push inline string address and length, skip over string data.
    ///
    /// ```text
    /// (s") ( -- c-addr u )
    /// ```
    Str = 0x05,

    // Branches
    /// Unconditional jump. Jump to IP.
    ///
    /// ```text
    /// (jmp) ( -- )
    /// ```
    Jmp = 0x06,

    /// Conditional jump. Jump to IP if the value on the top of the data stack is 0.
    ///
    /// ```text
    /// (jmpz) ( flag -- )
    /// ```
    JmpZ = 0x07,

    // Loop
    /// Save the loop state to the return stack.
    ///
    /// ```text
    /// (do) ( limit index -- ) ( R: -- limit index' )
    /// ```
    Do = 0x08,

    /// Branch if the top two stack items are equal, otherwise set up a loop state.
    ///
    /// ```text
    /// (?do) ( limit index -- ) ( R: limit index' | )
    /// ```
    QDo = 0x09,

    /// Perform one iteration of a loop.
    ///
    /// ```text
    /// (+loop) ( step -- ) ( R: -- limit index' )
    /// ```
    PlusLoop = 0x0A,

    /// Reset the current loop state.
    ///
    /// ```text
    /// (unloop) ( -- ) ( R: limit index' -- )
    /// ```
    Unloop = 0x0B,

    /// Push the current loop index to the data stack.
    ///
    /// ```text
    /// i ( -- n )
    /// ```
    I = 0x0C,

    /// Push the outer loop index to the data stack.
    ///
    /// ```text
    /// j ( -- n )
    /// ```
    J = 0x0D,

    // Data stack
    /// Drop the value at the top of the data stack.
    ///
    /// ```text
    /// drop ( x -- )
    /// ```
    Drop = 0x0E,

    /// Swap the top two items on the data stack.
    ///
    /// ```text
    /// swap ( x1 x2 -- x2 x1 )
    /// ```
    Swap = 0x0F,

    /// Duplicate the value at the top of the data stack.
    ///
    /// ```text
    /// dup ( x -- x x )
    /// ```
    Dup = 0x14,

    /// Push the address of the top of the data stack before `SpFetch` executes.
    ///
    /// ```text
    /// (sp@) ( -- a-addr )
    /// ```
    SpFetch = 0x10,

    /// Set the data stack pointer.
    ///
    /// ```text
    /// (sp!) ( a-addr -- )
    /// ```
    SpStore = 0x11,

    // Return stack
    /// Move the value at the top of the data stack to the return stack.
    ///
    /// ```text
    /// >r ( x -- ) ( R: -- x )
    /// ```
    ToR = 0x12,

    /// Move the value at the top of the return stack to the data stack.
    ///
    /// ```text
    /// r> ( -- x ) ( R: x -- )
    /// ```
    RFrom = 0x13,

    /// Push the return stack pointer to the data stack.
    ///
    /// ```text
    /// (rp@) ( -- a-addr )
    /// ```
    RpFetch = 0x15,

    /// Set the return stack pointer.
    ///
    /// ```text
    /// (rp!) ( a-addr -- )
    /// ```
    RpStore = 0x16,

    // Memory
    /// Fetch a cell from memory.
    ///
    /// ```text
    /// @ ( a-addr -- x )
    /// ```
    Fetch = 0x17,

    /// Store a cell in memory.
    ///
    /// ```text
    /// ! ( x a-addr -- )
    /// ```
    Store = 0x18,

    /// Fetch a single character (byte) from memory.
    ///
    /// ```text
    /// c@ ( c-addr -- char )
    /// ```
    CFetch = 0x19,

    /// Store a single character (byte) in memory.
    ///
    /// ```text
    /// c! ( char c-addr -- )
    /// ```
    CStore = 0x1A,

    // Arithmetic
    /// Add the two two values on the data stack.
    ///
    /// ```text
    /// + ( n1 n2 -- n3 )
    /// ```
    Add = 0x1B,

    /// Multiply the top two items on the data stack.
    ///
    /// ```text
    /// * ( u1 u2 -- ud )
    /// ```
    UmMul = 0x1C,

    /// Fundamental division operation.
    ///
    /// Divide the double `ud` by `u1`, pushing the quotion `u3` and the remainder `u2`.
    ///
    /// ```text
    /// um/mod (ud u1 -- u2 u3)
    /// ```
    UmDivMod = 0x1D,

    // Bitwise
    /// Logical NAND.
    ///
    /// ```text
    /// (nand) ( x1 x2 -- x3 )
    /// ```
    ///
    /// | A | B | NAND |
    /// |:-:|:-:|:----:|
    /// | 0 | 0 | 1    |
    /// | 0 | 1 | 1    |
    /// | 1 | 0 | 1    |
    /// | 1 | 1 | 0    |
    ///
    /// NAND is functionally complete. Any Boolean expression can be expressed using NAND.
    Nand = 0x1E,

    /// Shift the next value on the data stack left by the value on the top of the data stack.
    ///
    /// ```text
    /// lshift ( x u -- x )
    /// ```
    LShift = 0x1F,

    /// Shift the next value on the data stack right by the value on the top of the data stack.
    ///
    /// ```text
    /// rshift ( x u -- x )
    /// ```
    RShift = 0x20,

    // Comparison
    /// Push `TRUE` ([`usize::MAX`]) to the data stack if the value at the top of the data stack is
    /// less than 0, `FALSE` (0) otherwise.
    ///
    /// ```text
    /// 0< ( x -- flag )
    /// ```
    LtZ = 0x21,

    /// Push [`TRUE`] ([`usize::MAX`]) to the data stack if the value at the top of the stack is
    /// equal to 0, `FALSE` (0) otherwise.
    ///
    /// ```text
    /// 0= ( x -- flag )
    /// ```
    EqZ = 0x22,

    // TODO: Renumber.
    /// Execute a create/does> definition.
    ///
    /// ```text
    /// (docreate) ( -- a-addr )
    /// ```
    DoCreate = 0x23,

    Execute = 0x24,

    Call = 0x25,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn try_from_valid() {
        assert_eq!(Op::try_from(0x00).unwrap(), Op::Halt);
        assert_eq!(Op::try_from(0x01).unwrap(), Op::Yield);
        assert_eq!(Op::try_from(0x23).unwrap(), Op::DoCreate);
    }

    #[test]
    fn try_from_invalid() {
        assert_eq!(Op::try_from(0xfe), Err(VmError::InvalidOpCode(0xfe)));
    }
}

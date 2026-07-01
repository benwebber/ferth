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
    ///
    /// ```text
    /// (halt) ( -- )
    /// ```
    Halt = 0x00,

    /// Yield execution to the host.
    Yield = 0x01,

    /// Call the inline XT.
    ///
    /// ```text
    /// (call) ( -- )
    /// ```
    Call = 0x02,

    /// Pop an XT from the data stock and then execute it like `Call`.
    ///
    /// ```text
    /// execute ( xt -- )
    /// ```
    Execute = 0x03,

    /// Execute a create/does> definition.
    ///
    /// ```text
    /// (docreate) ( -- a-addr )
    /// ```
    DoCreate = 0x04,

    /// Exit from the current nested definition.
    ///
    /// ```text
    /// exit ( -- ) ( R: nest-sys -- )
    /// ```
    Exit = 0x05,

    // Literals
    /// Place a literal value on the data stack.
    ///
    /// ```text
    /// (lit) ( -- x )
    /// ```
    Lit = 0x06,

    /// Push inline string address and length, skip over string data.
    ///
    /// ```text
    /// (s") ( -- c-addr u )
    /// ```
    Str = 0x07,

    // Branches
    /// Unconditional jump. Jump to the immediate address.
    ///
    /// ```text
    /// (jmp) ( -- )
    /// ```
    Jmp = 0x08,

    /// Conditional jump. Jump to IP if the value on the top of the data stack is 0.
    ///
    /// ```text
    /// (jmpz) ( flag -- )
    /// ```
    JmpZ = 0x09,

    // Loop
    /// Save the loop state to the return stack.
    ///
    /// ```text
    /// (do) ( limit index -- ) ( R: -- limit index' )
    /// ```
    Do = 0x0a,

    /// Branch if the top two stack items are equal, otherwise set up a loop state.
    ///
    /// ```text
    /// (?do) ( limit index -- ) ( R: | limit index' )
    /// ```
    QDo = 0x0b,

    /// Perform one iteration of a loop.
    ///
    /// ```text
    /// (+loop) ( step -- ) ( R: -- limit index' )
    /// ```
    PlusLoop = 0x0c,

    /// Reset the current loop state.
    ///
    /// ```text
    /// (unloop) ( -- ) ( R: limit index' -- )
    /// ```
    Unloop = 0x0d,

    /// Push the current loop index to the data stack.
    ///
    /// ```text
    /// i ( -- n )
    /// ```
    I = 0x0e,

    /// Push the outer loop index to the data stack.
    ///
    /// ```text
    /// j ( -- n )
    /// ```
    J = 0x0f,

    // Data stack
    /// Drop the value at the top of the data stack.
    ///
    /// ```text
    /// drop ( x -- )
    /// ```
    Drop = 0x10,

    /// Swap the top two items on the data stack.
    ///
    /// ```text
    /// swap ( x1 x2 -- x2 x1 )
    /// ```
    Swap = 0x11,

    /// Duplicate the value at the top of the data stack.
    ///
    /// ```text
    /// dup ( x -- x x )
    /// ```
    Dup = 0x12,

    /// Push the address of the top of the data stack before `SpFetch` executes.
    ///
    /// ```text
    /// (sp@) ( -- a-addr )
    /// ```
    SpFetch = 0x13,

    /// Set the data stack pointer.
    ///
    /// ```text
    /// (sp!) ( a-addr -- )
    /// ```
    SpStore = 0x14,

    // Return stack
    /// Move the value at the top of the data stack to the return stack.
    ///
    /// ```text
    /// >r ( x -- ) ( R: -- x )
    /// ```
    ToR = 0x15,

    /// Move the value at the top of the return stack to the data stack.
    ///
    /// ```text
    /// r> ( -- x ) ( R: x -- )
    /// ```
    RFrom = 0x16,

    /// Push the return stack pointer to the data stack.
    ///
    /// ```text
    /// (rp@) ( -- a-addr )
    /// ```
    RpFetch = 0x17,

    /// Set the return stack pointer.
    ///
    /// ```text
    /// (rp!) ( a-addr -- )
    /// ```
    RpStore = 0x18,

    // Memory
    /// Fetch a cell from memory.
    ///
    /// ```text
    /// @ ( a-addr -- x )
    /// ```
    Fetch = 0x19,

    /// Store a cell in memory.
    ///
    /// ```text
    /// ! ( x a-addr -- )
    /// ```
    Store = 0x1a,

    /// Fetch a single character (byte) from memory.
    ///
    /// ```text
    /// c@ ( c-addr -- char )
    /// ```
    CFetch = 0x1b,

    /// Store a single character (byte) in memory.
    ///
    /// ```text
    /// c! ( char c-addr -- )
    /// ```
    CStore = 0x1c,

    // Arithmetic
    /// Add the top two values on the data stack.
    ///
    /// ```text
    /// + ( n1 n2 -- n3 )
    /// ```
    Add = 0x1d,

    /// Multiply the top two items on the data stack.
    ///
    /// ```text
    /// um* ( u1 u2 -- ud )
    /// ```
    UmMul = 0x1e,

    /// Fundamental division operation.
    ///
    /// Divide the double `ud` by `u1`, pushing the quotient `u3` and the remainder `u2`.
    ///
    /// ```text
    /// um/mod (ud u1 -- u2 u3)
    /// ```
    UmDivMod = 0x1f,

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
    Nand = 0x20,

    /// Shift the next value on the data stack left by the value on the top of the data stack.
    ///
    /// ```text
    /// lshift ( x u -- x )
    /// ```
    LShift = 0x21,

    /// Shift the next value on the data stack right by the value on the top of the data stack.
    ///
    /// ```text
    /// rshift ( x u -- x )
    /// ```
    RShift = 0x22,

    // Comparison
    /// Push `TRUE` ([`usize::MAX`]) to the data stack if the value at the top of the data stack is
    /// less than 0, `FALSE` (0) otherwise.
    ///
    /// ```text
    /// 0< ( x -- flag )
    /// ```
    LtZ = 0x23,

    /// Push [`TRUE`][`crate::TRUE`] ([`usize::MAX`]) to the data stack if the value at the top of the stack is
    /// equal to 0, `FALSE` (0) otherwise.
    ///
    /// ```text
    /// 0= ( x -- flag )
    /// ```
    EqZ = 0x24,

    /// Parse a token from the input buffer, delimited by `char`.
    ///
    /// ```text
    /// (parse) ( char source-addr source-len pos -- c-addr u pos' )
    /// ```
    Parse = 0x25,

    /// Attempt to convert a string to a number.
    ///
    /// ```text
    /// (number) ( c-addr u base -- n 1 | c-addr u 0 )
    /// ```
    Number = 0x26,

    /// Accumulate digits from a string into a double-cell number.
    ///
    /// ```text
    /// (>number) ( lo hi c-addr u base -- lo' hi' c-addr' u' )
    /// ```
    ToNumber = 0x27,

    /// Compile a call to *xt* to the current definition.
    ///
    /// ```text
    /// (compile,) ( xt here -- here' )
    /// ```
    CompileComma = 0x28,

    /// Decode the packed instruction at *ip*.
    ///
    /// ```text
    /// (decode) ( ip -- op operand next )
    /// ```
    Decode = 0x29,

    ParseEscaped = 0x2a,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn try_from_valid() {
        assert_eq!(Op::try_from(0x00).unwrap(), Op::Halt);
        assert_eq!(Op::try_from(0x01).unwrap(), Op::Yield);
        assert_eq!(Op::try_from(0x04).unwrap(), Op::DoCreate);
    }

    #[test]
    fn try_from_invalid() {
        assert_eq!(Op::try_from(0xfe), Err(VmError::InvalidOpCode(0xfe)));
    }
}

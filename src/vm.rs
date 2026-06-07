//! The inner interpreter.
use core::mem::size_of;

use crate::data::{Data, Mem};
use crate::{Error, Result};

macro_rules! ops {
    ($($(#[$attr:meta])* $name:ident = $val:literal),+ $(,)?) => {
        /// An instruction.
        #[repr(usize)]
        #[derive(Debug, Copy, Clone, PartialEq, Eq)]
        pub enum Op {
            $($(#[$attr])* $name = $val,)+
        }

        impl Op {
            fn from_usize(u: usize) -> Result<Self> {
                match u {
                    $($val => Ok(Op::$name),)+
                    _ => Err(Error::InvalidOpCode(u as u8)),
                }
            }
        }
    }
}

ops! {
    /// Halt execution.
    Halt = 0x00,

    /// Yield execution to the host.
    Yield = 0x0F,

    /// Exit from the current nested definition.
    ///
    /// ```text
    /// exit ( -- ) ( R: nest-sys -- )
    /// ```
    Exit = 0x01,

    /// Execute a colon definition.
    DoCol = 0x02,

    /// Place a literal value on the data stack.
    ///
    /// ```text
    /// (lit) ( -- x )
    /// ```
    Lit = 0x03,

    /// Conditional jump. Jump to IP if the value on the top of the data stack is 0.
    ///
    /// ```text
    /// (jmpz) ( flag -- )
    /// ```
    JmpZ = 0x04,

    /// Fetch a cell from memory.
    ///
    /// ```text
    /// @ ( a-addr -- x )
    /// ```
    Fetch = 0x05,

    /// Store a cell in memory.
    ///
    /// ```text
    /// ! ( x a-addr -- )
    /// ```
    Store = 0x06,

    /// Add the two two values on the data stack.
    ///
    /// ```text
    /// + ( n1 n2 -- n3 )
    /// ```
    Add = 0x07,

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
    Nand = 0x08,

    /// Push `true` ([`usize::MAX`]) to the data stack if the value at the top of the data stack is
    /// less than 0, `false` (0) otherwise.
    ///
    /// ```text
    /// 0< ( x -- flag )
    /// ```
    LtZ = 0x09,

    /// Drop the value at the top of the data stack.
    ///
    /// ```text
    /// drop ( x -- )
    /// ```
    Drop = 0x0B,

    /// Swap the top two items on the data stack.
    ///
    /// ```text
    /// swap ( x1 x2 -- x2 x1 )
    /// ```
    Swap = 0x0C,

    /// Move the value at the top of the return stack to the data stack.
    ///
    /// ```text
    /// r> ( -- x ) ( R: x -- )
    /// ```
    RFrom = 0x0D,

    /// Move the value at the top of the data stack to the return stack.
    ///
    /// ```text
    /// >r ( x -- ) ( R: -- x )
    /// ```
    ToR = 0x0E,

    /// Multiply the top two items on the data stack.
    ///
    /// ```text
    /// * ( u1 u2 -- ud )
    /// ```
    UmMul = 0x12,

    /// Copy the value at the top of the return stack to the data stack.
    ///
    /// ```text
    /// r@ ( -- x ) ( R: x -- x )
    /// ```
    RFetch = 0x13,

    /// Fetch a single character (byte) from memory.
    ///
    /// ```text
    /// c@ ( c-addr -- char )
    /// ```
    CFetch = 0x14,

    /// Store a single character (byte) in memory.
    ///
    /// ```text
    /// c! ( char c-addr -- )
    /// ```
    CStore = 0x15,

    /// Push `true` ([`usize::MAX`]) to the data stack if the value at the top of the stack is
    /// equal to 0, `false` (0) otherwise.
    ///
    /// ```text
    /// 0= ( x -- flag )
    /// ```
    EqZ = 0x16,

    /// Unconditional jump. Jump to IP.
    ///
    /// ```text
    /// (jmp) ( -- )
    /// ```
    Jmp = 0x17,

    /// Save the loop state to the return stack.
    ///
    /// ```text
    /// (do) ( limit index -- ) ( R: -- limit index' )
    /// ```
    Do = 0x19,

    /// Perform one iteration of a loop.
    ///
    /// ```text
    /// (+loop) ( step -- ) ( R: -- limit index' )
    /// ```
    PlusLoop = 0x30,

    /// Reset the current loop state.
    ///
    /// ```text
    /// (unloop) ( -- ) ( R: limit index' -- )
    /// ```
    Unloop = 0x31,

    /// Push the current loop index to the data stack.
    ///
    /// ```text
    /// i ( -- n )
    /// ```
    I = 0x32,

    /// Push the outer loop index to the data stack.
    ///
    /// ```text
    /// j ( -- n )
    /// ```
    J = 0x33,

    /// Push inline string address and length, skip over string data.
    ///
    /// ```text
    /// (s") ( -- c-addr u )
    /// ```
    Str = 0x1c,

    /// Push the data field address of the current word.
    ///
    /// ```text
    /// (dovar) ( -- a-addr )
    /// ```
    DoVar = 0x1d,

    /// Fundamental division operation.
    ///
    /// Divide the double `ud` by `u1`, pushing the quotion `u3` and the remainder `u2`.
    ///
    /// ```text
    /// um/mod (ud u1 -- u2 u3)
    /// ```
    UmDivMod = 0x1e,

    /// Shift the next value on the data stack left by the value on the top of the data stack.
    ///
    /// ```text
    /// lshift ( x u -- x )
    /// ```
    LShift = 0x1f,

    /// Shift the next value on the data stack right by the value on the top of the data stack.
    ///
    /// ```text
    /// rshift ( x u -- x )
    /// ```
    RShift = 0x20,

    /// Push the address of the top of the data stack before `SpFetch` executes.
    ///
    /// ```text
    /// (sp@) ( -- a-addr )
    /// ```
    SpFetch = 0x21,
}

/// A token used to continue exection after yielding.
///
/// The outer interpreter must return this to the `Vm` in [`Vm::resume`].  This type intentionally
/// does not implement [`Copy`] and [`Clone`]. The caller must use the exact token yielded by the VM
/// (i.e., move it).
#[derive(Debug, PartialEq, Eq)]
pub struct YieldToken {
    /// Return to this address after executing.
    ///
    /// This is private so that the caller can never manipulate the IP.
    ip: usize,
    /// Execute the builtin with this index.
    pub index: usize,
}

/// A stop condition.
#[derive(Debug, PartialEq, Eq)]
pub enum Stop {
    /// Halt execution.
    Halt,
    /// Yield execution to the outer interpreter.
    Yield(YieldToken),
}

/// The inner interpreter.
///
/// The inner interpreter executes a sequence of threaded instructions in memory. It provides a
/// number of primitive stack and memory operations, most which correspond to standard Forth words.
///
/// The inner interpreter does not own the system memory. It reserves a portion of memory at the
/// bottom of the address space for its data stack and return stack. It maintains execution state
/// with a set of internal registers, inaccessible to the host.
pub struct Vm {
    /// The instruction pointer (IP).
    ///
    /// Points to a cell containing the next instruction.
    ip: usize,
    /// The word register (W).
    ///
    /// Contains the address of the currently executing word.
    w: usize,
    /// The data stack pointer.
    ///
    /// The data stack pointer (SP) and return stack pointers (RP) are registers so that stack access
    /// is infallible. If they were stored in main memory, accessing them would require handling the
    /// [`Data::read_cell`] [`Result`].
    sp: usize,
    /// The return stack pointer.
    rp: usize,
    /// The length of the data stack.
    ds_len: usize,
    /// The length of the return stack.
    rs_len: usize,
}

impl Vm {
    /// The size of a cell in bytes.
    pub const SIZE: usize = size_of::<usize>();
    /// The address of the bottom of the data stack.
    const DS_ADDR: usize = 0;

    pub fn new(ds_len: usize, rs_len: usize) -> Self {
        Self {
            ip: 0,
            w: 0,
            sp: 0,
            rp: ds_len * Self::SIZE,
            ds_len,
            rs_len,
        }
    }

    /// Execute instructions until a stop condition.
    pub fn run<M: Mem>(&mut self, data: &mut Data<M>) -> Result<Stop> {
        loop {
            if self.ip == 0 {
                // Sentinel address, always outside range of data space.
                return Ok(Stop::Halt);
            }
            self.w = data.read_cell(self.ip)?;
            self.ip += Self::SIZE;
            if let Some(stop) = self.dispatch(data)? {
                return Ok(stop);
            };
        }
    }

    /// Resume execution after yielding.
    pub fn resume<M: Mem>(&mut self, data: &mut Data<M>, token: YieldToken) -> Result<Stop> {
        self.ip = token.ip;
        self.run(data)
    }

    /// Execute the instruction at `addr`.
    pub fn call<M: Mem>(&mut self, data: &mut Data<M>, addr: usize) -> Result<Stop> {
        self.ip = 0;
        self.w = addr;
        match self.dispatch(data)? {
            Some(stop) => Ok(stop),
            None => self.run(data),
        }
    }

    /// Reset stacks.
    pub fn reset(&mut self) {
        self.sp = 0;
        self.rp = self.rs_addr();
    }

    /// Return the number of bytes reserved in memory by the VM's internal state (e.g. stacks).
    pub fn reserved(&self) -> usize {
        (self.ds_len + self.rs_len + 1) * Self::SIZE
    }

    pub fn stack<'a, M: Mem>(&self, data: &'a Data<M>) -> impl Iterator<Item = usize> + 'a {
        (Self::DS_ADDR..self.sp)
            .step_by(Self::SIZE)
            .map(move |addr| {
                data.read_cell(addr)
                    .expect("unreachable: stack cell within validated range")
            })
    }

    /// Return the address of the bottom of the return stack.
    fn rs_addr(&self) -> usize {
        self.ds_len * Self::SIZE
    }

    /// Push a cell onto the data stack.
    pub fn push<M: Mem>(&mut self, data: &mut Data<M>, x: usize) -> Result<()> {
        if self.sp >= Self::DS_ADDR + self.ds_len * Self::SIZE {
            return Err(Error::StackOverflow);
        }
        data.write_cell(self.sp, x)?;
        self.sp += Self::SIZE;
        Ok(())
    }

    /// Pop a cell from the data stack.
    pub fn pop<M: Mem>(&mut self, data: &mut Data<M>) -> Result<usize> {
        if self.sp == Self::DS_ADDR {
            return Err(Error::StackUnderflow);
        }
        self.sp -= Self::SIZE;
        data.read_cell(self.sp)
    }

    /// Push a cell onto the return stack.
    fn rpush<M: Mem>(&mut self, data: &mut Data<M>, x: usize) -> Result<()> {
        if self.rp >= self.rs_addr() + self.rs_len * Self::SIZE {
            return Err(Error::ReturnStackOverflow);
        }
        data.write_cell(self.rp, x)?;
        self.rp += Self::SIZE;
        Ok(())
    }

    /// Pop a cell from the return stack.
    fn rpop<M: Mem>(&mut self, data: &mut Data<M>) -> Result<usize> {
        if self.rp == self.rs_addr() {
            return Err(Error::ReturnStackUnderflow);
        }
        self.rp -= Self::SIZE;
        data.read_cell(self.rp)
    }

    /// Execute a single [`Op`] code.
    fn execute<M: Mem>(&mut self, data: &mut Data<M>, op: Op) -> Result<Option<Stop>> {
        match op {
            Op::Halt => {
                return Ok(Some(Stop::Halt));
            }
            Op::Exit => {
                self.ip = self.rpop(data)?;
            }
            Op::DoCol => {
                self.rpush(data, self.ip)?;
                self.ip = self.w + Self::SIZE;
            }
            Op::Lit => {
                let val = data.read_cell(self.ip)?;
                self.push(data, val)?;
                self.ip += Self::SIZE;
            }
            Op::Jmp => {
                let target = data.read_cell(self.ip)?;
                self.ip = target; // TODO: validate target
            }
            Op::JmpZ => {
                let target = data.read_cell(self.ip)?;
                if self.pop(data)? == 0 {
                    self.ip = target; // TODO: validate target
                } else {
                    self.ip += Self::SIZE;
                }
            }
            Op::Fetch => {
                let addr = self.pop(data)?;
                let val = data.read_cell(addr)?;
                self.push(data, val)?;
            }
            Op::Store => {
                let addr = self.pop(data)?;
                let val = self.pop(data)?;
                data.write_cell(addr, val)?;
            }
            Op::CFetch => {
                let addr = self.pop(data)?;
                let u = data.read_char(addr)?;
                self.push(data, u as usize)?;
            }
            Op::CStore => {
                let addr = self.pop(data)?;
                let c = self.pop(data)? as u8;
                data.write_char(addr, c)?;
            }
            Op::Add => {
                let b = self.pop(data)?;
                let a = self.pop(data)?;
                self.push(data, a.wrapping_add(b))?;
            }
            Op::UmMul => {
                let u1 = self.pop(data)? as u128;
                let u2 = self.pop(data)? as u128;
                let ud = u1 * u2;
                let ud_lo = ud as usize;
                let ud_hi = (ud >> (8 * Self::SIZE)) as usize;
                self.push(data, ud_lo)?;
                self.push(data, ud_hi)?;
            }
            Op::Nand => {
                let b = self.pop(data)?;
                let a = self.pop(data)?;
                self.push(data, !(a & b))?;
            }
            Op::LtZ => {
                let a = self.pop(data)?;
                self.push(data, if (a as isize) < 0 { usize::MAX } else { 0 })?;
            }
            Op::EqZ => {
                let n = self.pop(data)? as isize;
                self.push(data, if n == 0 { usize::MAX } else { 0 })?;
            }
            Op::Drop => {
                self.pop(data)?;
            }
            Op::Swap => {
                let b = self.pop(data)?;
                let a = self.pop(data)?;
                self.push(data, b)?;
                self.push(data, a)?;
            }
            Op::RFrom => {
                let x = self.rpop(data)?;
                self.push(data, x)?;
            }
            Op::ToR => {
                let x = self.pop(data)?;
                self.rpush(data, x)?;
            }
            Op::RFetch => {
                if self.rp == self.rs_addr() {
                    return Err(Error::StackUnderflow);
                }
                let x = data.read_cell(self.rp - Self::SIZE)?;
                self.push(data, x)?;
            }
            Op::Yield => {
                // Unreachable, but don't panic. `dispatch()` intercepts `Yield` first.
                return Err(Error::InvalidOpCode(op as u8));
            }
            Op::Do => {
                let index = self.pop(data)?;
                let limit = self.pop(data)?;
                self.rpush(data, limit)?;
                // index' = index - limit + isize::MIN
                let fudged = index.wrapping_sub(limit).wrapping_add(isize::MIN as usize);
                self.rpush(data, fudged)?;
            }
            Op::PlusLoop => {
                let step = self.pop(data)? as isize;
                let fudged = data.read_cell(self.rp - Self::SIZE)? as isize;
                let (next, overflow) = fudged.overflowing_add(step);
                if overflow {
                    self.rpop(data)?;
                    self.rpop(data)?;
                    self.ip += Self::SIZE;
                } else {
                    data.write_cell(self.rp - Self::SIZE, next as usize)?;
                    self.ip = data.read_cell(self.ip)?;
                }
            }
            Op::I => {
                let fudged = data.read_cell(self.rp - Self::SIZE)?;
                let limit = data.read_cell(self.rp - 2 * Self::SIZE)?;
                self.push(
                    data,
                    fudged.wrapping_sub(isize::MIN as usize).wrapping_add(limit),
                )?;
            }
            Op::J => {
                let fudged = data.read_cell(self.rp - 3 * Self::SIZE)?;
                let limit = data.read_cell(self.rp - 4 * Self::SIZE)?;
                self.push(
                    data,
                    fudged.wrapping_sub(isize::MIN as usize).wrapping_add(limit),
                )?;
            }
            Op::Unloop => {
                self.rpop(data)?;
                self.rpop(data)?;
            }
            Op::Str => {
                let len = data.read_cell(self.ip)?;
                self.ip += Self::SIZE;
                self.push(data, self.ip)?;
                self.push(data, len)?;
                self.ip += (len + Self::SIZE - 1) & !(Self::SIZE - 1);
            }
            Op::DoVar => {
                self.push(data, self.w + Self::SIZE)?;
            }
            Op::LShift => {
                let u = self.pop(data)?;
                let x = self.pop(data)?;
                self.push(data, x << u)?;
            }
            Op::RShift => {
                let u = self.pop(data)?;
                let x = self.pop(data)?;
                self.push(data, x >> u)?;
            }
            Op::UmDivMod => {
                let u1 = self.pop(data)? as u128;
                let ud_hi = self.pop(data)? as u128;
                let ud_lo = self.pop(data)? as u128;
                if u1 == 0 {
                    return Err(Error::DivisionByZero);
                }
                let ud = (ud_hi << (8 * Self::SIZE)) | ud_lo;
                self.push(data, (ud % u1) as usize)?;
                self.push(data, (ud / u1) as usize)?;
            }
            Op::SpFetch => {
                self.push(data, self.sp)?;
            }
        }
        Ok(None)
    }

    /// Execute the code referenced by the W register.
    fn dispatch<M: Mem>(&mut self, data: &mut Data<M>) -> Result<Option<Stop>> {
        let op = Op::from_usize(data.read_cell(self.w)?)?;
        match op {
            Op::Yield => {
                let index = data.read_cell(self.w + Self::SIZE)?;
                let token = YieldToken { ip: self.ip, index };
                Ok(Some(Stop::Yield(token)))
            }
            _ => self.execute(data, op),
        }
    }
}

//! The inner interpreter.
use core::mem::size_of;

use crate::data::{Data, Mem};
use crate::types::Double;

mod error;
mod op;

pub use error::{VmError, VmResult};
pub use op::Op;

/// Read a cell. Forgo bounds and alignment checks when the `unsafe` feature is enabled.
macro_rules! maybe_read_cell_unchecked {
    ($data:expr, $addr:expr) => {{
        #[cfg(feature = "unsafe")]
        {
            Ok(unsafe { $data.read_cell_unchecked($addr) })
        }
        #[cfg(not(feature = "unsafe"))]
        {
            $data.read_cell($addr)
        }
    }};
}

/// Write a cell. Forgo bounds and alignment checks when the `unsafe` feature is enabled.
macro_rules! maybe_write_cell_unchecked {
    ($data:expr, $addr:expr, $val:expr) => {{
        #[cfg(feature = "unsafe")]
        {
            unsafe { $data.write_cell_unchecked($addr, $val) };
            Ok(())
        }
        #[cfg(not(feature = "unsafe"))]
        {
            $data.write_cell($addr, $val)
        }
    }};
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
///
/// The `unsafe` crate feature enables pointer access IP and stack pointer optimizations. They are
/// safe in practice because the interpreter controls access to and validates the pointer
/// addresses.
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
    pub const DS_ADDR: usize = 0;

    pub fn new(ds_len: usize, rs_len: usize) -> Self {
        assert!(Self::layout_ok(rs_len, rs_len), "stacks too small");
        Self {
            ip: 0,
            w: 0,
            sp: 0,
            rp: ds_len * Self::SIZE,
            ds_len,
            rs_len,
        }
    }

    pub const fn layout_ok(ds_len: usize, rs_len: usize) -> bool {
        (ds_len + rs_len + 1) * Self::SIZE > Op::MAX
    }

    /// Execute instructions until a stop condition.
    pub fn run<M: Mem>(&mut self, data: &mut Data<M>) -> VmResult<Stop> {
        loop {
            if self.ip == 0 {
                // Sentinel address, always outside range of data space.
                return Ok(Stop::Halt);
            }
            self.w = maybe_read_cell_unchecked!(data, self.ip)?;
            self.ip += Self::SIZE;
            if let Some(stop) = self.dispatch(data)? {
                return Ok(stop);
            };
        }
    }

    /// Resume execution after yielding.
    pub fn resume<M: Mem>(&mut self, data: &mut Data<M>, token: YieldToken) -> VmResult<Stop> {
        self.ip = token.ip;
        self.run(data)
    }

    /// Execute the instruction at `addr`.
    pub fn call<M: Mem>(&mut self, data: &mut Data<M>, addr: usize) -> VmResult<Stop> {
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
    pub fn rs_addr(&self) -> usize {
        self.ds_len * Self::SIZE
    }

    /// Push a cell onto the data stack.
    pub fn push<M: Mem>(&mut self, data: &mut Data<M>, x: usize) -> VmResult<()> {
        if self.sp >= Self::DS_ADDR + self.ds_len * Self::SIZE {
            return Err(VmError::StackOverflow);
        }
        maybe_write_cell_unchecked!(data, self.sp, x)?;
        self.sp += Self::SIZE;
        Ok(())
    }

    /// Pop a cell from the data stack.
    pub fn pop<M: Mem>(&mut self, data: &mut Data<M>) -> VmResult<usize> {
        if self.sp == Self::DS_ADDR {
            return Err(VmError::StackUnderflow);
        }
        self.sp -= Self::SIZE;
        maybe_read_cell_unchecked!(data, self.sp)
    }

    /// Push a cell onto the return stack.
    fn rpush<M: Mem>(&mut self, data: &mut Data<M>, x: usize) -> VmResult<()> {
        if self.rp >= self.rs_addr() + self.rs_len * Self::SIZE {
            return Err(VmError::ReturnStackOverflow);
        }
        maybe_write_cell_unchecked!(data, self.rp, x)?;
        self.rp += Self::SIZE;
        Ok(())
    }

    /// Pop a cell from the return stack.
    fn rpop<M: Mem>(&mut self, data: &mut Data<M>) -> VmResult<usize> {
        if self.rp == self.rs_addr() {
            return Err(VmError::ReturnStackUnderflow);
        }
        self.rp -= Self::SIZE;
        maybe_read_cell_unchecked!(data, self.rp)
    }

    /// Execute a single [`Op`] code.
    fn execute<M: Mem>(&mut self, data: &mut Data<M>, op: Op) -> VmResult<Option<Stop>> {
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
                let val = maybe_read_cell_unchecked!(data, self.ip)?;
                self.push(data, val)?;
                self.ip += Self::SIZE;
            }
            Op::Jmp => {
                let target = maybe_read_cell_unchecked!(data, self.ip)?;
                self.ip = target; // TODO: validate target
            }
            Op::JmpZ => {
                let target = maybe_read_cell_unchecked!(data, self.ip)?;
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
                let u1 = Double::from(self.pop(data)?);
                let u2 = Double::from(self.pop(data)?);
                let ud = Double(u1.0 * u2.0);
                let (lo, hi): (usize, usize) = ud.into();
                self.push(data, lo)?;
                self.push(data, hi)?;
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
                    return Err(VmError::ReturnStackUnderflow);
                }
                let x = maybe_read_cell_unchecked!(data, self.rp - Self::SIZE)?;
                self.push(data, x)?;
            }
            Op::Yield => {
                // Unreachable, but don't panic. `dispatch()` intercepts `Yield` first.
                return Err(VmError::InvalidOpCode(op as u8));
            }
            Op::DoCreate => {
                let does_addr = data.read_cell(self.w + Self::SIZE)?;
                self.push(data, self.w + 2 * Self::SIZE)?;
                if does_addr != 0 {
                    self.rpush(data, self.ip)?;
                    self.ip = does_addr;
                }
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
                let fudged = maybe_read_cell_unchecked!(data, self.rp - Self::SIZE)? as isize;
                let (next, overflow) = fudged.overflowing_add(step);
                if overflow {
                    self.rpop(data)?;
                    self.rpop(data)?;
                    self.ip += Self::SIZE;
                } else {
                    maybe_write_cell_unchecked!(data, self.rp - Self::SIZE, next as usize)?;
                    self.ip = maybe_read_cell_unchecked!(data, self.ip)?;
                }
            }
            Op::I => {
                let fudged = maybe_read_cell_unchecked!(data, self.rp - Self::SIZE)?;
                let limit = maybe_read_cell_unchecked!(data, self.rp - 2 * Self::SIZE)?;
                self.push(
                    data,
                    fudged.wrapping_sub(isize::MIN as usize).wrapping_add(limit),
                )?;
            }
            Op::J => {
                let fudged = maybe_read_cell_unchecked!(data, self.rp - 3 * Self::SIZE)?;
                let limit = maybe_read_cell_unchecked!(data, self.rp - 4 * Self::SIZE)?;
                self.push(
                    data,
                    fudged.wrapping_sub(isize::MIN as usize).wrapping_add(limit),
                )?;
            }
            Op::Unloop => {
                self.rpop(data)?;
                self.rpop(data)?;
            }
            Op::QDo => {
                let index = self.pop(data)?;
                let limit = self.pop(data)?;
                if index == limit {
                    // Jump.
                    self.ip = maybe_read_cell_unchecked!(data, self.ip)?;
                } else {
                    // Step over target.
                    self.ip += Self::SIZE;
                    self.rpush(data, limit)?;
                    // index' = index - limit + isize::MIN
                    let fudged = index.wrapping_sub(limit).wrapping_add(isize::MIN as usize);
                    self.rpush(data, fudged)?;
                }
            }
            Op::Str => {
                let len = maybe_read_cell_unchecked!(data, self.ip)?;
                self.ip += Self::SIZE;
                self.push(data, self.ip)?;
                self.push(data, len)?;
                self.ip += (len + Self::SIZE - 1) & !(Self::SIZE - 1);
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
                let u1 = self.pop(data)?;
                let ud_hi = self.pop(data)?;
                let ud_lo = self.pop(data)?;
                if u1 == 0 {
                    return Err(VmError::DivisionByZero);
                }
                let ud = Double::from((ud_lo, ud_hi));
                let u1 = Double::from(u1);
                self.push(data, (ud.0 % u1.0) as usize)?;
                self.push(data, (ud.0 / u1.0) as usize)?;
            }
            Op::SpFetch => {
                self.push(data, self.sp)?;
            }
            Op::SpStore => {
                let addr = self.pop(data)?;
                if addr > Self::DS_ADDR + self.ds_len * Self::SIZE {
                    return Err(VmError::AddressOutOfRange(addr));
                }
                self.sp = addr;
            }
            Op::RpFetch => {
                self.push(data, self.rp)?;
            }
            Op::RpStore => {
                let addr = self.pop(data)?;
                if addr < self.rs_addr() || addr > self.rs_addr() + self.rs_len * Self::SIZE {
                    return Err(VmError::AddressOutOfRange(addr));
                }
                self.rp = addr;
            }
        }
        Ok(None)
    }

    /// Execute the code referenced by the W register.
    fn dispatch<M: Mem>(&mut self, data: &mut Data<M>) -> VmResult<Option<Stop>> {
        let op = data.read_cell(self.w)?.try_into()?;
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

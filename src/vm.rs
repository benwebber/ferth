//! The inner interpreter.
use crate::data::{Data, Mem};
use crate::double::Double;
use crate::{FALSE, SIZE, TRUE};

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

macro_rules! unary {
    ($self:expr, |$x:ident| $body:expr) => {{
        if $self.sp == Self::DS_ADDR {
            return Err(VmError::StackUnderflow);
        }
        let $x = $self.tos;
        $self.tos = $body;
    }};
}

macro_rules! binary {
    ($self:expr, $data:expr, |$a:ident, $b:ident| $body:expr) => {{
        if $self.sp < (Self::DS_ADDR + 2 * SIZE) {
            return Err(VmError::StackUnderflow);
        }
        let $b = $self.tos;
        let $a = maybe_read_cell_unchecked!($data, $self.sp - 2 * SIZE)?;
        $self.tos = $body;
        $self.sp -= SIZE;
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
    /// The value on the top of the data stack.
    tos: usize,
    /// The length of the data stack.
    ds_len: usize,
    /// The length of the return stack.
    rs_len: usize,
    /// The maximum value of the stack pointer.
    ///
    /// Cached for performance.
    sp_max: usize,
    /// The maximum value of the return stack pointer.
    ///
    /// Cached for performance.
    rp_max: usize,
}

impl Vm {
    /// The address of the bottom of the data stack.
    ///
    /// Address 0x00 is a scratch cell. [`Vm::push`] spills the value in TOS to memory. [`Vm::pop`]
    /// reloads TOS from the same address. The scratch cell absorbs both operations, eliminating a
    /// bounds check in two hot paths.
    pub const DS_ADDR: usize = SIZE;

    pub fn new(ds_len: usize, rs_len: usize) -> Self {
        let sp_max = Self::DS_ADDR + ds_len * SIZE;
        let rp_max = sp_max + rs_len * SIZE;
        Self {
            ip: 0,
            w: 0,
            sp: Self::DS_ADDR,
            tos: 0,
            rp: sp_max,
            ds_len,
            rs_len,
            sp_max,
            rp_max,
        }
    }

    /// Execute instructions until a stop condition.
    pub fn run<M: Mem>(&mut self, data: &mut Data<M>) -> VmResult<Stop> {
        loop {
            if self.ip == 0 {
                // Sentinel address, always outside range of data space.
                return Ok(Stop::Halt);
            }
            self.w = maybe_read_cell_unchecked!(data, self.ip)?;
            self.ip += SIZE;
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
        self.sp = Self::DS_ADDR;
        self.rp = self.rs_addr();
    }

    /// Return the number of bytes reserved in memory by the VM's internal state (e.g. stacks).
    pub fn reserved(&self) -> usize {
        (self.ds_len + self.rs_len + 1) * SIZE
    }

    pub fn stack<'a, M: Mem>(&self, data: &'a Data<M>) -> impl Iterator<Item = usize> + 'a {
        let bottom = (Self::DS_ADDR..self.sp.saturating_sub(SIZE))
            .step_by(SIZE)
            .map(move |addr| {
                data.read_cell(addr)
                    .expect("unreachable: stack cell within validated range")
            });
        bottom.chain((self.sp != Self::DS_ADDR).then_some(self.tos))
    }

    /// Return the address of the bottom of the return stack.
    #[inline]
    pub fn rs_addr(&self) -> usize {
        self.sp_max
    }

    /// Push a cell onto the data stack.
    pub fn push<M: Mem>(&mut self, data: &mut Data<M>, x: usize) -> VmResult<()> {
        if self.sp >= self.sp_max {
            return Err(VmError::StackOverflow);
        }
        maybe_write_cell_unchecked!(data, self.sp - SIZE, self.tos)?;
        self.tos = x;
        self.sp += SIZE;
        Ok(())
    }

    /// Pop a cell from the data stack.
    pub fn pop<M: Mem>(&mut self, data: &mut Data<M>) -> VmResult<usize> {
        if self.sp == Self::DS_ADDR {
            return Err(VmError::StackUnderflow);
        }
        let x = self.tos;
        self.sp -= SIZE;
        self.tos = maybe_read_cell_unchecked!(data, self.sp - SIZE)?;
        Ok(x)
    }

    /// Push a cell onto the return stack.
    fn rpush<M: Mem>(&mut self, data: &mut Data<M>, x: usize) -> VmResult<()> {
        if self.rp >= self.rp_max {
            return Err(VmError::ReturnStackOverflow);
        }
        maybe_write_cell_unchecked!(data, self.rp, x)?;
        self.rp += SIZE;
        Ok(())
    }

    /// Pop a cell from the return stack.
    fn rpop<M: Mem>(&mut self, data: &mut Data<M>) -> VmResult<usize> {
        if self.rp == self.rs_addr() {
            return Err(VmError::ReturnStackUnderflow);
        }
        self.rp -= SIZE;
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
            Op::Execute => {
                self.w = self.pop(data)?;
                return self.dispatch(data);
            }
            Op::DoCol => {
                self.rpush(data, self.ip)?;
                self.ip = self.w + SIZE;
            }
            Op::Lit => {
                let val = maybe_read_cell_unchecked!(data, self.ip)?;
                self.push(data, val)?;
                self.ip += SIZE;
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
                    self.ip += SIZE;
                }
            }
            Op::Fetch => {
                if self.sp == Self::DS_ADDR {
                    return Err(VmError::StackUnderflow);
                }
                let addr = self.tos;
                self.tos = data.read_cell(addr)?;
            }
            Op::Store => {
                if self.sp < Self::DS_ADDR + 2 * SIZE {
                    return Err(VmError::StackUnderflow);
                }
                let addr = self.tos;
                let val = maybe_read_cell_unchecked!(data, self.sp - 2 * SIZE)?;
                self.sp -= 2 * SIZE;
                self.tos = maybe_read_cell_unchecked!(data, self.sp - SIZE)?;
                data.write_cell(addr, val)?;
            }
            Op::CFetch => {
                if self.sp == Self::DS_ADDR {
                    return Err(VmError::StackUnderflow);
                }
                let addr = self.tos;
                self.tos = data.read_char(addr)? as usize;
            }
            Op::CStore => {
                if self.sp < Self::DS_ADDR + 2 * SIZE {
                    return Err(VmError::StackUnderflow);
                }
                let addr = self.tos;
                let c = maybe_read_cell_unchecked!(data, self.sp - 2 * SIZE)? as u8;
                self.sp -= 2 * SIZE;
                self.tos = maybe_read_cell_unchecked!(data, self.sp - SIZE)?;
                data.write_char(addr, c)?;
            }
            Op::Add => {
                binary!(self, data, |a, b| a.wrapping_add(b));
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
                binary!(self, data, |a, b| !(a & b));
            }
            Op::LtZ => {
                unary!(self, |n| if (n as isize) < 0 { TRUE } else { FALSE });
            }
            Op::EqZ => {
                unary!(self, |n| if n == 0 { TRUE } else { FALSE });
            }
            Op::Drop => {
                self.pop(data)?;
            }
            Op::Swap => {
                if self.sp < (Self::DS_ADDR + 2 * SIZE) {
                    return Err(VmError::StackUnderflow);
                }
                let tos = self.tos;
                self.tos = maybe_read_cell_unchecked!(data, self.sp - 2 * SIZE)?;
                maybe_write_cell_unchecked!(data, self.sp - 2 * SIZE, tos)?;
            }
            Op::Dup => {
                if self.sp == Self::DS_ADDR {
                    return Err(VmError::StackUnderflow);
                }
                self.push(data, self.tos)?;
            }
            Op::RFrom => {
                let x = self.rpop(data)?;
                self.push(data, x)?;
            }
            Op::ToR => {
                let x = self.pop(data)?;
                self.rpush(data, x)?;
            }
            Op::Yield => {
                // Unreachable, but don't panic. `dispatch()` intercepts `Yield` first.
                return Err(VmError::InvalidOpCode(op as u8));
            }
            Op::DoCreate => {
                let does_addr = data.read_cell(self.w + SIZE)?;
                self.push(data, self.w + 2 * SIZE)?;
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
                let fudged = maybe_read_cell_unchecked!(data, self.rp - SIZE)? as isize;
                let (next, overflow) = fudged.overflowing_add(step);
                if overflow {
                    self.rpop(data)?;
                    self.rpop(data)?;
                    self.ip += SIZE;
                } else {
                    maybe_write_cell_unchecked!(data, self.rp - SIZE, next as usize)?;
                    self.ip = maybe_read_cell_unchecked!(data, self.ip)?;
                }
            }
            Op::I => {
                let fudged = maybe_read_cell_unchecked!(data, self.rp - SIZE)?;
                let limit = maybe_read_cell_unchecked!(data, self.rp - 2 * SIZE)?;
                self.push(
                    data,
                    fudged.wrapping_sub(isize::MIN as usize).wrapping_add(limit),
                )?;
            }
            Op::J => {
                let fudged = maybe_read_cell_unchecked!(data, self.rp - 3 * SIZE)?;
                let limit = maybe_read_cell_unchecked!(data, self.rp - 4 * SIZE)?;
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
                    self.ip += SIZE;
                    self.rpush(data, limit)?;
                    // index' = index - limit + isize::MIN
                    let fudged = index.wrapping_sub(limit).wrapping_add(isize::MIN as usize);
                    self.rpush(data, fudged)?;
                }
            }
            Op::Str => {
                let len = maybe_read_cell_unchecked!(data, self.ip)?;
                self.ip += SIZE;
                self.push(data, self.ip)?;
                self.push(data, len)?;
                self.ip += (len + SIZE - 1) & !(SIZE - 1);
            }
            Op::LShift => {
                binary!(self, data, |x, u| x << u);
            }
            Op::RShift => {
                binary!(self, data, |x, u| x >> u);
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
                if addr < Self::DS_ADDR || addr > self.sp_max {
                    return Err(VmError::AddressOutOfRange(addr));
                }
                self.sp = addr;
                self.tos = maybe_read_cell_unchecked!(data, self.sp - SIZE)?;
            }
            Op::RpFetch => {
                self.push(data, self.rp)?;
            }
            Op::RpStore => {
                let addr = self.pop(data)?;
                if addr < self.rs_addr() || addr > self.rp_max {
                    return Err(VmError::AddressOutOfRange(addr));
                }
                self.rp = addr;
            }
        }
        Ok(None)
    }

    /// Execute the code referenced by the W register.
    fn dispatch<M: Mem>(&mut self, data: &mut Data<M>) -> VmResult<Option<Stop>> {
        let op = (data.read_cell(self.w)? & 0xff).try_into()?;
        match op {
            Op::Yield => {
                let index = data.read_cell(self.w + SIZE)?;
                let token = YieldToken { ip: self.ip, index };
                Ok(Some(Stop::Yield(token)))
            }
            _ => self.execute(data, op),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DS_LEN: usize = 16;
    const RS_LEN: usize = 16;
    const MEM: usize = 1024;

    fn vm() -> (Vm, Data<[u8; MEM]>) {
        let v = Vm::new(DS_LEN, RS_LEN);
        let d = Data::new([0u8; MEM]);
        (v, d)
    }

    fn ds(v: &Vm, d: &Data<[u8; MEM]>) -> Vec<usize> {
        v.stack(d).collect()
    }

    fn rlen(v: &Vm) -> usize {
        (v.rp - v.rs_addr()) / SIZE
    }

    fn rpeek(v: &mut Vm, d: &mut Data<[u8; MEM]>) -> usize {
        let x = v.rpop(d).unwrap();
        v.rpush(d, x).unwrap();
        x
    }

    // reserved

    #[test]
    fn reserved_correct() {
        let (v, _) = vm();
        assert_eq!(v.reserved(), (DS_LEN + RS_LEN + 1) * SIZE);
    }

    // reset

    #[test]
    fn reset_clears_stacks() {
        let (mut v, mut d) = vm();
        v.push(&mut d, 1).unwrap();
        v.push(&mut d, 2).unwrap();
        v.rpush(&mut d, 3).unwrap();
        v.reset();
        assert_eq!(ds(&v, &d), vec![]);
        assert_eq!(rlen(&v), 0);
    }

    // stack

    #[test]
    fn stack_empty() {
        let (v, d) = vm();
        assert_eq!(ds(&v, &d), vec![]);
    }

    #[test]
    fn stack_nonempty() {
        let (mut v, mut d) = vm();
        v.push(&mut d, 10).unwrap();
        v.push(&mut d, 20).unwrap();
        assert_eq!(ds(&v, &d), vec![10, 20]);
    }

    // push, pop

    #[test]
    fn push_ok() {
        let (mut v, mut d) = vm();
        v.push(&mut d, 42).unwrap();
        assert_eq!(ds(&v, &d), vec![42]);
    }

    #[test]
    fn push_overflow() {
        let (mut v, mut d) = vm();
        for i in 0..DS_LEN {
            v.push(&mut d, i).unwrap();
        }
        assert_eq!(v.push(&mut d, 99), Err(VmError::StackOverflow));
    }

    #[test]
    fn pop_ok() {
        let (mut v, mut d) = vm();
        v.push(&mut d, 7).unwrap();
        assert_eq!(v.pop(&mut d).unwrap(), 7);
        assert_eq!(ds(&v, &d), vec![]);
    }

    #[test]
    fn pop_underflow() {
        let (mut v, mut d) = vm();
        assert_eq!(v.pop(&mut d), Err(VmError::StackUnderflow));
    }

    // rpush, rpop

    #[test]
    fn rpush_ok() {
        let (mut v, mut d) = vm();
        v.rpush(&mut d, 99).unwrap();
        assert_eq!(rlen(&v), 1);
        assert_eq!(v.rpop(&mut d).unwrap(), 99);
    }

    #[test]
    fn rpush_overflow() {
        let (mut v, mut d) = vm();
        for i in 0..RS_LEN {
            v.rpush(&mut d, i).unwrap();
        }
        assert_eq!(v.rpush(&mut d, 0), Err(VmError::ReturnStackOverflow));
    }

    #[test]
    fn rpop_ok() {
        let (mut v, mut d) = vm();
        v.rpush(&mut d, 55).unwrap();
        assert_eq!(v.rpop(&mut d).unwrap(), 55);
        assert_eq!(rlen(&v), 0);
    }

    #[test]
    fn rpop_underflow() {
        let (mut v, mut d) = vm();
        assert_eq!(v.rpop(&mut d), Err(VmError::ReturnStackUnderflow));
    }

    // run

    #[test]
    fn run_ip_zero_halts() {
        let (mut v, mut d) = vm();
        v.ip = 0;
        assert_eq!(v.run(&mut d).unwrap(), Stop::Halt);
    }

    #[test]
    fn run_executes_thread() {
        let (mut v, mut d) = vm();
        let base = v.reserved();
        let s = SIZE;
        d.write_cell(base, Op::Halt as usize).unwrap();
        let thread = base + s;
        d.write_cell(thread, base).unwrap();
        v.ip = thread;
        assert_eq!(v.run(&mut d).unwrap(), Stop::Halt);
    }

    #[test]
    fn run_returns_stop() {
        let (mut v, mut d) = vm();
        let base = v.reserved();
        let s = SIZE;
        d.write_cell(base, Op::Lit as usize).unwrap();
        d.write_cell(base + s, Op::Halt as usize).unwrap();
        let thread = base + 2 * s;
        d.write_cell(thread, base).unwrap();
        d.write_cell(thread + s, 42usize).unwrap();
        d.write_cell(thread + 2 * s, base + s).unwrap();
        v.ip = thread;
        assert_eq!(v.run(&mut d).unwrap(), Stop::Halt);
        assert_eq!(ds(&v, &d), vec![42]);
    }

    // call

    #[test]
    fn call_halt_word() {
        let (mut v, mut d) = vm();
        let base = v.reserved();
        d.write_cell(base, Op::Halt as usize).unwrap();
        assert_eq!(v.call(&mut d, base).unwrap(), Stop::Halt);
    }

    #[test]
    fn call_docol_word() {
        let (mut v, mut d) = vm();
        let base = v.reserved();
        let s = SIZE;
        d.write_cell(base, Op::DoCol as usize).unwrap();
        d.write_cell(base + s, base + 2 * s).unwrap();
        d.write_cell(base + 2 * s, Op::Halt as usize).unwrap();
        assert_eq!(v.call(&mut d, base).unwrap(), Stop::Halt);
    }

    // resume

    #[test]
    fn resume_continues_after_yield() {
        let (mut v, mut d) = vm();
        let base = v.reserved();
        let s = SIZE;
        d.write_cell(base, Op::Yield as usize).unwrap();
        d.write_cell(base + s, 7usize).unwrap();
        d.write_cell(base + 2 * s, Op::Halt as usize).unwrap();
        let thread = base + 3 * s;
        d.write_cell(thread, base).unwrap();
        d.write_cell(thread + s, base + 2 * s).unwrap();
        v.ip = thread;

        let stop = v.run(&mut d).unwrap();

        let token = match stop {
            Stop::Yield(t) => t,
            _ => panic!("expected Yield"),
        };
        assert_eq!(token.index, 7);
        assert_eq!(v.resume(&mut d, token).unwrap(), Stop::Halt);
    }

    // dispatch

    #[test]
    fn dispatch_yield_produces_stop() {
        let (mut v, mut d) = vm();
        let base = v.reserved();
        d.write_cell(base, Op::Yield as usize).unwrap();
        d.write_cell(base + SIZE, 3usize).unwrap();
        v.w = base;
        v.ip = base + 2 * SIZE;
        let stop = v.dispatch(&mut d).unwrap();
        assert!(matches!(stop, Some(Stop::Yield(ref t)) if t.index == 3));
    }

    #[test]
    fn dispatch_invalid_opcode() {
        let (mut v, mut d) = vm();
        let base = v.reserved();
        d.write_cell(base, 0xFFusize).unwrap();
        v.w = base;
        assert_eq!(v.dispatch(&mut d), Err(VmError::InvalidOpCode(0xFF)));
    }

    #[test]
    fn dispatch_delegates_to_execute() {
        let (mut v, mut d) = vm();
        let base = v.reserved();
        d.write_cell(base, Op::Halt as usize).unwrap();
        v.w = base;
        assert_eq!(v.dispatch(&mut d).unwrap(), Some(Stop::Halt));
    }

    // Halt

    #[test]
    fn op_halt() {
        let (mut v, mut d) = vm();
        assert_eq!(v.execute(&mut d, Op::Halt).unwrap(), Some(Stop::Halt));
    }

    // Yield

    #[test]
    fn op_yield_via_execute_is_invalid() {
        let (mut v, mut d) = vm();
        assert_eq!(
            v.execute(&mut d, Op::Yield),
            Err(VmError::InvalidOpCode(Op::Yield as u8))
        );
    }

    // Exit

    #[test]
    fn op_exit_ok() {
        let (mut v, mut d) = vm();
        let ret = v.reserved();
        v.rpush(&mut d, ret).unwrap();
        v.execute(&mut d, Op::Exit).unwrap();
        assert_eq!(v.ip, ret);
    }

    #[test]
    fn op_exit_underflow() {
        let (mut v, mut d) = vm();
        assert_eq!(
            v.execute(&mut d, Op::Exit),
            Err(VmError::ReturnStackUnderflow)
        );
    }

    // DoCol

    #[test]
    fn op_docol_ok() {
        let (mut v, mut d) = vm();
        let base = v.reserved();
        v.w = base;
        v.ip = base + 2 * SIZE;
        v.execute(&mut d, Op::DoCol).unwrap();
        assert_eq!(v.ip, base + SIZE);
        assert_eq!(v.rpop(&mut d).unwrap(), base + 2 * SIZE);
    }

    #[test]
    fn op_docol_overflow() {
        let (mut v, mut d) = vm();
        for i in 0..RS_LEN {
            v.rpush(&mut d, i).unwrap();
        }
        let base = v.reserved();
        v.w = base;
        assert_eq!(
            v.execute(&mut d, Op::DoCol),
            Err(VmError::ReturnStackOverflow)
        );
    }

    // Lit

    #[test]
    fn op_lit_ok() {
        let (mut v, mut d) = vm();
        let base = v.reserved();
        d.write_cell(base, 99usize).unwrap();
        v.ip = base;
        v.execute(&mut d, Op::Lit).unwrap();
        assert_eq!(ds(&v, &d), vec![99]);
        assert_eq!(v.ip, base + SIZE);
    }

    #[test]
    fn op_lit_overflow() {
        let (mut v, mut d) = vm();
        for i in 0..DS_LEN {
            v.push(&mut d, i).unwrap();
        }
        let base = v.reserved();
        d.write_cell(base, 1usize).unwrap();
        v.ip = base;
        assert_eq!(v.execute(&mut d, Op::Lit), Err(VmError::StackOverflow));
    }

    // Str

    #[test]
    fn op_str_ok() {
        let (mut v, mut d) = vm();
        let base = v.reserved();
        let len: usize = 3;
        d.write_cell(base, len).unwrap();
        d.write(base + SIZE, b"abc").unwrap();
        v.ip = base;
        v.execute(&mut d, Op::Str).unwrap();
        let padded = (len + SIZE - 1) & !(SIZE - 1);
        let stack = ds(&v, &d);
        assert_eq!(stack[0], base + SIZE);
        assert_eq!(stack[1], len);
        assert_eq!(v.ip, base + SIZE + padded);
    }

    #[test]
    fn op_str_overflow() {
        let (mut v, mut d) = vm();
        for i in 0..DS_LEN - 1 {
            v.push(&mut d, i).unwrap();
        }
        let base = v.reserved();
        d.write_cell(base, 1usize).unwrap();
        v.ip = base;
        assert_eq!(v.execute(&mut d, Op::Str), Err(VmError::StackOverflow));
    }

    // Jmp

    #[test]
    fn op_jmp_ok() {
        let (mut v, mut d) = vm();
        let base = v.reserved();
        let target = base + 8 * SIZE;
        d.write_cell(base, target).unwrap();
        v.ip = base;
        v.execute(&mut d, Op::Jmp).unwrap();
        assert_eq!(v.ip, target);
    }

    // JmpZ

    #[test]
    fn op_jmpz_zero_jumps() {
        let (mut v, mut d) = vm();
        let base = v.reserved();
        let target = base + 8 * SIZE;
        d.write_cell(base, target).unwrap();
        v.ip = base;
        v.push(&mut d, 0).unwrap();
        v.execute(&mut d, Op::JmpZ).unwrap();
        assert_eq!(v.ip, target);
    }

    #[test]
    fn op_jmpz_nonzero_falls_through() {
        let (mut v, mut d) = vm();
        let base = v.reserved();
        let target = base + 8 * SIZE;
        d.write_cell(base, target).unwrap();
        v.ip = base;
        v.push(&mut d, 1).unwrap();
        v.execute(&mut d, Op::JmpZ).unwrap();
        assert_eq!(v.ip, base + SIZE);
    }

    #[test]
    fn op_jmpz_underflow() {
        let (mut v, mut d) = vm();
        let base = v.reserved();
        d.write_cell(base, 0usize).unwrap();
        v.ip = base;
        assert_eq!(v.execute(&mut d, Op::JmpZ), Err(VmError::StackUnderflow));
    }

    // Do

    #[test]
    fn op_do_ok() {
        let (mut v, mut d) = vm();
        v.push(&mut d, 0).unwrap();
        v.push(&mut d, 5).unwrap();
        v.execute(&mut d, Op::Do).unwrap();
        assert_eq!(rlen(&v), 2);
    }

    #[test]
    fn op_do_pop_underflow() {
        let (mut v, mut d) = vm();
        assert_eq!(v.execute(&mut d, Op::Do), Err(VmError::StackUnderflow));
    }

    #[test]
    fn op_do_rpush_overflow() {
        let (mut v, mut d) = vm();
        for i in 0..RS_LEN - 1 {
            v.rpush(&mut d, i).unwrap();
        }
        v.push(&mut d, 0).unwrap();
        v.push(&mut d, 5).unwrap();
        assert_eq!(v.execute(&mut d, Op::Do), Err(VmError::ReturnStackOverflow));
    }

    // QDo

    #[test]
    fn op_qdo_equal_jumps() {
        let (mut v, mut d) = vm();
        let base = v.reserved();
        let target = base + 4 * SIZE;
        d.write_cell(base, target).unwrap();
        v.ip = base;
        v.push(&mut d, 3).unwrap();
        v.push(&mut d, 3).unwrap();
        v.execute(&mut d, Op::QDo).unwrap();
        assert_eq!(v.ip, target);
        assert_eq!(rlen(&v), 0);
    }

    #[test]
    fn op_qdo_unequal_sets_up_loop() {
        let (mut v, mut d) = vm();
        let base = v.reserved();
        d.write_cell(base, 0usize).unwrap();
        v.ip = base;
        v.push(&mut d, 0).unwrap();
        v.push(&mut d, 5).unwrap();
        v.execute(&mut d, Op::QDo).unwrap();
        assert_eq!(v.ip, base + SIZE);
        assert_eq!(rlen(&v), 2);
    }

    #[test]
    fn op_qdo_pop_underflow() {
        let (mut v, mut d) = vm();
        let base = v.reserved();
        d.write_cell(base, 0usize).unwrap();
        v.ip = base;
        assert_eq!(v.execute(&mut d, Op::QDo), Err(VmError::StackUnderflow));
    }

    // PlusLoop

    #[test]
    fn op_plusloop_continues() {
        let (mut v, mut d) = vm();
        let limit: usize = 3;
        let index: usize = 0;
        let fudged = index.wrapping_sub(limit).wrapping_add(isize::MIN as usize);
        v.rpush(&mut d, limit).unwrap();
        v.rpush(&mut d, fudged).unwrap();
        let base = v.reserved() + 8 * SIZE;
        let back = base;
        d.write_cell(base, back).unwrap();
        v.ip = base;
        v.push(&mut d, 1usize).unwrap();
        v.execute(&mut d, Op::PlusLoop).unwrap();
        assert_eq!(v.ip, back);
        assert_eq!(rlen(&v), 2);
    }

    #[test]
    fn op_plusloop_exits() {
        let (mut v, mut d) = vm();
        let limit: usize = 3;
        let index: usize = 2;
        let fudged = index.wrapping_sub(limit).wrapping_add(isize::MIN as usize);
        v.rpush(&mut d, limit).unwrap();
        v.rpush(&mut d, fudged).unwrap();
        let base = v.reserved() + 8 * SIZE;
        d.write_cell(base, 0usize).unwrap();
        v.ip = base;
        v.push(&mut d, 1usize).unwrap();
        v.execute(&mut d, Op::PlusLoop).unwrap();
        assert_eq!(v.ip, base + SIZE);
        assert_eq!(rlen(&v), 0);
    }

    #[test]
    fn op_plusloop_underflow() {
        let (mut v, mut d) = vm();
        v.rpush(&mut d, 0).unwrap();
        v.rpush(&mut d, 0).unwrap();
        let base = v.reserved() + 8 * SIZE;
        d.write_cell(base, 0usize).unwrap();
        v.ip = base;
        assert_eq!(
            v.execute(&mut d, Op::PlusLoop),
            Err(VmError::StackUnderflow)
        );
    }

    // Unloop

    #[test]
    fn op_unloop_ok() {
        let (mut v, mut d) = vm();
        v.rpush(&mut d, 10).unwrap();
        v.rpush(&mut d, 20).unwrap();
        v.execute(&mut d, Op::Unloop).unwrap();
        assert_eq!(rlen(&v), 0);
    }

    #[test]
    fn op_unloop_underflow() {
        let (mut v, mut d) = vm();
        assert_eq!(
            v.execute(&mut d, Op::Unloop),
            Err(VmError::ReturnStackUnderflow)
        );
    }

    // I

    #[test]
    fn op_i_ok() {
        let (mut v, mut d) = vm();
        let limit: usize = 3;
        let index: usize = 1;
        let fudged = index.wrapping_sub(limit).wrapping_add(isize::MIN as usize);
        v.rpush(&mut d, limit).unwrap();
        v.rpush(&mut d, fudged).unwrap();
        v.execute(&mut d, Op::I).unwrap();
        assert_eq!(ds(&v, &d), vec![index]);
    }

    #[test]
    fn op_i_overflow() {
        let (mut v, mut d) = vm();
        for i in 0..DS_LEN {
            v.push(&mut d, i).unwrap();
        }
        let limit: usize = 0;
        let fudged = isize::MIN as usize;
        v.rpush(&mut d, limit).unwrap();
        v.rpush(&mut d, fudged).unwrap();
        assert_eq!(v.execute(&mut d, Op::I), Err(VmError::StackOverflow));
    }

    // J

    #[test]
    fn op_j_ok() {
        let (mut v, mut d) = vm();
        let outer_limit: usize = 5;
        let outer_index: usize = 2;
        let outer_fudged = outer_index
            .wrapping_sub(outer_limit)
            .wrapping_add(isize::MIN as usize);
        let inner_limit: usize = 3;
        let inner_index: usize = 1;
        let inner_fudged = inner_index
            .wrapping_sub(inner_limit)
            .wrapping_add(isize::MIN as usize);
        v.rpush(&mut d, outer_limit).unwrap();
        v.rpush(&mut d, outer_fudged).unwrap();
        v.rpush(&mut d, inner_limit).unwrap();
        v.rpush(&mut d, inner_fudged).unwrap();
        v.execute(&mut d, Op::J).unwrap();
        assert_eq!(ds(&v, &d), vec![outer_index]);
    }

    #[test]
    fn op_j_overflow() {
        let (mut v, mut d) = vm();
        for i in 0..DS_LEN {
            v.push(&mut d, i).unwrap();
        }
        v.rpush(&mut d, 5).unwrap();
        v.rpush(&mut d, isize::MIN as usize).unwrap();
        v.rpush(&mut d, 3).unwrap();
        v.rpush(&mut d, isize::MIN as usize).unwrap();
        assert_eq!(v.execute(&mut d, Op::J), Err(VmError::StackOverflow));
    }

    // Drop

    #[test]
    fn op_drop_ok() {
        let (mut v, mut d) = vm();
        v.push(&mut d, 42).unwrap();
        v.execute(&mut d, Op::Drop).unwrap();
        assert_eq!(ds(&v, &d), vec![]);
    }

    #[test]
    fn op_drop_underflow() {
        let (mut v, mut d) = vm();
        assert_eq!(v.execute(&mut d, Op::Drop), Err(VmError::StackUnderflow));
    }

    // Swap

    #[test]
    fn op_swap_ok() {
        let (mut v, mut d) = vm();
        v.push(&mut d, 1).unwrap();
        v.push(&mut d, 2).unwrap();
        v.execute(&mut d, Op::Swap).unwrap();
        assert_eq!(ds(&v, &d), vec![2, 1]);
    }

    #[test]
    fn op_swap_underflow() {
        let (mut v, mut d) = vm();
        v.push(&mut d, 1).unwrap();
        assert_eq!(v.execute(&mut d, Op::Swap), Err(VmError::StackUnderflow));
    }

    // Dup

    #[test]
    fn op_dup_ok() {
        let (mut v, mut d) = vm();
        v.push(&mut d, 1).unwrap();
        v.push(&mut d, 2).unwrap();
        v.execute(&mut d, Op::Dup).unwrap();
        assert_eq!(ds(&v, &d), vec![1, 2, 2]);
    }

    #[test]
    fn op_dup_underflow() {
        let (mut v, mut d) = vm();
        assert_eq!(v.execute(&mut d, Op::Dup), Err(VmError::StackUnderflow));
    }

    #[test]
    fn op_dup_overflow() {
        let (mut v, mut d) = vm();
        for i in 0..DS_LEN {
            v.push(&mut d, i).unwrap();
        }
        assert_eq!(v.execute(&mut d, Op::Dup), Err(VmError::StackOverflow));
    }

    // SpFetch

    #[test]
    fn op_spfetch_ok() {
        let (mut v, mut d) = vm();
        v.push(&mut d, 1).unwrap();
        let sp_before = v.sp;
        v.execute(&mut d, Op::SpFetch).unwrap();
        let stack = ds(&v, &d);
        assert_eq!(stack[stack.len() - 1], sp_before);
    }

    #[test]
    fn op_spfetch_overflow() {
        let (mut v, mut d) = vm();
        for i in 0..DS_LEN {
            v.push(&mut d, i).unwrap();
        }
        assert_eq!(v.execute(&mut d, Op::SpFetch), Err(VmError::StackOverflow));
    }

    // SpStore

    #[test]
    fn op_spstore_ok() {
        let (mut v, mut d) = vm();
        v.push(&mut d, 1).unwrap();
        v.push(&mut d, 2).unwrap();
        let target = Vm::DS_ADDR + SIZE;
        v.push(&mut d, target).unwrap();
        v.execute(&mut d, Op::SpStore).unwrap();
        assert_eq!(v.sp, target);
    }

    #[test]
    fn op_spstore_below_ds() {
        let (mut v, mut d) = vm();
        v.push(&mut d, 0usize).unwrap();
        assert_eq!(
            v.execute(&mut d, Op::SpStore),
            Err(VmError::AddressOutOfRange(0))
        );
    }

    #[test]
    fn op_spstore_above_sp_max() {
        let (mut v, mut d) = vm();
        let too_high = v.sp_max + SIZE;
        v.push(&mut d, too_high).unwrap();
        assert_eq!(
            v.execute(&mut d, Op::SpStore),
            Err(VmError::AddressOutOfRange(too_high))
        );
    }

    #[test]
    fn op_spstore_underflow() {
        let (mut v, mut d) = vm();
        assert_eq!(v.execute(&mut d, Op::SpStore), Err(VmError::StackUnderflow));
    }

    // ToR

    #[test]
    fn op_tor_ok() {
        let (mut v, mut d) = vm();
        v.push(&mut d, 77).unwrap();
        v.execute(&mut d, Op::ToR).unwrap();
        assert_eq!(ds(&v, &d), vec![]);
        assert_eq!(rlen(&v), 1);
        assert_eq!(rpeek(&mut v, &mut d), 77);
    }

    #[test]
    fn op_tor_pop_underflow() {
        let (mut v, mut d) = vm();
        assert_eq!(v.execute(&mut d, Op::ToR), Err(VmError::StackUnderflow));
    }

    #[test]
    fn op_tor_rpush_overflow() {
        let (mut v, mut d) = vm();
        for i in 0..RS_LEN {
            v.rpush(&mut d, i).unwrap();
        }
        v.push(&mut d, 1).unwrap();
        assert_eq!(
            v.execute(&mut d, Op::ToR),
            Err(VmError::ReturnStackOverflow)
        );
    }

    // RFrom

    #[test]
    fn op_rfrom_ok() {
        let (mut v, mut d) = vm();
        v.rpush(&mut d, 88).unwrap();
        v.execute(&mut d, Op::RFrom).unwrap();
        assert_eq!(ds(&v, &d), vec![88]);
        assert_eq!(rlen(&v), 0);
    }

    #[test]
    fn op_rfrom_rpop_underflow() {
        let (mut v, mut d) = vm();
        assert_eq!(
            v.execute(&mut d, Op::RFrom),
            Err(VmError::ReturnStackUnderflow)
        );
    }

    #[test]
    fn op_rfrom_push_overflow() {
        let (mut v, mut d) = vm();
        for i in 0..DS_LEN {
            v.push(&mut d, i).unwrap();
        }
        v.rpush(&mut d, 1).unwrap();
        assert_eq!(v.execute(&mut d, Op::RFrom), Err(VmError::StackOverflow));
    }

    // RpFetch

    #[test]
    fn op_rpfetch_ok() {
        let (mut v, mut d) = vm();
        let rp = v.rp;
        v.execute(&mut d, Op::RpFetch).unwrap();
        assert_eq!(ds(&v, &d), vec![rp]);
    }

    #[test]
    fn op_rpfetch_overflow() {
        let (mut v, mut d) = vm();
        for i in 0..DS_LEN {
            v.push(&mut d, i).unwrap();
        }
        assert_eq!(v.execute(&mut d, Op::RpFetch), Err(VmError::StackOverflow));
    }

    // RpStore

    #[test]
    fn op_rpstore_ok() {
        let (mut v, mut d) = vm();
        let target = v.rs_addr();
        v.push(&mut d, target).unwrap();
        v.execute(&mut d, Op::RpStore).unwrap();
        assert_eq!(v.rp, target);
    }

    #[test]
    fn op_rpstore_below_rs() {
        let (mut v, mut d) = vm();
        let too_low = v.rs_addr() - SIZE;
        v.push(&mut d, too_low).unwrap();
        assert_eq!(
            v.execute(&mut d, Op::RpStore),
            Err(VmError::AddressOutOfRange(too_low))
        );
    }

    #[test]
    fn op_rpstore_above_rp_max() {
        let (mut v, mut d) = vm();
        let too_high = v.rp_max + SIZE;
        v.push(&mut d, too_high).unwrap();
        assert_eq!(
            v.execute(&mut d, Op::RpStore),
            Err(VmError::AddressOutOfRange(too_high))
        );
    }

    #[test]
    fn op_rpstore_underflow() {
        let (mut v, mut d) = vm();
        assert_eq!(v.execute(&mut d, Op::RpStore), Err(VmError::StackUnderflow));
    }

    // Fetch

    #[test]
    fn op_fetch_ok() {
        let (mut v, mut d) = vm();
        let base = v.reserved();
        d.write_cell(base, 0xBEEFusize).unwrap();
        v.push(&mut d, base).unwrap();
        v.execute(&mut d, Op::Fetch).unwrap();
        assert_eq!(ds(&v, &d), vec![0xBEEF]);
    }

    #[test]
    fn op_fetch_underflow() {
        let (mut v, mut d) = vm();
        assert_eq!(v.execute(&mut d, Op::Fetch), Err(VmError::StackUnderflow));
    }

    #[test]
    fn op_fetch_misaligned() {
        let (mut v, mut d) = vm();
        v.push(&mut d, v.reserved() + 1).unwrap();
        assert_eq!(
            v.execute(&mut d, Op::Fetch),
            Err(VmError::AddressMisaligned(v.reserved() + 1))
        );
    }

    // Store

    #[test]
    fn op_store_ok() {
        let (mut v, mut d) = vm();
        let base = v.reserved();
        v.push(&mut d, 0xcafeusize).unwrap();
        v.push(&mut d, base).unwrap();
        v.execute(&mut d, Op::Store).unwrap();
        assert_eq!(d.read_cell(base).unwrap(), 0xcafe);
        assert_eq!(ds(&v, &d), vec![]);
    }

    #[test]
    fn op_store_underflow() {
        let (mut v, mut d) = vm();
        v.push(&mut d, 0).unwrap();
        assert_eq!(v.execute(&mut d, Op::Store), Err(VmError::StackUnderflow));
    }

    #[test]
    fn op_store_misaligned() {
        let (mut v, mut d) = vm();
        v.push(&mut d, 1usize).unwrap();
        v.push(&mut d, v.reserved() + 1).unwrap();
        assert_eq!(
            v.execute(&mut d, Op::Store),
            Err(VmError::AddressMisaligned(v.reserved() + 1))
        );
    }

    // CFetch

    #[test]
    fn op_cfetch_ok() {
        let (mut v, mut d) = vm();
        let base = v.reserved();
        d.write_char(base, b'X').unwrap();
        v.push(&mut d, base).unwrap();
        v.execute(&mut d, Op::CFetch).unwrap();
        assert_eq!(ds(&v, &d), vec![b'X' as usize]);
    }

    #[test]
    fn op_cfetch_underflow() {
        let (mut v, mut d) = vm();
        assert_eq!(v.execute(&mut d, Op::CFetch), Err(VmError::StackUnderflow));
    }

    #[test]
    fn op_cfetch_out_of_range() {
        let (mut v, mut d) = vm();
        v.push(&mut d, MEM + 1).unwrap();
        assert_eq!(
            v.execute(&mut d, Op::CFetch),
            Err(VmError::AddressOutOfRange(MEM + 1))
        );
    }

    // CStore

    #[test]
    fn op_cstore_ok() {
        let (mut v, mut d) = vm();
        let base = v.reserved();
        v.push(&mut d, b'Z' as usize).unwrap();
        v.push(&mut d, base).unwrap();
        v.execute(&mut d, Op::CStore).unwrap();
        assert_eq!(d.read_char(base).unwrap(), b'Z');
        assert_eq!(ds(&v, &d), vec![]);
    }

    #[test]
    fn op_cstore_underflow() {
        let (mut v, mut d) = vm();
        v.push(&mut d, 0).unwrap();
        assert_eq!(v.execute(&mut d, Op::CStore), Err(VmError::StackUnderflow));
    }

    #[test]
    fn op_cstore_out_of_range() {
        let (mut v, mut d) = vm();
        v.push(&mut d, b'A' as usize).unwrap();
        v.push(&mut d, MEM + 1).unwrap();
        assert_eq!(
            v.execute(&mut d, Op::CStore),
            Err(VmError::AddressOutOfRange(MEM + 1))
        );
    }

    // Add

    #[test]
    fn op_add_ok() {
        let (mut v, mut d) = vm();
        v.push(&mut d, 3).unwrap();
        v.push(&mut d, 4).unwrap();
        v.execute(&mut d, Op::Add).unwrap();
        assert_eq!(ds(&v, &d), vec![7]);
    }

    #[test]
    fn op_add_underflow() {
        let (mut v, mut d) = vm();
        v.push(&mut d, 1).unwrap();
        assert_eq!(v.execute(&mut d, Op::Add), Err(VmError::StackUnderflow));
    }

    // UmMul

    #[test]
    fn op_ummul_ok() {
        let (mut v, mut d) = vm();
        v.push(&mut d, 6).unwrap();
        v.push(&mut d, 7).unwrap();
        v.execute(&mut d, Op::UmMul).unwrap();
        let stack = ds(&v, &d);
        assert_eq!(stack[0], 42);
        assert_eq!(stack[1], 0);
    }

    #[test]
    fn op_ummul_underflow() {
        let (mut v, mut d) = vm();
        v.push(&mut d, 1).unwrap();
        assert_eq!(v.execute(&mut d, Op::UmMul), Err(VmError::StackUnderflow));
    }

    #[test]
    fn op_umdivmod_ok() {
        let (mut v, mut d) = vm();
        v.push(&mut d, 17).unwrap();
        v.push(&mut d, 0).unwrap();
        v.push(&mut d, 5).unwrap();
        v.execute(&mut d, Op::UmDivMod).unwrap();
        let stack = ds(&v, &d);
        assert_eq!(stack[0], 2);
        assert_eq!(stack[1], 3);
    }

    #[test]
    fn op_umdivmod_zero() {
        let (mut v, mut d) = vm();
        v.push(&mut d, 1).unwrap();
        v.push(&mut d, 0).unwrap();
        v.push(&mut d, 0).unwrap();
        assert_eq!(
            v.execute(&mut d, Op::UmDivMod),
            Err(VmError::DivisionByZero)
        );
    }

    #[test]
    fn op_umdivmod_underflow() {
        let (mut v, mut d) = vm();
        v.push(&mut d, 1).unwrap();
        v.push(&mut d, 2).unwrap();
        assert_eq!(
            v.execute(&mut d, Op::UmDivMod),
            Err(VmError::StackUnderflow)
        );
    }

    // Nand

    #[test]
    fn op_nand_ok() {
        let (mut v, mut d) = vm();
        v.push(&mut d, 0b1010).unwrap();
        v.push(&mut d, 0b1100).unwrap();
        v.execute(&mut d, Op::Nand).unwrap();
        assert_eq!(ds(&v, &d), vec![!(0b1010 & 0b1100usize)]);
    }

    #[test]
    fn op_nand_underflow() {
        let (mut v, mut d) = vm();
        v.push(&mut d, 1).unwrap();
        assert_eq!(v.execute(&mut d, Op::Nand), Err(VmError::StackUnderflow));
    }

    // LShift

    #[test]
    fn op_lshift_ok() {
        let (mut v, mut d) = vm();
        v.push(&mut d, 1).unwrap();
        v.push(&mut d, 3).unwrap();
        v.execute(&mut d, Op::LShift).unwrap();
        assert_eq!(ds(&v, &d), vec![8]);
    }

    #[test]
    fn op_lshift_underflow() {
        let (mut v, mut d) = vm();
        v.push(&mut d, 1).unwrap();
        assert_eq!(v.execute(&mut d, Op::LShift), Err(VmError::StackUnderflow));
    }

    // RShift

    #[test]
    fn op_rshift_ok() {
        let (mut v, mut d) = vm();
        v.push(&mut d, 16).unwrap();
        v.push(&mut d, 2).unwrap();
        v.execute(&mut d, Op::RShift).unwrap();
        assert_eq!(ds(&v, &d), vec![4]);
    }

    #[test]
    fn op_rshift_underflow() {
        let (mut v, mut d) = vm();
        v.push(&mut d, 1).unwrap();
        assert_eq!(v.execute(&mut d, Op::RShift), Err(VmError::StackUnderflow));
    }

    // LtZ

    #[test]
    fn op_ltz_negative() {
        let (mut v, mut d) = vm();
        v.push(&mut d, -1isize as usize).unwrap();
        v.execute(&mut d, Op::LtZ).unwrap();
        assert_eq!(ds(&v, &d), vec![TRUE]);
    }

    #[test]
    fn op_ltz_nonnegative() {
        let (mut v, mut d) = vm();
        v.push(&mut d, 0).unwrap();
        v.execute(&mut d, Op::LtZ).unwrap();
        assert_eq!(ds(&v, &d), vec![FALSE]);
    }

    #[test]
    fn op_ltz_underflow() {
        let (mut v, mut d) = vm();
        assert_eq!(v.execute(&mut d, Op::LtZ), Err(VmError::StackUnderflow));
    }

    // EqZ

    #[test]
    fn op_eqz_zero() {
        let (mut v, mut d) = vm();
        v.push(&mut d, 0).unwrap();
        v.execute(&mut d, Op::EqZ).unwrap();
        assert_eq!(ds(&v, &d), vec![TRUE]);
    }

    #[test]
    fn op_eqz_nonzero() {
        let (mut v, mut d) = vm();
        v.push(&mut d, 5).unwrap();
        v.execute(&mut d, Op::EqZ).unwrap();
        assert_eq!(ds(&v, &d), vec![FALSE]);
    }

    #[test]
    fn op_eqz_underflow() {
        let (mut v, mut d) = vm();
        assert_eq!(v.execute(&mut d, Op::EqZ), Err(VmError::StackUnderflow));
    }

    // DoCreate

    #[test]
    fn op_docreate_no_does() {
        let (mut v, mut d) = vm();
        let base = v.reserved();
        d.write_cell(base, Op::DoCreate as usize).unwrap();
        d.write_cell(base + SIZE, 0usize).unwrap();
        v.w = base;
        v.execute(&mut d, Op::DoCreate).unwrap();
        assert_eq!(ds(&v, &d), vec![base + 2 * SIZE]);
        assert_eq!(rlen(&v), 0);
    }

    #[test]
    fn op_docreate_with_does() {
        let (mut v, mut d) = vm();
        let base = v.reserved();
        let does_addr = base + 8 * SIZE;
        d.write_cell(base, Op::DoCreate as usize).unwrap();
        d.write_cell(base + SIZE, does_addr).unwrap();
        v.w = base;
        let saved_ip = base + 4 * SIZE;
        v.ip = saved_ip;
        v.execute(&mut d, Op::DoCreate).unwrap();
        assert_eq!(ds(&v, &d), vec![base + 2 * SIZE]);
        assert_eq!(rlen(&v), 1);
        assert_eq!(v.ip, does_addr);
        assert_eq!(v.rpop(&mut d).unwrap(), saved_ip);
    }

    #[test]
    fn op_docreate_push_overflow() {
        let (mut v, mut d) = vm();
        for i in 0..DS_LEN {
            v.push(&mut d, i).unwrap();
        }
        let base = v.reserved();
        d.write_cell(base, Op::DoCreate as usize).unwrap();
        d.write_cell(base + SIZE, 0usize).unwrap();
        v.w = base;
        assert_eq!(v.execute(&mut d, Op::DoCreate), Err(VmError::StackOverflow));
    }
}

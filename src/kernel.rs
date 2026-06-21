use crate::data::{Data, Mem};
use crate::error::{Ior, KernelError, Severity};
use crate::io::{Io, NoIo};
use crate::vm::{Op, Stop, Vm};
use crate::{Error, FALSE, Result, SIZE, TRUE};

mod boot;
mod builtins;
mod env;
mod host;
mod layout;

use env::Environment;
use layout::{INPUT_BUFFER_SIZE, Layout};

pub use env::Config;
pub use host::Host;

/// The maximum word length in bytes.
const MAX_WORD_LEN: usize = 31;
/// The maximum number of builtins in the builtins table.
const MAX_BUILTINS: usize = 256;

/// The offset of the `info` field from the `code` field.
const INFO_FROM_CFA: usize = 2 * SIZE;
/// The immediate bitflag.
const IMMEDIATE: u8 = 0b01;
/// The hidden bitflag.
const HIDDEN: u8 = 0b10;
/// The bootstrap bitflag. Words defined during the bootstrap phase have this flag.
const BOOTSTRAP: u8 = 0b100;
const PRIMITIVE: u8 = 0b0001000;
const BUILTIN: u8 = 0b0010000;
const COLON: u8 = 0b0100000;
#[allow(dead_code)]
const CREATE: u8 = 0b1000000;

pub type Builtin = fn(&mut dyn Host) -> Result<()>;

pub trait State {}
pub struct Bootstrapping {}
pub struct Ready {
    xt_catch: usize,
    xt_interpret: usize,
}
impl State for Bootstrapping {}
impl State for Ready {}

/// The outer interpreter.
pub struct Kernel<M: Mem = [u8; 65536], I: Io = NoIo, S: State = Bootstrapping> {
    vm: Vm,
    data: Data<M>,
    io: I,
    // lookup table for Op CFAs
    op_xts: [usize; 256],
    builtins: [Option<Builtin>; MAX_BUILTINS],
    builtins_len: usize,
    layout_base: usize,
    env: Environment,
    state: S,
}

impl<M: Mem, I: Io, S: State> Kernel<M, I, S> {
    /// Push an item to the data stack.
    ///
    /// ```text
    /// ( -- x )
    /// ```
    pub fn push(&mut self, x: usize) -> Result<()> {
        self.vm.push(&mut self.data, x)?;
        Ok(())
    }

    /// Pop an item from the data stack.
    ///
    /// ```text
    /// ( x --  )
    /// ```
    pub fn pop(&mut self) -> Result<usize> {
        Ok(self.vm.pop(&mut self.data)?)
    }

    /// Reset the data and return stacks.
    pub fn reset(&mut self) {
        self.vm.reset()
    }

    pub fn stack(&self) -> impl Iterator<Item = usize> + '_ {
        self.vm.stack(&self.data)
    }

    pub(super) fn find(&self, name: &[u8]) -> Result<Option<(usize, isize)>> {
        if name.len() > MAX_WORD_LEN {
            return Ok(None);
        }
        let mut xt = self.data.read_cell(self.layout_addr(Layout::LATEST))?;
        while xt != 0 {
            let info = self.data.read_cell(xt - INFO_FROM_CFA)?;
            let flags = (info >> 8) as u8;
            let wlen = info & 0xFF;
            if flags & HIDDEN == 0 && wlen == name.len() {
                let name_at = xt - INFO_FROM_CFA - SIZE - wlen;
                let b = self.data.read(name_at, wlen)?;
                if name.eq_ignore_ascii_case(b) {
                    let flag = if flags & IMMEDIATE != 0 { 1 } else { -1 };
                    return Ok(Some((xt, flag)));
                }
            }
            xt = self.data.read_cell(xt - SIZE)?;
        }
        Ok(None)
    }

    fn create(&mut self, name: &[u8], flags: u8) -> Result<usize> {
        let len: u8 = name
            .len()
            .try_into()
            .map_err(|_| Error::Throw(Ior::DEFINITION_NAME_TOO_LONG))?;
        let latest = self.data.read_cell(self.layout_addr(Layout::LATEST))?;
        let here = self.data.read_cell(self.layout_addr(Layout::HERE))?;
        // pad the name so as to always align info
        let pad = (SIZE - ((here + 1 + len as usize) % SIZE)) % SIZE;
        // name
        let nfa = here + pad;
        self.data.write_char(nfa, len)?;
        self.data.write(nfa + 1, name)?;
        // bodylen (0 until ;)
        let body_len = nfa + 1 + len as usize;
        self.data.write_cell(body_len, 0)?;
        // info
        let info = body_len + SIZE;
        self.data.write_cell(info, pack_info(flags, len))?;
        self.data.write_cell(info + SIZE, latest)?;
        // code
        let cfa = info + 2 * SIZE;
        Ok(cfa)
    }

    fn layout_addr(&self, offset: usize) -> usize {
        self.layout_base + offset
    }

    fn undefined(&mut self, addr: usize, len: usize) -> Result<()> {
        self.set_diagnostic(addr, len)?;
        Err(Error::Throw(Ior::UNDEFINED_WORD))
    }

    pub(super) fn execute(&mut self, xt: usize) -> Result<()> {
        let kind = (self.data.read_cell(xt - INFO_FROM_CFA)? >> 8) as u8;
        let mut stop = if kind & PRIMITIVE != 0 {
            let op: Op = (self.data.read_cell(xt)? & 0xff)
                .try_into()
                .map_err(Error::from)?;
            if op == Op::Execute {
                let target = self.pop()?;
                return self.execute(target);
            }
            match self.vm.step(&mut self.data, op) {
                Ok(Some(s)) => s,
                Ok(None) => return Ok(()),
                Err(e) => self.throw(e.into())?,
            }
        } else {
            match self.vm.call(&mut self.data, xt) {
                Ok(s) => s,
                Err(e) => self.throw(e.into())?,
            }
        };
        loop {
            match stop {
                Stop::Halt => return Ok(()),
                Stop::Yield(token) => {
                    let f = self.builtins[token.index]
                        .ok_or(KernelError::InvalidBuiltin(token.index as u8))?;
                    stop = match f(self) {
                        Ok(()) => match self.vm.resume(&mut self.data, token) {
                            Ok(s) => s,
                            Err(e) => self.throw(e.into())?,
                        },
                        Err(e) => self.throw(e)?,
                    };
                }
            }
        }
    }

    /// Raise a catachable error as a Forth exception, or abort.
    fn throw(&mut self, e: Error) -> Result<Stop> {
        let ior = match e.severity() {
            Severity::Throw(ior) => ior,
            Severity::Abort => return Err(self.abort(e)),
        };
        match self.find(b"throw")? {
            Some((throw_xt, _)) => {
                // Throw in Forth.
                self.push(ior as usize)?;
                match self.vm.call(&mut self.data, throw_xt) {
                    Ok(stop) => Ok(stop),
                    Err(e) => Err(self.abort(e.into())),
                }
            }
            // `throw` is not defined yet (bootstrap). Bubble up.
            None => Err(Error::Throw(ior)),
        }
    }

    /// Abort and re-raise a fatal [`Error`].
    fn abort(&mut self, e: Error) -> Error {
        let _ = self.data.write_cell(self.layout_addr(Layout::STATE), FALSE);
        self.vm.reset();
        e
    }

    fn set_diagnostic(&mut self, addr: usize, len: usize) -> Result<()> {
        self.data
            .write_cell(self.layout_addr(Layout::DIAGNOSTIC_ADDR), addr)?;
        self.data
            .write_cell(self.layout_addr(Layout::DIAGNOSTIC_LEN), len)?;
        Ok(())
    }

    pub(super) fn set_source(&mut self, code: &[u8]) -> Result<()> {
        if code.len() > INPUT_BUFFER_SIZE {
            return Err(Error::Throw(Ior::PARSED_STRING_OVERFLOW));
        }
        let input_addr = self.layout_addr(Layout::INPUT);
        self.data.write(input_addr, code)?;
        self.data
            .write_cell(self.layout_addr(Layout::SOURCE_ADDR), input_addr)?;
        self.data
            .write_cell(self.layout_addr(Layout::SOURCE_LEN), code.len())?;
        self.data
            .write_cell(self.layout_addr(Layout::SOURCE_ID), -1isize as usize)?;
        self.data.write_cell(self.layout_addr(Layout::TO_IN), 0)?;
        Ok(())
    }
}

impl<M: Mem, I: Io> Kernel<M, I, Ready> {
    pub(super) fn catch_interpret(&mut self) -> Result<()> {
        self.push(self.state.xt_interpret)?;
        self.execute(self.state.xt_catch)?;
        let code = self.pop()? as isize;
        if code != 0 {
            return Err(Error::Throw(code));
        }
        Ok(())
    }
}

/// Pack word flags and length into one cell.
///
/// The flags occupy the least significant byte. The cell occupies the next most significant
/// byte.
fn pack_info(flags: u8, len: u8) -> usize {
    (len as usize) | ((flags as usize) << 8)
}

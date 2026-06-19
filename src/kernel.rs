use core::marker::PhantomData;

use crate::data::{Data, Mem};
use crate::error::{Ior, KernelError};
use crate::io::{Io, NoIo};
use crate::vm::{Stop, Vm};
use crate::{Error, FALSE, Result, SIZE, TRUE};

mod boot;
mod builtins;
mod env;
mod host;
mod layout;

use env::Environment;
use layout::{INPUT_BUFFER_SIZE, Layout};

pub use builtins::refill;
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

pub type Builtin = fn(&mut dyn Host) -> Result<()>;

pub trait State {}
pub enum Bootstrapping {}
pub enum Ready {}
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
    _state: PhantomData<S>,
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

    pub(super) fn catch_interpret(&mut self) -> Result<()> {
        let (interpret, _) = self
            .lookup(b"(interpret)")?
            .ok_or(Error::Throw(Ior::UNDEFINED_WORD))?;
        let (catch, _) = self
            .lookup(b"catch")?
            .ok_or(Error::Throw(Ior::UNDEFINED_WORD))?;
        self.push(interpret)?;
        self.run(catch)?;
        let code = self.pop()? as isize;
        if code != 0 {
            return Err(Error::Throw(code));
        }
        Ok(())
    }

    pub(super) fn lookup(&self, name: &[u8]) -> Result<Option<(usize, isize)>> {
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

    fn write_header(&mut self, name: &[u8], flags: u8) -> Result<usize> {
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
        self.diagnostic(addr, len)?;
        Err(Error::Throw(Ior::UNDEFINED_WORD))
    }

    pub(super) fn run(&mut self, xt: usize) -> Result<()> {
        let mut stop = self.vm.call(&mut self.data, xt)?;
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

    /// Throw an [`Error`] as a Forth exception.
    fn throw(&mut self, e: Error) -> Result<Stop> {
        let ior = match Ior::try_from(e) {
            Ok(ior) => ior,
            Err(e) => {
                let _ = self.data.write_cell(self.layout_addr(Layout::STATE), FALSE);
                self.vm.reset();
                return Err(e);
            }
        };
        match self.lookup(b"throw")? {
            Some((throw_xt, _)) => {
                // TODO: This push will fail if the data stack is already full.
                self.push(isize::from(ior) as usize)?;
                Ok(self.vm.call(&mut self.data, throw_xt)?)
            }
            // Bootstrap errors bubble up.
            None => Err(Error::Throw(ior.into())),
        }
    }

    fn diagnostic(&mut self, addr: usize, len: usize) -> Result<()> {
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

impl<M: Mem, I: Io, S: State> Host for Kernel<M, I, S> {
    fn push(&mut self, x: usize) -> Result<()> {
        self.push(x)
    }
    fn pop(&mut self) -> Result<usize> {
        self.pop()
    }
    fn read(&self, addr: usize, u: usize) -> Result<&[u8]> {
        Ok(self.data.read(addr, u)?)
    }
    fn read_cell(&self, addr: usize) -> Result<usize> {
        Ok(self.data.read_cell(addr)?)
    }
    fn read_char(&self, addr: usize) -> Result<u8> {
        Ok(self.data.read_char(addr)?)
    }
    fn write(&mut self, addr: usize, bytes: &[u8]) -> Result<()> {
        Ok(self.data.write(addr, bytes)?)
    }
    fn write_char(&mut self, addr: usize, c: u8) -> Result<()> {
        Ok(self.data.write_char(addr, c)?)
    }
    fn write_cell(&mut self, addr: usize, x: usize) -> Result<()> {
        Ok(self.data.write_cell(addr, x)?)
    }
    fn emit(&mut self, c: u8) -> Result<()> {
        self.io.emit(c)
    }
    fn refill(&mut self, buf: &mut [u8]) -> Result<Option<usize>> {
        self.io.refill(buf)
    }
    fn key(&mut self) -> Result<Option<u8>> {
        self.io.key()
    }
    fn diagnostic(&mut self, addr: usize, u: usize) -> Result<()> {
        self.diagnostic(addr, u)
    }
    fn lookup(&self, name: &[u8]) -> Result<Option<(usize, isize)>> {
        self.lookup(name)
    }
    fn write_header(&mut self, name: &[u8], flags: u8) -> Result<usize> {
        self.write_header(name, flags)
    }
    fn layout_addr(&self, offset: usize) -> usize {
        self.layout_addr(offset)
    }
}

/// Pack word flags and length into one cell.
///
/// The flags occupy the least significant byte. The cell occupies the next most significant
/// byte.
fn pack_info(flags: u8, len: u8) -> usize {
    (len as usize) | ((flags as usize) << 8)
}

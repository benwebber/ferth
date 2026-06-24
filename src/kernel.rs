use crate::data::{Data, Mem};
use crate::error::{Ior, KernelError, Severity};
use crate::header::{Flags, Header, Info};
use crate::io::{Io, NoIo};
use crate::packed::PackedInstr;
use crate::state::{Booted, Booting, State};
use crate::vm::{Op, Stop, Vm};
use crate::{Error, FALSE, Result, TRUE};

mod boot;
mod builtins;
pub(crate) mod dict;
mod env;
mod host;
mod layout;

use dict::Dict;
use env::Environment;
use layout::{INPUT_BUFFER_SIZE, Layout};

pub use env::Config;
pub use host::Host;

/// The maximum word length in bytes.
const MAX_WORD_LEN: usize = 31;
/// The maximum number of builtins in the builtins table.
const MAX_BUILTINS: usize = 256;

pub type Builtin = fn(&mut dyn Host) -> Result<()>;

/// The outer interpreter.
pub struct Kernel<M: Mem = [u8; 65536], I: Io = NoIo, S: State = Booting> {
    vm: Vm,
    data: Data<M>,
    io: I,
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

    fn layout_addr(&self, offset: usize) -> usize {
        self.layout_base + offset
    }

    pub(crate) fn dict(&mut self) -> Dict<'_, M> {
        Dict::new(&mut self.data, self.layout_base)
    }

    fn undefined(&mut self, addr: usize, len: usize) -> Result<()> {
        self.dict().set_diagnostic(addr, len)?;
        Err(Error::Throw(Ior::UNDEFINED_WORD))
    }

    pub(super) fn execute(&mut self, xt: usize) -> Result<()> {
        let info: Info = self.data.read_cell(Header::new(xt).info_addr())?.into();
        let flags = info.flags();
        let mut stop = if flags.contains(Flags::PRIMITIVE) {
            let op: Op = (self.data.read_cell(xt)? & PackedInstr::OP_MASK)
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
        match self.dict().find(b"throw")? {
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

impl<M: Mem, I: Io> Kernel<M, I, Booted> {
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

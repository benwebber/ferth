use crate::data::{Data, Mem};
use crate::error::{KernelError, Severity};
use crate::io::{Io, NoIo};
use crate::state::{Booted, Booting, State};
use crate::vm::{Stop, Vm};
use crate::{Error, FALSE, Result, TRUE};

mod boot;
mod builtins;
mod context;
pub(crate) mod dict;
mod env;
mod layout;

use context::Context;
use dict::Dict;
use env::Environment;
use layout::{INPUT_BUFFER_SIZE, Layout};

pub use env::Config;

/// The maximum word length in bytes.
const MAX_WORD_LEN: usize = 31;
/// The maximum number of builtins in the builtins table.
const MAX_BUILTINS: usize = 256;
/// The minimum size of the data space, in bytes.
///
/// 2<sup>15</sup> is a rough bound. As of 822297c, the real size of the kernel and VM state is
/// around 45 000 bytes. In the future we will implement an image format. That format can encode
/// the size of the system. We could load the REPL system from a compiled image.
pub(crate) const MIN_DATA_SPACE: usize = 2 << 15;

pub(crate) type Builtin<M, I> = fn(&mut Context<'_, M, I>) -> Result<()>;

/// The outer interpreter.
pub struct Kernel<M: Mem = [u8; 65536], I: Io = NoIo, S: State = Booting> {
    vm: Vm,
    data: Data<M>,
    io: I,
    builtins: [Option<Builtin<M, I>>; MAX_BUILTINS],
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

    pub(crate) fn context(&mut self) -> Context<'_, M, I> {
        Context::new(&mut self.vm, &mut self.data, &mut self.io, self.layout_base)
    }

    pub(crate) fn dict(&mut self) -> Dict<'_, M> {
        Dict::new(&mut self.data, self.layout_base)
    }

    pub(super) fn execute(&mut self, xt: usize) -> Result<()> {
        let mut stop = match self.vm.enter(&mut self.data, xt) {
            Ok(s) => s,
            Err(e) => self.throw(e.into())?,
        };
        loop {
            match stop {
                Stop::Halt => return Ok(()),
                Stop::Yield(token) => {
                    let f = self.builtins[token.index]
                        .ok_or(KernelError::InvalidBuiltin(token.index as u8))?;
                    stop = match f(&mut self.context()) {
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

    /// Raise a catchable error as a Forth exception, or abort.
    fn throw(&mut self, e: Error) -> Result<Stop> {
        let ior = match e.severity() {
            Severity::Throw(ior) => ior,
            Severity::Abort => return Err(self.abort(e)),
        };
        match self.state.throw_xt() {
            Some(throw_xt) => {
                // Throw in Forth.
                self.push(ior.into())?;
                match self.vm.enter(&mut self.data, throw_xt.into()) {
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
        let state_addr = self.dict().addr(Layout::STATE);
        let _ = self.data.write_cell(state_addr, FALSE);
        self.vm.reset();
        e
    }

    pub(super) fn set_source(&mut self, code: &[u8]) -> Result<()> {
        self.dict().set_source(code)
    }
}

impl<M: Mem, I: Io> Kernel<M, I, Booted> {
    pub(super) fn catch_interpret(&mut self) -> Result<()> {
        self.push(self.state.xt_interpret)?;
        self.execute(self.state.xt_catch)?;
        let code = self.pop()? as isize;
        if code != 0 {
            return Err(Error::Throw(code.into()));
        }
        Ok(())
    }
}

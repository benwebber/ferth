use crate::Result;
use crate::data::Mem;
use crate::io::Io;

use super::{Kernel, State};

mod private {
    use super::*;

    pub trait Sealed {}

    impl<M: Mem, I: Io, S: State> Sealed for Kernel<M, I, S> {}
}

/// The interface builtins have access to.
///
/// This is a [sealed trait][sealed]. Only [`Kernel`] may implement it.
///
/// [sealed]: https://rust-lang.github.io/api-guidelines/future-proofing.html#sealed-traits-protect-against-downstream-implementations-c-sealed
pub trait Host: private::Sealed {
    fn push(&mut self, x: usize) -> Result<()>;
    fn pop(&mut self) -> Result<usize>;
    fn read(&self, addr: usize, u: usize) -> Result<&[u8]>;
    fn read_char(&self, addr: usize) -> Result<u8>;
    fn read_cell(&self, addr: usize) -> Result<usize>;
    fn write_cell(&mut self, addr: usize, x: usize) -> Result<()>;
    #[allow(dead_code)]
    fn write_char(&mut self, addr: usize, c: u8) -> Result<()>;
    fn write(&mut self, addr: usize, bytes: &[u8]) -> Result<()>;
    fn emit(&mut self, c: u8) -> Result<()>;
    fn key(&mut self) -> Result<Option<u8>>;
    fn refill(&mut self, buf: &mut [u8]) -> Result<Option<usize>>;
    fn set_diagnostic(&mut self, addr: usize, u: usize) -> Result<()>;
    fn find(&self, name: &[u8]) -> Result<Option<(usize, isize)>>;
    fn create(&mut self, name: &[u8], flags: u8) -> Result<usize>;
    fn layout_addr(&self, offset: usize) -> usize;
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
    fn set_diagnostic(&mut self, addr: usize, u: usize) -> Result<()> {
        self.set_diagnostic(addr, u)
    }
    fn find(&self, name: &[u8]) -> Result<Option<(usize, isize)>> {
        self.find(name)
    }
    fn create(&mut self, name: &[u8], flags: u8) -> Result<usize> {
        self.create(name, flags)
    }
    fn layout_addr(&self, offset: usize) -> usize {
        self.layout_addr(offset)
    }
}

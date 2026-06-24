use crate::Result;
use crate::data::{Data, Mem};
use crate::io::Io;
use crate::vm::Vm;

use super::dict::Dict;

pub(crate) struct Context<'a, M: Mem, I: Io> {
    vm: &'a mut Vm,
    data: &'a mut Data<M>,
    io: &'a mut I,
    layout_base: usize,
}

impl<'a, M: Mem, I: Io> Context<'a, M, I> {
    pub(crate) fn new(
        vm: &'a mut Vm,
        data: &'a mut Data<M>,
        io: &'a mut I,
        layout_base: usize,
    ) -> Self {
        Self {
            vm,
            data,
            io,
            layout_base,
        }
    }

    pub(crate) fn push(&mut self, x: usize) -> Result<()> {
        Ok(self.vm.push(self.data, x)?)
    }

    pub(crate) fn pop(&mut self) -> Result<usize> {
        Ok(self.vm.pop(self.data)?)
    }

    pub(crate) fn read(&self, addr: usize, len: usize) -> Result<&[u8]> {
        Ok(self.data.read(addr, len)?)
    }

    pub(crate) fn emit(&mut self, c: u8) -> Result<()> {
        self.io.emit(c)
    }

    pub(crate) fn key(&mut self) -> Result<Option<u8>> {
        self.io.key()
    }

    pub(crate) fn refill(&mut self, buf: &mut [u8]) -> Result<Option<usize>> {
        self.io.refill(buf)
    }

    pub(crate) fn dict(&mut self) -> Dict<'_, M> {
        Dict::new(self.data, self.layout_base)
    }
}

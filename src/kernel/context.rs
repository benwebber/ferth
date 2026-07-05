use crate::Result;
use crate::data::{Data, Mem};
use crate::vm::Vm;

use super::INPUT_BUFFER_SIZE;
use super::dict::Dict;
use super::layout::Layout;

pub(crate) struct Context<'a, M: Mem> {
    vm: &'a mut Vm,
    data: &'a mut Data<M>,
    layout_base: usize,
}

impl<'a, M: Mem> Context<'a, M> {
    pub(crate) fn new(vm: &'a mut Vm, data: &'a mut Data<M>, layout_base: usize) -> Self {
        Self {
            vm,
            data,
            layout_base,
        }
    }

    pub(crate) fn push(&mut self, x: usize) -> Result<()> {
        Ok(self.vm.push(self.data, x)?)
    }

    pub(crate) fn pop(&mut self) -> Result<usize> {
        Ok(self.vm.pop(self.data)?)
    }

    pub(crate) fn dict(&mut self) -> Dict<'_, M> {
        Dict::new(self.data, self.layout_base)
    }

    pub(crate) fn input_mut(&mut self) -> Result<&mut [u8]> {
        let addr = self.layout_base + Layout::INPUT;
        Ok(self.data.slice_mut(addr, INPUT_BUFFER_SIZE)?)
    }
}

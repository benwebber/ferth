use crate::data::{Data, Mem};
use crate::error::{Error, Ior};
use crate::header::{Flags, Header, Info};
use crate::{Result, SIZE};

use super::layout::Layout;
use super::{INPUT_BUFFER_SIZE, MAX_WORD_LEN};

pub(crate) struct Dict<'a, M: Mem> {
    data: &'a mut Data<M>,
    base_addr: usize,
}

impl<'a, M: Mem> Dict<'a, M> {
    pub(crate) fn new(data: &'a mut Data<M>, base_addr: usize) -> Self {
        Self { data, base_addr }
    }

    pub(crate) fn addr(&self, offset: usize) -> usize {
        self.base_addr.wrapping_add(offset)
    }

    pub(crate) fn here(&self) -> Result<usize> {
        Ok(self.data.read_cell(self.layout_addr(Layout::HERE))?)
    }

    pub(crate) fn set_here(&mut self, addr: usize) -> Result<()> {
        Ok(self.data.write_cell(self.layout_addr(Layout::HERE), addr)?)
    }

    pub(crate) fn latest(&self) -> Result<usize> {
        Ok(self.data.read_cell(self.layout_addr(Layout::LATEST))?)
    }

    pub(crate) fn to_in(&self) -> Result<usize> {
        Ok(self.data.read_cell(self.layout_addr(Layout::TO_IN))?)
    }

    pub(crate) fn set_to_in(&mut self, offset: usize) -> Result<()> {
        Ok(self
            .data
            .write_cell(self.layout_addr(Layout::TO_IN), offset)?)
    }

    pub(crate) fn set_latest(&mut self, addr: usize) -> Result<()> {
        Ok(self
            .data
            .write_cell(self.layout_addr(Layout::LATEST), addr)?)
    }

    fn header_at(&mut self, len: u8, flags: u8) -> Result<(usize, usize)> {
        let latest = self.latest()?;
        let here = self.here()?;
        // pad the name so as to always align info
        let pad = (SIZE - ((here + 1 + len as usize) % SIZE)) % SIZE;
        // name
        let nfa = here + pad;
        self.data.write_char(nfa, len)?;
        // bodylen (0 until ;)
        let body_len = nfa + 1 + len as usize;
        self.data.write_cell(body_len, 0)?;
        // info
        let info = body_len + SIZE;
        self.data
            .write_cell(info, Info::new(flags.into(), len).into())?;
        // link
        let link = info + SIZE;
        self.data.write_cell(link, latest)?;
        // code
        let cfa = link + SIZE;
        Ok((nfa, cfa))
    }

    pub(crate) fn create(&mut self, name: &[u8], flags: u8) -> Result<usize> {
        let len: u8 = name
            .len()
            .try_into()
            .map_err(|_| Error::Throw(Ior::DEFINITION_NAME_TOO_LONG))?;
        let (nfa, cfa) = self.header_at(len, flags)?;
        self.data.write(nfa + 1, name)?;
        Ok(cfa)
    }

    pub(crate) fn create_at(&mut self, src_addr: usize, len: usize, flags: u8) -> Result<usize> {
        let len: u8 = len
            .try_into()
            .map_err(|_| Error::Throw(Ior::DEFINITION_NAME_TOO_LONG))?;
        let (nfa, cfa) = self.header_at(len, flags)?;
        self.data.copy_within(src_addr, nfa + 1, len as usize)?;
        Ok(cfa)
    }

    pub(crate) fn find_at(&self, addr: usize, len: usize) -> Result<Option<(usize, usize)>> {
        let name = self.data.read(addr, len)?;
        self.find(name)
    }

    pub(crate) fn find(&self, name: &[u8]) -> Result<Option<(usize, usize)>> {
        if name.len() > MAX_WORD_LEN {
            return Ok(None);
        }
        let mut xt = self.latest()?;
        while xt != 0 {
            let header = Header::new(xt);
            let info: Info = self.data.read_cell(header.info_addr())?.into();
            let flags = info.flags();
            let wlen = info.name_len();
            if !flags.contains(Flags::HIDDEN) && wlen == name.len() {
                let name_at = header.bodylen_addr() - wlen;
                let b = self.data.read(name_at, wlen)?;
                if name.eq_ignore_ascii_case(b) {
                    let flag = if flags.contains(Flags::IMMEDIATE) {
                        1
                    } else {
                        -1isize as usize
                    };
                    return Ok(Some((xt, flag)));
                }
            }
            xt = self.data.read_cell(header.link_addr())?;
        }
        Ok(None)
    }

    pub(crate) fn set_diagnostic(&mut self, addr: usize, len: usize) -> Result<()> {
        self.data
            .write_cell(self.layout_addr(Layout::DIAGNOSTIC_ADDR), addr)?;
        self.data
            .write_cell(self.layout_addr(Layout::DIAGNOSTIC_LEN), len)?;
        Ok(())
    }

    pub(super) fn source(&self) -> Result<(usize, usize, usize)> {
        let addr = self.data.read_cell(self.layout_addr(Layout::SOURCE_ADDR))?;
        let len = self.data.read_cell(self.layout_addr(Layout::SOURCE_LEN))?;
        let to_in = self.to_in()?;
        Ok((addr, len, to_in))
    }

    pub(crate) fn input_mut(&mut self) -> Result<&mut [u8]> {
        let addr = self.layout_addr(Layout::INPUT);
        Ok(self.data.slice_mut(addr, INPUT_BUFFER_SIZE)?)
    }

    pub(super) fn set_source(&mut self, code: &[u8]) -> Result<()> {
        if code.len() > INPUT_BUFFER_SIZE {
            return Err(Error::Throw(Ior::PARSED_STRING_OVERFLOW));
        }
        let input_addr = self.layout_addr(Layout::INPUT);
        self.data.write(input_addr, code)?;
        self.set_source_len(code.len())
    }

    pub(crate) fn set_source_len(&mut self, len: usize) -> Result<()> {
        let input_addr = self.layout_addr(Layout::INPUT);
        self.data
            .write_cell(self.layout_addr(Layout::SOURCE_ADDR), input_addr)?;
        self.data
            .write_cell(self.layout_addr(Layout::SOURCE_LEN), len)?;
        self.data
            .write_cell(self.layout_addr(Layout::SOURCE_ID), -1isize as usize)?;
        self.set_to_in(0)
    }

    fn layout_addr(&self, offset: usize) -> usize {
        self.base_addr + offset
    }
}

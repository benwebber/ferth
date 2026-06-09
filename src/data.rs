//! The system data space.
use crate::types::AAddr;
use crate::{Error, Result};

pub trait Mem: AsRef<[u8]> + AsMut<[u8]> {}

impl<M: AsRef<[u8]> + AsMut<[u8]>> Mem for M {}

/// The system data space.
///
/// The data space is generic over any type that implements [`Mem`], such as `[u8]`, `&[u8]`, and
/// `Box<[u8]>`.
pub struct Data<M: Mem> {
    mem: M,
}

impl<M: Mem> Data<M> {
    pub fn new(mem: M) -> Self {
        Self { mem }
    }

    /// Return the size of the data space in bytes.
    pub fn len(&self) -> usize {
        self.mem.as_ref().len()
    }

    /// Read a slice of bytes.
    pub fn read(&self, addr: usize, len: usize) -> Result<&[u8]> {
        let end = addr
            .checked_add(len)
            .ok_or(Error::AddressOutOfRange(addr))?;
        self.mem
            .as_ref()
            .get(addr..end)
            .ok_or(Error::AddressOutOfRange(addr))
    }

    /// Read a single cell.
    pub fn read_cell(&self, addr: usize) -> Result<usize> {
        const SIZE: usize = size_of::<usize>();
        AAddr::try_from(addr)?;
        let bytes = self.read(addr, SIZE)?;
        let mut buf = [0u8; SIZE];
        buf.copy_from_slice(bytes);
        Ok(usize::from_le_bytes(buf))
    }

    /// Read a single character (byte).
    pub fn read_char(&self, addr: usize) -> Result<u8> {
        self.mem
            .as_ref()
            .get(addr)
            .ok_or(Error::AddressOutOfRange(addr))
            .copied()
    }

    /// Write a slice of bytes.
    pub fn write(&mut self, addr: usize, bytes: &[u8]) -> Result<()> {
        let end = addr
            .checked_add(bytes.len())
            .ok_or(Error::AddressOutOfRange(addr))?;
        let dst = self
            .mem
            .as_mut()
            .get_mut(addr..end)
            .ok_or(Error::AddressOutOfRange(addr))?;
        dst.copy_from_slice(bytes);
        Ok(())
    }

    /// Write a single cell.
    pub fn write_cell(&mut self, addr: usize, x: usize) -> Result<()> {
        AAddr::try_from(addr)?;
        self.write(addr, &x.to_le_bytes())
    }

    /// Write a single character (byte).
    pub fn write_char(&mut self, addr: usize, c: u8) -> Result<()> {
        *self
            .mem
            .as_mut()
            .get_mut(addr)
            .ok_or(Error::AddressOutOfRange(addr))? = c;
        Ok(())
    }
}

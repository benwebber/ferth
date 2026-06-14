//! The system data space.
use crate::SIZE;
use crate::vm::{VmError, VmResult};

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
    pub fn read(&self, addr: usize, len: usize) -> VmResult<&[u8]> {
        let end = addr
            .checked_add(len)
            .ok_or(VmError::AddressOutOfRange(addr))?;
        self.mem
            .as_ref()
            .get(addr..end)
            .ok_or(VmError::AddressOutOfRange(addr))
    }

    /// Read a single cell.
    pub fn read_cell(&self, addr: usize) -> VmResult<usize> {
        if !addr.is_multiple_of(SIZE) {
            return Err(VmError::AddressMisaligned(addr));
        }
        let bytes = self.read(addr, SIZE)?;
        let mut buf = [0u8; SIZE];
        buf.copy_from_slice(bytes);
        Ok(usize::from_le_bytes(buf))
    }

    #[cfg(feature = "unsafe")]
    pub unsafe fn read_cell_unchecked(&self, addr: usize) -> usize {
        unsafe {
            usize::from_le_bytes(*(self.mem.as_ref().as_ptr().add(addr) as *const [u8; SIZE]))
        }
    }

    /// Read a single character (byte).
    pub fn read_char(&self, addr: usize) -> VmResult<u8> {
        self.mem
            .as_ref()
            .get(addr)
            .ok_or(VmError::AddressOutOfRange(addr))
            .copied()
    }

    /// Write a slice of bytes.
    pub fn write(&mut self, addr: usize, bytes: &[u8]) -> VmResult<()> {
        let end = addr
            .checked_add(bytes.len())
            .ok_or(VmError::AddressOutOfRange(addr))?;
        let dst = self
            .mem
            .as_mut()
            .get_mut(addr..end)
            .ok_or(VmError::AddressOutOfRange(addr))?;
        dst.copy_from_slice(bytes);
        Ok(())
    }

    /// Write a single cell.
    pub fn write_cell(&mut self, addr: usize, x: usize) -> VmResult<()> {
        if !addr.is_multiple_of(size_of::<usize>()) {
            return Err(VmError::AddressMisaligned(addr));
        }
        self.write(addr, &x.to_le_bytes())
    }

    #[cfg(feature = "unsafe")]
    pub unsafe fn write_cell_unchecked(&mut self, addr: usize, x: usize) {
        const SIZE: usize = size_of::<usize>();
        unsafe { *(self.mem.as_mut().as_mut_ptr().add(addr) as *mut [u8; SIZE]) = x.to_le_bytes() };
    }

    /// Write a single character (byte).
    pub fn write_char(&mut self, addr: usize, c: u8) -> VmResult<()> {
        *self
            .mem
            .as_mut()
            .get_mut(addr)
            .ok_or(VmError::AddressOutOfRange(addr))? = c;
        Ok(())
    }
}

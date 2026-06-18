use crate::Result;

pub trait Host {
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
    fn diagnostic(&mut self, addr: usize, u: usize) -> Result<()>;
    fn lookup(&self, name: &[u8]) -> Result<Option<(usize, isize)>>;
    fn write_header(&mut self, name: &[u8], flags: u8) -> Result<usize>;
    fn layout_addr(&self, offset: usize) -> usize;
}

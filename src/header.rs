use core::mem::offset_of;
use core::ops::BitOr;

use crate::SIZE;

#[repr(C)]
struct Layout {
    bodylen: usize,
    info: usize,
    link: usize,
    code: usize,
}

impl Layout {
    const BODYLEN: usize = offset_of!(Self, bodylen);
    const INFO: usize = offset_of!(Self, info);
    const LINK: usize = offset_of!(Self, link);
    const CODE: usize = offset_of!(Self, code);
}

/// The header starts with a variable-length `pad` field that ensures the `info` field always
/// aligns to a cell address.
///
/// The length of the name follows as a single byte, then the bytes of the name.
///
/// The `bodylen` field encodes the length of the body in cells.
///
/// The `info` field packs the flags into the least significant byte and the length into the
/// next byte. It currently reserves two additional bytes of space.
///
/// The `link` field links to the `code` field of the next word in the dictionary.
///
/// The `code` field contains an [`Op`][crate::vm::Op] code. The compiled `body` of the word follows the op code.
///
/// Assuming a 32-bit cell size, the header looks like this in memory:
///
/// ```text
///  0 1 2 3 4 5 6 7 8 9 a b c d e f 0 1 2 3 4 5 6 7 8 9 a b c d e f
/// +---------------+---------------+-------------------------------+
/// |      pad...   |      len      |             name...           |
/// +---------------+---------------+-------------------------------+
/// |                              name...                          |
/// +---------------------------------------------------------------+
/// |                            bodylen                            |
/// +---------------+---------------+-------------------------------+
/// |  info (len)   | info (flags)  |        info (reserved)        |
/// +---------------+---------------+-------------------------------+
/// |                              link                             |
/// +---------------------------------------------------------------+
/// |                              code                             |
/// +---------------------------------------------------------------+
/// |                              body...                          |
/// +---------------------------------------------------------------+
/// ```
///
pub struct Header(usize);

impl Header {
    pub fn new(addr: usize) -> Self {
        Self(addr)
    }

    pub(crate) fn from_name_len(here: usize, len: usize) -> Self {
        let bodylen_addr = (here + 1 + len).next_multiple_of(SIZE);
        let offset = Layout::CODE - Layout::BODYLEN;
        Header::new(bodylen_addr + offset)
    }

    pub fn code_addr(&self) -> usize {
        self.0
    }

    pub fn link_addr(&self) -> usize {
        self.0 - (Layout::CODE - Layout::LINK)
    }

    pub fn info_addr(&self) -> usize {
        self.0 - (Layout::CODE - Layout::INFO)
    }

    pub fn bodylen_addr(&self) -> usize {
        self.0 - (Layout::CODE - Layout::BODYLEN)
    }
}

#[derive(Clone, Copy)]
pub(super) struct Info(usize);

impl Info {
    pub const fn new(flags: Flags, len: u8) -> Self {
        Self((len as usize) | ((flags.0 as usize) << 8))
    }
    pub const fn name_len(self) -> usize {
        self.0 & 0xff
    }
    pub const fn flags(self) -> Flags {
        Flags((self.0 >> 8) as u8)
    }
}

impl From<usize> for Info {
    fn from(u: usize) -> Self {
        Self(u)
    }
}

impl From<Info> for usize {
    fn from(info: Info) -> Self {
        info.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Flags(pub u8);

impl Flags {
    pub const EMPTY: Self = Self(0);
    pub const IMMEDIATE: Self = Self(0b0000001);
    pub const HIDDEN: Self = Self(0b0000010);
    pub const BOOTSTRAP: Self = Self(0b0000100);
    pub const PRIMITIVE: Self = Self(0b0001000);
    pub const COMPILE_ONLY: Self = Self(0b0010000);

    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}

impl From<u8> for Flags {
    fn from(u: u8) -> Self {
        Self(u)
    }
}

impl From<Flags> for u8 {
    fn from(flags: Flags) -> Self {
        flags.0
    }
}

impl From<Flags> for usize {
    fn from(flags: Flags) -> Self {
        flags.0 as usize
    }
}

impl BitOr<Flags> for Flags {
    type Output = Self;

    fn bitor(self, rhs: Flags) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

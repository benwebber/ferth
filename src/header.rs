use core::mem::offset_of;
use core::ops::{BitAnd, BitOr};

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
    #[allow(dead_code)]
    const LINK: usize = offset_of!(Self, link);
    const CODE: usize = offset_of!(Self, code);
}

pub struct Header(usize);

impl Header {
    pub fn new(addr: usize) -> Self {
        Self(addr)
    }

    #[allow(dead_code)]
    pub fn code_addr(&self) -> usize {
        self.0
    }

    #[allow(dead_code)]
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
    #[allow(dead_code)]
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
    pub const IMMEDIATE: Self = Self(0b0000001);
    pub const HIDDEN: Self = Self(0b0000010);
    pub const BOOTSTRAP: Self = Self(0b0000100);
    pub const PRIMITIVE: Self = Self(0b0001000);
    pub const BUILTIN: Self = Self(0b0010000);
    pub const COLON: Self = Self(0b0100000);
    pub const CREATE: Self = Self(0b1000000);

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

impl BitAnd<Flags> for Flags {
    type Output = Self;

    fn bitand(self, rhs: Flags) -> Self::Output {
        self & rhs.0
    }
}

impl BitOr<Flags> for Flags {
    type Output = Self;

    fn bitor(self, rhs: Flags) -> Self::Output {
        self | rhs.0
    }
}

impl BitAnd<u8> for Flags {
    type Output = Self;

    fn bitand(self, rhs: u8) -> Self::Output {
        Self(self.0 & rhs)
    }
}

impl BitOr<u8> for Flags {
    type Output = Self;

    fn bitor(self, rhs: u8) -> Self::Output {
        Self(self.0 | rhs)
    }
}

impl BitAnd<Flags> for u8 {
    type Output = Self;

    fn bitand(self, rhs: Flags) -> Self::Output {
        self & rhs.0
    }
}

impl BitOr<Flags> for u8 {
    type Output = Self;

    fn bitor(self, rhs: Flags) -> Self::Output {
        self | rhs.0
    }
}

impl PartialEq<u8> for Flags {
    fn eq(&self, other: &u8) -> bool {
        self.0 == *other
    }
}

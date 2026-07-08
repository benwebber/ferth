//! A safe, native sized Forth.
//!
//! # Features
//!
//! * `std`: Add support for standard input and output.
//! * `repl`: Build the REPL.
//! ```rust
//! use ferth::Ferth;
//! use ferth::host::NullHost;
//!
//! let mut fe = Ferth::new([0u8; 65536], NullHost).unwrap();
//! fe.evaluate("
//! : square ( n -- n ) dup * ;
//! 2 dup square dup square dup square
//! ").unwrap();
//! assert_eq!(fe.stack().count(), 4);
//! assert_eq!(fe.stack().collect::<Vec<_>>(), vec![2, 4, 16, 256]);
//! ```
#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(not(feature = "unsafe"), forbid(unsafe_code))]

mod data;
pub mod double;
pub mod error;
mod ferth;
mod header;
pub mod host;
mod kernel;
mod log;
mod packed;
mod parser;
mod state;
pub mod time;
mod vm;

pub use error::{Error, Result};
pub use ferth::Ferth;
pub use kernel::Config;

/// The size of a cell in bytes.
pub const SIZE: usize = size_of::<usize>();
/// Boolean true. A cell with all bits set ([`usize::MAX`]).
pub const TRUE: usize = usize::MAX;
/// Boolean false. A cell with no bits set (`0`).
pub const FALSE: usize = 0;
/// SPACE.
const BL: usize = 0x20;

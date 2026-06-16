//! A safe, native sized Forth.
//!
//! # Features
//!
//! * `std`: Add support for standard input and output.
//! * `repl`: Build the REPL.
//! ```rust
//! use ferth::Fe;
//! use ferth::io::NoIo;
//!
//! let mut fe = Fe::new([0u8; 65536], NoIo).unwrap();
//! fe.evaluate(b"
//! : square ( n -- n ) dup * ;
//! 2 dup square dup square dup square
//! ").unwrap();
//! assert_eq!(fe.stack().count(), 4);
//! assert_eq!(fe.stack().collect::<Vec<_>>(), vec![2, 4, 16, 256]);
//! ```
#![cfg_attr(not(feature = "std"), no_std)]

mod data;
mod error;
mod fe;
pub mod io;
mod parser;
mod types;
mod vm;

pub use error::{Error, Result};
pub use fe::Fe;

pub const SIZE: usize = size_of::<usize>();
pub const TRUE: usize = usize::MAX;
pub const FALSE: usize = 0;

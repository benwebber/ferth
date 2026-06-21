use crate::double::Double;
use crate::error::Ior;
use crate::parser;
use crate::vm::Op;
use crate::{Error, Result};

use super::{
    BUILTIN, FALSE, Host, INFO_FROM_CFA, INPUT_BUFFER_SIZE, Layout, MAX_WORD_LEN, PRIMITIVE, SIZE,
    TRUE,
};

/// Receive a single character from the input device.
///
/// ```text
/// key ( -- char )
/// ```
///
/// See [`KEY`](https://forth-standard.org/standard/core/KEY).
pub fn key(host: &mut dyn Host) -> Result<()> {
    match host.key()? {
        Some(c) => host.push(c as usize),
        None => Err(Error::Io),
    }
}

/// Display a single character.
///
/// ```text
/// emit ( x -- )
/// ```
///
/// See [`EMIT`](https://forth-standard.org/standard/core/EMIT).
pub fn emit(host: &mut dyn Host) -> Result<()> {
    // TODO: What if the TOS is not a char?
    let c = host.pop()? as u8;
    host.emit(c)
}

/// A variant of `find` that reads a Forth string `( c-addr u )` instead of a counted string `(
/// c-addr )`.
///
/// ```text
/// (find) ( c-addr u -- 0 | xt 1 | xt -1 )
/// ```
///
/// Similar to [`search-wordlist`] except it does not accept a wordlist ID.
///
/// [`search-wordlist`]: https://forth-standard.org/standard/search/SEARCH-WORDLIST
pub fn find(host: &mut dyn Host) -> Result<()> {
    let len = host.pop()?;
    let addr = host.pop()?;
    if len > MAX_WORD_LEN {
        host.set_diagnostic(addr, len)?;
        return Err(Error::Throw(Ior::DEFINITION_NAME_TOO_LONG));
    }
    let mut buf = [0u8; MAX_WORD_LEN];
    buf[..len].copy_from_slice(host.read(addr, len)?);
    match host.find(&buf[..len])? {
        Some((xt, flag)) => {
            host.push(xt)?;
            host.push(flag as usize)
        }
        None => host.push(0),
    }
}

pub fn numberq(host: &mut dyn Host) -> Result<()> {
    let len = host.pop()?;
    let caddr = host.pop()?;
    let base = host.read_cell(host.layout_addr(Layout::BASE))?;
    if let Some(n) = parser::parse_num(host.read(caddr, len)?, base as u32) {
        host.push(n)?;
        host.push(1)
    } else {
        host.push(caddr)?;
        host.push(len)?;
        host.push(0)
    }
}

/// Parse digits and add them to an accumulator.
///
/// ```text
/// >number ( ud1 c-addr1 u1 -- ud2 c-addr2 u2 )
/// ```
#[allow(clippy::wrong_self_convention)]
pub fn to_number(host: &mut dyn Host) -> Result<()> {
    let u = host.pop()?;
    let caddr = host.pop()?;
    let hi = host.pop()?;
    let lo = host.pop()?;
    let acc = Double::from((lo, hi));
    let bytes = host.read(caddr, u)?;
    // TODO: Check base size.
    let base = host.read_cell(host.layout_addr(Layout::BASE))? as u32;
    let (acc, rest) = parser::to_number(acc, bytes, base);
    let len = bytes.len() - rest.len();
    let (lo, hi): (usize, usize) = acc.into();
    let caddr2 = caddr + len;
    let u2 = rest.len();
    host.push(lo)?;
    host.push(hi)?;
    host.push(caddr2)?;
    host.push(u2)
}

/// Parse the next token in the parse area.
///
/// ```text
/// parse ( char "ccc<char>" -- c-addr u )
/// ```
///
/// See [`PARSE`](https://forth-standard.org/standard/core/PARSE).
pub fn parse(host: &mut dyn Host) -> Result<()> {
    let delim = host.pop()? as u8;
    let src = host.read_cell(host.layout_addr(Layout::SOURCE_ADDR))?;
    let src_len = host.read_cell(host.layout_addr(Layout::SOURCE_LEN))?;
    let mut to_in = host.read_cell(host.layout_addr(Layout::TO_IN))?;
    let start = to_in;
    let is_delim = |c: u8| {
        if delim == b' ' {
            c.is_ascii_whitespace()
        } else {
            c == delim
        }
    };
    while to_in < src_len && !is_delim(host.read_char(src + to_in)?) {
        to_in += 1;
    }
    let len = to_in - start;
    if to_in < src_len {
        to_in += 1;
    }
    host.write_cell(host.layout_addr(Layout::TO_IN), to_in)?;
    host.push(src + start)?;
    host.push(len)
}

/// Attempt to fill the input buffer from the input source.
///
/// ```text
/// refill ( -- flag )
/// ```
///
/// See [`REFILL`](https://forth-standard.org/standard/core/REFILL).
pub fn refill(host: &mut dyn Host) -> Result<()> {
    let mut buf = [0u8; INPUT_BUFFER_SIZE];
    let input_addr = host.layout_addr(Layout::INPUT);
    match host.refill(&mut buf) {
        Ok(Some(len)) => {
            host.write(input_addr, &buf[..len])?;
            host.write_cell(host.layout_addr(Layout::SOURCE_ADDR), input_addr)?;
            host.write_cell(host.layout_addr(Layout::SOURCE_LEN), len)?;
            host.write_cell(host.layout_addr(Layout::TO_IN), 0)?;
            host.push(TRUE)?;
            Ok(())
        }
        Ok(None) => {
            host.push(FALSE)?;
            Ok(())
        }
        Err(e) => Err(e),
    }
}

/// Create a new dictionary header.
///
/// ```text
/// (header) ( c-addr u -- )
/// ```
///
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
/// The `code` field contains an [`Op`] code. The compiled `body` of the word, if it exists,
/// follows the `code` field.
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
/// After `(header)` executes, `here` points to the `code` field address.
pub fn header(host: &mut dyn Host) -> Result<()> {
    let len = host.pop()?;
    let addr = host.pop()?;
    if len > MAX_WORD_LEN {
        host.set_diagnostic(addr, len)?;
        return Err(Error::Throw(Ior::DEFINITION_NAME_TOO_LONG));
    }
    let mut buf = [0u8; MAX_WORD_LEN];
    buf[..len].copy_from_slice(host.read(addr, len)?);
    let cfa = host.create(&buf[..len], 0)?;
    host.write_cell(host.layout_addr(Layout::LATEST), cfa)?;
    host.write_cell(host.layout_addr(Layout::HERE), cfa)?;
    Ok(())
}

/// Compile a call to *xt* to the current definition.
///
/// ```text
/// compile, ( xt -- )
/// ```
///
/// In indirect-threaded systems, `,` can perform the function of `compile,`. This does not always
/// hold for other threading models.
pub fn compile_comma(host: &mut dyn Host) -> Result<()> {
    let xt = host.pop()?;
    let kind = (host.read_cell(xt - INFO_FROM_CFA)? >> 8) as u8;
    let comma = |host: &mut dyn Host, x: usize| -> Result<()> {
        let here = host.read_cell(host.layout_addr(Layout::HERE))?;
        host.write_cell(here, x)?;
        host.write_cell(host.layout_addr(Layout::HERE), here + SIZE)
    };
    if kind & PRIMITIVE != 0 {
        let op = host.read_cell(xt)? & 0xff;
        comma(host, op)
    } else if kind & BUILTIN != 0 {
        let index = host.read_cell(xt + SIZE)?;
        comma(host, Op::Yield as usize)?;
        comma(host, index)
    } else {
        comma(host, Op::Call as usize)?;
        comma(host, xt)
    }
}

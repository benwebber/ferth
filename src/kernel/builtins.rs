use crate::error::Ior;
use crate::{Error, Result};

use super::dict;
use super::{FALSE, Host, INPUT_BUFFER_SIZE, Layout, MAX_WORD_LEN, TRUE};

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
        dict::set_diagnostic(host, addr, len)?;
        return Err(Error::Throw(Ior::DEFINITION_NAME_TOO_LONG));
    }
    let mut buf = [0u8; MAX_WORD_LEN];
    buf[..len].copy_from_slice(host.read(addr, len)?);
    match dict::find(host, &buf[..len])? {
        Some((xt, flag)) => {
            host.push(xt)?;
            host.push(flag as usize)
        }
        None => host.push(0),
    }
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
/// After `(header)` executes, `here` points to the `code` field address.
pub fn header(host: &mut dyn Host) -> Result<()> {
    let len = host.pop()?;
    let addr = host.pop()?;
    if len > MAX_WORD_LEN {
        dict::set_diagnostic(host, addr, len)?;
        return Err(Error::Throw(Ior::DEFINITION_NAME_TOO_LONG));
    }
    let mut buf = [0u8; MAX_WORD_LEN];
    buf[..len].copy_from_slice(host.read(addr, len)?);
    let cfa = dict::create(host, &buf[..len], 0)?;
    host.write_cell(host.layout_addr(Layout::LATEST), cfa)?;
    host.write_cell(host.layout_addr(Layout::HERE), cfa)?;
    Ok(())
}

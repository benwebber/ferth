use crate::data::Mem;
use crate::error::Ior;
use crate::io::Io;
use crate::{Error, Result};

use super::context::Context;
use super::{FALSE, INPUT_BUFFER_SIZE, MAX_WORD_LEN, TRUE};

/// Receive a single character from the input device.
///
/// ```text
/// key ( -- char )
/// ```
///
/// See [`KEY`](https://forth-standard.org/standard/core/KEY).
pub fn key<M: Mem, I: Io>(ctx: &mut Context<'_, M, I>) -> Result<()> {
    match ctx.key()? {
        Some(c) => ctx.push(c as usize),
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
pub fn emit<M: Mem, I: Io>(ctx: &mut Context<'_, M, I>) -> Result<()> {
    // TODO: What if the TOS is not a char?
    let c = ctx.pop()? as u8;
    ctx.emit(c)
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
pub fn find<M: Mem, I: Io>(ctx: &mut Context<'_, M, I>) -> Result<()> {
    let len = ctx.pop()?;
    let addr = ctx.pop()?;
    if len > MAX_WORD_LEN {
        ctx.dict().set_diagnostic(addr, len)?;
        return Err(Error::Throw(Ior::DEFINITION_NAME_TOO_LONG));
    }
    match ctx.dict().find_at(addr, len)? {
        Some((xt, flag)) => {
            ctx.push(xt)?;
            ctx.push(flag)
        }
        None => ctx.push(0),
    }
}

/// Attempt to fill the input buffer from the input source.
///
/// ```text
/// refill ( -- flag )
/// ```
///
/// See [`REFILL`](https://forth-standard.org/standard/core/REFILL).
pub fn refill<M: Mem, I: Io>(ctx: &mut Context<'_, M, I>) -> Result<()> {
    let mut buf = [0u8; INPUT_BUFFER_SIZE];
    match ctx.refill(&mut buf) {
        Ok(Some(len)) => {
            ctx.dict().set_source(&buf[..len])?;
            ctx.push(TRUE)?;
            Ok(())
        }
        Ok(None) => {
            ctx.push(FALSE)?;
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
pub fn header<M: Mem, I: Io>(ctx: &mut Context<'_, M, I>) -> Result<()> {
    let len = ctx.pop()?;
    let addr = ctx.pop()?;
    if len > MAX_WORD_LEN {
        ctx.dict().set_diagnostic(addr, len)?;
        return Err(Error::Throw(Ior::DEFINITION_NAME_TOO_LONG));
    }
    let cfa = ctx.dict().create_at(addr, len, 0)?;
    ctx.dict().set_latest(cfa)?;
    ctx.dict().set_here(cfa)
}

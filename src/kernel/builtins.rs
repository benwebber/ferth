use crate::data::Mem;
use crate::error::Ior;
use crate::io::Io;
use crate::{Error, Result};

use super::context::Context;
use super::{FALSE, MAX_WORD_LEN, TRUE};

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
    match ctx.refill() {
        Ok(Some(len)) => {
            ctx.dict().set_source_len(len)?;
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
    let code_addr = ctx.dict().create_at(addr, len, 0)?;
    ctx.dict().set_latest(code_addr)?;
    ctx.dict().set_here(code_addr)
}

/// Return the current time and date, in UTC.
///
/// ```text
/// time&date ( -- ss mm hh DD MM YYYY )
/// ```
#[cfg(feature = "time")]
pub fn time_and_date<M: Mem, I: Io>(ctx: &mut Context<'_, M, I>) -> Result<()> {
    use chrono::{Datelike, Timelike};
    let now = chrono::Utc::now();
    ctx.push(now.second() as usize)?;
    ctx.push(now.minute() as usize)?;
    ctx.push(now.hour() as usize)?;
    ctx.push(now.day() as usize)?;
    ctx.push(now.month() as usize)?;
    ctx.push(now.year() as usize)
}

/// Wait for *u* milliseconds.
///
/// ```text
/// ms ( u -- )
/// ```
#[cfg(feature = "std")]
pub fn ms<M: Mem, I: Io>(ctx: &mut Context<'_, M, I>) -> Result<()> {
    use std::thread;
    use std::time::Duration;
    let ms = ctx.pop()? as u64;
    let d = Duration::from_millis(ms);
    thread::sleep(d);
    Ok(())
}

/// Push the value of a monotonic clock in microseconds.
///
/// ```text
/// (utime) ( -- ud )
/// ```
#[cfg(feature = "std")]
pub fn utime<M: Mem, I: Io>(ctx: &mut Context<'_, M, I>) -> Result<()> {
    use crate::double::Double;
    use std::sync::LazyLock;
    use std::time::Instant;
    static EPOCH: LazyLock<Instant> = LazyLock::new(Instant::now);
    let d = Instant::now() - *EPOCH;
    let us = d.as_micros() as u64;
    let (lo, hi): (usize, usize) = Double::from(us).into();
    ctx.push(lo)?;
    ctx.push(hi)
}

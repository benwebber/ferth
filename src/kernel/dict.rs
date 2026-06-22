use crate::error::{Error, Ior};
use crate::header::{Flags, Header, Info};
use crate::{Result, SIZE};

use super::layout::Layout;
use super::{Host, MAX_WORD_LEN};

pub(crate) fn find<H: Host + ?Sized>(host: &H, name: &[u8]) -> Result<Option<(usize, isize)>> {
    if name.len() > MAX_WORD_LEN {
        return Ok(None);
    }
    let mut xt = host.read_cell(host.layout_addr(Layout::LATEST))?;
    while xt != 0 {
        let header = Header::new(xt);
        let info: Info = host.read_cell(header.info_addr())?.into();
        let flags = info.flags();
        let wlen = info.name_len();
        if !flags.contains(Flags::HIDDEN) && wlen == name.len() {
            let name_at = header.bodylen_addr() - wlen;
            let b = host.read(name_at, wlen)?;
            if name.eq_ignore_ascii_case(b) {
                let flag = if flags.contains(Flags::IMMEDIATE) {
                    1
                } else {
                    -1
                };
                return Ok(Some((xt, flag)));
            }
        }
        xt = host.read_cell(header.link_addr())?;
    }
    Ok(None)
}

pub(crate) fn create<H: Host + ?Sized>(host: &mut H, name: &[u8], flags: u8) -> Result<usize> {
    let len: u8 = name
        .len()
        .try_into()
        .map_err(|_| Error::Throw(Ior::DEFINITION_NAME_TOO_LONG))?;
    let latest = host.read_cell(host.layout_addr(Layout::LATEST))?;
    let here = host.read_cell(host.layout_addr(Layout::HERE))?;
    // pad the name so as to always align info
    let pad = (SIZE - ((here + 1 + len as usize) % SIZE)) % SIZE;
    // name
    let nfa = here + pad;
    host.write_char(nfa, len)?;
    host.write(nfa + 1, name)?;
    // bodylen (0 until ;)
    let body_len = nfa + 1 + len as usize;
    host.write_cell(body_len, 0)?;
    // info
    let info = body_len + SIZE;
    host.write_cell(info, Info::new(flags.into(), len).into())?;
    // link
    let link = info + SIZE;
    host.write_cell(link, latest)?;
    // code
    let cfa = link + SIZE;
    Ok(cfa)
}

pub(crate) fn set_diagnostic<H: Host + ?Sized>(
    host: &mut H,
    addr: usize,
    len: usize,
) -> Result<()> {
    host.write_cell(host.layout_addr(Layout::DIAGNOSTIC_ADDR), addr)?;
    host.write_cell(host.layout_addr(Layout::DIAGNOSTIC_LEN), len)?;
    Ok(())
}

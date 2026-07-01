use crate::double::Double;
use crate::vm::{VmError, VmResult};

pub fn parse_num(bytes: &[u8], base: u32) -> Option<usize> {
    if bytes.is_empty() {
        return None;
    }

    if bytes.len() == 3 && bytes[0] == b'\'' && bytes[2] == b'\'' {
        return Some(bytes[1] as usize);
    }

    let (base, rest) = if let Some((&b'#', rest)) = bytes.split_first() {
        (10u32, rest)
    } else if let Some((&b'$', rest)) = bytes.split_first() {
        (16u32, rest)
    } else if let Some((&b'%', rest)) = bytes.split_first() {
        (2u32, rest)
    } else {
        (base, bytes)
    };

    let (neg, rest) = if let Some((&b'-', rest)) = rest.split_first() {
        (true, rest)
    } else {
        (false, rest)
    };

    if rest.is_empty() {
        return None;
    }

    let (acc, rest) = to_number(Double(0), rest, base);

    if !rest.is_empty() {
        return None;
    }

    if neg {
        Some(acc.0.wrapping_neg() as usize)
    } else {
        Some(acc.0 as usize)
    }
}

// TODO: This should return its own error, not a VM error.
pub fn parse_escaped(src: &[u8], dst: &mut [u8]) -> VmResult<(usize, usize)> {
    let mut input_pos = 0;
    let mut output_pos = 0;
    let mut put = |i: &mut usize, c: u8| -> VmResult<()> {
        *dst.get_mut(*i).ok_or(VmError::ParsedStringOverflow)? = c;
        *i += 1;
        Ok(())
    };
    while input_pos < src.len() {
        match src[input_pos] {
            b'"' => {
                input_pos += 1;
                break;
            }
            b'\\' => {
                input_pos += 1;
                let c = *src.get(input_pos).ok_or(VmError::InvalidEscape(b'\\'))?;
                input_pos += 1;
                match c {
                    b'a' => put(&mut output_pos, 0x07)?,  // BEL
                    b'b' => put(&mut output_pos, 0x08)?,  // BS
                    b'e' => put(&mut output_pos, 0x1b)?,  // ESC
                    b'f' => put(&mut output_pos, 0x0c)?,  // FF
                    b'l' => put(&mut output_pos, b'\n')?, // LF
                    b'm' => {
                        put(&mut output_pos, 0x0d)?; // CR
                        put(&mut output_pos, 0x0a)?; // LF
                    }
                    b'n' => put(&mut output_pos, 0x0a)?, // LF
                    b'q' | b'"' => put(&mut output_pos, b'"')?, // "
                    b'r' => put(&mut output_pos, b'\r')?, // CR
                    b't' => put(&mut output_pos, b'\t')?, // HT
                    b'v' => put(&mut output_pos, 0x0b)?, // VT
                    b'z' => put(&mut output_pos, b'\0')?, // NUL
                    b'\\' => put(&mut output_pos, b'\\')?,
                    b'x' => {
                        let hex = &src[input_pos..src.len().min(input_pos + 2)];
                        let (n, rest) = to_number(Double(0), hex, 16);
                        if hex.len() - rest.len() != 2 {
                            return Err(VmError::InvalidEscape(b'x'));
                        }
                        input_pos += 2;
                        put(&mut output_pos, n.0 as u8)?;
                    }
                    _ => return Err(VmError::InvalidEscape(c)),
                }
            }
            b => {
                put(&mut output_pos, b)?;
                input_pos += 1;
            }
        }
    }
    Ok((input_pos, output_pos))
}

pub fn to_number(mut acc: Double, bytes: &[u8], base: u32) -> (Double, &[u8]) {
    let digit = |c: u8| -> Option<u32> {
        let d = match c {
            b'0'..=b'9' => u32::from(c - b'0'),
            b'a'..=b'z' => u32::from(c - b'a') + 10,
            b'A'..=b'Z' => u32::from(c - b'A') + 10,
            _ => return None,
        };
        (d < base).then_some(d)
    };
    let mut rest = bytes;
    while let Some((&c, tail)) = rest.split_first() {
        let Some(d) = digit(c) else { break };
        acc = Double(acc.0 * Double::from(base).0 + Double::from(d).0);
        rest = tail;
    }
    (acc, rest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_num_char() {
        assert_eq!(parse_num(b"' '", 10), Some(0x20));
    }

    #[test]
    fn parse_num_decimal() {
        assert_eq!(parse_num(b"10", 10), Some(10));
        assert_eq!(parse_num(b"#10", 8), Some(10));
        assert_eq!(parse_num(b"-10", 10), Some((-10isize) as usize));
    }

    #[test]
    fn parse_num_hex() {
        assert_eq!(parse_num(b"$10", 10), Some(16));
        assert_eq!(parse_num(b"ff", 16), Some(255));
    }

    #[test]
    fn parse_num_binary() {
        assert_eq!(parse_num(b"%10", 10), Some(2));
    }

    #[test]
    fn parse_num_invalid() {
        assert_eq!(parse_num(b"", 10), None);
        assert_eq!(parse_num(b"-", 10), None);
        assert_eq!(parse_num(b"foo", 10), None);
        assert_eq!(parse_num(b"$", 10), None);
    }

    #[test]
    fn to_number_decimal() {
        assert_eq!(to_number(Double(0), b"123", 10), (Double(123), &b""[..]));
    }

    #[test]
    fn to_number_empty() {
        assert_eq!(to_number(Double(42), b"", 10), (Double(42), &b""[..]));
    }

    #[test]
    fn to_number_partial() {
        assert_eq!(to_number(Double(0), b"10z", 10), (Double(10), &b"z"[..]));
    }

    #[test]
    fn to_number_continues() {
        assert_eq!(
            to_number(Double(0xff), b"ff", 16),
            (Double(0xffff), &b""[..])
        );
    }

    #[test]
    fn to_number_case_folding() {
        assert_eq!(to_number(Double(0), b"abc", 16), (Double(0xabc), &b""[..]));
        assert_eq!(to_number(Double(0), b"ABC", 16), (Double(0xabc), &b""[..]));
    }

    #[test]
    fn parse_escaped_literal() {
        let mut dst = [0u8; 8];
        assert_eq!(parse_escaped(br#"abc"xyz"#, &mut dst), Ok((4, 3)));
        assert_eq!(&dst[..3], b"abc");
    }

    #[test]
    fn parse_escaped_alert() {
        let mut dst = [0u8; 8];
        assert_eq!(parse_escaped(br#"\a""#, &mut dst), Ok((3, 1)));
        assert_eq!(&dst[..1], &[0x07]);
    }

    #[test]
    fn parse_escaped_backspace() {
        let mut dst = [0u8; 8];
        assert_eq!(parse_escaped(br#"\b""#, &mut dst), Ok((3, 1)));
        assert_eq!(&dst[..1], &[0x08]);
    }

    #[test]
    fn parse_escaped_escape() {
        let mut dst = [0u8; 8];
        assert_eq!(parse_escaped(br#"\e""#, &mut dst), Ok((3, 1)));
        assert_eq!(&dst[..1], &[0x1b]);
    }

    #[test]
    fn parse_escaped_form_feed() {
        let mut dst = [0u8; 8];
        assert_eq!(parse_escaped(br#"\f""#, &mut dst), Ok((3, 1)));
        assert_eq!(&dst[..1], &[0x0c]);
    }

    #[test]
    fn parse_escaped_line_feed() {
        let mut dst = [0u8; 8];
        assert_eq!(parse_escaped(br#"\l""#, &mut dst), Ok((3, 1)));
        assert_eq!(&dst[..1], &[0x0a]);
    }

    #[test]
    fn parse_escaped_crlf() {
        let mut dst = [0u8; 8];
        assert_eq!(parse_escaped(br#"\m""#, &mut dst), Ok((3, 2)));
        assert_eq!(&dst[..2], &[0x0d, 0x0a]);
    }

    #[test]
    fn parse_escaped_newline() {
        let mut dst = [0u8; 8];
        assert_eq!(parse_escaped(br#"\n""#, &mut dst), Ok((3, 1)));
        assert_eq!(&dst[..1], &[0x0a]);
    }

    #[test]
    fn parse_escaped_quote() {
        let mut dst = [0u8; 8];
        assert_eq!(parse_escaped(br#"\q""#, &mut dst), Ok((3, 1)));
        assert_eq!(&dst[..1], b"\"");
        assert_eq!(parse_escaped(br#"\""""#, &mut dst), Ok((3, 1)));
        assert_eq!(&dst[..1], b"\"");
    }

    #[test]
    fn parse_escaped_carriage_return() {
        let mut dst = [0u8; 8];
        assert_eq!(parse_escaped(br#"\r""#, &mut dst), Ok((3, 1)));
        assert_eq!(&dst[..1], &[0x0d]);
    }

    #[test]
    fn parse_escaped_tab() {
        let mut dst = [0u8; 8];
        assert_eq!(parse_escaped(br#"\t""#, &mut dst), Ok((3, 1)));
        assert_eq!(&dst[..1], &[0x09]);
    }

    #[test]
    fn parse_escaped_vertical_tab() {
        let mut dst = [0u8; 8];
        assert_eq!(parse_escaped(br#"\v""#, &mut dst), Ok((3, 1)));
        assert_eq!(&dst[..1], &[0x0b]);
    }

    #[test]
    fn parse_escaped_nul() {
        let mut dst = [0u8; 8];
        assert_eq!(parse_escaped(br#"\z""#, &mut dst), Ok((3, 1)));
        assert_eq!(&dst[..1], &[0x00]);
    }

    #[test]
    fn parse_escaped_backslash() {
        let mut dst = [0u8; 8];
        assert_eq!(parse_escaped(br#"\\""#, &mut dst), Ok((3, 1)));
        assert_eq!(&dst[..1], b"\\");
    }

    #[test]
    fn parse_escaped_hex() {
        let mut dst = [0u8; 8];
        assert_eq!(parse_escaped(br#"\x41""#, &mut dst), Ok((5, 1)));
        assert_eq!(&dst[..1], b"A");
    }

    #[test]
    fn parse_escaped_unknown_escape() {
        let mut dst = [0u8; 8];
        assert_eq!(
            parse_escaped(br#"\y""#, &mut dst),
            Err(VmError::InvalidEscape(b'y'))
        );
    }

    #[test]
    fn parse_escaped_trailing_backslash() {
        let mut dst = [0u8; 8];
        assert_eq!(
            parse_escaped(b"\\", &mut dst),
            Err(VmError::InvalidEscape(b'\\'))
        );
    }

    #[test]
    fn parse_escaped_short_hex() {
        let mut dst = [0u8; 8];
        assert_eq!(
            parse_escaped(br#"\x4""#, &mut dst),
            Err(VmError::InvalidEscape(b'x'))
        );
    }

    #[test]
    fn parse_escaped_overflow() {
        let mut dst = [0u8; 2];
        assert_eq!(
            parse_escaped(br#"abcd""#, &mut dst),
            Err(VmError::ParsedStringOverflow)
        );
    }
}

pub fn parse_num(bytes: &[u8], base: u32) -> Option<usize> {
    if bytes.is_empty() {
        return None;
    }

    if bytes.len() == 3 && bytes[0] == b'\'' && bytes[2] == b'\'' {
        return Some(bytes[1] as usize);
    }

    let (neg, rest) = if let Some((&b'-', rest)) = bytes.split_first() {
        (true, rest)
    } else {
        (false, bytes)
    };

    if rest.is_empty() {
        return None;
    }

    let (base, rest) = if let Some((&b'#', rest)) = rest.split_first() {
        (10u32, rest)
    } else if let Some((&b'$', rest)) = rest.split_first() {
        (16u32, rest)
    } else if let Some((&b'%', rest)) = rest.split_first() {
        (2u32, rest)
    } else {
        (base, rest)
    };

    if rest.is_empty() {
        return None;
    }

    let s = core::str::from_utf8(rest).ok()?;
    let n = isize::from_str_radix(s, base).ok()?;
    if neg {
        Some(n.wrapping_neg() as usize)
    } else {
        Some(n as usize)
    }
}

pub fn to_number(mut acc: u128, bytes: &[u8], base: u32) -> (u128, &[u8]) {
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
        acc = acc * u128::from(base) + u128::from(d);
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
        assert_eq!(to_number(0, b"123", 10), (123, &b""[..]));
    }

    #[test]
    fn to_number_empty() {
        assert_eq!(to_number(42, b"", 10), (42, &b""[..]));
    }

    #[test]
    fn to_number_partial() {
        assert_eq!(to_number(0, b"10z", 10), (10, &b"z"[..]));
    }

    #[test]
    fn to_number_continues() {
        assert_eq!(to_number(0xff, b"ff", 16), (0xffff, &b""[..]));
    }

    #[test]
    fn to_number_case_folding() {
        assert_eq!(to_number(0, b"abc", 16), (0xabc, &b""[..]));
        assert_eq!(to_number(0, b"ABC", 16), (0xabc, &b""[..]));
    }
}

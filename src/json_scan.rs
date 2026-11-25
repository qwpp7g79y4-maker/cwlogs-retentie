use std::collections::HashSet;

pub(crate) enum KeyFault {
    Duplicate,
    Bad,
}

/// Walk one JSON value. A repeated key on the outer object, or on a log group
/// object, is [`KeyFault::Duplicate`]. Nested objects that are not groups are
/// not checked. The walk is linear in the number of bytes.
pub(crate) fn scan_json(text: &str) -> Result<(), KeyFault> {
    let bytes = text.as_bytes();
    let mut i = 0;
    skip_ws(bytes, &mut i);
    if i >= bytes.len() {
        return Err(KeyFault::Bad);
    }
    match bytes[i] {
        b'{' => scan_object(bytes, &mut i, 1, true),
        b'[' => scan_group_array(bytes, &mut i, 1),
        _ => Ok(()),
    }
}

fn scan_object(bytes: &[u8], i: &mut usize, depth: u32, top: bool) -> Result<(), KeyFault> {
    if depth > 128 || *i >= bytes.len() || bytes[*i] != b'{' {
        return Err(KeyFault::Bad);
    }
    *i += 1;
    let mut seen = HashSet::new();
    loop {
        skip_ws(bytes, i);
        if *i < bytes.len() && bytes[*i] == b'}' {
            *i += 1;
            return Ok(());
        }
        let key = parse_json_string(bytes, i)?;
        let is_log_groups = top && key == "logGroups";
        if !seen.insert(key) {
            return Err(KeyFault::Duplicate);
        }
        skip_ws(bytes, i);
        if *i >= bytes.len() || bytes[*i] != b':' {
            return Err(KeyFault::Bad);
        }
        *i += 1;
        skip_ws(bytes, i);
        if is_log_groups && *i < bytes.len() && bytes[*i] == b'[' {
            scan_group_array(bytes, i, depth + 1)?;
        } else {
            skip_value(bytes, i, depth + 1)?;
        }
        skip_ws(bytes, i);
        if *i < bytes.len() && bytes[*i] == b',' {
            *i += 1;
            continue;
        }
        if *i < bytes.len() && bytes[*i] == b'}' {
            *i += 1;
            return Ok(());
        }
        return Err(KeyFault::Bad);
    }
}

fn scan_group_array(bytes: &[u8], i: &mut usize, depth: u32) -> Result<(), KeyFault> {
    if depth > 128 || *i >= bytes.len() || bytes[*i] != b'[' {
        return Err(KeyFault::Bad);
    }
    *i += 1;
    loop {
        skip_ws(bytes, i);
        if *i < bytes.len() && bytes[*i] == b']' {
            *i += 1;
            return Ok(());
        }
        if *i < bytes.len() && bytes[*i] == b'{' {
            scan_object(bytes, i, depth + 1, false)?;
        } else {
            skip_value(bytes, i, depth + 1)?;
        }
        skip_ws(bytes, i);
        if *i < bytes.len() && bytes[*i] == b',' {
            *i += 1;
            continue;
        }
        if *i < bytes.len() && bytes[*i] == b']' {
            *i += 1;
            return Ok(());
        }
        return Err(KeyFault::Bad);
    }
}

fn skip_ws(bytes: &[u8], i: &mut usize) {
    while *i < bytes.len() && bytes[*i].is_ascii_whitespace() {
        *i += 1;
    }
}

fn skip_value(bytes: &[u8], i: &mut usize, depth: u32) -> Result<(), KeyFault> {
    if depth > 128 {
        return Err(KeyFault::Bad);
    }
    skip_ws(bytes, i);
    if *i >= bytes.len() {
        return Err(KeyFault::Bad);
    }
    match bytes[*i] {
        b'"' => {
            parse_json_string(bytes, i)?;
            Ok(())
        }
        b'{' => skip_container(bytes, i, b'{', b'}', depth),
        b'[' => skip_container(bytes, i, b'[', b']', depth),
        b't' => skip_lit(bytes, i, b"true"),
        b'f' => skip_lit(bytes, i, b"false"),
        b'n' => skip_lit(bytes, i, b"null"),
        b'-' | b'0'..=b'9' => skip_number(bytes, i),
        _ => Err(KeyFault::Bad),
    }
}

fn skip_container(
    bytes: &[u8],
    i: &mut usize,
    open: u8,
    close: u8,
    depth: u32,
) -> Result<(), KeyFault> {
    if bytes[*i] != open {
        return Err(KeyFault::Bad);
    }
    *i += 1;
    loop {
        skip_ws(bytes, i);
        if *i < bytes.len() && bytes[*i] == close {
            *i += 1;
            return Ok(());
        }
        if open == b'{' {
            parse_json_string(bytes, i)?;
            skip_ws(bytes, i);
            if *i >= bytes.len() || bytes[*i] != b':' {
                return Err(KeyFault::Bad);
            }
            *i += 1;
        }
        skip_value(bytes, i, depth + 1)?;
        skip_ws(bytes, i);
        if *i < bytes.len() && bytes[*i] == b',' {
            *i += 1;
            continue;
        }
        if *i < bytes.len() && bytes[*i] == close {
            *i += 1;
            return Ok(());
        }
        return Err(KeyFault::Bad);
    }
}

fn skip_lit(bytes: &[u8], i: &mut usize, lit: &[u8]) -> Result<(), KeyFault> {
    if bytes[*i..].starts_with(lit) {
        *i += lit.len();
        Ok(())
    } else {
        Err(KeyFault::Bad)
    }
}

fn skip_number(bytes: &[u8], i: &mut usize) -> Result<(), KeyFault> {
    let start = *i;
    if bytes[*i] == b'-' {
        *i += 1;
    }
    if *i >= bytes.len() || !bytes[*i].is_ascii_digit() {
        return Err(KeyFault::Bad);
    }
    while *i < bytes.len() && bytes[*i].is_ascii_digit() {
        *i += 1;
    }
    if *i < bytes.len() && bytes[*i] == b'.' {
        *i += 1;
        if *i >= bytes.len() || !bytes[*i].is_ascii_digit() {
            return Err(KeyFault::Bad);
        }
        while *i < bytes.len() && bytes[*i].is_ascii_digit() {
            *i += 1;
        }
    }
    if *i < bytes.len() && (bytes[*i] == b'e' || bytes[*i] == b'E') {
        *i += 1;
        if *i < bytes.len() && (bytes[*i] == b'+' || bytes[*i] == b'-') {
            *i += 1;
        }
        if *i >= bytes.len() || !bytes[*i].is_ascii_digit() {
            return Err(KeyFault::Bad);
        }
        while *i < bytes.len() && bytes[*i].is_ascii_digit() {
            *i += 1;
        }
    }
    if *i == start {
        return Err(KeyFault::Bad);
    }
    Ok(())
}

fn parse_json_string(bytes: &[u8], i: &mut usize) -> Result<String, KeyFault> {
    if *i >= bytes.len() || bytes[*i] != b'"' {
        return Err(KeyFault::Bad);
    }
    *i += 1;
    let mut out = String::new();
    while *i < bytes.len() {
        let byte = bytes[*i];
        *i += 1;
        match byte {
            b'"' => return Ok(out),
            b'\\' => {
                if *i >= bytes.len() {
                    return Err(KeyFault::Bad);
                }
                let esc = bytes[*i];
                *i += 1;
                match esc {
                    b'"' => out.push('"'),
                    b'\\' => out.push('\\'),
                    b'/' => out.push('/'),
                    b'b' => out.push('\u{0008}'),
                    b'f' => out.push('\u{000c}'),
                    b'n' => out.push('\n'),
                    b'r' => out.push('\r'),
                    b't' => out.push('\t'),
                    b'u' => out.push(unicode_escape(bytes, i)?),
                    _ => return Err(KeyFault::Bad),
                }
            }
            0x00..=0x1f => return Err(KeyFault::Bad),
            0x20..=0x7f => out.push(byte as char),
            _ => {
                *i -= 1;
                // One scalar only. Validating `&bytes[*i..]` would rescan the
                // rest of the file on every character.
                let (ch, width) = utf8_char(bytes, *i)?;
                out.push(ch);
                *i += width;
            }
        }
    }
    Err(KeyFault::Bad)
}

fn unicode_escape(bytes: &[u8], i: &mut usize) -> Result<char, KeyFault> {
    let code = hex4(bytes, i)?;
    if (0xD800..=0xDBFF).contains(&code) {
        if *i + 2 > bytes.len() || bytes[*i] != b'\\' || bytes[*i + 1] != b'u' {
            return Err(KeyFault::Bad);
        }
        *i += 2;
        let low = hex4(bytes, i)?;
        if !(0xDC00..=0xDFFF).contains(&low) {
            return Err(KeyFault::Bad);
        }
        let point = 0x10000 + (((code - 0xD800) << 10) | (low - 0xDC00));
        return char::from_u32(point).ok_or(KeyFault::Bad);
    }
    char::from_u32(code).ok_or(KeyFault::Bad)
}

fn hex4(bytes: &[u8], i: &mut usize) -> Result<u32, KeyFault> {
    if *i + 4 > bytes.len() {
        return Err(KeyFault::Bad);
    }
    let hex = std::str::from_utf8(&bytes[*i..*i + 4]).map_err(|_| KeyFault::Bad)?;
    let code = u32::from_str_radix(hex, 16).map_err(|_| KeyFault::Bad)?;
    *i += 4;
    Ok(code)
}

fn utf8_char(bytes: &[u8], i: usize) -> Result<(char, usize), KeyFault> {
    let b0 = *bytes.get(i).ok_or(KeyFault::Bad)?;
    let width = match b0 {
        0x00..=0x7F => 1,
        0xC2..=0xDF => 2,
        0xE0..=0xEF => 3,
        0xF0..=0xF4 => 4,
        _ => return Err(KeyFault::Bad),
    };
    let end = i + width;
    if end > bytes.len() {
        return Err(KeyFault::Bad);
    }
    let text = std::str::from_utf8(&bytes[i..end]).map_err(|_| KeyFault::Bad)?;
    let ch = text.chars().next().ok_or(KeyFault::Bad)?;
    if ch.len_utf8() != width {
        return Err(KeyFault::Bad);
    }
    Ok((ch, width))
}

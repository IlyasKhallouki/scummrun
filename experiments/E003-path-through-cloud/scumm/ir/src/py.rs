//! Small pieces of Python's behaviour the port has to match exactly: `str.splitlines`, `str.strip`,
//! `repr` of a string, `format(n, "04X")`, `int.from_bytes(..., "little")` and slicing that clips.

/// Python's `str.isspace` for one character (the characters `\s`, `strip` and `split` use).
#[inline]
pub fn is_space(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\r' | '\x0b' | '\x0c' | '\x1c'..='\x1f' | '\u{85}' | '\u{a0}')
        || (c > '\u{ff}' && c.is_whitespace())
}

/// Python's `str.strip()`.
pub fn strip(s: &str) -> &str {
    s.trim_matches(is_space)
}

/// Python's `str.rstrip()`.
pub fn rstrip(s: &str) -> &str {
    s.trim_end_matches(is_space)
}

/// Python's `str.splitlines()`: the lines without their breaks, split on \n, \r\n, \r, \v, \f,
/// \x1c-\x1e, \x85,   and  .
pub fn splitlines(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut it = s.char_indices().peekable();
    while let Some((i, c)) = it.next() {
        let brk = matches!(c, '\n' | '\r' | '\x0b' | '\x0c' | '\x1c' | '\x1d' | '\x1e' | '\u{85}' | '\u{2028}' | '\u{2029}');
        if !brk {
            continue;
        }
        out.push(&s[start..i]);
        let mut end = i + c.len_utf8();
        if c == '\r' {
            if let Some(&(_, '\n')) = it.peek() {
                it.next();
                end += 1;
            }
        }
        start = end;
    }
    if start < s.len() {
        out.push(&s[start..]);
    }
    out
}

/// Python's `repr(s)` for a str (ASCII printable kept, the usual escapes, `\xNN` for other control
/// characters; characters past ASCII are kept as they are).
pub fn repr(s: &str) -> String {
    let quote = if s.contains('\'') && !s.contains('"') { '"' } else { '\'' };
    let mut out = String::with_capacity(s.len() + 2);
    out.push(quote);
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            c if c == quote => {
                out.push('\\');
                out.push(c);
            }
            c if (c as u32) < 0x20 || c as u32 == 0x7f => out.push_str(&format!("\\x{:02x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push(quote);
    out
}

/// Python's `repr` of a list of strings: `['a', 'b']`.
pub fn repr_list(items: &[&str]) -> String {
    let parts: Vec<String> = items.iter().map(|s| repr(s)).collect();
    format!("[{}]", parts.join(", "))
}

/// Python's `f"{n:04X}"`: upper-case hex, zero-padded to 4 including a minus sign.
pub fn hex04(n: i64) -> String {
    if n < 0 {
        format!("-{:03X}", -n)
    } else {
        format!("{n:04X}")
    }
}

/// Python's `data[a:b]` for non-negative a and b: clipped to the data, empty when a >= b.
pub fn slice(data: &[u8], a: usize, b: usize) -> &[u8] {
    let b = b.min(data.len());
    let a = a.min(b);
    &data[a..b]
}

/// Python's `int.from_bytes(bytes, "little", signed=...)` for up to 8 bytes.
pub fn from_le(bytes: &[u8], signed: bool) -> i64 {
    if bytes.is_empty() {
        return 0;
    }
    let mut v: u64 = 0;
    for (k, &b) in bytes.iter().enumerate() {
        v |= (b as u64) << (8 * k);
    }
    let bits = 8 * bytes.len() as u32;
    if signed && bits < 64 && v >> (bits - 1) & 1 == 1 {
        (v as i64) - (1i64 << bits)
    } else {
        v as i64
    }
}

/// Python's `str[:n]` by characters.
pub fn prefix_chars(s: &str, n: usize) -> &str {
    match s.char_indices().nth(n) {
        Some((i, _)) => &s[..i],
        None => s,
    }
}

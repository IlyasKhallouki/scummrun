//! An action line of reading.actions.jsonl (packed_reading.py `action_line`), read without building a JSON tree:
//! the fields PackedTask._action reads, with `source` and each check kept as their JSON text.

use std::borrow::Cow;

/// One action line's fields (texts borrowed from the line where they hold no escape).
#[derive(Clone, Debug, PartialEq)]
pub struct Line<'a> {
    pub i: i64,
    pub rec: i64,
    pub label: Cow<'a, str>,
    /// the source's JSON text, as the line has it
    pub source: &'a str,
    /// each check's JSON text, as the line has it
    pub checks: Vec<&'a str>,
    pub pre_keys: Vec<Cow<'a, str>>,
    pub engine_pre: Vec<Cow<'a, str>>,
}

fn ws(b: &[u8], i: &mut usize) {
    while *i < b.len() && matches!(b[*i], b' ' | b'\t' | b'\n' | b'\r') {
        *i += 1;
    }
}

fn expect(b: &[u8], i: &mut usize, c: u8) -> Result<(), String> {
    ws(b, i);
    if b.get(*i) != Some(&c) {
        return Err(format!("expected {:?} at byte {}", c as char, *i));
    }
    *i += 1;
    Ok(())
}

fn hex4(b: &[u8], i: usize) -> Result<u32, String> {
    let h = b.get(i..i + 4).ok_or("a short \\u escape")?;
    u32::from_str_radix(std::str::from_utf8(h).map_err(|_| "a bad \\u escape")?, 16).map_err(|_| "a bad \\u escape".into())
}

/// A JSON string at `*i` (its opening quote), unescaped.
pub fn string(b: &[u8], i: &mut usize) -> Result<String, String> {
    expect(b, i, b'"')?;
    let mut out: Vec<u8> = Vec::new();
    loop {
        let start = *i;
        while *i < b.len() && b[*i] != b'"' && b[*i] != b'\\' {
            *i += 1;
        }
        out.extend_from_slice(&b[start..*i]);
        match b.get(*i) {
            None => return Err("an unterminated string".into()),
            Some(b'"') => {
                *i += 1;
                return String::from_utf8(out).map_err(|_| "a string that is not UTF-8".into());
            }
            _ => {
                let e = *b.get(*i + 1).ok_or("a short escape")?;
                *i += 2;
                match e {
                    b'"' | b'\\' | b'/' => out.push(e),
                    b'b' => out.push(8),
                    b'f' => out.push(12),
                    b'n' => out.push(b'\n'),
                    b'r' => out.push(b'\r'),
                    b't' => out.push(b'\t'),
                    b'u' => {
                        let mut cp = hex4(b, *i)?;
                        *i += 4;
                        if (0xD800..0xDC00).contains(&cp) && b.get(*i) == Some(&b'\\') && b.get(*i + 1) == Some(&b'u') {
                            let lo = hex4(b, *i + 2)?;
                            if (0xDC00..0xE000).contains(&lo) {
                                *i += 6;
                                cp = 0x10000 + ((cp - 0xD800) << 10) + (lo - 0xDC00);
                            }
                        }
                        let ch = char::from_u32(cp).unwrap_or('\u{FFFD}');
                        let mut buf = [0u8; 4];
                        out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                    }
                    c => return Err(format!("a bad escape \\{}", c as char)),
                }
            }
        }
    }
}

/// A JSON string at `*i`, borrowed from `s` when it holds no escape.
pub fn str_at<'a>(s: &'a str, i: &mut usize) -> Result<Cow<'a, str>, String> {
    let b = s.as_bytes();
    ws(b, i);
    if b.get(*i) != Some(&b'"') {
        return Err(format!("expected a string at byte {}", *i));
    }
    let st = *i + 1;
    let mut k = st;
    while k < b.len() && b[k] != b'"' && b[k] != b'\\' {
        k += 1;
    }
    if b.get(k) == Some(&b'"') {
        *i = k + 1;
        return Ok(Cow::Borrowed(&s[st..k]));
    }
    string(b, i).map(Cow::Owned)
}

/// Skips the JSON value at `*i`.
pub fn skip(b: &[u8], i: &mut usize) -> Result<(), String> {
    ws(b, i);
    match b.get(*i) {
        None => Err("a missing value".into()),
        Some(b'"') => {
            // a string, not unescaped: to the quote no backslash escapes
            let mut k = *i + 1;
            while k < b.len() && b[k] != b'"' {
                k += if b[k] == b'\\' { 2 } else { 1 };
            }
            if k >= b.len() {
                return Err("an unterminated string".into());
            }
            *i = k + 1;
            Ok(())
        }
        Some(&open @ (b'[' | b'{')) => {
            let close = if open == b'[' { b']' } else { b'}' };
            *i += 1;
            ws(b, i);
            if b.get(*i) == Some(&close) {
                *i += 1;
                return Ok(());
            }
            loop {
                if open == b'{' {
                    string(b, i)?;
                    expect(b, i, b':')?;
                }
                skip(b, i)?;
                ws(b, i);
                match b.get(*i) {
                    Some(b',') => *i += 1,
                    Some(&c) if c == close => {
                        *i += 1;
                        return Ok(());
                    }
                    _ => return Err(format!("a bad list or object at byte {}", *i)),
                }
            }
        }
        Some(_) => {
            let st = *i;
            while *i < b.len() && !matches!(b[*i], b',' | b']' | b'}' | b' ' | b'\n') {
                *i += 1;
            }
            if *i == st {
                Err(format!("no value at byte {st}"))
            } else {
                Ok(())
            }
        }
    }
}

fn int(b: &[u8], i: &mut usize) -> Result<i64, String> {
    ws(b, i);
    let st = *i;
    skip(b, i)?;
    std::str::from_utf8(&b[st..*i]).ok().and_then(|t| t.parse().ok()).ok_or_else(|| format!("not an int at byte {st}"))
}

/// A list of values: the text of each (`raw`) or each as a string.
fn list<T>(b: &[u8], i: &mut usize, mut item: impl FnMut(&[u8], &mut usize) -> Result<T, String>) -> Result<Vec<T>, String> {
    expect(b, i, b'[')?;
    let mut out = Vec::new();
    ws(b, i);
    if b.get(*i) == Some(&b']') {
        *i += 1;
        return Ok(out);
    }
    loop {
        ws(b, i);
        out.push(item(b, i)?);
        ws(b, i);
        match b.get(*i) {
            Some(b',') => *i += 1,
            Some(b']') => {
                *i += 1;
                return Ok(out);
            }
            _ => return Err(format!("a bad list at byte {}", *i)),
        }
    }
}

/// The text of the value at `*i`.
fn raw<'a>(s: &'a str, i: &mut usize) -> Result<&'a str, String> {
    let b = s.as_bytes();
    ws(b, i);
    let st = *i;
    skip(b, i)?;
    Ok(&s[st..*i])
}

/// Reads an action line (without its newline).
pub fn parse(s: &str) -> Result<Line<'_>, String> {
    let b = s.as_bytes();
    let mut i = 0;
    let mut line =
        Line { i: -1, rec: -1, label: Cow::Borrowed(""), source: "", checks: vec![], pre_keys: vec![], engine_pre: vec![] };
    expect(b, &mut i, b'{')?;
    ws(b, &mut i);
    if b.get(i) == Some(&b'}') {
        return Err("an empty line".into());
    }
    loop {
        let key = str_at(s, &mut i)?;
        expect(b, &mut i, b':')?;
        match &*key {
            "i" => line.i = int(b, &mut i)?,
            "rec" => line.rec = int(b, &mut i)?,
            "label" => line.label = str_at(s, &mut i)?,
            "source" => line.source = raw(s, &mut i)?,
            "checks" => line.checks = list(b, &mut i, |_, i| raw(s, i))?,
            "pre_keys" => line.pre_keys = list(b, &mut i, |_, i| str_at(s, i))?,
            "engine_pre" => line.engine_pre = list(b, &mut i, |_, i| str_at(s, i))?,
            _ => skip(b, &mut i)?,
        }
        ws(b, &mut i);
        match b.get(i) {
            Some(b',') => i += 1,
            Some(b'}') => return Ok(line),
            _ => return Err(format!("a bad line at byte {i}")),
        }
    }
}

/// The line's "i", "rec" and "label", read from its start (action_line writes them first, in that order; any
/// other line is read whole).
pub fn head(s: &str) -> Result<(i64, i64, Cow<'_, str>), String> {
    let b = s.as_bytes();
    let mut i = 0;
    let fast = (|| -> Result<(i64, i64, Cow<'_, str>), String> {
        expect(b, &mut i, b'{')?;
        let mut out = (0, 0, Cow::Borrowed(""));
        for (want, k) in [("i", 0), ("rec", 1), ("label", 2)] {
            if k > 0 {
                expect(b, &mut i, b',')?;
            }
            if str_at(s, &mut i)? != want {
                return Err("not in action_line's order".into());
            }
            expect(b, &mut i, b':')?;
            match k {
                0 => out.0 = int(b, &mut i)?,
                1 => out.1 = int(b, &mut i)?,
                _ => out.2 = str_at(s, &mut i)?,
            }
        }
        Ok(out)
    })();
    match fast {
        Ok(x) => Ok(x),
        Err(_) => parse(s).map(|l| (l.i, l.rec, l.label)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_line() {
        let s = r#"{"i": 3, "rec": 7, "label": "sentence(2,486)", "source": ["sentence", 2, 486, null], "checks": [["global/script-002.txt:9 [0007]", "Var[104] == Local[1]", false, "?(not a fact: var:104)"]], "pre_keys": ["bit:6", "room"], "engine_pre": ["room"]}"#;
        let l = parse(s).unwrap();
        assert_eq!((l.i, l.rec, &*l.label), (3, 7, "sentence(2,486)"));
        assert_eq!(head(s).unwrap(), (3, 7, Cow::Borrowed("sentence(2,486)")));
        assert_eq!(l.source, r#"["sentence", 2, 486, null]"#);
        assert_eq!(l.checks, vec![r#"["global/script-002.txt:9 [0007]", "Var[104] == Local[1]", false, "?(not a fact: var:104)"]"#]);
        assert_eq!(l.pre_keys, vec!["bit:6", "room"]);
        assert_eq!(l.engine_pre, vec!["room"]);
        let e = parse(r#"{"i": 0, "rec": 1, "label": "a\"é😀", "source": [], "checks": [], "pre_keys": [], "engine_pre": []}"#).unwrap();
        assert_eq!(e.label, "a\"é😀");
        assert_eq!(head(r#"{"rec": 1, "i": 0, "label": "x"}"#).unwrap(), (0, 1, Cow::Borrowed("x")));
    }
}

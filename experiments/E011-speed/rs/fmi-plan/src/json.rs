//! A small JSON reader for reading.json (the double's head: its goal, bounds and start), no dependency.

#[derive(Clone, Debug, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Num(f64),
    Int(i64),
    Str(String),
    Arr(Vec<Json>),
    Obj(Vec<(String, Json)>),
}

impl Json {
    pub fn get(&self, k: &str) -> Option<&Json> {
        match self {
            Json::Obj(v) => v.iter().find(|(a, _)| a == k).map(|(_, b)| b),
            _ => None,
        }
    }
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Json::Str(s) => Some(s),
            _ => None,
        }
    }
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Json::Int(n) => Some(*n),
            _ => None,
        }
    }
    pub fn as_arr(&self) -> Option<&[Json]> {
        match self {
            Json::Arr(v) => Some(v),
            _ => None,
        }
    }
    pub fn as_obj(&self) -> Option<&[(String, Json)]> {
        match self {
            Json::Obj(v) => Some(v),
            _ => None,
        }
    }
}

pub fn parse(s: &str) -> Result<Json, String> {
    let b = s.as_bytes();
    let mut p = 0;
    let v = value(b, &mut p)?;
    ws(b, &mut p);
    if p != b.len() {
        return Err(format!("json: trailing bytes at {}", p));
    }
    Ok(v)
}

fn ws(b: &[u8], p: &mut usize) {
    while *p < b.len() && matches!(b[*p], b' ' | b'\n' | b'\r' | b'\t') {
        *p += 1;
    }
}

fn value(b: &[u8], p: &mut usize) -> Result<Json, String> {
    ws(b, p);
    let c = *b.get(*p).ok_or("json: end of input")?;
    match c {
        b'{' => {
            *p += 1;
            let mut v = Vec::new();
            ws(b, p);
            if b.get(*p) == Some(&b'}') {
                *p += 1;
                return Ok(Json::Obj(v));
            }
            loop {
                ws(b, p);
                let k = match value(b, p)? {
                    Json::Str(s) => s,
                    _ => return Err(format!("json: a key at {}", p)),
                };
                ws(b, p);
                if b.get(*p) != Some(&b':') {
                    return Err(format!("json: ':' at {}", p));
                }
                *p += 1;
                let x = value(b, p)?;
                v.push((k, x));
                ws(b, p);
                match b.get(*p) {
                    Some(b',') => *p += 1,
                    Some(b'}') => {
                        *p += 1;
                        return Ok(Json::Obj(v));
                    }
                    _ => return Err(format!("json: ',' or '}}' at {}", p)),
                }
            }
        }
        b'[' => {
            *p += 1;
            let mut v = Vec::new();
            ws(b, p);
            if b.get(*p) == Some(&b']') {
                *p += 1;
                return Ok(Json::Arr(v));
            }
            loop {
                v.push(value(b, p)?);
                ws(b, p);
                match b.get(*p) {
                    Some(b',') => *p += 1,
                    Some(b']') => {
                        *p += 1;
                        return Ok(Json::Arr(v));
                    }
                    _ => return Err(format!("json: ',' or ']' at {}", p)),
                }
            }
        }
        b'"' => {
            *p += 1;
            let mut s = String::new();
            loop {
                let c = *b.get(*p).ok_or("json: end of a string")?;
                *p += 1;
                match c {
                    b'"' => return Ok(Json::Str(s)),
                    b'\\' => {
                        let e = *b.get(*p).ok_or("json: end of an escape")?;
                        *p += 1;
                        match e {
                            b'"' => s.push('"'),
                            b'\\' => s.push('\\'),
                            b'/' => s.push('/'),
                            b'b' => s.push('\u{8}'),
                            b'f' => s.push('\u{c}'),
                            b'n' => s.push('\n'),
                            b'r' => s.push('\r'),
                            b't' => s.push('\t'),
                            b'u' => {
                                let h = std::str::from_utf8(b.get(*p..*p + 4).ok_or("json: \\u")?).map_err(|e| e.to_string())?;
                                let mut cp = u32::from_str_radix(h, 16).map_err(|e| e.to_string())?;
                                *p += 4;
                                if (0xD800..0xDC00).contains(&cp) && b.get(*p) == Some(&b'\\') && b.get(*p + 1) == Some(&b'u') {
                                    let h2 = std::str::from_utf8(&b[*p + 2..*p + 6]).map_err(|e| e.to_string())?;
                                    let lo = u32::from_str_radix(h2, 16).map_err(|e| e.to_string())?;
                                    *p += 6;
                                    cp = 0x10000 + ((cp - 0xD800) << 10) + (lo - 0xDC00);
                                }
                                s.push(char::from_u32(cp).unwrap_or('\u{fffd}'));
                            }
                            _ => return Err(format!("json: escape {}", e as char)),
                        }
                    }
                    _ => {
                        // a UTF-8 sequence: copy its bytes
                        let start = *p - 1;
                        let mut end = *p;
                        while end < b.len() && (b[end] & 0xC0) == 0x80 {
                            end += 1;
                        }
                        s.push_str(std::str::from_utf8(&b[start..end]).map_err(|e| e.to_string())?);
                        *p = end;
                    }
                }
            }
        }
        b't' if b[*p..].starts_with(b"true") => {
            *p += 4;
            Ok(Json::Bool(true))
        }
        b'f' if b[*p..].starts_with(b"false") => {
            *p += 5;
            Ok(Json::Bool(false))
        }
        b'n' if b[*p..].starts_with(b"null") => {
            *p += 4;
            Ok(Json::Null)
        }
        _ => {
            let start = *p;
            while *p < b.len() && matches!(b[*p], b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9') {
                *p += 1;
            }
            let t = std::str::from_utf8(&b[start..*p]).map_err(|e| e.to_string())?;
            if let Ok(n) = t.parse::<i64>() {
                Ok(Json::Int(n))
            } else {
                t.parse::<f64>().map(Json::Num).map_err(|_| format!("json: a value at {}: {:?}", start, t))
            }
        }
    }
}

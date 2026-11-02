//! Python's values as the reference writes them: a JSON value with its dicts in insertion order (`J`), parsed from
//! text and written as `json.dumps` writes it (separators ", " and ": ", ensure_ascii), and Python's `repr`/`str` of
//! the tuples, lists and strings the window records print (`Py`), e.g. a child's `str(ch.why)[:80]`.

use std::fmt::Write as _;

/// A JSON value; an object keeps its keys in insertion order (a Python dict).
#[derive(Clone, Debug, PartialEq)]
pub enum J {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    Arr(Vec<J>),
    Obj(Vec<(String, J)>),
}

impl Default for J {
    fn default() -> J {
        J::Null
    }
}

impl J {
    pub fn get(&self, k: &str) -> Option<&J> {
        match self {
            J::Obj(v) => v.iter().find(|(a, _)| a == k).map(|(_, b)| b),
            _ => None,
        }
    }
    pub fn int(&self) -> Option<i64> {
        match self {
            J::Int(n) => Some(*n),
            J::Float(f) if f.fract() == 0.0 => Some(*f as i64),
            _ => None,
        }
    }
    pub fn i(&self, k: &str) -> Option<i64> {
        self.get(k).and_then(J::int)
    }
    pub fn str(&self) -> Option<&str> {
        match self {
            J::Str(s) => Some(s),
            _ => None,
        }
    }
    pub fn s(&self, k: &str) -> Option<&str> {
        self.get(k).and_then(J::str)
    }
    pub fn arr(&self) -> &[J] {
        match self {
            J::Arr(v) => v,
            _ => &[],
        }
    }
    pub fn a(&self, k: &str) -> &[J] {
        self.get(k).map(J::arr).unwrap_or(&[])
    }
    pub fn obj(&self) -> &[(String, J)] {
        match self {
            J::Obj(v) => v,
            _ => &[],
        }
    }
    pub fn is_null(&self) -> bool {
        matches!(self, J::Null)
    }
    pub fn truthy(&self) -> bool {
        match self {
            J::Null => false,
            J::Bool(b) => *b,
            J::Int(n) => *n != 0,
            J::Float(f) => *f != 0.0,
            J::Str(s) => !s.is_empty(),
            J::Arr(v) => !v.is_empty(),
            J::Obj(v) => !v.is_empty(),
        }
    }
    /// `d[k] = v`: replaces the value in place, or appends the key.
    pub fn set(&mut self, k: &str, v: J) {
        if let J::Obj(o) = self {
            if let Some(p) = o.iter_mut().find(|(a, _)| a == k) {
                p.1 = v;
            } else {
                o.push((k.to_string(), v));
            }
        }
    }
    pub fn obj_new() -> J {
        J::Obj(Vec::new())
    }
}

impl From<i64> for J {
    fn from(n: i64) -> J {
        J::Int(n)
    }
}
impl From<i32> for J {
    fn from(n: i32) -> J {
        J::Int(n as i64)
    }
}
impl From<u64> for J {
    fn from(n: u64) -> J {
        J::Int(n as i64)
    }
}
impl From<u32> for J {
    fn from(n: u32) -> J {
        J::Int(n as i64)
    }
}
impl From<usize> for J {
    fn from(n: usize) -> J {
        J::Int(n as i64)
    }
}
impl From<bool> for J {
    fn from(b: bool) -> J {
        J::Bool(b)
    }
}
impl From<&str> for J {
    fn from(s: &str) -> J {
        J::Str(s.to_string())
    }
}
impl From<String> for J {
    fn from(s: String) -> J {
        J::Str(s)
    }
}
impl<T: Into<J>> From<Option<T>> for J {
    fn from(o: Option<T>) -> J {
        match o {
            Some(x) => x.into(),
            None => J::Null,
        }
    }
}
impl<T: Into<J>> From<Vec<T>> for J {
    fn from(v: Vec<T>) -> J {
        J::Arr(v.into_iter().map(Into::into).collect())
    }
}

/// `{"a": 1, ...}` from pairs.
#[macro_export]
macro_rules! jobj {
    ($($k:expr => $v:expr),* $(,)?) => {
        $crate::pyj::J::Obj(vec![$(($k.to_string(), $crate::pyj::J::from($v))),*])
    };
}

// --- parsing -------------------------------------------------------------------------------------------------------

pub fn parse(s: &str) -> Result<J, String> {
    let mut p = Parser { b: s.as_bytes(), i: 0 };
    p.ws();
    let v = p.value()?;
    p.ws();
    if p.i != p.b.len() {
        return Err(format!("trailing data at {}", p.i));
    }
    Ok(v)
}

pub struct Parser<'a> {
    pub b: &'a [u8],
    pub i: usize,
}

impl<'a> Parser<'a> {
    #[inline]
    pub fn ws(&mut self) {
        while self.i < self.b.len() && matches!(self.b[self.i], b' ' | b'\n' | b'\r' | b'\t') {
            self.i += 1;
        }
    }
    #[inline]
    fn peek(&self) -> u8 {
        if self.i < self.b.len() {
            self.b[self.i]
        } else {
            0
        }
    }
    pub fn value(&mut self) -> Result<J, String> {
        match self.peek() {
            b'{' => {
                self.i += 1;
                let mut out = Vec::new();
                self.ws();
                if self.peek() == b'}' {
                    self.i += 1;
                    return Ok(J::Obj(out));
                }
                loop {
                    self.ws();
                    let k = self.string()?;
                    self.ws();
                    if self.peek() != b':' {
                        return Err(format!("expected : at {}", self.i));
                    }
                    self.i += 1;
                    self.ws();
                    let v = self.value()?;
                    out.push((k, v));
                    self.ws();
                    match self.peek() {
                        b',' => self.i += 1,
                        b'}' => {
                            self.i += 1;
                            return Ok(J::Obj(out));
                        }
                        _ => return Err(format!("expected , or }} at {}", self.i)),
                    }
                }
            }
            b'[' => {
                self.i += 1;
                let mut out = Vec::new();
                self.ws();
                if self.peek() == b']' {
                    self.i += 1;
                    return Ok(J::Arr(out));
                }
                loop {
                    self.ws();
                    out.push(self.value()?);
                    self.ws();
                    match self.peek() {
                        b',' => self.i += 1,
                        b']' => {
                            self.i += 1;
                            return Ok(J::Arr(out));
                        }
                        _ => return Err(format!("expected , or ] at {}", self.i)),
                    }
                }
            }
            b'"' => Ok(J::Str(self.string()?)),
            b't' if self.b[self.i..].starts_with(b"true") => {
                self.i += 4;
                Ok(J::Bool(true))
            }
            b'f' if self.b[self.i..].starts_with(b"false") => {
                self.i += 5;
                Ok(J::Bool(false))
            }
            b'n' if self.b[self.i..].starts_with(b"null") => {
                self.i += 4;
                Ok(J::Null)
            }
            b'N' if self.b[self.i..].starts_with(b"NaN") => {
                self.i += 3;
                Ok(J::Float(f64::NAN))
            }
            b'I' if self.b[self.i..].starts_with(b"Infinity") => {
                self.i += 8;
                Ok(J::Float(f64::INFINITY))
            }
            _ => self.number(),
        }
    }
    pub fn number(&mut self) -> Result<J, String> {
        let st = self.i;
        let mut float = false;
        if self.peek() == b'-' {
            self.i += 1;
        }
        if self.b[self.i..].starts_with(b"Infinity") {
            self.i += 8;
            return Ok(J::Float(f64::NEG_INFINITY));
        }
        while self.i < self.b.len() {
            match self.b[self.i] {
                b'0'..=b'9' => {}
                b'.' | b'e' | b'E' | b'+' | b'-' => float = true,
                _ => break,
            }
            self.i += 1;
        }
        let t = std::str::from_utf8(&self.b[st..self.i]).map_err(|e| e.to_string())?;
        if t.is_empty() || t == "-" {
            return Err(format!("bad value at {st}"));
        }
        if !float {
            if let Ok(n) = t.parse::<i64>() {
                return Ok(J::Int(n));
            }
        }
        t.parse::<f64>().map(J::Float).map_err(|e| format!("{e} at {st}"))
    }
    pub fn string(&mut self) -> Result<String, String> {
        if self.peek() != b'"' {
            return Err(format!("expected a string at {}", self.i));
        }
        self.i += 1;
        let mut out: Vec<u8> = Vec::new();
        loop {
            let st = self.i;
            while self.i < self.b.len() && self.b[self.i] != b'"' && self.b[self.i] != b'\\' {
                self.i += 1;
            }
            out.extend_from_slice(&self.b[st..self.i]);
            if self.i >= self.b.len() {
                return Err("unterminated string".into());
            }
            if self.b[self.i] == b'"' {
                self.i += 1;
                return String::from_utf8(out).map_err(|e| e.to_string());
            }
            self.i += 1;
            let c = self.peek();
            self.i += 1;
            match c {
                b'"' => out.push(b'"'),
                b'\\' => out.push(b'\\'),
                b'/' => out.push(b'/'),
                b'b' => out.push(8),
                b'f' => out.push(12),
                b'n' => out.push(b'\n'),
                b'r' => out.push(b'\r'),
                b't' => out.push(b'\t'),
                b'u' => {
                    let mut cp = self.hex4()?;
                    if (0xD800..0xDC00).contains(&cp) && self.b[self.i..].starts_with(b"\\u") {
                        self.i += 2;
                        let lo = self.hex4()?;
                        cp = 0x10000 + ((cp - 0xD800) << 10) + (lo - 0xDC00);
                    }
                    let ch = char::from_u32(cp).unwrap_or('\u{FFFD}');
                    let mut buf = [0u8; 4];
                    out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                }
                _ => return Err(format!("bad escape at {}", self.i)),
            }
        }
    }
    fn hex4(&mut self) -> Result<u32, String> {
        if self.i + 4 > self.b.len() {
            return Err("short \\u escape".into());
        }
        let t = std::str::from_utf8(&self.b[self.i..self.i + 4]).map_err(|e| e.to_string())?;
        self.i += 4;
        u32::from_str_radix(t, 16).map_err(|e| e.to_string())
    }
}

// --- json.dumps ----------------------------------------------------------------------------------------------------

/// `json.dumps(v)` with Python's defaults (", ", ": ", ensure_ascii).
pub fn dumps(v: &J) -> String {
    let mut s = String::new();
    write_json(&mut s, v);
    s
}

pub fn write_json(out: &mut String, v: &J) {
    match v {
        J::Null => out.push_str("null"),
        J::Bool(true) => out.push_str("true"),
        J::Bool(false) => out.push_str("false"),
        J::Int(n) => {
            let _ = write!(out, "{n}");
        }
        J::Float(f) => out.push_str(&py_float(*f)),
        J::Str(s) => write_str(out, s),
        J::Arr(a) => {
            out.push('[');
            for (i, x) in a.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                write_json(out, x);
            }
            out.push(']');
        }
        J::Obj(o) => {
            out.push('{');
            for (i, (k, x)) in o.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                write_str(out, k);
                out.push_str(": ");
                write_json(out, x);
            }
            out.push('}');
        }
    }
}

/// A JSON string as json.dumps writes it (ensure_ascii).
pub fn write_str(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 || (c as u32) > 0x7e => {
                let mut buf = [0u16; 2];
                for u in c.encode_utf16(&mut buf) {
                    let _ = write!(out, "\\u{:04x}", u);
                }
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

/// Python's `repr` of a float (the shortest text that reads back; "1.0", "21.22", "1e-05").
pub fn py_float(f: f64) -> String {
    if f.is_nan() {
        return "NaN".into();
    }
    if f.is_infinite() {
        return if f > 0.0 { "Infinity".into() } else { "-Infinity".into() };
    }
    if f == f.trunc() && f.abs() < 1e16 {
        return format!("{:.1}", f);
    }
    let s = format!("{:?}", f);
    // Rust prints 1e-5 as "1e-5"; Python as "1e-05"
    if let Some(p) = s.find('e') {
        let (m, e) = s.split_at(p);
        let e = &e[1..];
        let (sign, digits) = if let Some(d) = e.strip_prefix('-') {
            ("-", d)
        } else {
            ("+", e)
        };
        return format!("{m}e{sign}{digits:0>2}");
    }
    s
}

// --- Python's repr and str ----------------------------------------------------------------------------------------

/// A Python object as the reference prints it with `str` or `repr`: a tuple and a list differ.
#[derive(Clone, Debug, PartialEq)]
pub enum Py {
    None,
    Bool(bool),
    Int(i64),
    Str(String),
    Tuple(Vec<Py>),
    List(Vec<Py>),
    Dict(Vec<(Py, Py)>),
}

impl Py {
    pub fn s(t: impl Into<String>) -> Py {
        Py::Str(t.into())
    }
    /// `repr(x)`
    pub fn repr(&self) -> String {
        let mut s = String::new();
        self.write_repr(&mut s);
        s
    }
    /// `str(x)`: a string is itself, anything else its repr.
    pub fn str(&self) -> String {
        match self {
            Py::Str(s) => s.clone(),
            _ => self.repr(),
        }
    }
    fn write_repr(&self, out: &mut String) {
        match self {
            Py::None => out.push_str("None"),
            Py::Bool(true) => out.push_str("True"),
            Py::Bool(false) => out.push_str("False"),
            Py::Int(n) => {
                let _ = write!(out, "{n}");
            }
            Py::Str(s) => out.push_str(&repr_str(s)),
            Py::Tuple(v) => {
                out.push('(');
                for (i, x) in v.iter().enumerate() {
                    if i > 0 {
                        out.push_str(", ");
                    }
                    x.write_repr(out);
                }
                if v.len() == 1 {
                    out.push(',');
                }
                out.push(')');
            }
            Py::List(v) => {
                out.push('[');
                for (i, x) in v.iter().enumerate() {
                    if i > 0 {
                        out.push_str(", ");
                    }
                    x.write_repr(out);
                }
                out.push(']');
            }
            Py::Dict(v) => {
                out.push('{');
                for (i, (k, x)) in v.iter().enumerate() {
                    if i > 0 {
                        out.push_str(", ");
                    }
                    k.write_repr(out);
                    out.push_str(": ");
                    x.write_repr(out);
                }
                out.push('}');
            }
        }
    }
    /// A JSON value as the Python object it was (a list stays a list).
    pub fn from_j(j: &J) -> Py {
        match j {
            J::Null => Py::None,
            J::Bool(b) => Py::Bool(*b),
            J::Int(n) => Py::Int(*n),
            J::Float(f) => Py::Str(py_float(*f)),
            J::Str(s) => Py::Str(s.clone()),
            J::Arr(a) => Py::List(a.iter().map(Py::from_j).collect()),
            J::Obj(o) => Py::Dict(o.iter().map(|(k, v)| (Py::Str(k.clone()), Py::from_j(v))).collect()),
        }
    }
    /// The value as the reference's JSON writes it (a tuple as a list).
    pub fn to_j(&self) -> J {
        match self {
            Py::None => J::Null,
            Py::Bool(b) => J::Bool(*b),
            Py::Int(n) => J::Int(*n),
            Py::Str(s) => J::Str(s.clone()),
            Py::Tuple(v) | Py::List(v) => J::Arr(v.iter().map(Py::to_j).collect()),
            Py::Dict(v) => J::Obj(v.iter().map(|(k, x)| (k.str(), x.to_j())).collect()),
        }
    }
}

/// Python's `repr` of a str.
pub fn repr_str(s: &str) -> String {
    let q = if s.contains('\'') && !s.contains('"') {
        '"'
    } else {
        '\''
    };
    let mut out = String::with_capacity(s.len() + 2);
    out.push(q);
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c == q => {
                out.push('\\');
                out.push(c);
            }
            c if (c as u32) < 0x20 || c as u32 == 0x7f => {
                let _ = write!(out, "\\x{:02x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push(q);
    out
}

/// `text[:n]` in characters (Python slices by code point).
pub fn cut(text: &str, n: usize) -> String {
    match text.char_indices().nth(n) {
        Some((i, _)) => text[..i].to_string(),
        None => text.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dumps_like_python() {
        let v = parse("{\"a\": [1, 2.5, \"x\\u2032y\", null, true], \"b\": {}}").unwrap();
        assert_eq!(dumps(&v), "{\"a\": [1, 2.5, \"x\\u2032y\", null, true], \"b\": {}}");
        assert_eq!(py_float(21.22), "21.22");
        assert_eq!(py_float(1e-5), "1e-05");
        assert_eq!(py_float(3.0), "3.0");
    }

    #[test]
    fn repr_like_python() {
        let t = Py::Tuple(vec![Py::s("event(x)#1"), Py::s("end pos:script:2")]);
        assert_eq!(t.repr(), "('event(x)#1', 'end pos:script:2')");
        assert_eq!(Py::Tuple(vec![Py::Int(1)]).repr(), "(1,)");
        assert_eq!(Py::s("it's").repr(), "\"it's\"");
        assert_eq!(cut("Lemma 9′: no run", 8), "Lemma 9′");
    }
}

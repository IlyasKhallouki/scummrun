//! Python's `json.dumps` and `json.loads`, byte for byte, on `serde_json::Value` with its keys in insertion order (the
//! crate's `preserve_order`): the reference's files (explore.py, step.py, guide.py) are written this way, and the
//! archive's files must stay its bytes.
//!
//! * `dumps(v, opts)`: `json.dumps(v, indent=..., separators=..., sort_keys=...)` with `ensure_ascii=True` (Python's
//!   default): every character outside ' '..'~' escaped as `\uXXXX` (lower-case hex, surrogate pairs above U+FFFF).
//! * floats as Python's `repr` (the shortest digits that round-trip; fixed notation for 1e-4 <= |x| < 1e16, else
//!   `1e-05`, `1.5e+16`), `Infinity`, `-Infinity` and `NaN` as `json.dumps` writes them.
//! * integers as Python prints them; bool `true`/`false`; None `null`.

use serde_json::{Map, Number, Value};
use std::fmt::Write as _;

/// The options of one `json.dumps` call.
#[derive(Clone, Copy, Debug)]
pub struct Opts {
    /// `indent=n` (None: one line)
    pub indent: Option<usize>,
    /// `separators=(item, key)`; None: Python's default for the indent
    pub compact: bool,
    pub sort_keys: bool,
}

impl Opts {
    /// `json.dumps(v)`: `, ` and `: `
    pub const DEFAULT: Opts = Opts { indent: None, compact: false, sort_keys: false };
    /// `separators=(",", ":")`
    pub const COMPACT: Opts = Opts { indent: None, compact: true, sort_keys: false };
    /// `sort_keys=True, separators=(",", ":")` (explore.py `canonical`)
    pub const CANONICAL: Opts = Opts { indent: None, compact: true, sort_keys: true };
    /// `indent=1` (`,` and `: `)
    pub const INDENT1: Opts = Opts { indent: Some(1), compact: false, sort_keys: false };
}

/// `json.dumps(v, ...)` as Python writes it.
pub fn dumps(v: &Value, o: Opts) -> String {
    let mut s = String::with_capacity(256);
    write_value(&mut s, v, o, 0);
    s
}

/// `json.dumps(v)`.
pub fn dumps_default(v: &Value) -> String {
    dumps(v, Opts::DEFAULT)
}

/// `json.dumps(v, separators=(",", ":"))`.
pub fn compact(v: &Value) -> String {
    dumps(v, Opts::COMPACT)
}

/// explore.py `canonical`: `json.dumps(v, sort_keys=True, separators=(",", ":"))`.
pub fn canonical(v: &Value) -> String {
    dumps(v, Opts::CANONICAL)
}

fn item_sep(o: Opts) -> &'static str {
    if o.compact || o.indent.is_some() {
        ","
    } else {
        ", "
    }
}

fn key_sep(o: Opts) -> &'static str {
    if o.compact {
        ":"
    } else {
        ": "
    }
}

fn newline(s: &mut String, o: Opts, level: usize) {
    if let Some(n) = o.indent {
        s.push('\n');
        for _ in 0..n * level {
            s.push(' ');
        }
    }
}

fn write_value(s: &mut String, v: &Value, o: Opts, level: usize) {
    match v {
        Value::Null => s.push_str("null"),
        Value::Bool(true) => s.push_str("true"),
        Value::Bool(false) => s.push_str("false"),
        Value::Number(n) => write_number(s, n),
        Value::String(t) => write_str(s, t),
        Value::Array(a) => {
            if a.is_empty() {
                s.push_str("[]");
                return;
            }
            s.push('[');
            for (i, x) in a.iter().enumerate() {
                if i > 0 {
                    s.push_str(item_sep(o));
                }
                newline(s, o, level + 1);
                write_value(s, x, o, level + 1);
            }
            newline(s, o, level);
            s.push(']');
        }
        Value::Object(m) => {
            if m.is_empty() {
                s.push_str("{}");
                return;
            }
            s.push('{');
            let mut keys: Vec<&String> = m.keys().collect();
            if o.sort_keys {
                keys.sort();
            }
            for (i, k) in keys.into_iter().enumerate() {
                if i > 0 {
                    s.push_str(item_sep(o));
                }
                newline(s, o, level + 1);
                write_str(s, k);
                s.push_str(key_sep(o));
                write_value(s, &m[k.as_str()], o, level + 1);
            }
            newline(s, o, level);
            s.push('}');
        }
    }
}

fn write_number(s: &mut String, n: &Number) {
    if let Some(i) = n.as_i64() {
        let _ = write!(s, "{i}");
    } else if let Some(u) = n.as_u64() {
        let _ = write!(s, "{u}");
    } else if let Some(f) = n.as_f64() {
        s.push_str(&float_repr(f));
    }
}

/// Python's `float.__repr__` (and `json.dumps`'s float text).
pub fn float_repr(f: f64) -> String {
    if f.is_nan() {
        return "NaN".into();
    }
    if f.is_infinite() {
        return if f > 0.0 { "Infinity".into() } else { "-Infinity".into() };
    }
    if f == 0.0 {
        return if f.is_sign_negative() { "-0.0".into() } else { "0.0".into() };
    }
    // the shortest round-trip digits, as d.ddde[-]x
    let e = format!("{:e}", f);
    let (mant, exp) = e.split_once('e').unwrap();
    let exp: i32 = exp.parse().unwrap();
    let neg = mant.starts_with('-');
    let mant = mant.trim_start_matches('-');
    let digits: String = mant.chars().filter(|c| *c != '.').collect();
    let mut out = String::new();
    if neg {
        out.push('-');
    }
    if (-4..16).contains(&exp) {
        // fixed: the point after digit exp + 1
        let point = exp + 1;
        if point <= 0 {
            out.push_str("0.");
            for _ in 0..(-point) {
                out.push('0');
            }
            out.push_str(&digits);
        } else if point as usize >= digits.len() {
            out.push_str(&digits);
            for _ in 0..(point as usize - digits.len()) {
                out.push('0');
            }
            out.push_str(".0");
        } else {
            out.push_str(&digits[..point as usize]);
            out.push('.');
            out.push_str(&digits[point as usize..]);
        }
    } else {
        out.push_str(&digits[..1]);
        if digits.len() > 1 {
            out.push('.');
            out.push_str(&digits[1..]);
        }
        let _ = write!(out, "e{}{:02}", if exp < 0 { '-' } else { '+' }, exp.abs());
    }
    out
}

/// A string as `json.dumps` writes it with `ensure_ascii=True`.
pub fn write_str(s: &mut String, t: &str) {
    s.push('"');
    for c in t.chars() {
        match c {
            '"' => s.push_str("\\\""),
            '\\' => s.push_str("\\\\"),
            '\n' => s.push_str("\\n"),
            '\r' => s.push_str("\\r"),
            '\t' => s.push_str("\\t"),
            '\u{8}' => s.push_str("\\b"),
            '\u{c}' => s.push_str("\\f"),
            ' '..='~' => s.push(c),
            _ => {
                let mut buf = [0u16; 2];
                for u in c.encode_utf16(&mut buf) {
                    let _ = write!(s, "\\u{:04x}", u);
                }
            }
        }
    }
    s.push('"');
}

/// A string's JSON text (`json.dumps(t)`).
pub fn str_text(t: &str) -> String {
    let mut s = String::new();
    write_str(&mut s, t);
    s
}

/// `json.loads`: Python reads `Infinity`, `-Infinity` and `NaN` too, which serde_json does not; they are mapped to
/// strings first only where they stand as values (the archive's select keys may hold `Infinity`).
pub fn loads(text: &str) -> Result<Value, serde_json::Error> {
    match serde_json::from_str(text) {
        Ok(v) => Ok(v),
        Err(e) => {
            if text.contains("Infinity") || text.contains("NaN") {
                let fixed = replace_specials(text);
                serde_json::from_str(&fixed)
            } else {
                Err(e)
            }
        }
    }
}

// the JSON texts that stand for them, and the strings they decode to
const INF_TOKEN: &str = "\"\\u0000fmi:Infinity\"";
const NINF_TOKEN: &str = "\"\\u0000fmi:-Infinity\"";
const NAN_TOKEN: &str = "\"\\u0000fmi:NaN\"";
const INF_STR: &str = "\u{0}fmi:Infinity";
const NINF_STR: &str = "\u{0}fmi:-Infinity";
const NAN_STR: &str = "\u{0}fmi:NaN";

fn replace_specials(text: &str) -> String {
    // outside strings only
    let b = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    let mut in_str = false;
    while i < b.len() {
        let c = b[i];
        if in_str {
            out.push(c as char);
            if c == b'\\' && i + 1 < b.len() {
                out.push(b[i + 1] as char);
                i += 2;
                continue;
            }
            if c == b'"' {
                in_str = false;
            }
            i += 1;
            continue;
        }
        if c == b'"' {
            in_str = true;
            out.push('"');
            i += 1;
            continue;
        }
        if text[i..].starts_with("-Infinity") {
            out.push_str(NINF_TOKEN);
            i += 9;
        } else if text[i..].starts_with("Infinity") {
            out.push_str(INF_TOKEN);
            i += 8;
        } else if text[i..].starts_with("NaN") {
            out.push_str(NAN_TOKEN);
            i += 3;
        } else {
            // a multi-byte character: copy it whole
            let ch = text[i..].chars().next().unwrap();
            out.push(ch);
            i += ch.len_utf8();
        }
    }
    out
}

/// A value read by `loads` that stood for Infinity (+1), -Infinity (-1) or NaN (0).
pub fn special(v: &Value) -> Option<i8> {
    match v.as_str()? {
        INF_STR => Some(1),
        NINF_STR => Some(-1),
        NAN_STR => Some(0),
        _ => None,
    }
}

/// An empty JSON object.
pub fn obj() -> Map<String, Value> {
    Map::new()
}

/// Python's `round(x, n)` for the n the reference uses (1, 2, 3): round half to even on the decimal value of the
/// double, as Python's correctly rounded `round`.
pub fn round(x: f64, n: i32) -> f64 {
    if !x.is_finite() {
        return x;
    }
    // Python: the exact decimal value of x, rounded half-even at n digits; format! with precision rounds the exact
    // binary value half-even as well
    let t = format!("{:.*}", n as usize, x);
    t.parse().unwrap_or(x)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn floats_as_python() {
        for (f, t) in [
            (0.0, "0.0"),
            (1.0, "1.0"),
            (0.1, "0.1"),
            (1e-5, "1e-05"),
            (0.0001, "0.0001"),
            (1e16, "1e+16"),
            (1e15, "1000000000000000.0"),
            (123.456, "123.456"),
            (-2.5, "-2.5"),
            (1.5e300, "1.5e+300"),
            (1762082541.123456, "1762082541.123456"),
            (f64::INFINITY, "Infinity"),
            (0.30000000000000004, "0.30000000000000004"),
        ] {
            assert_eq!(float_repr(f), t, "{f}");
        }
    }

    #[test]
    fn dumps_as_python() {
        let v = json!({"b": [1, 2.0, "é\n"], "a": {}, "c": [], "d": null, "e": true});
        assert_eq!(dumps_default(&v), r#"{"b": [1, 2.0, "\u00e9\n"], "a": {}, "c": [], "d": null, "e": true}"#);
        assert_eq!(compact(&v), r#"{"b":[1,2.0,"\u00e9\n"],"a":{},"c":[],"d":null,"e":true}"#);
        assert_eq!(canonical(&v), r#"{"a":{},"b":[1,2.0,"\u00e9\n"],"c":[],"d":null,"e":true}"#);
        assert_eq!(
            dumps(&json!({"a": [1, {"x": 2}], "b": {}}), Opts::INDENT1),
            "{\n \"a\": [\n  1,\n  {\n   \"x\": 2\n  }\n ],\n \"b\": {}\n}"
        );
        assert_eq!(str_text("\u{1F600}\u{7f}"), "\"\\ud83d\\ude00\\u007f\"");
    }

    #[test]
    fn loads_infinity() {
        let v = loads("[1, Infinity, \"Infinity\", -Infinity]").unwrap();
        assert_eq!(special(&v[1]), Some(1));
        assert_eq!(special(&v[2]), None);
        assert_eq!(special(&v[3]), Some(-1));
    }

    #[test]
    fn rounding() {
        assert_eq!(round(2.675, 2), 2.67);
        assert_eq!(round(0.05, 1), 0.1);
        assert_eq!(round(1.25, 1), 1.2);
    }
}

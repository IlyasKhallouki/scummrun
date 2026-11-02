//! JSON as Python's `json.dumps` writes it (E011, reader-scope `--out-dir`): the reading's files are the bytes
//! packed_reading.py's `reading_from_golden` and replay_part1.py's `read_rust` write, so their values are written
//! the way Python writes the values they decode.
//!
//! * [`PJ`]: a JSON value as Python holds it after `json.loads` (objects keep their key order, as a dict does).
//! * [`PJ::compact`]: `json.dumps(v)` (separators `", "` and `": "`, ensure_ascii).
//! * [`PJ::indent1`]: `json.dumps(v, indent=1)`.
//! * [`PJ::from_py`]: what `packed_reading._dec` makes of golden.py's encoding of a value ([`Py`]), as `json.dumps`
//!   then writes it. A set has no such text that does not depend on Python's hash order, and a dict with keys that
//!   are not strings has none that json.dumps accepts in every case: both are refused (no field of reading.json
//!   holds one).
//! * [`PJ::from_serde`]: a value of the reader's report as Python reads it back from the report's file (serde_json
//!   writes object keys sorted).
//! * [`PJ::from_json`]: a value of an input file read with its keys in the file's order (scaling.rs `Json`).

use crate::py::Py;
use crate::scaling::Json;
use std::fmt::Write;

#[derive(Clone, Debug, PartialEq)]
pub enum PJ {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    List(Vec<PJ>),
    Obj(Vec<(String, PJ)>),
}

/// Python's `repr(float)` (the shortest text that reads back, fixed notation for exponents -4..16, else
/// `d.ddde+XX`), as json.dumps writes a finite float.
pub fn float_repr(x: f64) -> String {
    if x.is_nan() {
        return "NaN".into();
    }
    if x.is_infinite() {
        return if x > 0.0 { "Infinity".into() } else { "-Infinity".into() };
    }
    if x == 0.0 {
        return if x.is_sign_negative() { "-0.0".into() } else { "0.0".into() };
    }
    // the shortest round-trip digits and the decimal exponent, from Rust's `{:e}` (shortest digits too)
    let e = format!("{:e}", x.abs());
    let (mant, exp) = e.split_once('e').expect("an exponent");
    let exp: i32 = exp.parse().expect("an int exponent");
    let digits: String = mant.chars().filter(|c| *c != '.').collect();
    let decpt = exp + 1; // value = 0.DIGITS x 10^decpt
    let sign = if x < 0.0 { "-" } else { "" };
    if -4 < decpt && decpt <= 16 {
        let n = digits.len() as i32;
        let s = if decpt <= 0 {
            format!("0.{}{}", "0".repeat((-decpt) as usize), digits)
        } else if decpt >= n {
            format!("{}{}.0", digits, "0".repeat((decpt - n) as usize))
        } else {
            format!("{}.{}", &digits[..decpt as usize], &digits[decpt as usize..])
        };
        format!("{sign}{s}")
    } else {
        let m = if digits.len() == 1 { digits.clone() } else { format!("{}.{}", &digits[..1], &digits[1..]) };
        let e = decpt - 1;
        format!("{sign}{m}e{}{:02}", if e < 0 { '-' } else { '+' }, e.abs())
    }
}

impl PJ {
    pub fn str(s: impl Into<String>) -> PJ {
        PJ::Str(s.into())
    }

    pub fn obj(items: Vec<(&str, PJ)>) -> PJ {
        PJ::Obj(items.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
    }

    pub fn ints<I: IntoIterator<Item = i64>>(xs: I) -> PJ {
        PJ::List(xs.into_iter().map(PJ::Int).collect())
    }

    /// The value of key `k` of an object.
    pub fn get(&self, k: &str) -> Option<&PJ> {
        match self {
            PJ::Obj(items) => items.iter().find(|x| x.0 == k).map(|x| &x.1),
            _ => None,
        }
    }

    /// `d[k] = v`: replaces the value in place, or appends the key (a dict's order).
    pub fn set(&mut self, k: &str, v: PJ) {
        let PJ::Obj(items) = self else { panic!("set on a value that is not an object") };
        match items.iter_mut().find(|x| x.0 == k) {
            Some(x) => x.1 = v,
            None => items.push((k.to_string(), v)),
        }
    }

    fn scalar(&self, out: &mut String) -> bool {
        match self {
            PJ::Null => out.push_str("null"),
            PJ::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            PJ::Int(n) => {
                let _ = write!(out, "{n}");
            }
            PJ::Float(x) => out.push_str(&float_repr(*x)),
            PJ::Str(s) => scumm_ir::json::write_str(s, out),
            _ => return false,
        }
        true
    }

    /// `json.dumps(v)`.
    pub fn compact(&self, out: &mut String) {
        if self.scalar(out) {
            return;
        }
        match self {
            PJ::List(xs) => {
                out.push('[');
                for (i, x) in xs.iter().enumerate() {
                    if i > 0 {
                        out.push_str(", ");
                    }
                    x.compact(out);
                }
                out.push(']');
            }
            PJ::Obj(items) => {
                out.push('{');
                for (i, (k, v)) in items.iter().enumerate() {
                    if i > 0 {
                        out.push_str(", ");
                    }
                    scumm_ir::json::write_str(k, out);
                    out.push_str(": ");
                    v.compact(out);
                }
                out.push('}');
            }
            _ => unreachable!(),
        }
    }

    pub fn compact_text(&self) -> String {
        let mut s = String::new();
        self.compact(&mut s);
        s
    }

    /// `json.dumps(v, indent=1)` at nesting `level`.
    pub fn indent1(&self, out: &mut String, level: usize) {
        if self.scalar(out) {
            return;
        }
        let pad = |out: &mut String, n: usize| {
            out.push('\n');
            for _ in 0..n {
                out.push(' ');
            }
        };
        match self {
            PJ::List(xs) if xs.is_empty() => out.push_str("[]"),
            PJ::Obj(items) if items.is_empty() => out.push_str("{}"),
            PJ::List(xs) => {
                out.push('[');
                for (i, x) in xs.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    pad(out, level + 1);
                    x.indent1(out, level + 1);
                }
                pad(out, level);
                out.push(']');
            }
            PJ::Obj(items) => {
                out.push('{');
                for (i, (k, v)) in items.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    pad(out, level + 1);
                    scumm_ir::json::write_str(k, out);
                    out.push_str(": ");
                    v.indent1(out, level + 1);
                }
                pad(out, level);
                out.push('}');
            }
            _ => unreachable!(),
        }
    }

    /// `json.dumps(v, indent=1) + "\n"`.
    pub fn indent1_text(&self) -> String {
        let mut s = String::new();
        self.indent1(&mut s, 0);
        s.push('\n');
        s
    }

    /// `_dec` of golden.py's encoding, as json.dumps writes it (module docstring).
    pub fn from_py(p: &Py) -> PJ {
        match p {
            Py::None => PJ::Null,
            Py::Bool(b) => PJ::Bool(*b),
            Py::Int(n) => PJ::Int(*n),
            Py::Str(s) => PJ::Str(s.clone()),
            Py::List(xs) | Py::Tuple(xs) => PJ::List(xs.iter().map(PJ::from_py).collect()),
            Py::Dict(kv) | Py::Obj(_, kv) => {
                // golden.py writes a dict's keys sorted, and json.loads keeps that order
                let mut kv: Vec<&(String, Py)> = kv.iter().collect();
                kv.sort_by(|a, b| a.0.cmp(&b.0));
                PJ::Obj(kv.into_iter().map(|(k, v)| (k.clone(), PJ::from_py(v))).collect())
            }
            Py::Set(_) => panic!("a set in a value reading.json holds: {}", p.text()),
            Py::Map(_) => panic!("a dict with keys that are not str in a value reading.json holds: {}", p.text()),
        }
    }

    /// A value of the report as Python reads it back from the file serde_json wrote (keys sorted).
    pub fn from_serde(v: &serde_json::Value) -> PJ {
        use serde_json::Value;
        match v {
            Value::Null => PJ::Null,
            Value::Bool(b) => PJ::Bool(*b),
            Value::Number(n) => match n.as_i64() {
                Some(i) => PJ::Int(i),
                None => PJ::Float(n.as_f64().expect("a number")),
            },
            Value::String(s) => PJ::Str(s.clone()),
            Value::Array(xs) => PJ::List(xs.iter().map(PJ::from_serde).collect()),
            Value::Object(m) => PJ::Obj(m.iter().map(|(k, v)| (k.clone(), PJ::from_serde(v))).collect()),
        }
    }

    /// A value of an input file, with its keys in the file's order (json.loads).
    pub fn from_json(v: &Json) -> PJ {
        match v {
            Json::Null => PJ::Null,
            Json::Bool(b) => PJ::Bool(*b),
            Json::Num(t) => match t.parse::<i64>() {
                Ok(i) => PJ::Int(i),
                Err(_) => PJ::Float(t.parse::<f64>().unwrap_or_else(|_| panic!("a number: {t}"))),
            },
            Json::Str(s) => PJ::Str(s.clone()),
            Json::Arr(xs) => PJ::List(xs.iter().map(PJ::from_json).collect()),
            Json::Obj(items) => PJ::Obj(items.iter().map(|(k, v)| (k.clone(), PJ::from_json(v))).collect()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn floats_as_python() {
        for (x, t) in [
            (11.9, "11.9"),
            (12.0, "12.0"),
            (0.1, "0.1"),
            (1e-5, "1e-05"),
            (0.0001, "0.0001"),
            (1e16, "1e+16"),
            (1234567890123456.0, "1234567890123456.0"),
            (-2.5, "-2.5"),
            (123.45, "123.45"),
            (0.0, "0.0"),
            (1.5e300, "1.5e+300"),
        ] {
            assert_eq!(float_repr(x), t, "{x}");
        }
    }

    #[test]
    fn dumps_as_python() {
        let v = PJ::obj(vec![
            ("a", PJ::ints([1, 2])),
            ("b", PJ::Obj(vec![])),
            ("c", PJ::List(vec![])),
            ("d", PJ::obj(vec![("x", PJ::Null), ("y", PJ::str("é\"\n"))])),
        ]);
        assert_eq!(v.compact_text(), r#"{"a": [1, 2], "b": {}, "c": [], "d": {"x": null, "y": "\u00e9\"\n"}}"#);
        assert_eq!(
            v.indent1_text(),
            "{\n \"a\": [\n  1,\n  2\n ],\n \"b\": {},\n \"c\": [],\n \"d\": {\n  \"x\": null,\n  \"y\": \"\\u00e9\\\"\\n\"\n }\n}\n"
        );
    }
}

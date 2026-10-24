//! Canonical JSON, byte for byte what Python's
//! `json.dumps(obj, sort_keys=True, separators=(",", ":"), ensure_ascii=True)` writes.

use std::collections::BTreeMap;
use std::fmt::Write;

/// A JSON value. Objects keep their keys sorted (by code point, as Python's sort_keys).
#[derive(Clone, Debug, PartialEq)]
pub enum J {
    Null,
    Bool(bool),
    Int(i64),
    Str(String),
    Arr(Vec<J>),
    Obj(BTreeMap<String, J>),
}

impl From<bool> for J {
    fn from(b: bool) -> J {
        J::Bool(b)
    }
}
impl From<i32> for J {
    fn from(v: i32) -> J {
        J::Int(v as i64)
    }
}
impl From<u32> for J {
    fn from(v: u32) -> J {
        J::Int(v as i64)
    }
}
impl From<i64> for J {
    fn from(v: i64) -> J {
        J::Int(v)
    }
}
impl From<usize> for J {
    fn from(v: usize) -> J {
        J::Int(v as i64)
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
    fn from(v: Option<T>) -> J {
        v.map_or(J::Null, Into::into)
    }
}
impl<T: Into<J>> From<Vec<T>> for J {
    fn from(v: Vec<T>) -> J {
        J::Arr(v.into_iter().map(Into::into).collect())
    }
}

/// An object from (key, value) pairs.
pub fn obj<const N: usize>(pairs: [(&str, J); N]) -> J {
    J::Obj(pairs.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
}

/// An array from values of mixed types.
#[macro_export]
macro_rules! arr {
    ($($x:expr),* $(,)?) => { $crate::json::J::Arr(vec![$($crate::json::J::from($x)),*]) };
}

impl J {
    /// Adds a key to an object.
    pub fn set(&mut self, k: &str, v: impl Into<J>) {
        match self {
            J::Obj(m) => {
                m.insert(k.to_string(), v.into());
            }
            _ => panic!("set on a non-object"),
        }
    }

    pub fn write(&self, out: &mut String) {
        match self {
            J::Null => out.push_str("null"),
            J::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            J::Int(v) => write!(out, "{v}").unwrap(),
            J::Str(s) => write_str(s, out),
            J::Arr(a) => {
                out.push('[');
                for (k, x) in a.iter().enumerate() {
                    if k > 0 {
                        out.push(',');
                    }
                    x.write(out);
                }
                out.push(']');
            }
            J::Obj(m) => {
                out.push('{');
                for (k, (key, x)) in m.iter().enumerate() {
                    if k > 0 {
                        out.push(',');
                    }
                    write_str(key, out);
                    out.push(':');
                    x.write(out);
                }
                out.push('}');
            }
        }
    }

    /// The value as one line, with its "\n".
    pub fn line(&self) -> String {
        let mut s = String::new();
        self.write(&mut s);
        s.push('\n');
        s
    }
}

/// A string as Python's ensure_ascii encoder writes it.
pub fn write_str(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\x08' => out.push_str("\\b"),
            '\x0c' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ' '..='~' => out.push(c),
            c => {
                let mut buf = [0u16; 2];
                for u in c.encode_utf16(&mut buf) {
                    write!(out, "\\u{:04x}", u).unwrap();
                }
            }
        }
    }
    out.push('"');
}

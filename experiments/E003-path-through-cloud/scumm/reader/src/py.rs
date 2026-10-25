//! Python values as golden.py's canonical encoding writes them, and the few Python behaviours the reader's
//! output rests on (a `Counter` that keeps keys added with 0, `str()` of values, `repr()` of tuples).
//!
//! [`Py`] is a typed tree: golden.py's `enc` maps tuples, sets, non-str-keyed dicts and dataclasses to
//! `{"$tuple": ...}`, `{"$set": ...}`, `{"$map": ...}`, `{"$Name": {...}}`; [`Py::text`] writes the canonical text
//! (compact, keys sorted, ASCII only, golden.py's order rule for set elements and map keys) and [`Py::parse`] reads
//! it back. A line that parses and writes back to the same text is held whole by the tree.

use serde_json::Value;
use std::collections::BTreeMap;
use std::fmt::Write;

/// A Python value in golden.py's canonical encoding.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Py {
    None,
    Bool(bool),
    Int(i64),
    Str(String),
    List(Vec<Py>),
    Tuple(Vec<Py>),
    /// a set or frozenset: written sorted by the order rule
    Set(Vec<Py>),
    /// a dict whose keys are all str (a JSON object, keys sorted)
    Dict(Vec<(String, Py)>),
    /// any other dict (`$map`, sorted by key with the order rule)
    Map(Vec<(Py, Py)>),
    /// a dataclass instance: class name, fields
    Obj(String, Vec<(String, Py)>),
}

impl Py {
    pub fn str(s: impl Into<String>) -> Py {
        Py::Str(s.into())
    }
    pub fn tuple(items: Vec<Py>) -> Py {
        Py::Tuple(items)
    }
    pub fn obj(name: &str, fields: Vec<(&str, Py)>) -> Py {
        Py::Obj(name.to_string(), fields.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
    }
    pub fn dict(fields: Vec<(&str, Py)>) -> Py {
        Py::Dict(fields.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
    }
    pub fn opt_int(v: Option<i64>) -> Py {
        v.map_or(Py::None, Py::Int)
    }
    pub fn opt_bool(v: Option<bool>) -> Py {
        v.map_or(Py::None, Py::Bool)
    }
    pub fn strs<S: AsRef<str>>(items: &[S]) -> Py {
        Py::List(items.iter().map(|s| Py::str(s.as_ref())).collect())
    }

    /// The canonical text (golden.py `dumps`).
    pub fn text(&self) -> String {
        let mut s = String::new();
        self.write(&mut s);
        s
    }

    pub fn write(&self, out: &mut String) {
        match self {
            Py::None => out.push_str("null"),
            Py::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            Py::Int(n) => {
                let _ = write!(out, "{n}");
            }
            Py::Str(s) => scumm_ir::json::write_str(s, out),
            Py::List(xs) => write_seq(xs.iter().map(|x| x.text()), out),
            Py::Tuple(xs) => {
                out.push_str("{\"$tuple\":");
                write_seq(xs.iter().map(|x| x.text()), out);
                out.push('}');
            }
            Py::Set(xs) => {
                out.push_str("{\"$set\":");
                let mut items: Vec<(&Py, String)> = xs.iter().map(|x| (x, x.text())).collect();
                order(&mut items);
                write_seq(items.into_iter().map(|p| p.1), out);
                out.push('}');
            }
            Py::Dict(kv) => {
                let mut kv: Vec<&(String, Py)> = kv.iter().collect();
                kv.sort_by(|a, b| a.0.cmp(&b.0));
                out.push('{');
                for (i, (k, v)) in kv.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    scumm_ir::json::write_str(k, out);
                    out.push(':');
                    v.write(out);
                }
                out.push('}');
            }
            Py::Map(kv) => {
                if kv.is_empty() {
                    out.push_str("{}");
                    return;
                }
                let mut items: Vec<(&Py, String, &Py)> = kv.iter().map(|(k, v)| (k, k.text(), v)).collect();
                let mut keys: Vec<(&Py, String)> = items.iter().map(|x| (x.0, x.1.clone())).collect();
                order(&mut keys);
                let pos: BTreeMap<String, usize> = keys.iter().enumerate().map(|(i, k)| (k.1.clone(), i)).collect();
                items.sort_by_key(|x| pos[&x.1]);
                out.push_str("{\"$map\":[");
                for (i, (_, kt, v)) in items.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    out.push('[');
                    out.push_str(kt);
                    out.push(',');
                    v.write(out);
                    out.push(']');
                }
                out.push_str("]}");
            }
            Py::Obj(name, fields) => {
                out.push_str("{\"$");
                out.push_str(name);
                out.push_str("\":");
                Py::Dict(fields.clone()).write(out);
                out.push('}');
            }
        }
    }

    /// Reads golden.py's canonical encoding (a parsed JSON value) back into a typed tree.
    pub fn parse(v: &Value) -> Py {
        match v {
            Value::Null => Py::None,
            Value::Bool(b) => Py::Bool(*b),
            Value::Number(n) => Py::Int(n.as_i64().unwrap_or_else(|| panic!("not an int: {n}"))),
            Value::String(s) => Py::Str(s.clone()),
            Value::Array(xs) => Py::List(xs.iter().map(Py::parse).collect()),
            Value::Object(m) => {
                if m.len() == 1 {
                    let (k, x) = m.iter().next().unwrap();
                    if let Some(name) = k.strip_prefix('$') {
                        return match name {
                            "tuple" => Py::Tuple(arr(x).iter().map(Py::parse).collect()),
                            "set" => Py::Set(arr(x).iter().map(Py::parse).collect()),
                            "map" => Py::Map(
                                arr(x)
                                    .iter()
                                    .map(|p| {
                                        let p = arr(p);
                                        (Py::parse(&p[0]), Py::parse(&p[1]))
                                    })
                                    .collect(),
                            ),
                            "float" => panic!("a float in the canonical encoding: {x}"),
                            _ => match x {
                                Value::Object(f) => {
                                    Py::Obj(name.to_string(), f.iter().map(|(a, b)| (a.clone(), Py::parse(b))).collect())
                                }
                                _ => panic!("a dataclass that is not an object: {v}"),
                            },
                        };
                    }
                }
                Py::Dict(m.iter().map(|(a, b)| (a.clone(), Py::parse(b))).collect())
            }
        }
    }

    pub fn from_text(s: &str) -> Py {
        Py::parse(&serde_json::from_str::<Value>(s).expect("a JSON line"))
    }

    // --- typed access (panics on another shape: the oracles' shapes are fixed) ---------------------------------------

    pub fn int(&self) -> i64 {
        match self {
            Py::Int(n) => *n,
            _ => panic!("not an int: {}", self.text()),
        }
    }
    pub fn as_str(&self) -> &str {
        match self {
            Py::Str(s) => s,
            _ => panic!("not a str: {}", self.text()),
        }
    }
    pub fn bool(&self) -> bool {
        match self {
            Py::Bool(b) => *b,
            _ => panic!("not a bool: {}", self.text()),
        }
    }
    pub fn opt_bool_of(&self) -> Option<bool> {
        match self {
            Py::None => None,
            Py::Bool(b) => Some(*b),
            _ => panic!("not a bool or None: {}", self.text()),
        }
    }
    pub fn is_none(&self) -> bool {
        matches!(self, Py::None)
    }
    /// The items of a list, tuple or set.
    pub fn items(&self) -> &[Py] {
        match self {
            Py::List(x) | Py::Tuple(x) | Py::Set(x) => x,
            _ => panic!("not a sequence: {}", self.text()),
        }
    }
    /// The pairs of a dict (str keys) as (key, value); an empty `$map` reads as `{}` too.
    pub fn dict_items(&self) -> &[(String, Py)] {
        match self {
            Py::Dict(x) => x,
            _ => panic!("not a dict: {}", self.text()),
        }
    }
    /// The pairs of a `$map`, or of an empty dict.
    pub fn map_items(&self) -> Vec<(&Py, &Py)> {
        match self {
            Py::Map(x) => x.iter().map(|(a, b)| (a, b)).collect(),
            Py::Dict(x) if x.is_empty() => Vec::new(),
            _ => panic!("not a map: {}", self.text()),
        }
    }
    /// A field of a dict or of a dataclass.
    pub fn get(&self, k: &str) -> &Py {
        let fields = match self {
            Py::Dict(x) | Py::Obj(_, x) => x,
            _ => panic!("no field {k} in {}", self.text()),
        };
        fields.iter().find(|f| f.0 == k).map(|f| &f.1).unwrap_or_else(|| panic!("no field {k}"))
    }
    pub fn has(&self, k: &str) -> bool {
        match self {
            Py::Dict(x) | Py::Obj(_, x) => x.iter().any(|f| f.0 == k),
            _ => false,
        }
    }
    /// A dataclass's fields, checking its class.
    pub fn class(&self, name: &str) -> &Py {
        match self {
            Py::Obj(n, _) if n == name => self,
            _ => panic!("not a {name}: {}", self.text()),
        }
    }
}

fn arr(v: &Value) -> &Vec<Value> {
    match v {
        Value::Array(a) => a,
        _ => panic!("not an array: {v}"),
    }
}

fn write_seq(items: impl Iterator<Item = String>, out: &mut String) {
    out.push('[');
    for (i, t) in items.enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(&t);
    }
    out.push(']');
}

/// golden.py's order rule on encoded values: all ints numerically, all strs by code point, else by the text.
fn order(items: &mut [(&Py, String)]) {
    if items.iter().all(|x| matches!(x.0, Py::Int(_))) {
        items.sort_by_key(|x| x.0.int());
    } else if items.iter().all(|x| matches!(x.0, Py::Str(_))) {
        items.sort_by(|a, b| a.0.as_str().cmp(b.0.as_str()));
    } else {
        items.sort_by(|a, b| a.1.cmp(&b.1));
    }
}

/// `collections.Counter` with str keys, as the reader keeps its counts: a key added with 0 stays (Python's
/// `c[k] += 0` stores it), encoded with sorted keys.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Counter(pub BTreeMap<String, i64>);

impl Counter {
    pub fn add(&mut self, k: impl AsRef<str>, n: i64) {
        let k = k.as_ref();
        match self.0.get_mut(k) {
            Some(v) => *v += n,
            None => {
                self.0.insert(k.to_string(), n);
            }
        }
    }
    pub fn inc(&mut self, k: impl AsRef<str>) {
        self.add(k, 1)
    }
    /// `c[k] = n`
    pub fn set(&mut self, k: impl AsRef<str>, n: i64) {
        self.0.insert(k.as_ref().to_string(), n);
    }
    pub fn get(&self, k: &str) -> i64 {
        self.0.get(k).copied().unwrap_or(0)
    }
    /// `Counter.update(other)`: adds every key of `other`, also those with 0.
    pub fn update(&mut self, other: &Counter) {
        for (k, v) in &other.0 {
            self.add(k, *v);
        }
    }
    pub fn clear(&mut self) {
        self.0.clear()
    }
    pub fn py(&self) -> Py {
        Py::Dict(self.0.iter().map(|(k, v)| (k.clone(), Py::Int(*v))).collect())
    }
    pub fn from_py(p: &Py) -> Counter {
        Counter(p.dict_items().iter().map(|(k, v)| (k.clone(), v.int())).collect())
    }
}

/// A Counter whose keys are Python values (tuples), encoded as golden.py writes a dict with non-str keys.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PyCounter(pub Vec<(Py, i64)>, rustc_hash::FxHashMap<String, usize>);

impl PyCounter {
    pub fn add(&mut self, k: Py, n: i64) {
        let t = k.text();
        match self.1.get(&t) {
            Some(&i) => self.0[i].1 += n,
            None => {
                self.1.insert(t, self.0.len());
                self.0.push((k, n));
            }
        }
    }
    pub fn clear(&mut self) {
        self.0.clear();
        self.1.clear();
    }
    pub fn py(&self) -> Py {
        if self.0.iter().all(|(k, _)| matches!(k, Py::Str(_))) {
            return Py::Dict(self.0.iter().map(|(k, v)| (k.as_str().to_string(), Py::Int(*v))).collect());
        }
        Py::Map(self.0.iter().map(|(k, v)| (k.clone(), Py::Int(*v))).collect())
    }
}

/// Python's `repr` of a tuple whose items are given as their reprs: `(a,)`, `(a, b)`.
pub fn repr_tuple(items: &[String]) -> String {
    match items.len() {
        1 => format!("({},)", items[0]),
        _ => format!("({})", items.join(", ")),
    }
}

pub use scumm_ir::py::repr as repr_str;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_round_trip() {
        for s in [
            r#"{"a":{"$set":[1,2,10]},"b":{"$tuple":["x",null,true]},"c":{"$map":[[{"$tuple":["a",1]},2],[{"$tuple":["b",0]},1]]}}"#,
            r#"{"$set":["above","below"]}"#,
            r#"{"$set":["above",1,2]}"#,
            r#"{"$Event":{"a":1,"b":[]}}"#,
            r#"{}"#,
        ] {
            assert_eq!(Py::from_text(s).text(), s);
        }
        let s = Py::Set(vec![Py::Int(10), Py::Int(2), Py::str("above")]).text();
        assert_eq!(s, r#"{"$set":["above",10,2]}"#);
    }

    #[test]
    fn repr_of_tuples() {
        assert_eq!(repr_tuple(&["'a'".into()]), "('a',)");
        assert_eq!(repr_tuple(&[repr_str("x"), "1".into()]), "('x', 1)");
        assert_eq!(repr_tuple(&[]), "()");
    }
}

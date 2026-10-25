//! A facts.py map (fact key -> int, absent is 0) that keeps its keys' order, and the key names the reader uses.
//!
//! The reader names facts by facts.py's strings and adds its own kinds (module docstring of reader.py): `args:<unit>`,
//! `saved:<verb>:<id>`, `pos:<unit>`, `escpos:<unit>`, and the clear mark `at:<unit>:*` / `esc:<unit>:*`. Keys stay
//! strings here: every place Python sorts keys sorts these strings, and the Builder's work is small next to the
//! machine's.

use crate::py::Py;
use rustc_hash::FxHashMap;
use scumm_ir::UnitKey;

/// A facts.py map in its insertion order (Python's dict order: reader.Scope.of walks the start's keys in order).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Facts {
    items: Vec<(String, i64)>,
    index: FxHashMap<String, usize>,
}

impl Facts {
    pub fn new() -> Facts {
        Facts::default()
    }
    pub fn from_items(items: impl IntoIterator<Item = (String, i64)>) -> Facts {
        let mut f = Facts::new();
        for (k, v) in items {
            f.insert(k, v);
        }
        f
    }
    /// `d[k] = v`: a new key goes last, an old one keeps its place.
    pub fn insert(&mut self, k: String, v: i64) {
        match self.index.get(&k) {
            Some(&i) => self.items[i].1 = v,
            None => {
                self.index.insert(k.clone(), self.items.len());
                self.items.push((k, v));
            }
        }
    }
    pub fn get(&self, k: &str) -> Option<i64> {
        self.index.get(k).map(|&i| self.items[i].1)
    }
    /// `d.get(k, 0)`
    pub fn get0(&self, k: &str) -> i64 {
        self.get(k).unwrap_or(0)
    }
    pub fn contains(&self, k: &str) -> bool {
        self.index.contains_key(k)
    }
    pub fn iter(&self) -> impl Iterator<Item = (&String, i64)> {
        self.items.iter().map(|(k, v)| (k, *v))
    }
    pub fn len(&self) -> usize {
        self.items.len()
    }
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
    pub fn py(&self) -> Py {
        Py::Dict(self.items.iter().map(|(k, v)| (k.clone(), Py::Int(*v))).collect())
    }
    /// From a list of [key, value] pairs (the oracles' ``start_items``) or a dict.
    pub fn from_py(p: &Py) -> Facts {
        match p {
            Py::List(xs) => Facts::from_items(xs.iter().map(|x| {
                let x = x.items();
                (x[0].as_str().to_string(), x[1].int())
            })),
            _ => Facts::from_items(p.dict_items().iter().map(|(k, v)| (k.clone(), v.int()))),
        }
    }
}

/// model.py `unit_name`: the unit as `at:` and `esc:` keys write it.
pub fn unit_name(key: UnitKey) -> String {
    match key {
        UnitKey::Global(n) => format!("script:{n}"),
        UnitKey::Local(_, n) => format!("local:{n}"),
        UnitKey::Obj(o) => format!("obj:{o}"),
        UnitKey::Entry(_) => "entry".into(),
        UnitKey::Exit(_) => "exit".into(),
    }
}

/// model.py `slot_name`: a slot fact's unit name (objscript:O is obj:O).
pub fn slot_name(key: &str) -> String {
    match key.strip_prefix("objscript:") {
        Some(o) => format!("obj:{o}"),
        None => key.to_string(),
    }
}

/// model.py `family`: a fact's kind, with `at:` and `esc:` split by unit kind.
pub fn family(key: &str) -> String {
    let parts: Vec<&str> = key.split(':').collect();
    if parts[0] == "at" || parts[0] == "esc" {
        parts[..parts.len().min(2)].join(":")
    } else {
        parts[0].to_string()
    }
}

/// A unit key as golden.py encodes the tuple.
pub fn unit_py(k: UnitKey) -> Py {
    let (nums, n) = k.numbers();
    let mut items = vec![Py::str(k.kind())];
    items.extend(nums[..n].iter().map(|&v| Py::Int(v as i64)));
    Py::Tuple(items)
}

pub fn unit_from_py(p: &Py) -> UnitKey {
    let t = p.items();
    let n = |i: usize| t[i].int() as i32;
    match t[0].as_str() {
        "entry" => UnitKey::Entry(n(1)),
        "exit" => UnitKey::Exit(n(1)),
        "global" => UnitKey::Global(n(1)),
        "local" => UnitKey::Local(n(1), n(2)),
        "obj" => UnitKey::Obj(n(1)),
        k => panic!("not a unit kind: {k}"),
    }
}

/// golden.py's order of unit keys (the tuples' canonical text).
pub fn unit_text(k: UnitKey) -> String {
    unit_py(k).text()
}

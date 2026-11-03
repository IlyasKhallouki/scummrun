//! Values and views as the reference holds them: a task value (`Val`, with domains.py's labels as sentinels), Python's
//! None where a domain does not hold an engine value, and the set of built lists an `args:` fact can be
//! (strict.ArgsView). A view (replay.view, window.FastView) maps keys to these values.

use crate::pyj::{Py, J};
use fmi_core::{Domain, Key, Val, ABOVE, BELOW, NONE};
use rustc_hash::FxHashMap;
use std::sync::Arc;

pub const NOWHERE: Val = -1; // reader.NOWHERE
pub const ELSEWHERE: Val = -2; // replay.ELSEWHERE

/// A value is an int (not one of the labels nor None).
#[inline]
pub fn is_int(v: Val) -> bool {
    v != BELOW && v != ABOVE && v != NONE
}

/// `str(v)` of a task value: "below", "above", "None" or the int.
pub fn vstr(v: Val) -> String {
    fmi_core::val_text(v)
}

/// A value as JSON (an int, "below"/"above", or null).
pub fn vj(v: Val) -> J {
    match v {
        BELOW => J::Str("below".into()),
        ABOVE => J::Str("above".into()),
        NONE => J::Null,
        n => J::Int(n),
    }
}

/// A value as Python prints it in a repr.
pub fn vpy(v: Val) -> Py {
    match v {
        BELOW => Py::s("below"),
        ABOVE => Py::s("above"),
        NONE => Py::None,
        n => Py::Int(n),
    }
}

/// A value from the reference's JSON: an int, "below", "above" or null.
pub fn val_of(j: &J) -> Val {
    match j {
        J::Int(n) => *n,
        J::Str(s) if s == "below" => BELOW,
        J::Str(s) if s == "above" => ABOVE,
        J::Str(s) => s.parse().unwrap_or(NONE),
        J::Float(f) => *f as i64,
        _ => NONE,
    }
}

/// `sorted(map(str, vals))`
pub fn txt(vals: &[Val]) -> Vec<String> {
    let mut v: Vec<String> = vals.iter().map(|&x| vstr(x)).collect();
    v.sort();
    v
}

/// Values sorted as `sorted(vs, key=str)` (the reference's files).
pub fn sorted_by_str(vals: &[Val]) -> Vec<Val> {
    let mut v = vals.to_vec();
    v.sort_by_key(|&x| vstr(x));
    v
}

/// domains.py `Domain.add`: the values `v + d` can have.
pub fn dom_add(dom: &Domain, v: Val, d: i64) -> Vec<Val> {
    if v == ABOVE {
        if d >= 0 {
            return vec![ABOVE];
        }
        let h = dom.high.unwrap_or(0);
        return dom
            .values
            .iter()
            .copied()
            .filter(|&x| x == ABOVE || (is_int(x) && x > h + d))
            .collect();
    }
    if v == BELOW {
        if d <= 0 {
            return vec![BELOW];
        }
        let l = dom.low.unwrap_or(0);
        return dom
            .values
            .iter()
            .copied()
            .filter(|&x| x == BELOW || (is_int(x) && x < l + d))
            .collect();
    }
    if v == NONE {
        return vec![];
    }
    let lab = dom.label(v + d);
    if lab == NONE {
        vec![]
    } else {
        vec![lab]
    }
}

/// A view's value: a task value (NONE: Python's None) or the lists an `args:` fact can be (sorted).
#[derive(Clone, PartialEq, Eq, Debug, Hash)]
pub enum VV {
    V(Val),
    Set(Arc<[Val]>),
}

pub static VNONE: VV = VV::V(NONE);

impl VV {
    #[inline]
    pub fn val(&self) -> Option<Val> {
        match self {
            VV::V(v) if *v != NONE => Some(*v),
            _ => None,
        }
    }
    #[inline]
    pub fn is_none(&self) -> bool {
        matches!(self, VV::V(NONE))
    }
    /// `x in values` for a set of task values (a frozenset value is never in it)
    #[inline]
    pub fn in_vals(&self, vals: &[Val]) -> bool {
        match self {
            VV::V(v) if *v != NONE => vals.contains(v),
            _ => false,
        }
    }
    pub fn py(&self) -> Py {
        match self {
            VV::V(v) => vpy(*v),
            VV::Set(s) => Py::Tuple(s.iter().map(|&x| vpy(x)).collect()), // (frozenset: shown as sorted list by _show)
        }
    }
    pub fn j(&self) -> J {
        match self {
            VV::V(v) => vj(*v),
            VV::Set(s) => J::Arr(s.iter().map(|&x| vj(x)).collect()),
        }
    }
    pub fn str(&self) -> String {
        match self {
            VV::V(v) => vstr(*v),
            VV::Set(s) => format!(
                "frozenset({{{}}})",
                s.iter().map(|&x| vstr(x)).collect::<Vec<_>>().join(", ")
            ),
        }
    }
}

/// A view or a belief: key -> value. A key absent and a key whose value is None both `get` as None, as in Python.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct View {
    pub m: FxHashMap<Key, VV>,
}

impl View {
    pub fn new() -> View {
        View {
            m: FxHashMap::default(),
        }
    }
    pub fn with_capacity(n: usize) -> View {
        let mut m = FxHashMap::default();
        m.reserve(n);
        View { m }
    }
    #[inline]
    pub fn get(&self, k: Key) -> &VV {
        self.m.get(&k).unwrap_or(&VNONE)
    }
    /// The int value, None for None or a set.
    #[inline]
    pub fn val(&self, k: Key) -> Val {
        match self.m.get(&k) {
            Some(VV::V(v)) => *v,
            _ => NONE,
        }
    }
    #[inline]
    pub fn contains(&self, k: Key) -> bool {
        self.m.contains_key(&k)
    }
    #[inline]
    pub fn set(&mut self, k: Key, v: VV) {
        self.m.insert(k, v);
    }
    #[inline]
    pub fn setv(&mut self, k: Key, v: Val) {
        self.m.insert(k, VV::V(v));
    }
    pub fn len(&self) -> usize {
        self.m.len()
    }
    pub fn is_empty(&self) -> bool {
        self.m.is_empty()
    }
    /// `dict(self, **other)`
    pub fn merged(&self, other: &View) -> View {
        let mut out = self.clone();
        for (k, v) in &other.m {
            out.m.insert(*k, v.clone());
        }
        out
    }
    pub fn keys_sorted(&self) -> Vec<Key> {
        let mut ks: Vec<(String, Key)> = self.m.keys().map(|k| (k.text(), *k)).collect();
        ks.sort();
        ks.into_iter().map(|x| x.1).collect()
    }
    pub fn to_j(&self) -> J {
        J::Arr(
            self.keys_sorted()
                .into_iter()
                .map(|k| J::Arr(vec![J::Str(k.text()), self.m[&k].j()]))
                .collect(),
        )
    }
}

/// Key prefix tests of the reference (``k.startswith(prefixes)``) on packed keys: a set of key kinds, and for an
/// interned key its text.
#[derive(Clone, Debug, Default)]
pub struct Prefixes {
    pub kinds: u64, // bit per key kind
    pub texts: Vec<&'static str>,
}

impl Prefixes {
    pub fn of(texts: &[&'static str]) -> Prefixes {
        let mut kinds = 0u64;
        for t in texts {
            let k = match *t {
                "var:" => fmi_core::K_VAR,
                "bit:" => fmi_core::K_BIT,
                "owner:" => fmi_core::K_OWNER,
                "state:" => fmi_core::K_STATE,
                "class:" => fmi_core::K_CLASS,
                "script:" => fmi_core::K_SCRIPT,
                "local:" => fmi_core::K_LOCAL,
                "objscript:" => fmi_core::K_OBJSCRIPT,
                "at:" => fmi_core::K_AT,
                "frozen:" => fmi_core::K_FROZEN,
                "esc:" => fmi_core::K_ESC,
                "verb:" => fmi_core::K_VERB,
                "pos:" => fmi_core::K_POS,
                "escpos:" => fmi_core::K_ESCPOS,
                "args:" => fmi_core::K_ARGS,
                "saved:" => fmi_core::K_SAVED,
                "actorroom:" => fmi_core::K_ACTORROOM,
                "objpos:" => fmi_core::K_OBJPOS,
                "cutscene" => fmi_core::K_CUTSCENE,
                "room" => fmi_core::K_ROOM,
                _ => panic!("prefix {t} has no key kind"),
            };
            kinds |= 1 << k;
        }
        Prefixes {
            kinds,
            texts: texts.to_vec(),
        }
    }
    #[inline]
    pub fn has(&self, k: Key) -> bool {
        let kind = k.kind();
        if kind == fmi_core::K_OTHER {
            let t = k.text();
            return self.texts.iter().any(|p| t.starts_with(p));
        }
        kind < 64 && self.kinds & (1 << kind) != 0
    }
}

/// facts.py's slot prefixes (replay.SLOT_PREFIXES) etc. are built where used.
pub fn key_unit(k: Key) -> (u8, u64) {
    (k.unit(), k.a())
}

/// `pos:<unit>` for a unit (u, a).
#[inline]
pub fn pos_key(u: u8, a: u64) -> Key {
    Key::pack(fmi_core::K_POS, u, a, 0)
}

/// The text of a unit (u, a): script:N, local:N, obj:O, entry, exit.
pub fn unit_text(u: u8, a: u64) -> String {
    match u {
        fmi_core::U_SCRIPT => format!("script:{a}"),
        fmi_core::U_LOCAL => format!("local:{a}"),
        fmi_core::U_OBJ => format!("obj:{a}"),
        fmi_core::U_ENTRY => "entry".into(),
        fmi_core::U_EXIT => "exit".into(),
        _ => "?".into(),
    }
}

/// A unit name ("script:2", "entry") as (u, a).
pub fn unit_of_text(t: &str) -> Option<(u8, u64)> {
    match t {
        "entry" => Some((fmi_core::U_ENTRY, 0)),
        "exit" => Some((fmi_core::U_EXIT, 0)),
        _ => {
            let (k, n) = t.split_once(':')?;
            let n: u64 = n.parse().ok()?;
            Some((
                match k {
                    "script" => fmi_core::U_SCRIPT,
                    "local" => fmi_core::U_LOCAL,
                    "obj" => fmi_core::U_OBJ,
                    _ => return None,
                },
                n,
            ))
        }
    }
}

/// A unit (kind, number) as one value: facts.unit_of's names.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub struct Unit(pub u8, pub u64);

impl Unit {
    pub fn text(self) -> String {
        unit_text(self.0, self.1)
    }
    pub fn parse(t: &str) -> Option<Unit> {
        unit_of_text(t).map(|(u, a)| Unit(u, a))
    }
    pub fn pos(self) -> Key {
        pos_key(self.0, self.1)
    }
    pub fn args(self) -> Key {
        Key::pack(fmi_core::K_ARGS, self.0, self.1, 0)
    }
    /// facts.unit_of(where, number)
    pub fn of(where_: i32, number: i32) -> Option<Unit> {
        fmi_core::unit_of(where_, number).map(|(u, a)| Unit(u, a))
    }
    /// the unit of a key of a unit kind (pos:, escpos:, args:, at:, esc:, frozen:)
    pub fn of_key(k: Key) -> Unit {
        Unit(k.unit(), k.a())
    }
}

/// Sort keys by their text (Python's sorted on str keys).
pub fn sort_keys(ks: &mut [Key]) {
    ks.sort_by_cached_key(|k| k.text());
}

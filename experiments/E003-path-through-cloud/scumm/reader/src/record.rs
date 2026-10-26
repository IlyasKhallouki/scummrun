//! The reader's records and the values they hold: reader.py `Record`, `Event`, the effects, and the machine values
//! the Builder sees (`Val`: an int or model.py's `Unknown`, with reader.py's `Cond` and `Copy`). Each type converts
//! to and from golden.py's canonical encoding ([`Py`]), so a record read from a golden file writes back to the same
//! line.
//!
//! [`PathOut`] is what the Builder reads from one path of a machine run (reader.py's `SymPath` fields that
//! `Builder.record` reads): the interface the machine-dependent half fills (PORT-NOTES.md).

use crate::domain::{labels_from_py, labels_py, Label, Labels};
use crate::facts::{unit_from_py, unit_py};
use crate::py::{repr_str, repr_tuple, Py};
use scumm_ir::UnitKey;
use std::collections::BTreeMap;

/// reader.py's subclasses of `Unknown`: a plain one, `Cond` (key := the value where binary fact `what` is 1) and
/// `Copy` (key := the value of binary fact `what`).
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum UClass {
    Unknown,
    Cond,
    Copy,
}

/// model.py `Unknown(reason, what, values)` (and its subclasses).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Unknown {
    pub class: UClass,
    pub reason: String,
    pub what: String,
    /// sorted, distinct
    pub values: Option<Vec<i64>>,
}

pub const R_FACT: &str = "fact";
pub const R_NONFACT: &str = "not a fact";
pub const R_COLLAPSED: &str = "collapsed value";
pub const R_STEP: &str = "step";
pub const R_COND: &str = "conditional write";
pub const R_COPY: &str = "copy";
pub const R_LOCAL: &str = "local";
pub const R_ACTOR: &str = "actor state";
pub const R_HELD: &str = "not held";
pub const STEP_BIG: i64 = 1 << 20;

impl Unknown {
    pub fn new(reason: &str, what: impl Into<String>, values: Option<Vec<i64>>) -> Unknown {
        let values = values.map(|mut v| {
            v.sort_unstable();
            v.dedup();
            v
        });
        Unknown { class: UClass::Unknown, reason: reason.into(), what: what.into(), values }
    }
    /// model.py `Unknown.__str__`: `?(reason: what in [values])`.
    pub fn show(&self) -> String {
        let mut s = format!("?({}", self.reason);
        if !self.what.is_empty() {
            s.push_str(": ");
            s.push_str(&self.what);
        }
        if let Some(v) = &self.values {
            let parts: Vec<String> = v.iter().map(|x| x.to_string()).collect();
            s.push_str(&format!(" in [{}]", parts.join(", ")));
        }
        s.push(')');
        s
    }
    pub fn class_name(&self) -> &'static str {
        match self.class {
            UClass::Unknown => "Unknown",
            UClass::Cond => "Cond",
            UClass::Copy => "Copy",
        }
    }
}

/// A machine value: an int or an unknown.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum Val {
    Int(i64),
    Unk(Unknown),
}

impl Val {
    pub fn int(&self) -> Option<i64> {
        match self {
            Val::Int(n) => Some(*n),
            _ => None,
        }
    }
    pub fn unk(&self) -> Option<&Unknown> {
        match self {
            Val::Unk(u) => Some(u),
            _ => None,
        }
    }
    /// Python's `str(v)`.
    pub fn show(&self) -> String {
        match self {
            Val::Int(n) => n.to_string(),
            Val::Unk(u) => u.show(),
        }
    }
    /// reader.py `is_step`.
    pub fn is_step(&self) -> bool {
        matches!(self, Val::Unk(u) if u.reason == R_STEP)
    }
    /// reader.py `step_of`: the least of the step's values.
    pub fn step_of(&self) -> i64 {
        self.unk().and_then(|u| u.values.as_ref()).map(|v| v[0]).expect("a step")
    }
    pub fn py(&self) -> Py {
        match self {
            Val::Int(n) => Py::Int(*n),
            Val::Unk(u) => Py::obj(
                u.class_name(),
                vec![
                    ("reason", Py::str(&u.reason)),
                    ("what", Py::str(&u.what)),
                    ("values", u.values.as_ref().map_or(Py::None, |v| Py::Set(v.iter().map(|&x| Py::Int(x)).collect()))),
                ],
            ),
        }
    }
    pub fn from_py(p: &Py) -> Val {
        match p {
            Py::Int(n) => Val::Int(*n),
            Py::Obj(name, _) => {
                let class = match name.as_str() {
                    "Unknown" => UClass::Unknown,
                    "Cond" => UClass::Cond,
                    "Copy" => UClass::Copy,
                    other => panic!("not a machine value: {other}"),
                };
                let v = p.get("values");
                let values = if v.is_none() {
                    None
                } else {
                    let mut xs: Vec<i64> = v.items().iter().map(|x| x.int()).collect();
                    xs.sort_unstable();
                    Some(xs)
                };
                Val::Unk(Unknown {
                    class,
                    reason: p.get("reason").as_str().to_string(),
                    what: p.get("what").as_str().to_string(),
                    values,
                })
            }
            _ => panic!("not a machine value: {}", p.text()),
        }
    }
}

/// A precondition: fact -> the domain values it may have (reader.py's dicts of frozensets). Sorted by key; no
/// output rests on the order Python's dict had (PORT-NOTES.md).
pub type Pre = BTreeMap<String, Labels>;
/// The values a run fixes (the oracle's pins).
pub type Pins = BTreeMap<String, Val>;

pub fn pre_py(p: &Pre) -> Py {
    Py::Dict(p.iter().map(|(k, v)| (k.clone(), labels_py(v))).collect())
}
pub fn pre_from_py(p: &Py) -> Pre {
    p.dict_items().iter().map(|(k, v)| (k.clone(), labels_from_py(v))).collect()
}
pub fn pins_py(p: &Pins) -> Py {
    Py::Dict(p.iter().map(|(k, v)| (k.clone(), v.py())).collect())
}
pub fn pins_from_py(p: &Py) -> Pins {
    p.dict_items().iter().map(|(k, v)| (k.clone(), Val::from_py(v))).collect()
}

/// An effect of a record (reader.py's tuples): `("set", v)`, `("guess", values)`, `("add", d)`, `("when", fact, v)`
/// (key := v where binary fact `fact` is 1) or `("copy", fact)`. Fix round 1 adds `("cases", [(conditions,
/// values), ...])` (`Cases`): key := the case's value (one of its values: a guess) where its conditions hold before
/// the action ({fact: values}, all of them); the cases' conditions are disjoint and cover every state the record's
/// precondition allows. Only switch due_first makes it (due.rs), so no reading with the switches off holds one.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum Eff {
    Set(Label),
    Guess(Labels),
    Add(i64),
    When(String, i64),
    Copy(String),
    Cases(Vec<Case>),
}

/// One case of an [`Eff::Cases`]: (conditions in key order, the values written: one, or several for a guess).
pub type Case = (Vec<(String, Labels)>, Labels);

fn case_py(c: &Case) -> Py {
    Py::Tuple(vec![
        Py::Tuple(c.0.iter().map(|(k, v)| Py::Tuple(vec![Py::str(k), labels_py(v)])).collect()),
        labels_py(&c.1),
    ])
}

fn case_from_py(p: &Py) -> Case {
    let t = p.items();
    let conds = t[0]
        .items()
        .iter()
        .map(|kv| {
            let kv = kv.items();
            (kv[0].as_str().to_string(), labels_from_py(&kv[1]))
        })
        .collect();
    (conds, labels_from_py(&t[1]))
}

/// A set's text in a fold key: the values' texts, sorted.
fn labels_key(ls: &[Label]) -> String {
    let mut v: Vec<String> = ls.iter().map(|l| l.repr()).collect();
    v.sort();
    format!("{{{}}}", v.join(", "))
}

impl Eff {
    pub fn py(&self) -> Py {
        match self {
            Eff::Set(l) => Py::Tuple(vec![Py::str("set"), l.py()]),
            Eff::Guess(ls) => Py::Tuple(vec![Py::str("guess"), labels_py(ls)]),
            Eff::Add(d) => Py::Tuple(vec![Py::str("add"), Py::Int(*d)]),
            Eff::When(k, v) => Py::Tuple(vec![Py::str("when"), Py::str(k), Py::Int(*v)]),
            Eff::Copy(k) => Py::Tuple(vec![Py::str("copy"), Py::str(k)]),
            Eff::Cases(cs) => Py::Tuple(vec![Py::str("cases"), Py::List(cs.iter().map(case_py).collect())]),
        }
    }
    pub fn from_py(p: &Py) -> Eff {
        let t = p.items();
        match t[0].as_str() {
            "set" => Eff::Set(Label::from_py(&t[1])),
            "guess" => Eff::Guess(labels_from_py(&t[1])),
            "add" => Eff::Add(t[1].int()),
            "when" => Eff::When(t[1].as_str().to_string(), t[2].int()),
            "copy" => Eff::Copy(t[1].as_str().to_string()),
            "cases" => Eff::Cases(t[1].items().iter().map(case_from_py).collect()),
            k => panic!("not an effect kind: {k}"),
        }
    }
    /// Python's `str(e)` up to set order: a canonical text for build_task's fold key.
    pub fn key_text(&self) -> String {
        match self {
            Eff::Set(l) => format!("('set', {})", l.repr()),
            Eff::Guess(ls) => {
                let parts: Vec<String> = ls.iter().map(|l| l.repr()).collect();
                format!("('guess', frozenset({{{}}}))", parts.join(", "))
            }
            Eff::Add(d) => format!("('add', {d})"),
            Eff::When(k, v) => format!("('when', {}, {v})", repr_str(k)),
            Eff::Copy(k) => format!("('copy', {})", repr_str(k)),
            Eff::Cases(cs) => {
                let parts: Vec<String> = cs
                    .iter()
                    .map(|(conds, vals)| {
                        let c: Vec<String> = conds.iter().map(|(k, v)| format!("{}={}", repr_str(k), labels_key(v))).collect();
                        format!("[{}]->{}", c.join(" "), labels_key(vals))
                    })
                    .collect();
                format!("('cases', {})", parts.join("; "))
            }
        }
    }
    /// The facts the effect reads besides its own key (a `when` or `copy` source, the cases' conditions).
    pub fn sources(&self) -> Vec<&str> {
        match self {
            Eff::When(k, _) | Eff::Copy(k) => vec![k.as_str()],
            Eff::Cases(cs) => {
                let mut v: Vec<&str> = cs.iter().flat_map(|c| c.0.iter().map(|w| w.0.as_str())).collect();
                v.sort_unstable();
                v.dedup();
                v
            }
            _ => vec![],
        }
    }
}

/// A condition the path assumed (model.py's assumption tuples): (cite, condition, side, why). `side` is kept as
/// the Python value it was (a bool in every record so far).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Assumption {
    pub cite: String,
    pub cond: String,
    pub side: Py,
    pub why: String,
}

impl Assumption {
    pub fn new(cite: impl Into<String>, cond: impl Into<String>, side: bool, why: impl Into<String>) -> Assumption {
        Assumption { cite: cite.into(), cond: cond.into(), side: Py::Bool(side), why: why.into() }
    }
    pub fn py(&self) -> Py {
        Py::Tuple(vec![Py::str(&self.cite), Py::str(&self.cond), self.side.clone(), Py::str(&self.why)])
    }
    pub fn from_py(p: &Py) -> Assumption {
        let t = p.items();
        assert_eq!(t.len(), 4, "an assumption is a 4-tuple: {}", p.text());
        Assumption { cite: t[0].as_str().into(), cond: t[1].as_str().into(), side: t[2].clone(), why: t[3].as_str().into() }
    }
    /// Python's `str(a[2])`.
    pub fn side_str(&self) -> String {
        match &self.side {
            Py::Bool(true) => "True".into(),
            Py::Bool(false) => "False".into(),
            Py::Int(n) => n.to_string(),
            Py::Str(s) => s.clone(),
            Py::None => "None".into(),
            other => panic!("an assumption side reader.py never makes: {}", other.text()),
        }
    }
}

/// reader.py `Event`: a slot's later part.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Event {
    pub name: String,
    pub unit: UnitKey,
    pub w: i64,
    pub sig: i64,
    /// Local[n] -> value, sorted by n
    pub locals: Vec<(i64, Val)>,
    pub args_known: bool,
    pub recursive: Py,
    pub resistant: Py,
    pub owned: Py,
    pub abort: bool,
    pub override_: i64,
    pub cut_data: Vec<i64>,
}

impl Event {
    /// `Event.label()`.
    pub fn label(&self) -> String {
        let what = if self.abort { "esc" } else { "event" };
        let sig = if self.sig != 0 { format!("#{:x}", self.sig) } else { String::new() };
        format!("{what}({}@{}{sig})", self.name, self.w)
    }
    pub fn py(&self) -> Py {
        Py::obj(
            "Event",
            vec![
                ("name", Py::str(&self.name)),
                ("unit", unit_py(self.unit)),
                ("w", Py::Int(self.w)),
                ("sig", Py::Int(self.sig)),
                ("locals", Py::Map(self.locals.iter().map(|(k, v)| (Py::Int(*k), v.py())).collect())),
                ("args_known", Py::Bool(self.args_known)),
                ("recursive", self.recursive.clone()),
                ("resistant", self.resistant.clone()),
                ("owned", self.owned.clone()),
                ("abort", Py::Bool(self.abort)),
                ("override", Py::Int(self.override_)),
                ("cut_data", Py::Tuple(self.cut_data.iter().map(|&c| Py::Int(c)).collect())),
            ],
        )
    }
    pub fn from_py(p: &Py) -> Event {
        let p = p.class("Event");
        let mut locals: Vec<(i64, Val)> = p.get("locals").map_items().into_iter().map(|(k, v)| (k.int(), Val::from_py(v))).collect();
        locals.sort_by_key(|x| x.0);
        Event {
            name: p.get("name").as_str().into(),
            unit: unit_from_py(p.get("unit")),
            w: p.get("w").int(),
            sig: p.get("sig").int(),
            locals,
            args_known: p.get("args_known").bool(),
            recursive: p.get("recursive").clone(),
            resistant: p.get("resistant").clone(),
            owned: p.get("owned").clone(),
            abort: p.get("abort").bool(),
            override_: p.get("override").int(),
            cut_data: p.get("cut_data").items().iter().map(|c| c.int()).collect(),
        }
    }
    /// `dataclasses.replace(ev, sig=0, locals={}, args_known=False, override=0, cut_data=())`, with `recursive`
    /// None too when `forget_recursive` (the replans' copy in `_read_once`; `_limit` keeps it).
    pub fn unknown_locals(&self, forget_recursive: bool) -> Event {
        Event {
            sig: 0,
            locals: Vec::new(),
            args_known: false,
            override_: 0,
            cut_data: Vec::new(),
            recursive: if forget_recursive { Py::None } else { self.recursive.clone() },
            ..self.clone()
        }
    }
}

/// What a record's run was (`Record.source`, the job's `what`).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum What {
    Sentence { verb: i64, a: i64, b: Option<i64> },
    Verb(i64),
    Event(Event),
}

impl What {
    pub fn py(&self) -> Py {
        match self {
            What::Sentence { verb, a, b } => {
                Py::Tuple(vec![Py::str("sentence"), Py::Int(*verb), Py::Int(*a), Py::opt_int(*b)])
            }
            What::Verb(v) => Py::Tuple(vec![Py::str("verb"), Py::Int(*v)]),
            What::Event(e) => Py::Tuple(vec![Py::str("event"), e.py()]),
        }
    }
    pub fn from_py(p: &Py) -> What {
        let t = p.items();
        match t[0].as_str() {
            "sentence" => What::Sentence { verb: t[1].int(), a: t[2].int(), b: (!t[3].is_none()).then(|| t[3].int()) },
            "verb" => What::Verb(t[1].int()),
            "event" => What::Event(Event::from_py(&t[1])),
            k => panic!("not a run kind: {k}"),
        }
    }
}

/// reader.py `Record`: one path of one input or event.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Record {
    pub kind: String,
    pub label: String,
    pub pre: Pre,
    /// in Python's dict order, which the task's effect order follows (build_task, write_sas)
    pub effects: Vec<(String, Eff)>,
    pub checks: Vec<Assumption>,
    /// the path's end (None: the machine left it without one; the real machine never does, model.py `while p.end is
    /// None`)
    pub end: Option<String>,
    pub notes: Vec<Py>,
    pub next_events: Vec<Event>,
    pub source: What,
    pub either: Vec<(String, String)>,
    pub engine_pre: Vec<String>,
    pub unknown: Vec<Py>,
}

/// A path's end as golden.py writes it: a text, or null.
pub fn end_py(end: &Option<String>) -> Py {
    end.as_deref().map_or(Py::None, Py::str)
}

fn end_from_py(p: &Py) -> Option<String> {
    (!p.is_none()).then(|| p.as_str().to_string())
}

impl Record {
    /// The end as text (`str(r.end)`: "None" for none), for the packed file's `end` string.
    pub fn end_text(&self) -> &str {
        self.end.as_deref().unwrap_or("None")
    }

    pub fn effect(&self, k: &str) -> Option<&Eff> {
        self.effects.iter().find(|e| e.0 == k).map(|e| &e.1)
    }

    pub fn py(&self) -> Py {
        Py::obj(
            "Record",
            vec![
                ("kind", Py::str(&self.kind)),
                ("label", Py::str(&self.label)),
                ("pre", pre_py(&self.pre)),
                ("effects", Py::Dict(self.effects.iter().map(|(k, e)| (k.clone(), e.py())).collect())),
                ("checks", Py::List(self.checks.iter().map(|c| c.py()).collect())),
                ("end", end_py(&self.end)),
                ("notes", Py::List(self.notes.clone())),
                ("next_events", Py::List(self.next_events.iter().map(|e| e.py()).collect())),
                ("source", self.source.py()),
                (
                    "either",
                    Py::List(self.either.iter().map(|(a, b)| Py::Tuple(vec![Py::str(a), Py::str(b)])).collect()),
                ),
                ("engine_pre", Py::Set(self.engine_pre.iter().map(Py::str).collect())),
                ("unknown", Py::List(self.unknown.clone())),
            ],
        )
    }

    /// A record from a golden file. The effects' order is not in the file (golden.py sorts dict keys); it is
    /// rebuilt by `Builder.record`'s rule: the path's writes by key, then `args:`, `pos:` and `escpos:` keys, each
    /// sorted (`effect_order`).
    pub fn from_py(p: &Py) -> Record {
        let p = p.class("Record");
        let mut effects: Vec<(String, Eff)> =
            p.get("effects").dict_items().iter().map(|(k, v)| (k.clone(), Eff::from_py(v))).collect();
        effects.sort_by(|a, b| effect_order(&a.0).cmp(&effect_order(&b.0)));
        Record {
            kind: p.get("kind").as_str().into(),
            label: p.get("label").as_str().into(),
            pre: pre_from_py(p.get("pre")),
            effects,
            checks: p.get("checks").items().iter().map(Assumption::from_py).collect(),
            end: end_from_py(p.get("end")),
            notes: p.get("notes").items().to_vec(),
            next_events: p.get("next_events").items().iter().map(Event::from_py).collect(),
            source: What::from_py(p.get("source")),
            either: p
                .get("either")
                .items()
                .iter()
                .map(|t| {
                    let t = t.items();
                    (t[0].as_str().into(), t[1].as_str().into())
                })
                .collect(),
            engine_pre: {
                let mut v: Vec<String> = p.get("engine_pre").items().iter().map(|s| s.as_str().to_string()).collect();
                v.sort();
                v
            },
            unknown: p.get("unknown").items().to_vec(),
        }
    }
}

/// The place of an effect key in a record's effects (`Builder.record`'s order): (group, key).
pub fn effect_order(k: &str) -> (u8, &str) {
    let g = if k.starts_with("args:") {
        1
    } else if k.starts_with("pos:") {
        2
    } else if k.starts_with("escpos:") {
        3
    } else if k.starts_with("due:") {
        4 // fix round 1's due facts, added after the reading (due.rs); none with the switches off
    } else {
        0
    };
    (g, k)
}

/// A slot a path left waiting (reader.py `SymPath.kept`'s tuples): (unit, W, locals, args known, R, F, owned,
/// cutscenes it began and has not ended, their cutscene data).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Kept {
    pub unit: UnitKey,
    pub w: i64,
    pub locals: Vec<(i64, Val)>,
    pub known: bool,
    pub recursive: Py,
    pub resistant: Py,
    pub owned: Py,
    pub override_: i64,
    pub cut_data: Vec<Val>,
}

impl Kept {
    pub fn py(&self) -> Py {
        Py::Tuple(vec![
            unit_py(self.unit),
            Py::Int(self.w),
            Py::Map(self.locals.iter().map(|(k, v)| (Py::Int(*k), v.py())).collect()),
            Py::Bool(self.known),
            self.recursive.clone(),
            self.resistant.clone(),
            self.owned.clone(),
            Py::Int(self.override_),
            Py::Tuple(self.cut_data.iter().map(|v| v.py()).collect()),
        ])
    }
    pub fn from_py(p: &Py) -> Kept {
        let t = p.items();
        let mut locals: Vec<(i64, Val)> = t[2].map_items().into_iter().map(|(k, v)| (k.int(), Val::from_py(v))).collect();
        locals.sort_by_key(|x| x.0);
        Kept {
            unit: unit_from_py(&t[0]),
            w: t[1].int(),
            locals,
            known: t[3].bool(),
            recursive: t[4].clone(),
            resistant: t[5].clone(),
            owned: t[6].clone(),
            override_: t[7].int(),
            cut_data: t[8].items().iter().map(Val::from_py).collect(),
        }
    }
}

/// One path of a machine run, as `Builder.run` and `Builder.record` read it (reader.py `SymPath`): its end, whether
/// it is a SymPath, and for a path that can be a record its assumed values (`pre`), its last write to each fact
/// (`writes`), the slots it left waiting (`kept`), the writes whose target did not fold (`unknown_writes`: model.py's
/// (statement, cite, target) tuples), the conditions on non-facts it took (`assumptions`) and its last five notes.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct PathOut {
    pub end: Option<String>,
    pub sym: bool,
    pub pre: Pre,
    pub writes: BTreeMap<String, Val>,
    pub kept: BTreeMap<String, Kept>,
    pub unknown_writes: Vec<Py>,
    pub assumptions: Vec<Assumption>,
    pub notes: Vec<Py>,
}

impl PathOut {
    pub fn from_py(p: &Py) -> PathOut {
        let end = end_from_py(p.get("end"));
        let sym = p.get("sym").bool();
        if !p.has("writes") {
            return PathOut { end, sym, ..Default::default() };
        }
        PathOut {
            end,
            sym,
            pre: pre_from_py(p.get("pre")),
            writes: p.get("writes").dict_items().iter().map(|(k, v)| (k.clone(), Val::from_py(v))).collect(),
            kept: p.get("kept").dict_items().iter().map(|(k, v)| (k.clone(), Kept::from_py(v))).collect(),
            unknown_writes: p.get("unknown_writes").items().to_vec(),
            assumptions: p.get("assumptions").items().iter().map(Assumption::from_py).collect(),
            notes: p.get("notes").items().to_vec(),
        }
    }
}

/// `str(v)` of the items of a Kept slot's cutscene data, as `Builder.record` turns them into text.
pub fn cut_text(v: &Val) -> CutData {
    match v {
        Val::Int(n) => CutData::Int(*n),
        Val::Unk(u) => CutData::Str(u.show()),
    }
}

/// An item of the cutscene data as `Builder.record` keeps it: an int, or an unknown's text.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum CutData {
    Int(i64),
    Str(String),
}

impl CutData {
    pub fn repr(&self) -> String {
        match self {
            CutData::Int(n) => n.to_string(),
            CutData::Str(s) => repr_str(s),
        }
    }
}

/// Python's `str()` of a tuple of cutscene data.
pub fn cut_tuple_str(cd: &[CutData]) -> String {
    repr_tuple(&cd.iter().map(|c| c.repr()).collect::<Vec<_>>())
}

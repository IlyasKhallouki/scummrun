//! reader.py `FactTable`: how the reader reads each fact, its class (`kind`), its domain (domains.py's, plus the
//! reader's own), and how a domain value is a machine value.

use crate::domain::{Domain, Label, Labels};
use crate::domains::{self, Domains, SETTINGS};
use crate::facts::Facts;
use crate::py::{Counter, Py};
use crate::record::{Unknown, Val, R_COLLAPSED, R_FACT};
use crate::scope::{Scope, VAR_CURSORSTATE, VAR_USERPUT};
use scumm_ir::{known, Expr, Game, Stmt};
use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};

/// pos: and escpos: of a unit with no live slot, or no override armed.
pub const NOWHERE: i64 = -1;
/// pos: and escpos: of a unit whose slot waits (or whose override is armed) at a place that is none of the scope's
/// points for its name ([`FactTable::elsewhere`], the switch; PORT-NOTES.md, "ELSEWHERE"). No precondition accepts it.
pub const ELSEWHERE: i64 = -2;
/// model.py TIME_VARS and MOUSE_VARS: engine variables that change as frames pass.
pub const TIME_VARS: [i32; 4] = [2, 3, 23, 25];
pub const MOUSE_VARS: [i32; 4] = [20, 21, 44, 45];

/// A fact's class: a constant (the start's value, no precondition), a fact (forked on), or a non-fact.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Kind {
    Const,
    Fact,
    NonFact,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Const => "const",
            Kind::Fact => "fact",
            Kind::NonFact => "nonfact",
        }
    }
}

#[derive(Clone)]
pub struct FactTable {
    /// the scope's start with the settings' variables (FactTable.start)
    pub start: Facts,
    /// the reader's own domains (hook variables, cursor variables, room, the room variables, and the values
    /// `Builder.extend_domains` added)
    pub extra: BTreeMap<String, Domain>,
    pub counts: Counter,
    /// args:<unit> -> its list numbers
    pub args_vals: BTreeMap<String, BTreeSet<i64>>,
    pub doms: Domains,
    /// {"at"/"esc": {unit name: offsets}}
    pub points: BTreeMap<&'static str, BTreeMap<String, BTreeSet<i64>>>,
    pub engine: BTreeSet<i32>,
    /// The switch for the scope-1 finding (PORT-NOTES.md, "ELSEWHERE"), off by default: every `pos:` and `escpos:`
    /// domain holds ELSEWHERE, a place written outside the scope's points is written as ELSEWHERE, no precondition
    /// set holds it, and a start with a slot at such a place starts it there. Off, the reader is reader.py's.
    pub elsewhere: bool,
    binary: Domain,
}

impl FactTable {
    pub fn new(scope: &Scope, doms: Domains, game: &Game) -> FactTable {
        let ast = &game.ast;
        let mut start = scope.start.clone();
        for (n, v) in SETTINGS {
            start.insert(format!("var:{n}"), v);
        }
        let mut extra = BTreeMap::new();
        let mut counts = Counter::default();
        for n in domains::HOOKS {
            let k = format!("var:{n}");
            if doms.doms.get(&k).is_none() {
                let mut vals: BTreeSet<i64> = [start.get0(&k)].into_iter().collect();
                for &u in &scope.units {
                    for x in game.ins(u) {
                        if let Some(Stmt::Assign(a)) = x.st {
                            if ast.expr(a.lhs) == Expr::Var(n) {
                                if let Some(c) = ast.constant(a.rhs) {
                                    vals.insert(c as i64);
                                }
                            }
                        }
                    }
                }
                extra.insert(k, Domain::of(vals, None, None));
                counts.inc("hook variables given the domain start + constant writes (reader)");
            }
        }
        for n in [VAR_CURSORSTATE, VAR_USERPUT] {
            let k = format!("var:{n}");
            let seeds = [start.get0(&k), 0, 1];
            let lo = seeds.iter().min().unwrap() - 1;
            let hi = seeds.iter().max().unwrap() + 1;
            extra.insert(k, Domain::of(lo - 1..hi + 2, Some(lo), Some(hi)));
            counts.inc("cursor variables given a window of +-1 around start, 0 and 1 (reader)");
        }
        let mut rooms: BTreeSet<i64> = scope.rooms.iter().map(|&r| r as i64).collect();
        rooms.insert(start.get0("room"));
        let mut wild = 0;
        for &u in &scope.units {
            for x in game.ins(u) {
                if let Some(Stmt::Op { name, args, .. }) = x.st {
                    if name == known::LOAD_ROOM || name == known::LOAD_ROOM_WITH_EGO {
                        let a = ast.kids(args)[if name == known::LOAD_ROOM { 0 } else { 1 }];
                        match ast.constant(a) {
                            Some(c) => {
                                rooms.insert(c as i64);
                            }
                            None => wild += 1,
                        }
                    }
                }
            }
        }
        counts.set("room changes in the scope that do not fold (a guess among the scope's rooms)", wild);
        extra.insert("room".into(), Domain::of(rooms.iter().copied(), None, None));
        for n in [domains::VAR_ROOM, domains::VAR_ROOM_RESOURCE, domains::VAR_NEW_ROOM] {
            let mut v = rooms.clone();
            v.insert(start.get0(&format!("var:{n}")));
            extra.insert(format!("var:{n}"), Domain::of(v, None, None));
        }
        let mut engine: BTreeSet<i32> = domains::ENGINE.iter().map(|e| e.0).collect();
        engine.extend(TIME_VARS);
        engine.extend(MOUSE_VARS);
        let mut points: BTreeMap<&'static str, BTreeMap<String, BTreeSet<i64>>> = BTreeMap::new();
        points.insert("at", BTreeMap::new());
        points.insert("esc", BTreeMap::new());
        let from_start = start.iter().map(|(k, _)| k.clone()).filter(|k| k.starts_with("at:") || k.starts_with("esc:"));
        for k in scope.universe.iter().cloned().chain(from_start) {
            let kind = if k.starts_with("at:") {
                "at"
            } else if k.starts_with("esc:") {
                "esc"
            } else {
                continue;
            };
            let rest = &k[kind.len() + 1..];
            let (name, w) = rest.rsplit_once(':').unwrap();
            points.get_mut(kind).unwrap().entry(name.to_string()).or_default().insert(w.parse().unwrap());
        }
        FactTable {
            start,
            extra,
            counts,
            args_vals: BTreeMap::new(),
            doms,
            points,
            engine,
            elsewhere: false,
            binary: Domain::binary(),
        }
    }

    /// `FactTable.domain(key)`.
    pub fn domain(&self, key: &str) -> Option<Cow<'_, Domain>> {
        if key.starts_with("args:") {
            let mut v: BTreeSet<i64> = self.args_vals.get(key).cloned().unwrap_or_default();
            v.insert(0);
            return Some(Cow::Owned(Domain::of(v, None, None)));
        }
        if key.starts_with("saved:") {
            return Some(Cow::Borrowed(&self.binary));
        }
        if let Some((kind, name)) = pos_key(key) {
            let mut v: BTreeSet<i64> = self.points[kind].get(name).cloned().unwrap_or_default();
            v.insert(NOWHERE);
            if self.elsewhere {
                v.insert(ELSEWHERE);
            }
            return Some(Cow::Owned(Domain::of(v, None, None)));
        }
        self.extra.get(key).or_else(|| self.doms.get(key)).map(Cow::Borrowed)
    }

    /// `FactTable.kind(key)`.
    pub fn kind(&self, key: &str, scope: &Scope) -> Kind {
        if key.starts_with("args:") || key.starts_with("saved:") || pos_key(key).is_some() {
            return Kind::Fact;
        }
        if let Some(n) = key.strip_prefix("var:") {
            let n: i32 = n.parse().unwrap();
            if SETTINGS.iter().any(|s| s.0 == n) {
                return Kind::Const;
            }
            if self.engine.contains(&n) {
                return Kind::NonFact;
            }
        }
        if !scope.writes.has(key) {
            return Kind::Const;
        }
        if self.domain(key).is_some() {
            Kind::Fact
        } else {
            Kind::NonFact
        }
    }

    /// `FactTable.value(key, label)`: the machine value of a domain value (a collapsed value's representative).
    pub fn value(&self, key: &str, label: Label) -> Val {
        match label {
            Label::Int(n) => Val::Int(n),
            other => {
                let dom = self.domain(key).unwrap();
                let rep = if other == Label::Above { dom.high.unwrap() + 1 } else { dom.low.unwrap() - 1 };
                Val::Unk(Unknown::new(R_COLLAPSED, key, Some(vec![rep])))
            }
        }
    }

    /// `FactTable.lazy(key, labels)`: an unknown with the fact's values (None: all of them).
    pub fn lazy(&self, key: &str, labels: Option<&Labels>) -> Val {
        let dom = self.domain(key).unwrap();
        let labels = labels.cloned().unwrap_or(dom.values.clone());
        let mut vals = BTreeSet::new();
        for l in labels {
            match self.value(key, l) {
                Val::Int(n) => {
                    vals.insert(n);
                }
                Val::Unk(u) => vals.extend(u.values.unwrap()),
            }
        }
        Val::Unk(Unknown::new(R_FACT, key, Some(vals.into_iter().collect())))
    }

    pub fn label(&self, key: &str, v: i64) -> Option<Label> {
        self.domain(key).and_then(|d| d.label(v))
    }

    /// The oracle's encoding (reader_port_oracle.py's ``table``).
    pub fn py(&self) -> Py {
        Py::dict(vec![
            ("extra", Py::Dict(self.extra.iter().map(|(k, d)| (k.clone(), d.py())).collect())),
            ("counts", self.counts.py()),
            (
                "points",
                Py::Dict(
                    self.points
                        .iter()
                        .map(|(k, m)| {
                            (
                                k.to_string(),
                                Py::Dict(
                                    m.iter()
                                        .map(|(n, s)| (n.clone(), Py::Set(s.iter().map(|&w| Py::Int(w)).collect())))
                                        .collect(),
                                ),
                            )
                        })
                        .collect(),
                ),
            ),
            ("start", self.start.py()),
        ])
    }
}

impl FactTable {
    /// Whether `w` is one of the scope's points of `name` (`kind` "at" or "esc").
    pub fn is_point(&self, kind: &str, name: &str, w: i64) -> bool {
        self.points.get(kind).and_then(|m| m.get(name)).is_some_and(|s| s.contains(&w))
    }

    /// The value `pos:`/`escpos:` takes for a place `w` written to a slot: `w`, or ELSEWHERE when the switch is on
    /// and `w` is none of the scope's points of `name`.
    pub fn place(&self, kind: &str, name: &str, w: i64) -> i64 {
        if self.elsewhere && !self.is_point(kind, name, w) {
            ELSEWHERE
        } else {
            w
        }
    }
}

/// ("at" or "esc", unit name) for a `pos:<unit>` or `escpos:<unit>` key.
pub fn pos_key(key: &str) -> Option<(&'static str, &str)> {
    if let Some(name) = key.strip_prefix("pos:") {
        Some(("at", name))
    } else {
        key.strip_prefix("escpos:").map(|name| ("esc", name))
    }
}

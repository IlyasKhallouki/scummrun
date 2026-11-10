//! reader.py `FactTable`: how the reader reads each fact, its class (`kind`), its domain (domains.py's, plus the
//! reader's own), and how a domain value is a machine value.

use crate::domain::{Domain, Label, Labels};
use crate::domains::{self, Domains, SETTINGS};
use crate::facts::Facts;
use crate::py::{Counter, Py};
use crate::record::{Unknown, Val, R_COLLAPSED, R_FACT};
use crate::scope::{self, Scope, NUM_ACTORS, VAR_CURSORSTATE, VAR_USERPUT};
use scumm_ir::{known, Expr, Game, Stmt};
use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};

/// pos: and escpos: of a unit with no live slot, or no override armed.
pub const NOWHERE: i64 = -1;
/// pos: and escpos: of a unit whose slot waits (or whose override is armed) at a place that is none of the scope's
/// points for its name ([`FactTable::elsewhere`], the switch; PORT-NOTES.md, "ELSEWHERE"). No precondition accepts it.
pub const ELSEWHERE: i64 = -2;
/// Fix round 1, F9 (switch actor_rooms): `actorroom:A` while the reading does not know the actor's room (the start
/// holds none). No room: Actor::_room is a byte. The Reader reads it as getActorRoom's non-fact, as with the switch off.
pub const NOT_KNOWN: i64 = -1;
/// Fix round 1, F2 (due.rs): `due:<unit>`, 1 while the unit's slot waits where its next turn runs on in the engine's
/// next frame, before that frame's input. A binary fact of the reader's own; only switch due_first makes one.
pub const DUE: &str = "due:";
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
    /// The switch `--live-locals` (PORT-NOTES.md, "Live locals"), off (None) by default: each unit's read locals
    /// ([`crate::locals::LiveLocals`]); an event's list of locals, its number and the locals its run starts with keep
    /// only the locals its unit's code reads, and the number covers the slot's flags. Off, the reader is reader.py's.
    pub live_locals: Option<std::sync::Arc<crate::locals::LiveLocals>>,
    binary: Domain,
    /// fix round 1, F12 (switch value_sets, the Builder's): the machine half forks a fact read that only a
    /// comparison reads into the value sets it separates, and reads the mouse as ranges (machine.rs)
    pub value_sets: bool,
    /// the switch split_limit (the Builder's): a run past the path limit is split until no part passes it
    /// (`Builder::run_split`); the machine half then hands over no path of a run the limit cut (machine.rs)
    pub split_limit: bool,
    /// the switch grow_limit (the Builder's, `Builder::run_split`)
    pub grow_limit: bool,
    /// the switch havoc_cut (the Builder's, havoc.rs): the machine half hands over a path the step limit ended whole,
    /// with where it stood (machine.rs `path_out`)
    pub havoc_cut: bool,
    /// the rough reading (the Builder's switch step_limit, experiments/E004-fast-exact/rough-reading.md): the machine
    /// half reads every run with this step limit; 0, the machine's own (`STEP_LIMIT`)
    pub step_limit: u32,
    /// the switch havoc_starts (the Builder's, havoc.rs `closure`): a start the closure cannot fold names the units the
    /// engine can run for it, not every unit
    pub havoc_starts: bool,
    /// the switch havoc_rooms (the Builder's, havoc.rs `closure`, experiments/E005-narrow-havoc/havoc-rooms.md): a room
    /// change in a havoc closure names the rooms it leaves and loads, not every scope room
    pub havoc_rooms: bool,
    /// the switch havoc_args (the Builder's, args.rs, experiments/E005-narrow-havoc/havoc-args.md): the analysis a
    /// computed start's values in a havoc closure come from; None off
    pub havoc_args: Option<std::sync::Arc<crate::args::HavocArgs>>,
    /// the switch wait_both (the Builder's): a WaitFor op whose test no fact holds also passes (machine.rs `wait_both`)
    pub wait_both: bool,
    /// E026 C1, the switch wait_queued (the Builder's): wait_both's passing path only on a path that holds a sentence
    pub wait_queued: bool,
    /// the switch --reach: the reached values, shared with the machine half (reach.rs); None off
    pub reach: Option<std::sync::Arc<crate::reach::Reached>>,
    /// the switch pass2_reuse: pass 1's runs (memo.rs); None off
    pub memo: Option<std::sync::Arc<crate::memo::RunMemo>>,
    /// the switch --memo: the job memo across readings (memo.rs `CrossMemo`, experiments/E011-speed/memo.md); None off
    pub cross: Option<std::sync::Arc<crate::memo::CrossMemo>>,
    /// the switch shared_runs: the batch's runs of clicks that differ only in their verb (shared.rs); None off
    pub shared: Option<std::sync::Arc<crate::shared::SharedRuns>>,
    /// fix round 1, F1c (the switch input_settled, the Builder's): what the reading read from the scripts for it
    /// (inputs.rs); None with the switch off
    pub settled: Option<std::sync::Arc<crate::inputs::SettledRule>>,
    /// the variables the switch `--fact-vars` made facts (`add_fact_vars`); empty with the switch off. A comparison
    /// of one with a local the path knows splits its values as a comparison with a constant does (machine.rs
    /// `compare_sets`)
    pub refined: BTreeSet<i32>,
    /// the `pos:`/`escpos:` domains, made once from `points` (`PosDoms`)
    pos_doms: std::sync::Arc<PosDoms>,
}

/// The domains of `pos:<unit>` and `escpos:<unit>` (`FactTable::domain`), worked out once from the table's points,
/// which do not change after `FactTable::new`: per unit name its points with NOWHERE, without and with ELSEWHERE (the
/// switch `elsewhere`, set after `new`), and the same for a name with no points.
#[derive(Clone)]
struct PosDoms {
    at: rustc_hash::FxHashMap<String, [Domain; 2]>,
    esc: rustc_hash::FxHashMap<String, [Domain; 2]>,
    none: [Domain; 2],
}

impl PosDoms {
    fn of(points: &BTreeMap<&'static str, BTreeMap<String, BTreeSet<i64>>>) -> PosDoms {
        let both = |ws: Option<&BTreeSet<i64>>| -> [Domain; 2] {
            let mut v: BTreeSet<i64> = ws.cloned().unwrap_or_default();
            v.insert(NOWHERE);
            let off = Domain::of(v.iter().copied(), None, None);
            v.insert(ELSEWHERE);
            [off, Domain::of(v, None, None)]
        };
        let side = |kind: &str| points[kind].iter().map(|(n, ws)| (n.clone(), both(Some(ws)))).collect();
        PosDoms { at: side("at"), esc: side("esc"), none: both(None) }
    }
}

impl FactTable {
    pub fn new(scope: &Scope, doms: Domains, game: &Game) -> FactTable {
        let ast = &game.ast;
        let mut start = scope.start.clone();
        for (n, v) in SETTINGS {
            start.insert(format!("var:{n}"), v);
        }
        if scope.switches.actor_rooms {
            // F9: an actor's room the start does not give is not known (NOT_KNOWN), not room 0
            for a in 0..NUM_ACTORS {
                let k = format!("actorroom:{a}");
                if !start.contains(&k) {
                    start.insert(k, NOT_KNOWN);
                }
            }
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
        if scope.switches.actor_rooms {
            actor_room_domains(scope, game, &start, &doms, &mut extra, &mut counts);
        }
        let mut engine: BTreeSet<i32> = domains::ENGINE.iter().map(|e| e.0).collect();
        if crate::builder::read_sound_switch() {
            engine.insert(domains::VAR_SOUNDRESULT);
        }
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
        let pos_doms = std::sync::Arc::new(PosDoms::of(&points));
        FactTable {
            start,
            extra,
            counts,
            args_vals: BTreeMap::new(),
            doms,
            points,
            engine,
            elsewhere: false,
            live_locals: None,
            binary: Domain::binary(),
            value_sets: false,
            split_limit: false,
            grow_limit: false,
            havoc_cut: false,
            step_limit: 0,
            havoc_starts: false,
            havoc_rooms: false,
            havoc_args: None,
            wait_both: false,
            wait_queued: false,
            reach: None,
            memo: None,
            cross: None,
            shared: None,
            settled: None,
            refined: BTreeSet::new(),
            pos_doms,
        }
    }

    /// `FactTable.domain(key)`.
    pub fn domain(&self, key: &str) -> Option<Cow<'_, Domain>> {
        if key.starts_with("args:") {
            // the list numbers with 0, in order (`Domain::of` of the set with 0 added)
            let set = self.args_vals.get(key);
            let mut values = Vec::with_capacity(set.map_or(0, |s| s.len()) + 1);
            let mut zero = false;
            for &n in set.into_iter().flatten() {
                if !zero && n >= 0 {
                    zero = true;
                    if n != 0 {
                        values.push(Label::Int(0));
                    }
                }
                values.push(Label::Int(n));
            }
            if !zero {
                values.push(Label::Int(0));
            }
            return Some(Cow::Owned(Domain::plain(values)));
        }
        if key.starts_with("saved:") || key.starts_with(DUE) {
            return Some(Cow::Borrowed(&self.binary));
        }
        if let Some((kind, name)) = pos_key(key) {
            let side = if kind == "at" { &self.pos_doms.at } else { &self.pos_doms.esc };
            let both = side.get(name).unwrap_or(&self.pos_doms.none);
            return Some(Cow::Borrowed(&both[self.elsewhere as usize]));
        }
        self.extra.get(key).or_else(|| self.doms.get(key)).map(Cow::Borrowed)
    }

    /// `FactTable.kind(key)`.
    pub fn kind(&self, key: &str, scope: &Scope) -> Kind {
        if key.starts_with("args:") || key.starts_with("saved:") || key.starts_with(DUE) || pos_key(key).is_some() {
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

/// What the reader switch `--fact-vars` did with one listed variable (experiments/E007-fact-refinement/plan.md).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FactVar {
    pub n: i32,
    /// "added", or why the variable was left as it is
    pub what: &'static str,
    /// the domain's seeds: the table's start value and each further start's
    pub seeds: Vec<i64>,
}

impl FactTable {
    /// The reader switch `--fact-vars` (experiments/E007-fact-refinement/plan.md, Lemma R), off by default: each
    /// variable of `vars` that is a non-fact of the reading (the scope writes it, it is no engine or settings variable,
    /// and it has no domain) gets a domain in `extra`, seeded with its value in the table's start and in each of
    /// `starts`, as `Builder::extend_domains` gives a widened one; the reader's passes then add every value a path
    /// writes outside it. Every other variable is left as it is. Returns what was done, per variable in the order given.
    pub fn add_fact_vars(&mut self, scope: &Scope, vars: &[i32], starts: &[&Facts]) -> Vec<FactVar> {
        let mut out = Vec::new();
        for &n in vars {
            let key = format!("var:{n}");
            let mut seeds: BTreeSet<i64> = [self.start.get0(&key)].into_iter().collect();
            seeds.extend(starts.iter().map(|s| s.get0(&key)));
            let what = if SETTINGS.iter().any(|s| s.0 == n) {
                "fixed by the settings"
            } else if self.engine.contains(&n) {
                "written by the engine"
            } else if !scope.writes.has(&key) {
                "the scope does not write it (a constant at its start value)"
            } else if self.domain(&key).is_some() {
                "already a fact"
            } else {
                self.extra.insert(key, Domain::of(seeds.iter().copied(), None, None));
                self.refined.insert(n);
                "added"
            };
            out.push(FactVar { n, what, seeds: seeds.into_iter().collect() });
        }
        out
    }
}

/// Fix round 1, F9 (switch actor_rooms): the reader's domain of `actorroom:A` for each actor the scope writes, from
/// the scope's own code (the reading runs no other): its start (NOT_KNOWN when the start has none) and every room a
/// write of `scope::actor_room_writes` that can reach the actor gives it. An operand is a constant or a variable with
/// its values in the reading: a setting's value, the start's value of a variable the scope does not write, or the
/// values of its domain (VAR_ROOM's are the `room` domain's: the engine writes it with the room, room.cpp:148, 154).
/// An actor that a write with a room not worked out can reach gets no domain: its room stays a non-fact, read as
/// with the switch off.
fn actor_room_domains(
    scope: &Scope,
    game: &Game,
    start: &Facts,
    doms: &Domains,
    extra: &mut BTreeMap<String, Domain>,
    counts: &mut Counter,
) {
    let var = |n: i32| -> Option<BTreeSet<i64>> {
        let k = format!("var:{n}");
        if let Some(&(_, v)) = SETTINGS.iter().find(|s| s.0 == n) {
            return Some([v].into_iter().collect());
        }
        if domains::engine_why(n).is_some() || TIME_VARS.contains(&n) || MOUSE_VARS.contains(&n) {
            return None;
        }
        if !scope.writes.has(&k) {
            return Some([start.get0(&k)].into_iter().collect());
        }
        let d = extra.get(&k).or_else(|| doms.get(&k))?;
        d.values.iter().map(|l| match l {
            Label::Int(v) => Some(*v),
            _ => None,
        }).collect()
    };
    let writes = scope::actor_room_writes(game, &scope.units, &var);
    for a in 0..NUM_ACTORS {
        let k = format!("actorroom:{a}");
        if !scope.writes.has(&k) {
            continue;
        }
        let mine: Vec<&Option<BTreeSet<i64>>> =
            writes.iter().filter(|w| w.0.as_ref().is_none_or(|who| who.contains(&a))).map(|w| &w.1).collect();
        if mine.iter().any(|r| r.is_none()) {
            counts.inc("actors a room write that is not worked out can reach (actorroom: left a non-fact; reader, F9)");
            continue;
        }
        let mut vals: BTreeSet<i64> = [start.get0(&k)].into_iter().collect();
        vals.extend(mine.into_iter().flat_map(|r| r.as_ref().unwrap().iter().copied()));
        extra.insert(k, Domain::of(vals, None, None));
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

//! Fix round 1, F2 (fix-round1.md, decisions.md D8): the planner rule that a due event goes first. Switch
//! `due_first`, off by default; with it off nothing here runs and the reading is reader.py's, byte for byte.
//!
//! **The engine.** In one frame every live, unfrozen slot takes its turn in runAllScripts, and only then does
//! checkExecVerbs read the frame's input (scumm.cpp:3231-3232; script.cpp:968-987). processInput's own work comes
//! before the scripts (Esc's override test, input.cpp:1423-1427; '.', 1418-1421). So a slot whose wait is over runs
//! before the input of the same frame. The reader makes a slot's later part an event the planner orders freely
//! (D5), so a plan could put the input first.
//!
//! **The rule.** Where a slot waits is classed from the code (scumm-ir's `due.rs`, with const_polls on, F6):
//!
//! * a pure poll on facts (a breakHere in a loop whose exit tests read only facts, and nothing but jumps and
//!   constant writes between the wait and the test) or a plain breakHere (no test): the slot is **due** while it is
//!   live there (its slot fact 1), unfrozen (`frozen:` 0) and the test holds. `due:<unit>` is a binary fact of the
//!   task, kept by conditional effects (`Eff::Cases`) on every record that writes the unit's position, its slot fact,
//!   its freeze count, the room (when two units of the scope share the name) or a fact the test reads; its start
//!   value is worked out from the start (`Due::start_value`). No axioms (task.py: the landmark heuristic is not safe
//!   with them).
//! * An input (a sentence, a walk, a dialog line: what checkExecVerbs reads) whose reads or writes meet a due
//!   wait's next part (the facts the slot's event records at that wait read or write, and the test's facts)
//!   requires `due:<unit>` = 0. The meeting is one of three: the input writes what the next part reads or writes,
//!   or reads what it writes. A slot that never meets an input gets no `due:` fact.
//! * Every other wait (a WaitFor op, a delay, a loop whose exit tests read a non-fact, an ambient loop, a polled
//!   loop that is not a pure poll) is never due as a fact. An input it meets carries a replay check: the wait did
//!   not end in the input's frame (reason "wait over by time").
//! * Esc (skip_cutscene) needs no `due:` 0: its abortCutscene comes before the frame's scripts. Its records keep
//!   the `due:` facts they change, as every record does.
//!
//! A due wait whose event has no record that runs the slot on while the test holds (its paths all dropped) would
//! block the inputs it meets for good: such a wait is checked at replay instead, counted. Nothing here names a
//! game's script, object, variable or room (D1, plan.md's C2).

use crate::builder::Builder;
use crate::domain::{Label, Labels};
use crate::facts::unit_name;
use crate::py::Counter;
use crate::record::{Assumption, Case, Eff, Record, What};
use crate::table::{Kind, DUE, NOWHERE};
use rustc_hash::{FxHashMap, FxHashSet};
use scumm_ir::due::{exits, loop_facts, poll_shape, wait_class, WaitClass};
use scumm_ir::{Game, UnitId, UnitKey};
use std::collections::{BTreeMap, BTreeSet};

/// switch const_polls, approved on (D8, F6)
const CP: bool = true;
/// valuations of one wait's test facts past which the wait is checked at replay
const TABLE_LIMIT: usize = 4096;
/// pre-state valuations past which one record's `due:` write is a guess (counted)
const CASE_LIMIT: usize = 1 << 16;
/// the input kinds checkExecVerbs reads after the frame's scripts
const INPUT_KINDS: [&str; 3] = ["sentence", "walk_to", "dialog_line"];

/// A wait a slot can be due at: the facts its test reads (none for a plain breakHere) and, per valuation of them
/// (labels in `reads` order), whether the next turn runs on.
#[derive(Clone, Debug)]
struct DueWait {
    reads: Vec<String>,
    table: FxHashMap<Vec<Label>, bool>,
}

/// A wait's next part: (reads, writes).
type Part = (FxHashSet<String>, FxHashSet<String>);

/// A due wait's next part, and its work (`apply`).
struct DuePart {
    w: i64,
    ki: usize,
    rooms: Option<Labels>,
    part: Part,
    work: FxHashSet<String>,
}

/// A wait that is never due as a fact: where replay checks it.
#[derive(Clone, Debug)]
struct TimeWait {
    cite: String,
    why: String,
    /// the unit's slot fact, and the rooms that load it (None: not room-bound)
    slot: Option<String>,
    rooms: Option<Labels>,
}

/// The `due:` fact of one unit name (`script:N`, `local:N`, `obj:O`, `entry`, `exit`).
#[derive(Clone, Debug)]
pub struct DueUnit {
    pub name: String,
    /// the scope's units of that name, with the rooms that load them (room-bound units)
    keys: Vec<(UnitKey, Option<Vec<Label>>)>,
    /// (index into `keys`, W) -> the wait
    waits: BTreeMap<(usize, i64), DueWait>,
    pos: String,
    frozen: String,
    slot: Option<String>,
    /// two units of the scope share the name: the room tells them apart
    by_room: bool,
    /// the facts `due:<name>` is a function of: the position first, then the slot, the freeze count, the room, the
    /// tests' facts
    pub inputs: Vec<String>,
}

impl DueUnit {
    pub fn key(&self) -> String {
        format!("{DUE}{}", self.name)
    }

    /// `due:<name>` in a state (`get`: a fact's value; None where the state lacks one, read as not due).
    pub fn value(&self, get: &dyn Fn(&str) -> Option<Label>) -> bool {
        let Some(Label::Int(w)) = get(&self.pos) else { return false };
        if w == NOWHERE {
            return false;
        }
        if let Some(s) = &self.slot {
            if get(s) != Some(Label::Int(1)) {
                return false;
            }
        }
        if get(&self.frozen) != Some(Label::Int(0)) {
            return false;
        }
        let room = if self.by_room { get("room") } else { None };
        for (ki, (_, rooms)) in self.keys.iter().enumerate() {
            if self.by_room && !rooms.as_ref().is_some_and(|rs| room.is_some_and(|r| rs.contains(&r))) {
                continue;
            }
            if let Some(dw) = self.waits.get(&(ki, w)) {
                let labs: Option<Vec<Label>> = dw.reads.iter().map(|k| get(k)).collect();
                return labs.and_then(|l| dw.table.get(&l).copied()).unwrap_or(false);
            }
        }
        false
    }
}

/// What the switch added: the units with a `due:` fact, and its counts (also in the Builder's counts, "due: ...").
#[derive(Clone, Debug, Default)]
pub struct Due {
    pub units: BTreeMap<String, DueUnit>,
    pub counts: Counter,
}

impl Due {
    /// The `due:` facts of a start (task_start's): each from the start's position, slot, freeze and test facts.
    pub fn start_value(&self, key: &str, get: &dyn Fn(&str) -> Option<Label>) -> Option<i64> {
        let name = key.strip_prefix(DUE)?;
        self.units.get(name).map(|u| u.value(get) as i64)
    }
}

/// The facts one record reads (its precondition, its effects' conditions) and writes (its effects, and the
/// candidates of its writes whose target did not fold, where the code bounds them).
fn reads_writes(b: &mut Builder, r: &Record) -> (FxHashSet<String>, FxHashSet<String>) {
    let mut rd: FxHashSet<String> = r.pre.keys().cloned().collect();
    let mut wr: FxHashSet<String> = FxHashSet::default();
    for (k, e) in &r.effects {
        wr.insert(k.clone());
        rd.extend(e.sources().into_iter().map(String::from));
        if matches!(e, Eff::Add(_)) {
            rd.insert(k.clone());
        }
    }
    for w in &r.unknown {
        let t = w.items();
        if let Some(keys) = b.unknown_targets(t[0].as_str(), t[1].as_str()) {
            wr.extend(keys);
        }
    }
    (rd, wr)
}

/// Whether the input's precondition lets the unit's slot be live, unfrozen and at `w` (in one of `rooms`) when it
/// runs: an input that needs the slot stopped, frozen, elsewhere or another room never meets that wait.
fn admits(r: &Record, name: &str, w: i64, slot: Option<&str>, rooms: Option<&Labels>) -> bool {
    let has = |k: &str, l: Label| r.pre.get(k).is_none_or(|s| s.contains(&l));
    has(&format!("pos:{name}"), Label::Int(w))
        && slot.is_none_or(|k| has(k, Label::Int(1)))
        && has(&format!("frozen:{name}"), Label::Int(0))
        && rooms.is_none_or(|rs| r.pre.get("room").is_none_or(|s| s.iter().any(|l| rs.contains(l))))
}

/// A time wait of a unit name; two units of one name (locals of two rooms) at one offset join their rooms.
fn add_time(map: &mut BTreeMap<(String, i64), TimeWait>, name: &str, w: i64, tw: TimeWait) {
    match map.get_mut(&(name.to_string(), w)) {
        None => {
            map.insert((name.to_string(), w), tw);
        }
        Some(old) => {
            old.rooms = match (old.rooms.take(), tw.rooms) {
                (Some(mut a), Some(b)) => {
                    a.extend(b);
                    a.sort();
                    a.dedup();
                    Some(a)
                }
                _ => None,
            };
        }
    }
}

/// A label's value as the code compares it (a collapsed value's representative, `FactTable.value`).
fn label_value(b: &Builder, key: &str, l: Label) -> Option<i64> {
    match l {
        Label::Int(n) => Some(n),
        Label::Above => b.domain(key)?.high.map(|h| h + 1),
        Label::Below => b.domain(key)?.low.map(|lo| lo - 1),
    }
}

/// The values a fact can have: its start value for a constant, its domain's for a fact; None for a non-fact.
fn candidates(b: &Builder, key: &str) -> Option<Labels> {
    match b.kind(key) {
        Kind::Const => {
            let v = b.table.start.get0(key);
            Some(vec![b.table.label(key, v).unwrap_or(Label::Int(v))])
        }
        Kind::Fact => b.domain(key).map(|d| d.values),
        Kind::NonFact => None,
    }
}

/// The wait at offset `w` of unit `u` of the scope, by class.
enum Class {
    Due(DueWait),
    Time(TimeWait),
    Skip,
}

fn classify(b: &Builder, u: UnitId, name: &str, w: i64, counts: &mut Counter) -> Class {
    let g: &Game = b.game;
    let Some(idx) = g.at(u, w as i32) else { return Class::Skip };
    let c = wait_class(g, u, idx, CP);
    let yield_at = |i: u32| g.cite(u, i);
    let time = |why: String, at: u32| Class::Time(TimeWait { cite: yield_at(at), why: format!("{name}@{w}: {why}"), slot: None, rooms: None });
    match c {
        WaitClass::None => Class::Skip,
        WaitClass::Plain => {
            counts.inc("due: waits by class: plain (a breakHere in no loop)");
            Class::Due(DueWait { reads: vec![], table: [(vec![], true)].into_iter().collect() })
        }
        WaitClass::Ambient => {
            counts.inc("due: waits by class: ambient (checked at replay)");
            time("an ambient loop".into(), idx - 1)
        }
        WaitClass::Time(why) => {
            counts.inc(format!("due: waits by class: time, {why} (checked at replay)"));
            let at = if g.wait_point(u, idx, CP).kind == scumm_ir::WaitKind::Wait { idx } else { idx - 1 };
            time(why.into(), at)
        }
        WaitClass::Polled(li) => {
            let lp = &g.loops(u, CP)[li as usize];
            if let Err(why) = poll_shape(g, u, idx, lp) {
                counts.inc(format!("due: waits by class: polled, not a pure poll: {why} (checked at replay)"));
                return time(format!("a polled loop that is not a pure poll ({why})"), idx - 1);
            }
            let reads: Vec<String> = loop_facts(lp).iter().map(|f| f.to_string()).collect();
            let mut cands: Vec<Labels> = Vec::new();
            for k in &reads {
                match candidates(b, k) {
                    Some(c) => cands.push(c),
                    None => {
                        counts.inc("due: waits by class: polled on a fact the reader does not hold (checked at replay)");
                        return time(format!("a polled loop on {k}, which the reader does not hold"), idx - 1);
                    }
                }
            }
            let n: usize = cands.iter().map(|c| c.len()).product();
            if n > TABLE_LIMIT {
                counts.inc("due: waits by class: polled on too many values (checked at replay)");
                return time("a polled loop on too many values".into(), idx - 1);
            }
            let mut table = FxHashMap::default();
            for combo in product(&cands) {
                let vals: Vec<Option<i64>> = reads.iter().zip(&combo).map(|(k, &l)| label_value(b, k, l)).collect();
                let got = exits(g, u, idx, lp, &|f| {
                    let s = f.to_string();
                    reads.iter().position(|k| *k == s).and_then(|i| vals[i])
                });
                match got {
                    Ok(v) => {
                        table.insert(combo, v);
                    }
                    Err(why) => {
                        if std::env::var_os("DUE_DEBUG").is_some() {
                            eprintln!("due-debug noeval {name}@{w} {why} reads {reads:?} combo {combo:?}");
                        }
                        counts.inc("due: waits by class: polled, the test does not evaluate (checked at replay)");
                        return time(format!("a polled loop whose test does not evaluate ({why})"), idx - 1);
                    }
                }
            }
            if !table.values().any(|&v| v) {
                counts.inc("due: waits by class: polled, the test never holds (never due)");
            }
            counts.inc("due: waits by class: polled (a pure poll on facts)");
            Class::Due(DueWait { reads, table })
        }
    }
}

/// Every combination of one value per set, the first set slowest.
fn product(sets: &[Labels]) -> Vec<Vec<Label>> {
    let mut out: Vec<Vec<Label>> = vec![vec![]];
    for s in sets {
        out = out.iter().flat_map(|c| s.iter().map(move |&l| [c.as_slice(), &[l]].concat())).collect();
    }
    out
}

/// A record's write to one fact, as a function of the state before it.
enum Post {
    Fixed(Label),
    Choice(Labels),
    Same,
    Add(i64),
    When(String, i64),
    Copy(String),
}

/// The record's write to `due:<unit>` (None: it writes none of the facts the unit's due depends on).
fn due_effect(b: &Builder, du: &DueUnit, r: &Record, counts: &mut Counter) -> Option<Eff> {
    if !r.effects.iter().any(|(k, _)| du.inputs.contains(k)) {
        return None;
    }
    let post: Vec<Post> = du
        .inputs
        .iter()
        .map(|k| match r.effect(k) {
            None => Post::Same,
            Some(Eff::Set(l)) => Post::Fixed(*l),
            Some(Eff::Guess(ls)) => Post::Choice(ls.clone()),
            Some(Eff::Add(d)) => Post::Add(*d),
            Some(Eff::When(src, v)) => Post::When(src.clone(), *v),
            Some(Eff::Copy(src)) => Post::Copy(src.clone()),
            Some(Eff::Cases(_)) => panic!("a due: input written by cases: {k}"),
        })
        .collect();
    // the facts whose value before the record the result depends on, in `inputs` order, then the sources
    let mut free: Vec<String> = Vec::new();
    for (k, p) in du.inputs.iter().zip(&post) {
        match p {
            Post::Same | Post::Add(_) | Post::When(..) => free.push(k.clone()),
            _ => {}
        }
    }
    for p in &post {
        if let Post::When(src, _) | Post::Copy(src) = p {
            if !free.contains(src) {
                free.push(src.clone());
            }
        }
    }
    let mut cands: Vec<Labels> = Vec::new();
    for k in &free {
        let all = candidates(b, k).unwrap_or_else(|| panic!("due: {k} is not held"));
        cands.push(match r.pre.get(k) {
            Some(s) => all.into_iter().filter(|l| s.contains(l)).collect(),
            None => all,
        });
    }
    let n: usize = cands.iter().map(|c| c.len()).product();
    if n == 0 {
        counts.inc("due: records whose precondition no value meets (no due: write)");
        return None;
    }
    if n > CASE_LIMIT {
        counts.inc("due: writes guessed (too many states before the record)");
        return Some(Eff::Guess(vec![Label::Int(0), Label::Int(1)]));
    }
    let mut rows: Vec<(Vec<Label>, Labels)> = Vec::with_capacity(n);
    for combo in product(&cands) {
        let pre = |k: &str| free.iter().position(|f| f == k).map(|i| combo[i]);
        // each input's values after the record
        let mut outs: Vec<Labels> = Vec::with_capacity(du.inputs.len());
        for (k, p) in du.inputs.iter().zip(&post) {
            outs.push(match p {
                Post::Fixed(l) => vec![*l],
                Post::Choice(ls) => ls.clone(),
                Post::Same => vec![pre(k).unwrap()],
                Post::Add(d) => b.domain(k).unwrap().add(pre(k).unwrap(), *d),
                Post::When(src, v) => {
                    if pre(src) == Some(Label::Int(1)) {
                        vec![Label::Int(*v)]
                    } else {
                        vec![pre(k).unwrap()]
                    }
                }
                Post::Copy(src) => vec![pre(src).unwrap()],
            });
        }
        let mut got: BTreeSet<bool> = BTreeSet::new();
        for after in product(&outs) {
            got.insert(du.value(&|k: &str| du.inputs.iter().position(|x| x == k).map(|i| after[i])));
        }
        let res: Labels = got.into_iter().map(|v| Label::Int(v as i64)).collect();
        if res.is_empty() {
            continue; // an add with no room: build_task's precondition leaves this state out
        }
        rows.push((combo, res));
    }
    if rows.is_empty() {
        return None;
    }
    let mut cases: Vec<Case> = Vec::new();
    tree(&free, &rows, 0, &mut Vec::new(), &mut cases);
    counts.add("due: conditional cases written", cases.len() as i64);
    if cases.len() == 1 && cases[0].0.is_empty() {
        let vals = cases.pop().unwrap().1;
        if vals.len() > 1 {
            counts.inc("due: writes guessed (a guessed write of a fact the due depends on)");
            return Some(Eff::Guess(vals));
        }
        return Some(Eff::Set(vals[0]));
    }
    if cases.iter().any(|c| c.1.len() > 1) {
        counts.inc("due: writes guessed (a guessed write of a fact the due depends on)");
    }
    Some(Eff::Cases(cases))
}

/// A decision tree over the free facts, in their order: a leaf where every state left gives one result; the
/// values of one fact whose subtrees are such leaves with one result merge into one set condition.
fn tree(keys: &[String], rows: &[(Vec<Label>, Labels)], depth: usize, conds: &mut Vec<(String, Labels)>, out: &mut Vec<Case>) {
    let first = &rows[0].1;
    if rows.iter().all(|r| &r.1 == first) {
        out.push((conds.clone(), first.clone()));
        return;
    }
    let mut groups: BTreeMap<Label, Vec<(Vec<Label>, Labels)>> = BTreeMap::new();
    for r in rows {
        groups.entry(r.0[depth]).or_default().push(r.clone());
    }
    if groups.len() == 1 {
        return tree(keys, rows, depth + 1, conds, out);
    }
    let mut uniform: BTreeMap<Labels, Labels> = BTreeMap::new();
    let mut mixed: Vec<(Label, Vec<(Vec<Label>, Labels)>)> = Vec::new();
    for (l, rs) in groups {
        let f = rs[0].1.clone();
        if rs.iter().all(|r| r.1 == f) {
            uniform.entry(f).or_default().push(l);
        } else {
            mixed.push((l, rs));
        }
    }
    for (res, ls) in uniform {
        conds.push((keys[depth].clone(), ls));
        out.push((conds.clone(), res));
        conds.pop();
    }
    for (l, rs) in mixed {
        conds.push((keys[depth].clone(), vec![l]));
        tree(keys, &rs, depth + 1, conds, out);
        conds.pop();
    }
}

/// Switch due_first on the Builder's records (after the reading): the `due:` facts, the inputs' `due:` 0
/// preconditions, their replay checks on time waits, and every record's `due:` writes. The Builder keeps the result
/// (`Builder.due`) for the task's starts.
pub fn apply(b: &mut Builder) {
    assert!(b.keep_records, "switch due_first works on the kept records");
    let mut counts = Counter::default();
    let g: &Game = b.game;
    // the scope's units by name
    let mut by_name: BTreeMap<String, Vec<UnitId>> = BTreeMap::new();
    for &u in &b.scope.units {
        by_name.entry(unit_name(g.unit(u).key)).or_default().push(u);
    }
    let rooms: Labels = b.domain("room").map(|d| d.values).unwrap_or_default();
    let points: Vec<(String, Vec<i64>)> =
        b.table.points["at"].iter().map(|(n, ws)| (n.clone(), ws.iter().copied().collect())).collect();
    let mut units: BTreeMap<String, DueUnit> = BTreeMap::new();
    let mut time_waits: BTreeMap<(String, i64), TimeWait> = BTreeMap::new();
    for (name, ws) in &points {
        let Some(us) = by_name.get(name) else { continue };
        let keys: Vec<(UnitKey, Option<Vec<Label>>)> = us
            .iter()
            .map(|&u| {
                let k = g.unit(u).key;
                let res = match k {
                    UnitKey::Local(r, _) | UnitKey::Entry(r) | UnitKey::Exit(r) => Some(r),
                    _ => None,
                };
                let rs = res.map(|res| {
                    rooms.iter().copied().filter(|l| l.int().is_some_and(|r| g.resource(r as i32) == Some(res))).collect()
                });
                (k, rs)
            })
            .collect();
        let mut waits = BTreeMap::new();
        for (ki, &u) in us.iter().enumerate() {
            for &w in ws {
                match classify(b, u, name, w, &mut counts) {
                    Class::Due(dw) => {
                        waits.insert((ki, w), dw);
                    }
                    Class::Time(mut tw) => {
                        tw.slot = g.lifecycle_key(keys[ki].0).map(|f| f.to_string());
                        tw.rooms = keys[ki].1.clone();
                        add_time(&mut time_waits, name, w, tw);
                    }
                    Class::Skip => {}
                }
            }
        }
        if waits.is_empty() {
            continue;
        }
        let slot = scumm_ir::Game::lifecycle_key(g, keys[0].0).map(|f| f.to_string());
        let by_room = keys.len() > 1;
        let mut inputs = vec![format!("pos:{name}")];
        inputs.extend(slot.clone());
        inputs.push(format!("frozen:{name}"));
        if by_room {
            inputs.push("room".into());
        }
        for dw in waits.values() {
            for k in &dw.reads {
                if !inputs.contains(k) {
                    inputs.push(k.clone());
                }
            }
        }
        // keys the reader holds as constants never change: they stay out of the inputs (their start value is read)
        units.insert(
            name.clone(),
            DueUnit { name: name.clone(), keys, waits, pos: format!("pos:{name}"), frozen: format!("frozen:{name}"), slot, by_room, inputs },
        );
    }
    // the next parts: what each wait's event records read and write, and which records run the slot on
    let mut next: FxHashMap<(String, i64), Part> = FxHashMap::default();
    let mut event_recs: FxHashMap<(String, i64), Vec<usize>> = FxHashMap::default();
    let n = b.records.len();
    for i in 0..n {
        let r = b.records[i].clone();
        let What::Event(e) = &r.source else { continue };
        if e.abort {
            continue;
        }
        let key = (e.name.clone(), e.w);
        let rw = reads_writes(b, &r);
        let ent = next.entry(key.clone()).or_default();
        ent.0.extend(rw.0);
        ent.1.extend(rw.1);
        event_recs.entry(key).or_default().push(i);
    }
    // a due wait no record runs on from while its test holds would block the inputs it meets for good
    for du in units.values_mut() {
        let name = du.name.clone();
        let mut demote: Vec<(usize, i64)> = Vec::new();
        for (&(ki, w), dw) in &du.waits {
            let recs = event_recs.get(&(name.clone(), w)).cloned().unwrap_or_default();
            let unit = du.keys[ki].0;
            let mut ok = true;
            for (combo, &held) in &dw.table {
                if !held {
                    continue;
                }
                let found = recs.iter().any(|&i| {
                    let r = &b.records[i];
                    matches!(&r.source, What::Event(e) if e.unit == unit)
                        && dw.reads.iter().zip(combo).all(|(k, l)| r.pre.get(k).is_none_or(|s| s.contains(l)))
                        && r.effect(&du.pos).is_some()
                });
                if !found {
                    ok = false;
                    break;
                }
            }
            if !ok {
                demote.push((ki, w));
            }
        }
        for (ki, w) in demote {
            du.waits.remove(&(ki, w));
            if event_recs.contains_key(&(name.clone(), w)) {
                counts.inc("due: due waits whose event records never run on (checked at replay)");
            } else {
                counts.inc("due: due waits with no event record (no reading reaches them)");
                continue;
            }
            let u = g.id(du.keys[ki].0).unwrap();
            let idx = g.at(u, w as i32).unwrap();
            let tw = TimeWait {
                cite: g.cite(u, idx.saturating_sub(1)),
                why: format!("{name}@{w}: a due wait whose event records never run on"),
                slot: du.slot.clone(),
                rooms: du.keys[ki].1.clone(),
            };
            add_time(&mut time_waits, &name, w, tw);
        }
    }
    units.retain(|_, du| !du.waits.is_empty());
    // each wait's next part: what its event records read and write (for a due wait, also the test's facts and the
    // position); its work: what they write besides the unit's own position, locals list and override. A step of a
    // chain of breakHeres does no work, and meets no input.
    let own = |name: &str| [format!("pos:{name}"), format!("args:{name}"), format!("escpos:{name}")];
    let work_of = |name: &str, part: &Part| -> FxHashSet<String> {
        let o = own(name);
        part.1.iter().filter(|k| !o.contains(k)).cloned().collect()
    };
    let parts: Vec<(String, Option<String>, Vec<DuePart>)> = units
        .values()
        .map(|du| {
            let ps = du
                .waits
                .iter()
                .map(|(&(ki, w), dw)| {
                    let mut part = next.get(&(du.name.clone(), w)).cloned().unwrap_or_default();
                    part.0.extend(dw.reads.iter().cloned());
                    part.0.insert(du.pos.clone());
                    let work = work_of(&du.name, &part);
                    DuePart { w, ki, rooms: du.keys[ki].1.clone(), part, work }
                })
                .filter(|p| !p.work.is_empty())
                .collect();
            (du.name.clone(), du.slot.clone(), ps)
        })
        .collect();
    let time_parts: Vec<(String, i64, &TimeWait, FxHashSet<String>)> = time_waits
        .iter()
        .filter_map(|((name, w), tw)| {
            let work = work_of(name, next.get(&(name.clone(), *w))?);
            (!work.is_empty()).then(|| (name.clone(), *w, tw, work))
        })
        .collect();
    // the inputs: due: 0 where they meet a due wait's next part (they write what it reads or writes, or read what it
    // works on), a replay check where a time wait's work could change them (it writes what they read or write)
    let mut met_waits: BTreeSet<(String, usize, i64)> = BTreeSet::new();
    for i in 0..n {
        if !INPUT_KINDS.contains(&b.records[i].kind.as_str()) {
            continue;
        }
        let r = b.records[i].clone();
        let rw = reads_writes(b, &r);
        let mut pre_add: Vec<String> = Vec::new();
        for (name, slot, ps) in &parts {
            let mut hit = false;
            for p in ps {
                if admits(&r, name, p.w, slot.as_deref(), p.rooms.as_ref())
                    && (rw.1.iter().any(|k| p.part.0.contains(k) || p.part.1.contains(k)) || rw.0.iter().any(|k| p.work.contains(k)))
                {
                    met_waits.insert((name.clone(), p.ki, p.w));
                    hit = true;
                }
            }
            if hit {
                pre_add.push(format!("{DUE}{name}"));
            }
        }
        let mut met: Vec<String> = Vec::new();
        for (name, w, tw, work) in &time_parts {
            if admits(&r, name, *w, tw.slot.as_deref(), tw.rooms.as_ref()) && work.iter().any(|k| rw.0.contains(k) || rw.1.contains(k)) {
                if std::env::var_os("DUE_DEBUG").is_some() {
                    let mut ks: Vec<&String> = work.iter().filter(|k| rw.0.contains(*k) || rw.1.contains(*k)).collect();
                    ks.sort();
                    eprintln!("due-debug time {name}@{w} {} {:?} ({}, {})", r.label, ks, tw.why, tw.cite);
                }
                met.push(format!("{name}@{w}"));
            }
        }
        let mut checks: Vec<Assumption> = Vec::new();
        if !met.is_empty() {
            counts.add("due: time waits listed in those checks", met.len() as i64);
            checks.push(Assumption::new(
                "scumm.cpp:3231-3232",
                format!("no time wait this input meets ended in its frame before checkExecVerbs: {}", met.join(" ")),
                true,
                "?(wait over by time: a slot's turn in the input's frame)",
            ));
        }
        if !pre_add.is_empty() {
            counts.inc("due: inputs that need a due: 0");
            counts.add("due: due: 0 preconditions", pre_add.len() as i64);
        }
        if !checks.is_empty() {
            counts.inc("due: inputs with a time-wait replay check (one per input)");
        }
        let rec = &mut b.records[i];
        for k in pre_add {
            // an engine condition of the input: checkExecVerbs reads it after the frame's turns (Record.engine_pre)
            rec.pre.insert(k.clone(), vec![Label::Int(0)]);
            rec.engine_pre.push(k);
        }
        rec.engine_pre.sort();
        rec.engine_pre.dedup();
        for c in checks {
            if !rec.checks.contains(&c) {
                rec.checks.push(c);
            }
        }
    }
    // a unit is due only at the waits an input meets: elsewhere (a step of a chain, a wait no input meets) it is not
    for du in units.values_mut() {
        let name = du.name.clone();
        du.waits.retain(|&(ki, w), _| met_waits.contains(&(name.clone(), ki, w)));
    }
    units.retain(|_, du| !du.waits.is_empty());
    if std::env::var_os("DUE_DEBUG").is_some() {
        for du in units.values() {
            for (&(ki, w), dw) in &du.waits {
                let held = dw.table.values().filter(|&&v| v).count();
                eprintln!("due-debug unit {} {:?} @{w} reads {:?} holds {held}/{}", du.name, du.keys[ki].0, dw.reads, dw.table.len());
            }
        }
    }
    counts.set("due: due facts (units with a due wait an input meets)", units.len() as i64);
    counts.set("due: due waits of those units", units.values().map(|u| u.waits.len() as i64).sum());
    // every record's due: writes
    for i in 0..n {
        let mut add: Vec<(String, Eff)> = Vec::new();
        for du in units.values() {
            if let Some(e) = due_effect(b, du, &b.records[i], &mut counts) {
                add.push((du.key(), e));
            }
        }
        if !add.is_empty() {
            counts.inc("due: records that write a due: fact");
            counts.add("due: due: writes", add.len() as i64);
        }
        b.records[i].effects.extend(add);
    }
    b.counts.update(&counts);
    b.due = Some(Due { units, counts });
}

/// The check of the `due:` writes against a direct evaluation: for every record and every unit with a `due:` fact,
/// every state the record's precondition allows over the facts the unit's due depends on (and the sources of their
/// writes), the value the record leaves in `due:<unit>` (its `Set`, the `Cases` case whose conditions hold, or a
/// guess) against `DueUnit::value` of the state after it, worked out from the record's effects one by one. Returns
/// (states checked, the first differences).
pub fn verify(b: &Builder, max_states: usize) -> (u64, Vec<String>) {
    let Some(due) = &b.due else { return (0, vec![]) };
    let mut checked = 0u64;
    let mut bad: Vec<String> = Vec::new();
    for r in &b.records {
        for du in due.units.values() {
            let key = du.key();
            let eff = r.effect(&key);
            let touched = r.effects.iter().any(|(k, _)| du.inputs.contains(k));
            if !touched {
                if eff.is_some() {
                    bad.push(format!("{}: writes {key} but none of its inputs", r.label));
                }
                continue;
            }
            let Some(eff) = eff else {
                bad.push(format!("{}: writes an input of {key} but not {key}", r.label));
                continue;
            };
            // the facts to enumerate: the unit's inputs and every source the record's writes of them read
            let mut keys: Vec<String> = du.inputs.clone();
            for k in &du.inputs {
                if let Some(e) = r.effect(k) {
                    for s in e.sources() {
                        if !keys.iter().any(|x| x == s) {
                            keys.push(s.to_string());
                        }
                    }
                }
            }
            if let Eff::Cases(cs) = eff {
                for c in cs {
                    for (k, _) in &c.0 {
                        if !keys.contains(k) {
                            keys.push(k.clone());
                        }
                    }
                }
            }
            let cands: Vec<Labels> = keys
                .iter()
                .map(|k| {
                    let all = candidates(b, k).unwrap_or_default();
                    match r.pre.get(k) {
                        Some(s) => all.into_iter().filter(|l| s.contains(l)).collect(),
                        None => all,
                    }
                })
                .collect();
            let states = product(&cands);
            for st in states.iter().take(max_states) {
                checked += 1;
                let pre = |k: &str| keys.iter().position(|x| x == k).map(|i| st[i]);
                // the values after the record, effect by effect
                let mut outs: Vec<Labels> = Vec::new();
                for k in &du.inputs {
                    let p = pre(k).unwrap();
                    outs.push(match r.effect(k) {
                        None => vec![p],
                        Some(Eff::Set(l)) => vec![*l],
                        Some(Eff::Guess(ls)) => ls.clone(),
                        Some(Eff::Add(d)) => b.domain(k).unwrap().add(p, *d),
                        Some(Eff::When(src, v)) => vec![if pre(src) == Some(Label::Int(1)) { Label::Int(*v) } else { p }],
                        Some(Eff::Copy(src)) => vec![pre(src).unwrap()],
                        Some(Eff::Cases(_)) => unreachable!(),
                    });
                }
                if outs.iter().any(|o| o.is_empty()) {
                    continue;
                }
                let want: BTreeSet<Label> = product(&outs)
                    .iter()
                    .map(|after| Label::Int(du.value(&|k: &str| du.inputs.iter().position(|x| x == k).map(|i| after[i])) as i64))
                    .collect();
                let got: BTreeSet<Label> = match eff {
                    Eff::Set(l) => [*l].into(),
                    Eff::Guess(ls) => ls.iter().copied().collect(),
                    Eff::Cases(cs) => {
                        let hold: Vec<&Case> = cs.iter().filter(|c| c.0.iter().all(|(k, s)| pre(k).is_some_and(|v| s.contains(&v)))).collect();
                        if hold.len() != 1 {
                            if bad.len() < 20 {
                                bad.push(format!("{}: {key}: {} cases hold in {:?}", r.label, hold.len(), st));
                            }
                            continue;
                        }
                        hold[0].1.iter().copied().collect()
                    }
                    other => {
                        bad.push(format!("{}: {key} written as {other:?}", r.label));
                        continue;
                    }
                };
                let ok = if got.len() == 1 { got == want } else { want.is_subset(&got) };
                if !ok && bad.len() < 20 {
                    bad.push(format!("{}: {key}: want {want:?}, the record writes {got:?}, in {:?} = {:?}", r.label, keys, st));
                }
            }
        }
    }
    (checked, bad)
}

/// F2's premise against the engine (due_engine.py's frames, run B's Part I frame by frame): for every frame N and
/// every slot live and unfrozen at a wait at N - 1's stop, the wait's class and, for a pure poll or a plain breakHere,
/// whether the rule says the slot is due (its test on N - 1's facts), against whether the engine's slot left that
/// wait in frame N (its `at:` key gone at N's stop: it ran on, or it ended). Returns the tallies, (class, due,
/// outcome, input frame or not) -> count, and the first examples of each disagreement.
pub fn engine_check(game: &Game, frames: &std::path::Path) -> (BTreeMap<String, u64>, Vec<String>) {
    let mut tally: BTreeMap<String, u64> = BTreeMap::new();
    let mut examples: Vec<String> = Vec::new();
    let mut state: FxHashMap<String, i64> = FxHashMap::default();
    let mut first = true;
    for line in crate::gz_lines(frames) {
        let v: serde_json::Value = serde_json::from_str(&line).unwrap();
        let frame = v["frame"].as_i64().unwrap();
        let prev = state.clone();
        if let Some(full) = v.get("facts") {
            state = full.as_object().unwrap().iter().map(|(k, x)| (k.clone(), x.as_i64().unwrap())).collect();
        } else {
            for (k, d) in v["diff"].as_object().unwrap() {
                let new = d[1].as_i64().unwrap();
                if new != 0 {
                    state.insert(k.clone(), new);
                } else {
                    state.remove(k);
                }
            }
        }
        if first {
            first = false;
            continue;
        }
        let input = !v["inputs"].as_array().unwrap().is_empty();
        let get = |m: &FxHashMap<String, i64>, k: &str| m.get(k).copied().unwrap_or(0);
        let res = get(&prev, "var:22") as i32; // VAR_ROOM_RESOURCE, vars.cpp:58
        let mut ats: Vec<&String> = prev.keys().filter(|k| k.starts_with("at:")).collect();
        ats.sort();
        for k in ats {
            if prev[k] != 1 {
                continue;
            }
            let (name, w) = k[3..].rsplit_once(':').unwrap();
            let w: i64 = w.parse().unwrap();
            if get(&prev, &format!("frozen:{name}")) != 0 {
                continue;
            }
            let key = match name.split_once(':') {
                Some(("script", n)) => UnitKey::Global(n.parse().unwrap()),
                Some(("local", n)) => UnitKey::Local(res, n.parse().unwrap()),
                Some(("obj", n)) => UnitKey::Obj(n.parse().unwrap()),
                None if name == "entry" => UnitKey::Entry(res),
                None if name == "exit" => UnitKey::Exit(res),
                _ => continue,
            };
            let Some(u) = game.id(key) else {
                *tally.entry("no such unit".into()).or_default() += 1;
                continue;
            };
            let Some(idx) = game.at(u, w as i32) else {
                *tally.entry("no instruction at the offset".into()).or_default() += 1;
                continue;
            };
            let class = wait_class(game, u, idx, CP);
            let due: Option<bool> = match class {
                WaitClass::Plain => Some(true),
                WaitClass::Polled(li) => {
                    let lp = &game.loops(u, CP)[li as usize];
                    if poll_shape(game, u, idx, lp).is_ok() {
                        exits(game, u, idx, lp, &|f| Some(get(&prev, &f.to_string()))).ok()
                    } else {
                        None
                    }
                }
                _ => None,
            };
            let slot = game.lifecycle_key(key).map(|f| f.to_string());
            let ended = slot.as_ref().is_some_and(|s| get(&state, s) == 0);
            let left = get(&state, k) != 1;
            let outcome = if !left {
                "stayed"
            } else if ended {
                "ended"
            } else {
                "ran on"
            };
            let cname = match (class, due) {
                (WaitClass::Polled(_), None) => "polled, not a pure poll".to_string(),
                (WaitClass::Time(why), _) => format!("time: {why}"),
                (c, _) => c.name().to_string(),
            };
            let d = match due {
                Some(true) => "due",
                Some(false) => "not due",
                None => "-",
            };
            let fr = if input { "input frame" } else { "other frame" };
            *tally.entry(format!("{cname} | {d} | {outcome} | {fr}")).or_default() += 1;
            let odd = (due == Some(true) && !left) || (due == Some(false) && left && !ended);
            if odd {
                // why the frame's turn went the other way: a fact the test reads changed in the frame (a slot
                // before this one in the frame wrote it), a freeze in the frame, Esc's abortCutscene (processInput,
                // before the scripts), or nothing the facts show
                let read_changed = match class {
                    WaitClass::Polled(li) => loop_facts(&game.loops(u, CP)[li as usize])
                        .iter()
                        .any(|f| get(&prev, &f.to_string()) != get(&state, &f.to_string())),
                    _ => false,
                };
                let esc = v["inputs"].as_array().unwrap().iter().any(|i| i.get("key").and_then(|k| k.as_str()) == Some("esc"));
                let why = if !left && get(&state, &format!("frozen:{name}")) != 0 {
                    "frozen in the frame"
                } else if read_changed {
                    "a fact the test reads changed in the frame"
                } else if esc {
                    "Esc: abortCutscene before the scripts"
                } else {
                    "unexplained"
                };
                *tally.entry(format!("disagreement: {cname} | {d} | {outcome} | {why}")).or_default() += 1;
            }
            if odd && examples.len() < 40 {
                let reads: Vec<String> = match class {
                    WaitClass::Polled(li) => loop_facts(&game.loops(u, CP)[li as usize])
                        .iter()
                        .map(|f| format!("{f}={}->{}", get(&prev, &f.to_string()), get(&state, &f.to_string())))
                        .collect(),
                    _ => vec![],
                };
                examples.push(format!("frame {frame}: {name}@{w} ({cname}, {d}) {outcome}; {} {reads:?}", game.cite(u, idx)));
            }
        }
    }
    (tally, examples)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The tree's cases are disjoint, cover every row, and give each row its result; set conditions merge.
    #[test]
    fn tree_cases() {
        let keys = vec!["pos:x".to_string(), "frozen:x".to_string(), "bit:1".to_string()];
        let (i, l) = (Label::Int, |v: i64| vec![Label::Int(v)]);
        let mut rows = Vec::new();
        for pos in [-1, 5, 9] {
            for fr in [0, 1] {
                for bit in [0, 1] {
                    let due = pos == 5 && fr == 0 && bit == 1 || pos == 9 && fr == 0;
                    rows.push((vec![i(pos), i(fr), i(bit)], l(due as i64)));
                }
            }
        }
        let mut out = Vec::new();
        tree(&keys, &rows, 0, &mut Vec::new(), &mut out);
        for (st, res) in &rows {
            let hold: Vec<&Case> =
                out.iter().filter(|c| c.0.iter().all(|(k, s)| s.contains(&st[keys.iter().position(|x| x == k).unwrap()]))).collect();
            assert_eq!(hold.len(), 1, "{st:?}: {out:?}");
            assert_eq!(&hold[0].1, res);
        }
        // pos -1 alone is a leaf: due 0 whatever the rest
        assert!(out.iter().any(|c| c.0 == vec![("pos:x".to_string(), vec![i(-1)])] && c.1 == l(0)));
        // one result for every row: one case with no condition
        let same: Vec<(Vec<Label>, Labels)> = rows.iter().map(|r| (r.0.clone(), l(0))).collect();
        let mut out = Vec::new();
        tree(&keys, &same, 0, &mut Vec::new(), &mut out);
        assert_eq!(out, vec![(vec![], l(0))]);
    }

    #[test]
    fn product_order() {
        let p = product(&[vec![Label::Int(1), Label::Int(2)], vec![Label::Int(3)], vec![Label::Above, Label::Below]]);
        assert_eq!(p.len(), 4);
        assert_eq!(p[0], vec![Label::Int(1), Label::Int(3), Label::Above]);
        assert_eq!(p[3], vec![Label::Int(2), Label::Int(3), Label::Below]);
    }
}

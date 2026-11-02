//! strict.py (window-search.md section 9): B1's strict matcher over the reading's event records (`StrictMatcher`,
//! trace_explain's), one plan's strict parts (`Plan`), one branch's strict matches (`Track`), an input record at
//! s + 1 (`input_match`, `decide_on_log`), the parts of a run that passed a wait (`parts_of`, `Parter`,
//! `part_verdicts`), a branch's kept frames (`Kept`), and B1's explanation of a failure (`explain`, `short`).

use crate::check;
use crate::code::*;
use crate::pyj::{self, Py, J};
use crate::reading::{B1Eff, Check, PlayReading, B1};
use crate::recs::{Kind, MRec};
use crate::replay::{self, CheckRec};
use crate::val::*;
use fmi_core::*;
use rustc_hash::{FxHashMap, FxHashSet};
use std::collections::BTreeMap;
use std::sync::Arc;

pub const KEEP_FRAMES: u64 = 3;
pub const PART_STEP: i64 = 1; // in SEQ_SCALE units: 0.001

/// strict.cite_unit: (unit, code offset) of a check's cite.
pub fn cite_unit(cite: &str) -> Option<(Unit, i64)> {
    let (path, rest) = cite.rsplit_once(':')?;
    let (line, off) = rest.split_once(' ')?;
    if line.is_empty() || !line.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let off = off.strip_prefix('[')?.strip_suffix(']')?;
    if off.is_empty() || !off.bytes().all(|b| b.is_ascii_digit() || (b'A'..=b'F').contains(&b)) {
        return None;
    }
    let off = i64::from_str_radix(off, 16).ok()?;
    let file = path.strip_suffix(".txt")?;
    let digits = |s: &str| -> Option<u64> {
        if !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()) {
            s.parse().ok()
        } else {
            None
        }
    };
    if let Some(n) = file.strip_prefix("global/script-") {
        return Some((Unit(U_SCRIPT, digits(n)?), off));
    }
    let (dir, name) = file.split_once('/')?;
    if dir.is_empty() || name.contains('/') {
        return None;
    }
    if let Some(n) = name.strip_prefix("local-") {
        return Some((Unit(U_LOCAL, digits(n)?), off));
    }
    if name == "entry" {
        return Some((Unit(U_ENTRY, 0), off));
    }
    if name == "exit" {
        return Some((Unit(U_EXIT, 0), off));
    }
    if let Some(n) = name.strip_prefix("obj-") {
        let d: String = n.chars().take_while(|c| c.is_ascii_digit()).collect();
        return Some((Unit(U_OBJ, d.parse().ok()?), off));
    }
    None
}

/// decide_on_log's answer: no logged run of the check's unit, or the side (None: the log does not tell).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LogDec {
    NoLog,
    Got(Option<bool>),
}

/// strict.decide_on_log: M3 on the engine's own opcodes.
pub fn decide_on_log(code: &Code, ex: &Explained, runs: &[&RunR], check: &Check) -> LogDec {
    let Some((unit, off)) = cite_unit(&check.cite) else {
        return LogDec::NoLog;
    };
    let Some(side) = check.side else { return LogDec::NoLog };
    for r in runs {
        if r.unit != Some(unit) {
            continue;
        }
        let Some(lg) = ex.runs.get(&r.seq) else { continue };
        let Some(key) = code.key(lg.where_, lg.number, lg.room) else {
            return LogDec::Got(None);
        };
        let steps = steps_of(code, key, &[lg]);
        for (n, st) in steps.iter().enumerate() {
            if st.op.code != off || st.i.is_none() {
                continue;
            }
            let x = &code.ins(key)[st.i.unwrap() as usize];
            if Code::is_cond_jump(x) {
                let sides = taken(code, key, &steps, n);
                return match sides {
                    Some(s) if x.next != x.target => LogDec::Got(Some((s.0 == x.next) == side)),
                    _ => LogDec::Got(None),
                };
            }
            if x.yields == Some(scumm_ir::ast::Yield::Wait) && check.cond.ends_with(" blocks") {
                if steps.get(n + 1).is_some() {
                    return LogDec::Got(Some(!side));
                }
                return LogDec::Got(if lg.code1 == Some(off) { Some(side) } else { None });
            }
            return LogDec::Got(None);
        }
        return LogDec::Got(None);
    }
    LogDec::NoLog // (the loop returns at the first logged run of the unit)
}

// --- the strict matcher (trace_explain.StrictMatcher) ----------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct MEntry {
    pub idx: u32,
    pub pre: Vec<(Key, Vec<Val>)>,
    pub others: Vec<(Key, B1Eff)>,
    pub checks: Arc<Vec<Check>>,
    pub pos: Option<B1Eff>,
}

type IKey = (Unit, i64, Option<i64>);

/// trace_explain.StrictMatcher over the plan's units' event records.
pub struct StrictMatcher {
    pub index: FxHashMap<IKey, Vec<Arc<MEntry>>>,
    pub points: FxHashSet<(Unit, i64)>,
    pub keys: FxHashMap<IKey, FxHashSet<Key>>,
    pub by_args: FxHashMap<IKey, (FxHashMap<Val, Vec<Arc<MEntry>>>, Vec<Arc<MEntry>>)>,
}

/// A verdict (trace_explain.Verdict): explained (one record), ambiguous (several), unexplained, unplaced.
#[derive(Clone, Debug)]
pub struct Verdict {
    pub run: RunR,
    pub status: &'static str,
    pub record: Vec<u32>,
    pub why: Vec<(Option<u32>, String)>,
}

/// `got` holds a precondition's set (a set of lists holds when one of its values is in it).
#[inline]
pub fn holds_v(got: &VV, vs: &[Val]) -> bool {
    match got {
        VV::Set(s) => s.iter().any(|x| vs.contains(x)),
        VV::V(x) => vs.contains(x),
    }
}

/// StrictMatcher._value_ok
pub fn value_ok(eff: &B1Eff, got: &VV) -> bool {
    match got {
        VV::Set(s) => match eff {
            B1Eff::Set(v) => s.contains(v),
            B1Eff::Guess(vs) => s.iter().any(|x| vs.contains(x)),
            _ => true,
        },
        VV::V(x) => match eff {
            B1Eff::Set(v) => x == v,
            B1Eff::Guess(vs) => vs.contains(x),
            _ => true,
        },
    }
}

impl StrictMatcher {
    pub fn new(records: &[Arc<B1>]) -> StrictMatcher {
        let mut index: FxHashMap<IKey, Vec<Arc<MEntry>>> = FxHashMap::default();
        let mut order: Vec<IKey> = Vec::new();
        let mut points = FxHashSet::default();
        for rec in records {
            let (Some(unit), Some(w)) = (rec.unit, rec.wp) else {
                continue;
            };
            let pos = rec.eff(unit.pos()).cloned();
            let end = match &pos {
                Some(B1Eff::Set(v)) => Some(*v),
                _ => None,
            };
            let others: Vec<(Key, B1Eff)> = rec
                .eff
                .iter()
                .filter(|(k, e)| k.kind() != K_POS && matches!(e, B1Eff::Set(_) | B1Eff::Guess(_)))
                .cloned()
                .collect();
            let k = (unit, w, end);
            if !index.contains_key(&k) {
                order.push(k);
            }
            index.entry(k).or_default().push(Arc::new(MEntry {
                idx: rec.idx,
                pre: rec.pre.clone(),
                others,
                checks: rec.checks.clone(),
                pos,
            }));
            points.insert((unit, w));
        }
        let keys = index
            .iter()
            .map(|(k, v)| (*k, v.iter().flat_map(|r| r.others.iter().map(|o| o.0)).collect()))
            .collect();
        let mut by_args = FxHashMap::default();
        for (k, recs) in &index {
            let ak = k.0.args();
            let mut sub: FxHashMap<Val, Vec<Arc<MEntry>>> = FxHashMap::default();
            let mut rest = Vec::new();
            for rec in recs {
                match rec.pre.iter().find(|p| p.0 == ak) {
                    None => rest.push(rec.clone()),
                    Some((_, vs)) => {
                        // (Python iterates the set of values; each list gets the record once)
                        for &sig in vs {
                            sub.entry(sig).or_default().push(rec.clone());
                        }
                    }
                }
            }
            by_args.insert(*k, (sub, rest));
        }
        StrictMatcher {
            index,
            points,
            keys,
            by_args,
        }
    }

    /// StrictMatcher.match with a branch's check decisions.
    pub fn match_runs(
        &self,
        runs: &[RunR],
        before: &View,
        after: &View,
        open_keys: &FxHashSet<Key>,
        same_unit: bool,
        decide: &mut dyn FnMut(&Check, &RunR) -> Option<bool>,
    ) -> Vec<Verdict> {
        let mut state: std::borrow::Cow<View> = std::borrow::Cow::Borrowed(before);
        let entries: Vec<(IKey, IKey)> = runs
            .iter()
            .map(|r| {
                let end = if r.code1 >= 0 { r.code1 } else { NOWHERE };
                let u = r.unit.unwrap_or(Unit(0, 0));
                ((u, r.code0, Some(end)), (u, r.code0, None))
            })
            .collect();
        let later_writes = |n: usize, k: Key| -> bool {
            for (a2, b2) in &entries[n + 1..] {
                if self.keys.get(a2).is_some_and(|s| s.contains(&k))
                    || self.keys.get(b2).is_some_and(|s| s.contains(&k))
                {
                    return true;
                }
                if same_unit && a2.0 == entries[n].0 .0 && k == a2.0.args() {
                    return true;
                }
            }
            false
        };
        let after_in = |k: Key| -> Option<&VV> { after.m.get(&k).filter(|v| !v.is_none()) };
        let mut out = Vec::with_capacity(runs.len());
        for (n, r) in runs.iter().enumerate() {
            let (a, b) = entries[n];
            let Some(u) = r.unit else {
                out.push(Verdict {
                    run: *r,
                    status: "unplaced",
                    record: vec![],
                    why: vec![],
                });
                continue;
            };
            if !self.points.contains(&(u, r.code0)) {
                out.push(Verdict {
                    run: *r,
                    status: "unplaced",
                    record: vec![],
                    why: vec![],
                });
                continue;
            }
            if !self.index.contains_key(&a) && !self.index.contains_key(&b) {
                out.push(Verdict {
                    run: *r,
                    status: "unexplained",
                    record: vec![],
                    why: vec![(None, format!("end pos:{}", u.text()))],
                });
                continue;
            }
            let ak = u.args();
            let sigs: Option<VV> = if open_keys.contains(&ak) {
                None
            } else {
                Some(state.get(ak).clone())
            };
            let sigs = sigs.filter(|s| !s.is_none());
            let mut groups: Vec<Vec<Arc<MEntry>>> = Vec::new();
            for e in [a, b] {
                let Some(ix) = self.index.get(&e) else { continue };
                match &sigs {
                    None => groups.push(ix.clone()),
                    Some(sv) => {
                        let (sub, rest) = &self.by_args[&e];
                        let mut seen = FxHashSet::default();
                        let mut pick = rest.clone();
                        let list: Vec<Val> = match sv {
                            VV::Set(s) => s.to_vec(),
                            VV::V(x) => vec![*x],
                        };
                        for sig in list {
                            for rec in sub.get(&sig).map(|v| v.as_slice()).unwrap_or(&[]) {
                                if seen.insert(rec.idx) {
                                    pick.push(rec.clone());
                                }
                            }
                        }
                        groups.push(pick);
                    }
                }
            }
            let mut ok: Vec<Arc<MEntry>> = Vec::new();
            let mut why: Vec<(Option<u32>, String)> = Vec::new();
            if groups.iter().all(|g| g.is_empty()) {
                why.push((None, format!("pre {}", ak.text())));
            }
            let end = if r.code1 >= 0 { r.code1 } else { NOWHERE };
            for group in &groups {
                for rec in group {
                    if let Some((k, _)) = rec
                        .pre
                        .iter()
                        .find(|(k, vs)| !holds_v(state.get(*k), vs) && !open_keys.contains(k))
                    {
                        why.push((Some(rec.idx), format!("pre {}", k.text())));
                        continue;
                    }
                    if let Some(B1Eff::Guess(vs)) = &rec.pos {
                        if !vs.contains(&end) {
                            why.push((Some(rec.idx), format!("end pos:{}", u.text())));
                            continue;
                        }
                    }
                    if let Some((k, _)) = rec
                        .others
                        .iter()
                        .find(|(k, e)| after_in(*k).is_some_and(|g| !value_ok(e, g)) && !later_writes(n, *k))
                    {
                        why.push((Some(rec.idx), format!("effect {}", k.text())));
                        continue;
                    }
                    if !rec.checks.is_empty() {
                        if let Some(c) = rec.checks.iter().find(|c| decide(c, r) == Some(false)) {
                            why.push((Some(rec.idx), format!("check {}", c.cite)));
                            continue;
                        }
                    }
                    ok.push(rec.clone());
                }
            }
            let v = if ok.len() == 1 {
                Verdict {
                    run: *r,
                    status: "explained",
                    record: vec![ok[0].idx],
                    why: why.clone(),
                }
            } else {
                Verdict {
                    run: *r,
                    status: if ok.is_empty() { "unexplained" } else { "ambiguous" },
                    record: ok.iter().map(|o| o.idx).collect(),
                    why: why.clone(),
                }
            };
            out.push(v);
            if !ok.is_empty() {
                let sets: Vec<Vec<(Key, Val)>> = ok
                    .iter()
                    .map(|o| {
                        o.others
                            .iter()
                            .filter_map(|(k, e)| if let B1Eff::Set(v) = e { Some((*k, *v)) } else { None })
                            .collect()
                    })
                    .collect();
                let mut common: Vec<(Key, Val)> = Vec::new();
                for &(k, v) in &sets[0] {
                    // (a dict keeps one value per key: the last set of a key)
                    let v0 = sets[0].iter().rev().find(|p| p.0 == k).unwrap().1;
                    if v0 != v {
                        continue;
                    }
                    if sets[1..]
                        .iter()
                        .all(|s| s.iter().rev().find(|p| p.0 == k).map(|p| p.1) == Some(v))
                    {
                        common.push((k, v));
                    }
                }
                if same_unit {
                    common.push((u.pos(), end));
                }
                if !common.is_empty() {
                    let st = state.to_mut();
                    for (k, v) in common {
                        st.setv(k, v);
                    }
                }
            }
        }
        out
    }
}

// --- one plan --------------------------------------------------------------------------------------------------------

/// strict.Plan: one plan's strict parts.
pub struct Plan {
    pub units: Vec<Unit>,
    pub idx: FxHashMap<usize, Vec<Option<u32>>>,
    pub matcher: StrictMatcher,
    pub unit_set: FxHashSet<Unit>,
    pub prev_writer: FxHashMap<usize, Option<usize>>,
    pub args_units: Vec<Unit>,
    /// the event records (MRec.i), in plan order
    pub events: Vec<usize>,
}

impl Plan {
    pub fn new(rd: &PlayReading, recs: &[MRec]) -> Plan {
        let mut units: Vec<Unit> = recs
            .iter()
            .filter(|r| r.kind == Kind::Event)
            .filter_map(|r| r.unit)
            .collect();
        units.sort_by_cached_key(|u| u.text());
        units.dedup();
        let idx = recs
            .iter()
            .map(|r| (r.i, r.paths.iter().map(|a| rd.src.by_name(&a.name)).collect()))
            .collect();
        let (_, by_unit) = rd.events();
        let records: Vec<Arc<B1>> = units
            .iter()
            .flat_map(|u| by_unit.get(u).map(|v| v.as_slice()).unwrap_or(&[]).iter())
            .map(|&i| rd.b1(i))
            .collect();
        let matcher = StrictMatcher::new(&records);
        let mut prev_writer = FxHashMap::default();
        for r in recs {
            if r.kind != Kind::Event {
                continue;
            }
            let key = r.unit.map(|u| u.pos());
            let prev = recs
                .iter()
                .rev()
                .find(|q| q.i < r.i && key.is_some_and(|k| q.writes.contains(&k)));
            prev_writer.insert(r.i, prev.map(|q| q.i));
        }
        let mut au: Vec<Unit> = recs
            .iter()
            .flat_map(|r| r.reads.iter().chain(r.writes.iter()))
            .filter(|k| k.kind() == K_ARGS)
            .map(|k| Unit::of_key(*k))
            .collect();
        au.sort_by_cached_key(|u| u.text());
        au.dedup();
        Plan {
            unit_set: units.iter().copied().collect(),
            units,
            idx,
            matcher,
            prev_writer,
            args_units: au,
            events: recs.iter().filter(|r| r.kind == Kind::Event).map(|r| r.i).collect(),
        }
    }
}

// --- one branch ------------------------------------------------------------------------------------------------------

/// strict.Track: one branch's strict matches.
#[derive(Clone, Debug, Default)]
pub struct Track {
    /// MRec.i -> (frame, path, seq of its run), in the order matched
    pub matched: Vec<(usize, (u64, usize, Seq))>,
    pub refuted: Vec<(usize, (u64, String, RunR))>,
    pub choices: Vec<(usize, Vec<(Key, VV)>)>,
    pub unit_seq: FxHashMap<usize, FxHashMap<Unit, Seq>>,
    pub first_seq: FxHashMap<usize, Seq>,
    pub outside: Vec<(usize, Vec<(Key, Option<i64>)>)>,
}

impl Track {
    pub fn m(&self, i: usize) -> Option<&(u64, usize, Seq)> {
        self.matched.iter().find(|x| x.0 == i).map(|x| &x.1)
    }
    pub fn is_matched(&self, i: usize) -> bool {
        self.matched.iter().any(|x| x.0 == i)
    }
    pub fn r(&self, i: usize) -> Option<&(u64, String, RunR)> {
        self.refuted.iter().find(|x| x.0 == i).map(|x| &x.1)
    }
    pub fn is_refuted(&self, i: usize) -> bool {
        self.refuted.iter().any(|x| x.0 == i)
    }
    pub fn set_matched(&mut self, i: usize, v: (u64, usize, Seq)) {
        match self.matched.iter_mut().find(|x| x.0 == i) {
            Some(x) => x.1 = v,
            None => self.matched.push((i, v)),
        }
    }

    /// Track.armed: the seq after which a run of r's unit from its wait point is r's own.
    pub fn armed(&self, plan: &Plan, r: &MRec) -> Option<Seq> {
        let Some(prev) = plan.prev_writer.get(&r.i).copied().flatten() else {
            return Some(SEQ_BEFORE);
        };
        let m = self.m(prev)?;
        Some(
            self.unit_seq
                .get(&prev)
                .and_then(|u| r.unit.and_then(|un| u.get(&un).copied()))
                .unwrap_or(m.2),
        )
    }

    /// Track.verdicts: B1's verdicts on R_k's runs of the plan's units.
    #[allow(clippy::too_many_arguments)]
    pub fn verdicts(
        &self,
        plan: &Plan,
        code: &Code,
        runs: &[RunR],
        before: &View,
        after: &View,
        dump_before: Option<&Obs>,
        log: &mut BranchLog,
    ) -> Vec<(RunR, Verdict)> {
        let mine = plan_runs(plan, code, runs, log);
        if mine.is_empty() {
            return vec![];
        }
        let open = open_keys_of(runs, before, after);
        let ex = &log.kept.ex;
        let mut decide = |c: &Check, r: &RunR| matcher_decide(code, ex, dump_before, c, r);
        let vs = plan
            .matcher
            .match_runs(&mine, before, after, &open, log.parter.is_some(), &mut decide);
        vs.into_iter().map(|v| (v.run, v)).collect()
    }

    /// Track.apply: Definition S for one run.
    pub fn apply(
        &mut self,
        plan: &Plan,
        recs: &[MRec],
        rd: &PlayReading,
        k: u64,
        run: &RunR,
        v: &Verdict,
        after: &View,
        dump: Option<&Obs>,
    ) -> Vec<(usize, &'static str)> {
        let mut out = Vec::new();
        for &ri in &plan.events {
            let r = &recs[ri];
            if self.is_matched(ri) || self.is_refuted(ri) || r.unit != run.unit || r.wait != Some(run.code0) {
                continue;
            }
            let Some(at) = self.armed(plan, r) else { continue };
            if run.seq <= at {
                continue;
            }
            let ok: Vec<u32> = match v.status {
                "explained" | "ambiguous" => v.record.clone(),
                _ => vec![],
            };
            let idx = &plan.idx[&ri];
            let path = idx.iter().position(|i| i.is_some_and(|x| ok.contains(&x)));
            let late = r.preds.iter().any(|&p| {
                plan.idx.contains_key(&p)
                    && match self.m(p) {
                        None => true,
                        Some(m) => self.first_seq.get(&p).copied().unwrap_or(m.2) >= run.seq,
                    }
            });
            if let (Some(path), false) = (path, late) {
                self.set_matched(ri, (k, path, run.seq));
                let ch: Vec<(Key, VV)> = r.exps[path]
                    .iter()
                    .filter(|(kk, x)| {
                        let a = after.get(*kk);
                        x.kind == "guess" && x.chosen.is_some() && x.holds(a) && *a != VV::V(x.chosen.unwrap())
                    })
                    .map(|(kk, _)| (*kk, after.get(*kk).clone()))
                    .collect();
                if !ch.is_empty() {
                    self.choices.retain(|c| c.0 != ri);
                    self.choices.push((ri, ch));
                }
                let od = outside_of(&rd.b1(idx[path].unwrap()).eff, after, dump);
                if !od.is_empty() {
                    self.outside.retain(|c| c.0 != ri);
                    self.outside.push((ri, od));
                }
                out.push((ri, "matched"));
            } else {
                let why = if path.is_some() {
                    "order: a record it follows had not matched".to_string()
                } else {
                    let mine: Vec<&String> = v.why.iter().filter(|(i, _)| idx.contains(i)).map(|w| &w.1).collect();
                    mine.first()
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| format!("end pos:{}", r.unit.map(|u| u.text()).unwrap_or_default()))
                };
                self.refuted.push((ri, (k, why, *run)));
                out.push((ri, "refuted"));
            }
            break;
        }
        out
    }
}

/// _MatcherView's decide: M3 on the run's own opcodes where the log has them, else on the dump before.
pub fn matcher_decide(code: &Code, ex: &Explained, dump_before: Option<&Obs>, c: &Check, r: &RunR) -> Option<bool> {
    match decide_on_log(code, ex, &[r], c) {
        LogDec::Got(g) => g,
        LogDec::NoLog => {
            let d = dump_before?;
            let src = [J::Str("event".into())];
            replay::eval_check(
                c,
                &CheckRec {
                    kind: "event",
                    src: &src,
                },
                d,
            )
        }
    }
}

/// strict.is_stutter
pub fn is_stutter(code: &Code, r: &RunR) -> bool {
    if r.ops != 1 || r.code1 != r.code0 {
        return false;
    }
    let Some(key) = code.key(r.where_, r.number, r.room) else {
        return false;
    };
    code.index(key, r.code0)
        .is_some_and(|i| code.ins(key)[i as usize].yields == Some(scumm_ir::ast::Yield::Wait))
}

/// strict.open_keys_of
pub fn open_keys_of(runs: &[RunR], before: &View, after: &View) -> FxHashSet<Key> {
    let mut out = FxHashSet::default();
    if !runs.iter().any(|r| r.after) {
        return out;
    }
    for (k, v) in before.m.iter() {
        if after.get(*k) != v {
            out.insert(*k);
        }
    }
    for (k, v) in after.m.iter() {
        if before.get(*k) != v {
            out.insert(*k);
        }
    }
    for r in runs {
        if let Some(u) = r.unit {
            out.insert(u.pos());
            out.insert(u.args());
            out.insert(Key::pack(K_FROZEN, u.0, u.1, 0));
            match u.0 {
                // "script:" + "script:N" etc. are no keys; facts.py's slot keys of the unit itself are
                U_SCRIPT => {}
                _ => {}
            }
        }
    }
    out
}

/// strict.outside_of: the effects whose engine value lies outside the fact's domain.
pub fn outside_of(eff: &[(Key, B1Eff)], view: &View, dump: Option<&Obs>) -> Vec<(Key, Option<i64>)> {
    let keys: Vec<Key> = eff
        .iter()
        .filter(|(k, e)| matches!(e, B1Eff::Set(_) | B1Eff::Guess(_)) && view.contains(*k) && view.get(*k).is_none())
        .map(|p| p.0)
        .collect();
    if keys.is_empty() {
        return vec![];
    }
    let obs = dump.map(facts_of);
    keys.into_iter()
        .map(|k| (k, obs.as_ref().and_then(|o| o.get(k))))
        .collect()
}

/// strict.plan_runs: the plan's units' runs at depth 0 from a wait point, stutters left out; with parts, a run that
/// passed a wait as its parts.
pub fn plan_runs(plan: &Plan, code: &Code, runs: &[RunR], log: &mut BranchLog) -> Vec<RunR> {
    let mine: Vec<RunR> = runs
        .iter()
        .filter(|r| {
            r.depth == 0 && r.code0 >= 0 && r.unit.is_some_and(|u| plan.unit_set.contains(&u)) && !is_stutter(code, r)
        })
        .copied()
        .collect();
    if log.parter.is_none() {
        return mine;
    }
    let mut out = Vec::with_capacity(mine.len());
    for r in mine {
        match log.parts(code, &r) {
            Some(ps) => out.extend(ps),
            None => out.push(r),
        }
    }
    out
}

/// strict.input_match's result
#[derive(Clone, Debug, Default)]
pub struct InMatch {
    pub ok: bool,
    pub took: bool,
    pub why: Option<String>,
    pub run: Option<RunR>,
    pub engine: J,
    pub want: Option<Val>,
    pub first_seq: Seq,
    pub last_seq: Seq,
    pub outside: Vec<(Key, Option<i64>)>,
    /// unit -> the click's run's parts (9.9)
    pub split: Vec<(Unit, Vec<RunR>)>,
    pub has_want: bool,
    /// set by the child: the frame it was decided, and its view
    pub frame: u64,
    pub view: Option<Arc<View>>,
}

fn show_vv(v: &VV) -> J {
    match v {
        VV::Set(s) => J::Arr(s.iter().take(4).map(|&x| vj(x)).collect()),
        VV::V(x) => vj(*x),
    }
}

/// strict.input_match: (I1)-(I4) and "took effect" for an input record at s + 1.
#[allow(clippy::too_many_arguments)]
pub fn input_match(
    rd: &PlayReading,
    plan: &Plan,
    code: &Code,
    idx: u32,
    v0: &View,
    d0: Option<&Obs>,
    applied: &[(Key, Val)],
    own: &[(Unit, RunR)],
    vk: &View,
    later: &[RunR],
    decide: &mut dyn FnMut(&Check, &mut BranchLog) -> LogDec,
    dk: Option<&Obs>,
    log: &mut BranchLog,
) -> InMatch {
    let b1 = rd.b1(idx);
    let state: std::borrow::Cow<View> = if applied.is_empty() {
        std::borrow::Cow::Borrowed(v0)
    } else {
        let mut s = v0.clone();
        for &(k, v) in applied {
            s.setv(k, v);
        }
        std::borrow::Cow::Owned(s)
    };
    if let Some((k, _)) = b1.pre.iter().find(|(k, vs)| !holds_v(state.get(*k), vs)) {
        return InMatch {
            ok: false,
            why: Some(format!("pre {}", k.text())),
            engine: show_vv(state.get(*k)),
            ..Default::default()
        };
    }
    let mut changed = false;
    let mut split: Vec<(Unit, Vec<RunR>)> = Vec::new();
    let own_of = |u: Unit| own.iter().find(|o| o.0 == u).map(|o| o.1);
    for (k, e) in &b1.eff {
        let B1Eff::Set(want) = e else { continue };
        if k.kind() != K_POS {
            continue;
        }
        let u = Unit::of_key(*k);
        let whole = own_of(u);
        let mut r = whole;
        if let Some(w) = whole {
            if log.parter.is_some() {
                if let Some(ps) = log.parts(code, &w) {
                    r = Some(ps[0]);
                    split.push((u, ps));
                }
            }
        }
        let Some(r) = r else {
            if *want == NOWHERE {
                if vk.val(*k) != NOWHERE {
                    return InMatch {
                        ok: false,
                        why: Some(format!("end {}", k.text())),
                        engine: vk.get(*k).j(),
                        want: Some(*want),
                        has_want: true,
                        ..Default::default()
                    };
                }
                changed = changed || v0.val(*k) != NOWHERE;
                continue;
            }
            return InMatch {
                ok: false,
                why: Some(format!("no run {}", k.text())),
                engine: J::Null,
                want: Some(*want),
                has_want: true,
                ..Default::default()
            };
        };
        let end = if r.code1 >= 0 { r.code1 } else { NOWHERE };
        if end != *want {
            return InMatch {
                ok: false,
                why: Some(format!("end {}", k.text())),
                run: whole,
                engine: J::Int(end),
                want: Some(*want),
                has_want: true,
                ..Default::default()
            };
        }
        if *v0.get(*k) != VV::V(end) {
            changed = true;
        }
    }
    let seqs: Vec<Seq> = own.iter().map(|o| o.1.seq).collect();
    let mut mine: Vec<RunR> = own.iter().map(|o| o.1).collect();
    mine.sort_by_key(|r| r.seq);
    let mut later_all: Vec<RunR> = Vec::new();
    if !split.is_empty() {
        for (_, ps) in &split {
            later_all.extend(ps[1..].iter().copied());
        }
    }
    later_all.extend(later.iter().copied());
    for (k, e) in &b1.eff {
        if k.kind() == K_POS || !matches!(e, B1Eff::Set(_) | B1Eff::Guess(_)) {
            continue;
        }
        let got = vk.get(*k);
        if got.is_none() {
            continue;
        }
        if !value_ok(e, got) {
            let written_later = later_all.iter().any(|r| {
                let u = r.unit.unwrap_or(Unit(0, 0));
                let end = if r.code1 >= 0 { r.code1 } else { NOWHERE };
                plan.matcher
                    .keys
                    .get(&(u, r.code0, Some(end)))
                    .is_some_and(|s| s.contains(k))
                    || plan
                        .matcher
                        .keys
                        .get(&(u, r.code0, None))
                        .is_some_and(|s| s.contains(k))
            });
            if written_later {
                continue;
            }
            if k.kind() == K_ARGS && split.iter().any(|s| s.0 == Unit::of_key(*k)) {
                continue;
            }
            return InMatch {
                ok: false,
                why: Some(format!("effect {}", k.text())),
                run: mine.last().copied(),
                engine: show_vv(got),
                ..Default::default()
            };
        }
        if !value_ok(e, v0.get(*k)) {
            changed = true;
        }
    }
    let what = rd.rec(idx);
    for c in b1.checks.iter() {
        let mut got = decide(c, log);
        if got == LogDec::NoLog {
            got = LogDec::Got(d0.and_then(|d| {
                replay::eval_check(
                    c,
                    &CheckRec {
                        kind: &what.kind,
                        src: &what.src,
                    },
                    d,
                )
            }));
        }
        if got == LogDec::Got(Some(false)) {
            return InMatch {
                ok: false,
                why: Some(format!("check {}", c.cite)),
                engine: J::Null,
                ..Default::default()
            };
        }
    }
    InMatch {
        ok: true,
        took: changed,
        why: None,
        run: None,
        engine: J::Null,
        first_seq: seqs.iter().copied().min().unwrap_or(SEQ_BEFORE),
        last_seq: seqs.iter().copied().max().unwrap_or(SEQ_BEFORE),
        outside: outside_of(&b1.eff, vk, dk),
        split,
        ..Default::default()
    }
}

// --- a run that passed a wait (9.9) --------------------------------------------------------------------------------

/// strict.parts_of (Lemma P)
pub fn parts_of(code: &Code, lg: &Logged, resumed: bool) -> Option<Vec<Logged>> {
    let key = code.key(lg.where_, lg.number, lg.room)?;
    let ins = code.ins(key);
    let first = lg.ops.iter().position(|op| op.code >= 0);
    let mut cuts = Vec::new();
    for (n, op) in lg.ops.iter().enumerate() {
        if op.code < 0 || n + 1 >= lg.ops.len() {
            continue;
        }
        if resumed && Some(n) == first && op.code == lg.code0 {
            continue;
        }
        if let Some(i) = code.index(key, op.code) {
            if ins[i as usize].yields == Some(scumm_ir::ast::Yield::Wait) {
                cuts.push(n);
            }
        }
    }
    if cuts.is_empty() || cuts.len() + 1 >= 1000 {
        return None;
    }
    let mut bounds = vec![0];
    bounds.extend(cuts.iter().copied());
    bounds.push(lg.ops.len());
    let mut out = Vec::new();
    for j in 0..bounds.len() - 1 {
        let (a, b) = (bounds[j], bounds[j + 1]);
        let last = j == bounds.len() - 2;
        out.push(Logged {
            seq: lg.seq + (j as i64 + 1) * PART_STEP,
            frame: lg.frame,
            after: lg.after,
            slot: lg.slot,
            where_: lg.where_,
            number: lg.number,
            room: lg.room,
            code0: if j == 0 { lg.code0 } else { lg.ops[a].code },
            ops: lg.ops[a..b].to_vec(),
            code1: if last { lg.code1 } else { Some(lg.ops[b].code) },
            status: if last { lg.status } else { Some(SS_RUNNING) },
            frozen: lg.frozen,
            delay: if last { lg.delay } else { Some(0) },
        });
    }
    Some(out)
}

/// strict.part_entries
pub fn part_entries(lg: &Logged) -> Vec<Ent> {
    let mut out = vec![Ent::B {
        seq: lg.seq,
        frame: lg.frame,
        after: lg.after,
        slot: lg.slot,
        where_: lg.where_,
        number: lg.number,
        room: lg.room,
        code0: lg.code0,
    }];
    for op in &lg.ops {
        out.push(Ent::O {
            seq: lg.seq,
            code: op.code,
            opcode: op.opcode,
        });
        out.extend(op.reads.iter().map(|&(v, x)| Ent::R {
            seq: lg.seq,
            var: v,
            value: x,
        }));
        out.extend(op.writes.iter().map(|&(v, x)| Ent::W {
            seq: lg.seq,
            var: v,
            value: x,
        }));
    }
    out.push(Ent::E {
        seq: lg.seq,
        code1: lg.code1.unwrap_or(-1),
        status: lg.status.unwrap_or(0),
        frozen: lg.frozen.unwrap_or(0),
        delay: lg.delay.unwrap_or(0),
    });
    out
}

/// strict.Parter's state: run seq -> its parts (None: not split), the splits counted.
#[derive(Clone, Default)]
pub struct Parter {
    pub cache: indexmap::IndexMap<Seq, Option<Vec<RunR>>>,
    pub splits: u64,
}

/// A branch's kept frames and its parts (strict.Kept and strict.Parter).
#[derive(Clone, Default)]
pub struct BranchLog {
    pub kept: Kept,
    pub parter: Option<Parter>,
}

impl BranchLog {
    pub fn new(split_wait: bool) -> BranchLog {
        BranchLog {
            kept: Kept::default(),
            parter: if split_wait { Some(Parter::default()) } else { None },
        }
    }
    /// Parter.__call__
    pub fn parts(&mut self, code: &Code, r: &RunR) -> Option<Vec<RunR>> {
        let p = self.parter.as_mut()?;
        if let Some(c) = p.cache.get(&r.seq) {
            return c.clone();
        }
        let ps = self
            .kept
            .ex
            .runs
            .get(&r.seq)
            .and_then(|lg| parts_of(code, lg, r.depth == 0));
        let mut out = None;
        if let Some(ps) = ps {
            let ents: Vec<Ent> = ps.iter().flat_map(part_entries).collect();
            self.kept.add_entries(&ents);
            out = Some(
                ps.iter()
                    .map(|p| RunR {
                        frame: r.frame,
                        after: r.after,
                        seq: p.seq,
                        depth: r.depth,
                        slot: r.slot,
                        where_: r.where_,
                        number: r.number,
                        room: r.room,
                        code0: p.code0,
                        code1: p.code1.unwrap_or(-1),
                        status: p.status.unwrap_or(0),
                        frozen: p.frozen.unwrap_or(0),
                        delay: p.delay.unwrap_or(0),
                        ops: p.ops.len() as u64,
                        unit: r.unit,
                    })
                    .collect(),
            );
            self.parter.as_mut().unwrap().splits += 1;
        }
        let p = self.parter.as_mut().unwrap();
        p.cache.insert(r.seq, out.clone());
        while p.cache.len() > 4096 {
            p.cache.shift_remove_index(0);
        }
        out
    }
    pub fn splits(&self) -> u64 {
        self.parter.as_ref().map(|p| p.splits).unwrap_or(0)
    }
}

/// strict.part_verdicts (9.9)
#[allow(clippy::too_many_arguments)]
pub fn part_verdicts(
    plan: &Plan,
    code: &Code,
    parts: &[RunR],
    state: &View,
    after: &View,
    later: &[RunR],
    dump_before: Option<&Obs>,
    ex: &Explained,
    open_keys: &FxHashSet<Key>,
) -> Vec<(RunR, Verdict)> {
    let mut decide = |c: &Check, r: &RunR| matcher_decide(code, ex, dump_before, c, r);
    let runs: Vec<RunR> = parts.iter().chain(later.iter()).copied().collect();
    let vs = plan
        .matcher
        .match_runs(&runs, state, after, open_keys, true, &mut decide);
    vs.into_iter().take(parts.len()).map(|v| (v.run, v)).collect()
}

// --- a failure and its cause (9.5) -----------------------------------------------------------------------------------

/// strict.Kept: a branch's last KEEP_FRAMES frames.
#[derive(Clone, Default)]
pub struct Kept {
    pub runs: BTreeMap<u64, Vec<RunR>>,
    pub entries: Vec<Ent>,
    pub seq_frame: FxHashMap<Seq, u64>,
    pub dumps: BTreeMap<u64, Arc<Obs>>,
    pub views: BTreeMap<u64, Arc<View>>,
    pub ex: Explained,
    pub pinned: FxHashSet<Seq>,
}

/// A failure's kept material (Kept.freeze).
#[derive(Clone, Default)]
pub struct Material {
    pub runs: BTreeMap<u64, Vec<RunR>>,
    pub entries: Vec<Ent>,
    pub dumps: BTreeMap<u64, Arc<Obs>>,
    pub views: BTreeMap<u64, Arc<View>>,
}

impl Kept {
    pub fn add_entries(&mut self, entries: &[Ent]) {
        for e in entries {
            if let Ent::B { seq, frame, .. } = e {
                self.seq_frame.insert(*seq, *frame);
            }
        }
        self.entries.extend_from_slice(entries);
        self.ex.add(entries);
    }
    pub fn frame(&mut self, k: u64, runs: Vec<RunR>, dump: Option<Arc<Obs>>, view: Option<Arc<View>>) {
        self.runs.insert(k, runs);
        if let Some(d) = dump {
            self.dumps.insert(k, d);
        }
        if let Some(v) = view {
            self.views.insert(k, v);
        }
        let lo = k as i64 - KEEP_FRAMES as i64;
        let keep = |f: &u64| (*f as i64) >= lo;
        self.runs.retain(|f, _| keep(f));
        self.dumps.retain(|f, _| keep(f));
        self.views.retain(|f, _| keep(f));
        let pinned = &self.pinned;
        self.ex
            .runs
            .retain(|sq, r| !((r.frame as i64) < lo - 1 && !pinned.contains(sq)));
        if self.entries.len() > 4096 {
            let keep_seq: FxHashSet<Seq> = self
                .seq_frame
                .iter()
                .filter(|(_, &f)| f as i64 >= lo - 1)
                .map(|(s, _)| *s)
                .collect();
            self.entries.retain(|e| keep_seq.contains(&e.seq()));
            self.seq_frame.retain(|s, _| keep_seq.contains(s));
        }
    }
    pub fn freeze(&self, extra_views: &[(u64, Arc<View>)], extra_dumps: &[(u64, Arc<Obs>)]) -> Material {
        let mut dumps = self.dumps.clone();
        for (k, d) in extra_dumps {
            dumps.insert(*k, d.clone());
        }
        let mut views = self.views.clone();
        for (k, v) in extra_views {
            views.insert(*k, v.clone());
        }
        Material {
            runs: self.runs.clone(),
            entries: self.entries.clone(),
            dumps,
            views,
        }
    }
}

/// A child's (or the parent's) first failure (9.5).
#[derive(Clone)]
pub struct Fail {
    pub kind: &'static str,
    pub record: Option<String>,
    pub frame: u64,
    pub why: String,
    pub run: Option<RunR>,
    pub want: Option<Val>,
    pub engine: J,
    pub own: Vec<RunR>,
    pub asks: Option<Vec<Val>>,
    pub unit: Option<Unit>,
    pub fact: Option<Key>,
    pub material: Option<Arc<Material>>,
}

impl Fail {
    pub fn new(
        kind: &'static str,
        record: Option<String>,
        frame: u64,
        why: String,
        run: Option<RunR>,
        material: Material,
    ) -> Fail {
        Fail {
            kind,
            record,
            frame,
            why,
            run,
            want: None,
            engine: J::Null,
            own: vec![],
            asks: None,
            unit: None,
            fact: None,
            material: Some(Arc::new(material)),
        }
    }
    /// Search._sig
    pub fn sig(&self) -> (String, Option<String>, String) {
        (self.kind.to_string(), self.record.clone(), self.why.clone())
    }
}

/// The explain log of a failure's material (strict._Eng).
pub fn eng_of(m: &Material) -> Explained {
    let mut ex = Explained::default();
    ex.add(&m.entries);
    ex
}

/// strict._logged: a run's log (a part by its seq).
pub fn logged<'a>(ex: &'a Explained, run: &RunR) -> Option<&'a Logged> {
    if run.is_part() {
        if let Some(lg) = ex.runs.get(&run.seq) {
            return Some(lg);
        }
    }
    ex.runs.values().find(|r| {
        r.frame == run.frame
            && r.after == run.after
            && r.code0 == run.code0
            && r.where_ == run.where_
            && r.number == run.number
    })
}

/// A cause (an ordered dict, as the reference writes it).
pub type Cause = J;

fn base_of(f: &Fail) -> Vec<(String, J)> {
    vec![
        ("failure".into(), J::Str(f.kind.into())),
        ("record".into(), f.record.clone().map(J::Str).unwrap_or(J::Null)),
        ("frame".into(), J::Int(f.frame as i64)),
        ("why".into(), J::Str(f.why.clone())),
    ]
}

/// `base | d`
pub fn merge(mut base: Vec<(String, J)>, d: Vec<(String, J)>) -> J {
    for (k, v) in d {
        if let Some(p) = base.iter_mut().find(|p| p.0 == k) {
            p.1 = v;
        } else {
            base.push((k, v));
        }
    }
    J::Obj(base)
}

pub fn kv(k: &str, v: impl Into<J>) -> (String, J) {
    (k.to_string(), v.into())
}

pub fn vals_j(vals: &[Value]) -> J {
    J::Arr(vals.iter().map(Value::j).collect())
}

/// strict.explain: B1's explanation of one failure.
pub fn explain(code: &Code, rd: &PlayReading, fail: &Fail) -> J {
    let base = base_of(fail);
    let (kind, why, k) = (fail.kind, fail.why.as_str(), fail.frame);
    let Some(mat) = fail.material.as_ref() else {
        return merge(base, vec![kv("cause", J::Null), kv("kind", "no material")]);
    };
    let eng = eng_of(mat);
    let run = fail.run;
    let first = why.split(' ').next().unwrap_or("");
    if (kind == "refuted" || kind == "mismatch") && run.is_some() && ["pre", "end", "effect"].contains(&first) {
        if !mat.views.contains_key(&(k.wrapping_sub(1))) || !mat.dumps.contains_key(&(k.wrapping_sub(1))) {
            return merge(
                base,
                vec![kv("cause", J::Null), kv("kind", "the frame before is not kept")],
            );
        }
        let mut views = mat.views.clone();
        views.entry(k).or_insert_with(|| Arc::new(View::new()));
        return merge(
            base,
            check::explain_unexplained(code, &eng, k, run.as_ref().unwrap(), why, rd, &views, &mat.dumps),
        );
    }
    if kind == "input" {
        if why.starts_with("end pos:") && run.is_some() {
            let r = run.unwrap();
            let df = if !r.after { r.frame.wrapping_sub(1) } else { r.frame };
            return merge(
                base,
                input_end_cause(
                    code,
                    &eng,
                    &r,
                    fail.want.unwrap_or(NONE),
                    mat.dumps.get(&df).map(|d| &**d),
                ),
            );
        }
        if why.starts_with("effect") && run.is_some() {
            let r = run.unwrap();
            let kk = if r.after { r.frame + 1 } else { r.frame };
            if !mat.views.contains_key(&(kk.wrapping_sub(1))) || !mat.views.contains_key(&kk) {
                return merge(base, vec![kv("cause", J::Null), kv("kind", "the frames are not kept")]);
            }
            return merge(
                base,
                check::explain_unexplained(code, &eng, kk, &r, why, rd, &mat.views, &mat.dumps),
            );
        }
        if let Some(u) = why.strip_prefix("no run pos:") {
            return merge(base, no_start_cause(code, &eng, &fail.own, u, &mat.dumps));
        }
        if why.starts_with("pre") {
            let fk = why.split_once(' ').map(|x| x.1).unwrap_or("");
            let mut asks: Vec<String> = fail.asks.clone().unwrap_or_default().iter().map(|&v| vstr(v)).collect();
            asks.sort();
            asks.truncate(6);
            let asks_txt = Py::List(asks.into_iter().map(Py::Str).collect()).repr();
            let vals = vec![Value::new(
                fk,
                fail.engine.clone(),
                format!("the view before the click; the record asks {asks_txt}"),
                "fact",
            )];
            return merge(
                base,
                vec![
                    kv("cause", true),
                    kv("kind", "the input's precondition fails before its first click"),
                    kv("unit", J::Null),
                    kv("offset", J::Null),
                    kv("opcode", J::Null),
                    kv("values", vals_j(&vals)),
                    kv("class", J::Null),
                ],
            );
        }
        return merge(base, vec![kv("cause", J::Null), kv("kind", why)]);
    }
    if kind == "not run" {
        return merge(base, not_run_cause(code, &eng, fail, mat));
    }
    if kind == "choice" && run.is_some() {
        let r = run.unwrap();
        let kk = if r.after { r.frame + 1 } else { r.frame };
        return merge(
            base,
            check::explain_unexplained(code, &eng, kk, &r, why, rd, &mat.views, &mat.dumps),
        );
    }
    if kind == "goal writer" && run.is_some() {
        return merge(
            base,
            goal_writer_cause(
                code,
                &eng,
                run.as_ref().unwrap(),
                &fail.fact.map(|k| k.text()).unwrap_or_default(),
                &mat.dumps,
            ),
        );
    }
    merge(
        base,
        vec![kv("cause", J::Null), kv("kind", format!("no rule for {kind} {why}"))],
    )
}

/// strict.input_end_cause
pub fn input_end_cause(code: &Code, eng: &Explained, run: &RunR, want: Val, dump: Option<&Obs>) -> Vec<(String, J)> {
    let Some(key) = code.key(run.where_, run.number, run.room) else {
        return vec![kv("cause", J::Null), kv("kind", "unit not in the IR")];
    };
    let Some(lg) = logged(eng, run) else {
        return vec![
            kv("cause", J::Null),
            kv("kind", "the run is not in the explain log"),
            kv("unit", unit_j(run.unit)),
        ];
    };
    let steps = steps_of(code, key, &[lg]);
    let targets = if want == NOWHERE {
        code.end_sites(key)
    } else {
        code.yield_sites(key, want)
    };
    let mut base = vec![
        kv("unit", unit_j(run.unit)),
        kv("engine_end", run.code1),
        kv("record_end", vj(want)),
    ];
    if let Some(n) = first_off(code, key, &steps, &targets, false) {
        let x = &code.ins(key)[steps[n].i.unwrap() as usize];
        if x.yields == Some(scumm_ir::ast::Yield::Wait) {
            let vals = wait_values_read(code, x, Some(&steps[n]), dump);
            base.extend(vec![
                kv("cause", true),
                kv("kind", "the wait did not block"),
                kv("offset", steps[n].op.code),
                kv("opcode", code.head(x.st.as_ref())),
                kv("values", vals_j(&vals)),
                kv("class", "split"),
            ]);
            return base;
        }
        let vals = slice_test(code, key, &steps, n, dump);
        base.extend(vec![
            kv("cause", true),
            kv("kind", "left the record's path"),
            kv("offset", steps[n].op.code),
            kv("opcode", code.head(x.st.as_ref())),
            kv("values", vals_j(&vals)),
            kv("class", cause_class(&vals)),
        ]);
        return base;
    }
    base.extend(vec![
        kv("cause", J::Null),
        kv("kind", "no branch off the record's path found"),
        kv("steps", steps.len()),
    ]);
    base
}

pub fn unit_j(u: Option<Unit>) -> J {
    u.map(|u| J::Str(u.text())).unwrap_or(J::Null)
}

/// strict.wait_values_read
pub fn wait_values_read(code: &Code, x: &scumm_ir::ast::Ins, step: Option<&StepX>, dump: Option<&Obs>) -> Vec<Value> {
    let ast = &code.game.script.ast;
    if let (Some(scumm_ir::ast::Stmt::Op { name, args, .. }), Some(st)) = (&x.st, step) {
        if code.name(*name) == "WaitForActor" {
            let mut a: Vec<Option<i64>> = Vec::new();
            for &e in ast.kids(*args) {
                let vid = var_id(code, e);
                let got = vid.and_then(|v| st.op.reads.iter().find(|r| r.0 == v).map(|r| r.1 as i64));
                a.push(got.or_else(|| ast.constant(e).map(|c| c as i64)));
            }
            return vec![getter_value_args("getActorMoving", &a, "actor state".into(), dump)];
        }
    }
    wait_values(code, x, 0, dump)
}

/// strict.no_start_cause
pub fn no_start_cause(
    code: &Code,
    eng: &Explained,
    own: &[RunR],
    unit: &str,
    dumps: &BTreeMap<u64, Arc<Obs>>,
) -> Vec<(String, J)> {
    let mut own: Vec<RunR> = own.to_vec();
    own.sort_by_key(|r| r.seq);
    for r in &own {
        let key = code.key(r.where_, r.number, r.room);
        let target = code.key_of_name(unit, r.room);
        let (Some(key), Some(target)) = (key, target) else {
            continue;
        };
        let tk = code.game.script.unit(target).key;
        let mut sites = code.start_sites(key, tk, true);
        sites.extend(code.start_sites(key, tk, false));
        sites.sort();
        sites.dedup();
        if sites.is_empty() {
            continue;
        }
        let Some(lg) = logged(eng, r) else { continue };
        let steps = steps_of(code, key, &[lg]);
        let n = first_off(code, key, &steps, &sites, false);
        let df = if !r.after { r.frame.wrapping_sub(1) } else { r.frame };
        let d = dumps.get(&df).map(|d| &**d);
        let Some(n) = n else {
            return vec![
                kv("cause", J::Null),
                kv(
                    "kind",
                    format!(
                        "the click's run of {} kept a path to the start of {unit}",
                        r.unit.map(|u| u.text()).unwrap_or_default()
                    ),
                ),
                kv("unit", unit_j(r.unit)),
            ];
        };
        let x = &code.ins(key)[steps[n].i.unwrap() as usize];
        let wait = x.yields == Some(scumm_ir::ast::Yield::Wait);
        let vals = if wait {
            wait_values_read(code, x, Some(&steps[n]), d)
        } else {
            slice_test(code, key, &steps, n, d)
        };
        return vec![
            kv("cause", true),
            kv("kind", format!("the click's run left the path to the start of {unit}")),
            kv("unit", unit_j(r.unit)),
            kv("offset", steps[n].op.code),
            kv("opcode", code.head(x.st.as_ref())),
            kv("values", vals_j(&vals)),
            kv("class", if wait { "split" } else { cause_class(&vals) }),
        ];
    }
    vec![
        kv("cause", J::Null),
        kv("kind", format!("no run of the click's units starts {unit}")),
        kv("unit", J::Null),
    ]
}

/// strict.not_run_cause
pub fn not_run_cause(code: &Code, eng: &Explained, fail: &Fail, mat: &Material) -> Vec<(String, J)> {
    let unit = fail.unit;
    let uj = unit_j(unit);
    let dump = mat.dumps.get(&fail.frame);
    let runs: Vec<&RunR> = mat
        .runs
        .values()
        .flat_map(|rs| rs.iter())
        .filter(|r| r.unit == unit)
        .collect();
    if let Some(d) = dump {
        let sl = d.slots.iter().find(|s| Unit::of(s.where_, s.number) == unit);
        if sl.is_none() {
            return vec![
                kv("cause", true),
                kv("kind", "the unit has no live slot"),
                kv("unit", uj),
                kv("offset", J::Null),
                kv("opcode", J::Null),
                kv("values", J::Arr(vec![])),
                kv("class", "semantics"),
            ];
        }
    }
    let Some(last) = runs.last() else {
        return vec![
            kv("cause", J::Null),
            kv("kind", "the unit did not run in the kept frames"),
            kv("unit", uj),
        ];
    };
    let key = code.key(last.where_, last.number, last.room);
    let Some(key) = key.filter(|_| last.code1 >= 0) else {
        return vec![
            kv("cause", J::Null),
            kv("kind", "its last run ended the slot"),
            kv("unit", uj),
        ];
    };
    let lg = logged(eng, last);
    let steps = lg.map(|l| steps_of(code, key, &[l])).unwrap_or_default();
    let m = if steps.is_empty() {
        None
    } else {
        last_test(code, key, &steps)
    };
    let df = if !last.after {
        last.frame.wrapping_sub(1)
    } else {
        last.frame
    };
    let d = mat.dumps.get(&df).map(|d| &**d);
    if let Some(m) = m {
        let vals = slice_test(code, key, &steps, m, d);
        let x = &code.ins(key)[steps[m].i.unwrap() as usize];
        return vec![
            kv("cause", true),
            kv("kind", "not there yet: the slot polls"),
            kv("unit", uj),
            kv("offset", steps[m].op.code),
            kv("opcode", code.head(x.st.as_ref())),
            kv("values", vals_j(&vals)),
            kv("class", if vals.is_empty() { "split" } else { cause_class(&vals) }),
            kv("waits_at", last.code1),
        ];
    }
    let ys = code.yield_sites(key, last.code1);
    let x = ys.first().map(|&i| &code.ins(key)[i as usize]);
    let vals = x
        .map(|x| wait_values(code, x, last.delay as i64, d))
        .unwrap_or_default();
    vec![
        kv("cause", true),
        kv("kind", "not there yet: the slot waits"),
        kv("unit", uj),
        kv("offset", x.map(|x| x.offset as i64).unwrap_or(last.code1)),
        kv("opcode", x.map(|x| J::Str(code.head(x.st.as_ref()))).unwrap_or(J::Null)),
        kv("values", vals_j(&vals)),
        kv("class", "split"),
        kv("waits_at", last.code1),
    ]
}

/// strict.goal_writer_cause
pub fn goal_writer_cause(
    code: &Code,
    eng: &Explained,
    run: &RunR,
    fact: &str,
    dumps: &BTreeMap<u64, Arc<Obs>>,
) -> Vec<(String, J)> {
    let key = code.key(run.where_, run.number, run.room);
    let lg = logged(eng, run);
    let (Some(key), Some(lg)) = (key, lg) else {
        return vec![
            kv("cause", J::Null),
            kv("kind", "the writing run is not in the explain log"),
            kv("unit", unit_j(run.unit)),
        ];
    };
    let steps = steps_of(code, key, &[lg]);
    let sites = code.write_sites(key, fact, None, false);
    let n = steps.iter().position(|s| s.i.is_some_and(|i| sites.contains(&i)));
    let df = if !run.after {
        run.frame.wrapping_sub(1)
    } else {
        run.frame
    };
    let d = dumps.get(&df).map(|d| &**d);
    let Some(n) = n else {
        return vec![
            kv("cause", J::Null),
            kv("kind", "no write site of the fact in the run"),
            kv("unit", unit_j(run.unit)),
        ];
    };
    let m = last_test(code, key, &steps[..n]);
    let vals = m.map(|m| slice_test(code, key, &steps, m, d)).unwrap_or_default();
    let x = &code.ins(key)[steps[n].i.unwrap() as usize];
    vec![
        kv("cause", true),
        kv("kind", "a unit the plan does not name wrote the goal's fact"),
        kv("unit", unit_j(run.unit)),
        kv("offset", steps[n].op.code),
        kv("opcode", code.head(x.st.as_ref())),
        kv("values", vals_j(&vals)),
        kv("test_offset", m.map(|m| J::Int(steps[m].op.code)).unwrap_or(J::Null)),
        kv(
            "class",
            if vals.is_empty() {
                J::Null
            } else {
                J::Str(cause_class(&vals).into())
            },
        ),
    ]
}

/// strict.short: one line of a cause.
pub fn short(c: Option<&J>) -> String {
    let Some(c) = c.filter(|c| c.truthy()) else {
        return "no cause".into();
    };
    let s = |k: &str| -> String {
        match c.get(k) {
            None | Some(J::Null) => "None".into(),
            Some(J::Str(t)) => t.clone(),
            Some(v) => Py::from_j(v).str(),
        }
    };
    let or_empty = |k: &str| -> String {
        match c.get(k) {
            None | Some(J::Null) => String::new(),
            Some(v) if !v.truthy() => String::new(),
            Some(J::Str(t)) => t.clone(),
            Some(v) => Py::from_j(v).str(),
        }
    };
    let vals: Vec<String> = c
        .a("values")
        .iter()
        .take(3)
        .map(|v| {
            let kind = v.get("kind");
            let tail = match kind {
                None | Some(J::Null) => String::new(),
                Some(J::Str(k)) if k == "fact" => String::new(),
                Some(k) => format!(" ({})", Py::from_j(k).str()),
            };
            format!(
                "{} = {}{}",
                Py::from_j(v.get("what").unwrap_or(&J::Null)).str(),
                Py::from_j(v.get("value").unwrap_or(&J::Null)).str(),
                tail
            )
        })
        .collect();
    let vals = vals.join("; ");
    match c.get("offset") {
        Some(J::Int(off)) => format!(
            "{} {} at {}: {}: {}; {} [{:04X}] {}; {}; class {}",
            s("failure"),
            or_empty("record"),
            s("frame"),
            s("why"),
            s("kind"),
            or_empty("unit"),
            off,
            or_empty("opcode"),
            vals,
            s("class")
        ),
        _ => format!(
            "{} {} at {}: {}: {}; {} {}; {}; class {}",
            s("failure"),
            or_empty("record"),
            s("frame"),
            s("why"),
            s("kind"),
            or_empty("unit"),
            or_empty("opcode"),
            vals,
            s("class")
        ),
    }
}

/// strict.by_frame: runs grouped as R_k, each in seq order.
pub fn by_frame(runs: &[RunR]) -> BTreeMap<u64, Vec<RunR>> {
    let mut out: BTreeMap<u64, Vec<RunR>> = BTreeMap::new();
    for r in runs {
        out.entry(r.k()).or_default().push(*r);
    }
    for v in out.values_mut() {
        v.sort_by_key(|r| r.seq);
    }
    out
}

/// strict.unit_where
pub fn unit_where(u: Unit) -> Vec<(i32, i32)> {
    match u.0 {
        U_SCRIPT => vec![(WIO_GLOBAL, u.1 as i32)],
        U_LOCAL => vec![(WIO_LOCAL, u.1 as i32)],
        U_OBJ => vec![
            (WIO_ROOM, u.1 as i32),
            (WIO_INVENTORY, u.1 as i32),
            (WIO_FLOBJECT, u.1 as i32),
        ],
        U_ENTRY => vec![(WIO_ROOM, ENCD)],
        U_EXIT => vec![(WIO_ROOM, EXCD)],
        _ => vec![],
    }
}

pub fn runs_of(rows: &[fmi_core::Run]) -> Vec<RunR> {
    rows.iter().map(RunR::of).collect()
}

pub fn ents_of(rows: &[fmi_core::Explain]) -> Vec<Ent> {
    rows.iter().map(Ent::of).collect()
}

/// The plain JSON of a cause for the record (window._plain).
pub fn plain(c: Option<&J>) -> J {
    c.cloned().unwrap_or(J::Null)
}

#[allow(dead_code)]
fn _unused(_: &B1, _: &pyj::J) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cites() {
        assert_eq!(
            cite_unit("global/script-002.txt:170 [02E0]"),
            Some((Unit(U_SCRIPT, 2), 0x2E0))
        );
        assert_eq!(
            cite_unit("room-033-x/local-215.txt:12 [0010]"),
            Some((Unit(U_LOCAL, 215), 0x10))
        );
        assert_eq!(cite_unit("room-033-x/entry.txt:3 [0003]"), Some((Unit(U_ENTRY, 0), 3)));
        assert_eq!(
            cite_unit("room-033-x/obj-0428-door.txt:5 [001A]"),
            Some((Unit(U_OBJ, 428), 0x1A))
        );
        assert_eq!(cite_unit("a/b/local-1.txt:1 [0000]"), None);
    }
}

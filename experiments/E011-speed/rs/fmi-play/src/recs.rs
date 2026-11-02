//! window.py sections 1, 2 and 4: the plan's records as the search tracks them (`MRec`, `build_recs`), the macro
//! steps (`macro_steps`), the checked writes (`checked_keys`), Lemma 4's chain (`chain_frames`), and the matcher of
//! sections 2 and 3 (`Matcher`, `is_deviation`, `EndRule`, `strict_stop`, `click_gate`).

use crate::pyj::Py;
use crate::reading::{PlayReading, Rec};
use crate::replay::{self, Expect, Exps, Guessed};
use crate::val::*;
use fmi_core::*;
use rustc_hash::{FxHashMap, FxHashSet};
use std::sync::Arc;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Input,
    Event,
}

/// One plan step as the window search tracks it (window.MRec).
#[derive(Clone, Debug)]
pub struct MRec {
    pub i: usize,
    pub name: String,
    pub step: PlanStep,
    /// the step's actions (one, or the paths the engine chooses between) and their records
    pub paths: Vec<Arc<Action>>,
    pub whats: Vec<Arc<Rec>>,
    pub kind: Kind,
    pub reads: FxHashSet<Key>,
    pub writes: FxHashSet<Key>,
    pub unit: Option<Unit>,
    pub wait: Option<i64>,
    pub cost: i64,
    pub exps: Vec<Exps>,
    pub pres: Vec<Vec<(Key, Vec<Val>)>>,
    pub before: View,
    /// the steps it follows (≺), sorted
    pub preds: Vec<usize>,
    pub macro_: usize,
    pub done: bool,
    pub done_frame: Option<u64>,
    pub path: usize,
    pub checked: Option<FxHashSet<Key>>,
}

impl MRec {
    pub fn has_pred(&self, j: usize) -> bool {
        self.preds.binary_search(&j).is_ok()
    }
}

/// `^event\(([^@#()]+)@(-?\d+)`: an event label's unit and wait point.
pub fn event_of_label(label: &str) -> Option<(Unit, i64)> {
    let rest = label.strip_prefix("event(")?;
    let at = rest.find(|c: char| matches!(c, '@' | '#' | '(' | ')'))?;
    if at == 0 || rest.as_bytes()[at] != b'@' {
        return None;
    }
    let unit = Unit::parse(&rest[..at])?;
    let tail = &rest[at + 1..];
    let neg = tail.starts_with('-');
    let digits: String = tail[neg as usize..]
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect();
    if digits.is_empty() {
        return None;
    }
    let w: i64 = digits.parse().ok()?;
    Some((unit, if neg { -w } else { w }))
}

/// window.interferes
pub fn interferes(a: &MRec, b: &MRec) -> bool {
    a.writes.iter().any(|k| b.reads.contains(k) || b.writes.contains(k)) || a.reads.iter().any(|k| b.writes.contains(k))
}

/// The harness's fields build_recs reads.
pub struct RecsCtx<'a> {
    pub rd: &'a PlayReading<'a>,
    pub result: &'a PlanResult,
    pub unobserved: &'a Prefixes,
    pub belief: View,
}

/// window.build_recs: the plan's steps as MRecs, each path's predicted writes from the plan's state before it.
pub fn build_recs(c: &RecsCtx) -> Vec<MRec> {
    let mut state = c.belief.clone();
    let mut out = Vec::new();
    for (i, st) in c.result.steps.iter().enumerate() {
        let si = st.action;
        let paths: Vec<Arc<Action>> = match c.result.choices.iter().find(|ch| ch.0 == si).map(|ch| &ch.1) {
            Some(ps) if !ps.is_empty() => ps.iter().map(|a| Arc::new(a.clone())).collect(),
            _ => vec![Arc::new(c.result.actions[si as usize].clone())],
        };
        let whats: Vec<Arc<Rec>> = paths.iter().map(|a| c.rd.rec(a.what)).collect();
        let mut reads = FxHashSet::default();
        let mut writes = FxHashSet::default();
        for (a, w) in paths.iter().zip(&whats) {
            let (r1, w1) = replay::rec_reads_writes(w);
            let (r2, w2) = replay::reads_writes(a);
            reads.extend(r1);
            reads.extend(r2);
            writes.extend(w1);
            writes.extend(w2);
        }
        let rec = &whats[0];
        let kind = if rec.kind == "event" { Kind::Event } else { Kind::Input };
        let m = if kind == Kind::Event {
            event_of_label(&rec.label)
        } else {
            None
        };
        let guessed = guessed_of(st);
        let exps: Vec<Exps> = whats
            .iter()
            .map(|w| replay::expectations(w, &state, c.rd.doms, &guessed, c.unobserved))
            .collect();
        let pick = paths
            .iter()
            .position(|a| replay::holds_all(&a.pre, &state, c.unobserved))
            .unwrap_or(0);
        let next = replay::after(&state, &exps[pick]);
        out.push(MRec {
            i,
            name: st.name.clone(),
            step: st.clone(),
            whats: whats.clone(),
            kind,
            reads,
            writes,
            unit: m.map(|x| x.0),
            wait: m.map(|x| x.1),
            cost: paths.iter().map(|a| a.cost).min().unwrap_or(1),
            exps,
            pres: paths.iter().map(|a| a.pre.clone()).collect(),
            before: state.clone(),
            paths,
            preds: vec![],
            macro_: 0,
            done: false,
            done_frame: None,
            path: 0,
            checked: None,
        });
        state = next;
    }
    out
}

/// plan_fd.Step.guessed from fmi-core's PlanStep, whose `when` is the conditions' JSON text ([[key, [values]], ...]).
pub fn guessed_of(st: &PlanStep) -> Guessed {
    st.guessed
        .iter()
        .map(|(k, v, w)| {
            let when = crate::pyj::parse(w)
                .map(|j| {
                    j.arr()
                        .iter()
                        .map(|c| {
                            let a = c.arr();
                            let mut vs: Vec<Val> = a
                                .get(1)
                                .map(|x| x.arr().iter().map(val_of).collect())
                                .unwrap_or_default();
                            vs.sort();
                            (Key::parse(a.first().and_then(|x| x.str()).unwrap_or("")), vs)
                        })
                        .collect()
                })
                .unwrap_or_default();
            (*k, *v, when)
        })
        .collect()
}

/// window.macro_steps: ≺ (each record's preds) and the macro steps, as index lists (the input first).
pub fn macro_steps(recs: &mut [MRec]) -> Vec<Vec<usize>> {
    let n = recs.len();
    let mut preds: Vec<FxHashSet<usize>> = vec![FxHashSet::default(); n];
    for j in 0..n {
        for i in 0..j {
            if interferes(&recs[i], &recs[j]) {
                preds[j].insert(i);
                let pi: Vec<usize> = preds[i].iter().copied().collect();
                preds[j].extend(pi);
            }
        }
    }
    for (j, r) in recs.iter_mut().enumerate() {
        let mut p: Vec<usize> = preds[j].iter().copied().collect();
        p.sort();
        r.preds = p;
    }
    let inputs: Vec<usize> = (0..n).filter(|&i| recs[i].kind == Kind::Input).collect();
    if inputs.is_empty() {
        for r in recs.iter_mut() {
            r.macro_ = 0;
        }
        return if n > 0 { vec![(0..n).collect()] } else { vec![] };
    }
    let k_of: FxHashMap<usize, usize> = inputs.iter().enumerate().map(|(k, &i)| (i, k)).collect();
    for j in 0..n {
        if recs[j].kind == Kind::Input {
            recs[j].macro_ = k_of[&j];
            continue;
        }
        let after: Vec<usize> = inputs
            .iter()
            .filter(|&&i| preds[j].contains(&i))
            .map(|i| k_of[i])
            .collect();
        let before: Vec<usize> = inputs
            .iter()
            .filter(|&&i| preds[i].contains(&j))
            .map(|i| k_of[i])
            .collect();
        recs[j].macro_ = after
            .iter()
            .max()
            .copied()
            .or_else(|| before.iter().min().copied())
            .unwrap_or(inputs.len() - 1);
    }
    let mut out: Vec<Vec<usize>> = vec![vec![]; inputs.len()];
    for j in 0..n {
        out[recs[j].macro_].push(j);
    }
    for (k, &i) in inputs.iter().enumerate() {
        let rest: Vec<usize> = out[k].iter().copied().filter(|&j| j != i).collect();
        out[k] = std::iter::once(i).chain(rest).collect();
    }
    out
}

/// window.checked_keys (Lemma 8): sets each record's `checked`.
pub fn checked_keys(recs: &mut [MRec], goal: &[(Key, Vec<Val>)]) {
    let n = recs.len();
    for i in 0..n {
        let mut keep = FxHashSet::default();
        for &k in &recs[i].writes {
            let mut ended = false;
            for q in &recs[i + 1..] {
                if q.reads.contains(&k) {
                    keep.insert(k);
                    ended = true;
                    break;
                }
                if q.writes.contains(&k) {
                    ended = true;
                    break;
                }
            }
            if !ended && goal.iter().any(|g| g.0 == k) {
                keep.insert(k);
            }
        }
        if let Some(u) = recs[i].unit {
            keep.insert(u.pos());
        }
        if recs[i].kind == Kind::Input {
            keep.extend(recs[i].writes.iter().copied().filter(|k| k.kind() == K_POS));
        }
        recs[i].checked = Some(keep);
    }
}

/// window.later_reads
pub fn later_reads(recs: &[MRec], i: usize, keys: &[Key]) -> FxHashSet<Key> {
    let mut out = FxHashSet::default();
    for &k in keys {
        for q in &recs[i + 1..] {
            if q.reads.contains(&k) {
                out.insert(k);
                break;
            }
            if q.writes.contains(&k) {
                break;
            }
        }
    }
    out
}

/// window.chain_frames (Lemmas 4 and 4′): (frames, the chain's step names).
pub fn chain_frames(recs: &[MRec], members: &[usize], zero: bool) -> (i64, Vec<String>) {
    let first = &recs[members[0]];
    if first.kind != Kind::Input || first.paths.len() != 1 {
        return (0, vec![]);
    }
    let rest: Vec<&MRec> = members[1..].iter().map(|&j| &recs[j]).collect();
    fn leaves(x: &MRec) -> Vec<(Unit, i64)> {
        x.whats[0]
            .effects
            .iter()
            .filter_map(|(k, e)| match e {
                RecEff::Set(v) if k.kind() == K_POS && is_int(*v) && *v >= 0 => Some((Unit::of_key(*k), *v)),
                _ => None,
            })
            .collect()
    }
    fn best_from(x: &MRec, seen: &[usize], rest: &[&MRec], zero: bool) -> (i64, Vec<String>) {
        let (mut top, mut top_names) = (0i64, Vec::new());
        for (unit, w) in leaves(x) {
            let nxt = rest
                .iter()
                .find(|r| r.i > x.i && r.kind == Kind::Event && r.unit == Some(unit) && r.wait == Some(w));
            let Some(nxt) = nxt else { continue };
            if nxt.paths.len() != 1 || seen.contains(&nxt.i) {
                continue;
            }
            if let Some(RecEff::Set(v)) = nxt.whats[0].eff(unit.pos()) {
                if *v == w {
                    continue;
                }
            }
            let own = x.kind == Kind::Input || x.unit == Some(unit);
            let add = if own {
                if zero {
                    nxt.cost
                } else {
                    nxt.cost.max(1)
                }
            } else {
                (nxt.cost - 1).max(0)
            };
            let mut s2 = seen.to_vec();
            s2.push(nxt.i);
            let (sub, sub_names) = best_from(nxt, &s2, rest, zero);
            if add + sub > top {
                top = add + sub;
                top_names = std::iter::once(nxt.name.clone()).chain(sub_names).collect();
            }
        }
        (top, top_names)
    }
    best_from(first, &[], &rest, zero)
}

// --- the matcher (section 2) ----------------------------------------------------------------------------------------

/// window.Matcher over the tracked records.
#[derive(Clone)]
pub struct Matcher {
    pub tracked: Vec<usize>,
    pub needed: FxHashSet<usize>,
    pub input: Option<usize>,
    pub issued: FxHashSet<usize>,
    /// i -> (frame, path); frame None: matched in the parent before
    pub matched: FxHashMap<usize, (Option<u64>, usize)>,
    pub final_: FxHashMap<usize, Vec<Exps>>,
    pub preds: FxHashMap<usize, Vec<usize>>,
    pub input_issued: bool,
    pub choices: Vec<(usize, Vec<(Key, VV)>)>,
    pub plan_pending: Vec<usize>,
}

impl Matcher {
    pub fn new(
        recs: &[MRec],
        tracked: &[usize],
        needed: &FxHashSet<usize>,
        input: Option<usize>,
        done: &[usize],
        issued: &FxHashSet<usize>,
    ) -> Matcher {
        let idx: FxHashSet<usize> = tracked.iter().copied().collect();
        let mut final_ = FxHashMap::default();
        for &i in tracked {
            let r = &recs[i];
            let later: FxHashSet<Key> = tracked
                .iter()
                .filter(|&&q| q > i)
                .flat_map(|&q| recs[q].writes.iter().copied())
                .collect();
            let fins = r
                .exps
                .iter()
                .map(|e| {
                    e.iter()
                        .filter(|(k, _)| !later.contains(k) && r.checked.as_ref().is_none_or(|c| c.contains(k)))
                        .cloned()
                        .collect()
                })
                .collect();
            final_.insert(i, fins);
        }
        let preds = tracked
            .iter()
            .map(|&i| (i, recs[i].preds.iter().copied().filter(|p| idx.contains(p)).collect()))
            .collect();
        Matcher {
            tracked: tracked.to_vec(),
            needed: needed.clone(),
            input,
            issued: issued.clone(),
            matched: done.iter().map(|&i| (i, (None, 0))).collect(),
            final_,
            preds,
            input_issued: false,
            choices: Vec::new(),
            plan_pending: Vec::new(),
        }
    }

    pub fn note(&mut self, recs: &[MRec], frame: u64, now: &View) {
        for &i in &self.tracked.clone() {
            if self.matched.contains_key(&i) {
                continue;
            }
            let r = &recs[i];
            if r.kind == Kind::Input && !self.issued.contains(&i) && !(self.input_issued && Some(i) == self.input) {
                continue;
            }
            if self.preds[&i].iter().any(|p| !self.matched.contains_key(p)) {
                continue;
            }
            for (n, fin) in self.final_[&i].iter().enumerate() {
                if fin.iter().all(|(k, x)| x.holds(now.get(*k))) {
                    self.matched.insert(i, (Some(frame), n));
                    let ch: Vec<(Key, VV)> = replay::match_(fin, now)
                        .into_iter()
                        .filter(|m| m.kind == "choice")
                        .map(|m| (m.key, m.observed))
                        .collect();
                    if !ch.is_empty() {
                        self.choices.retain(|c| c.0 != i);
                        self.choices.push((i, ch));
                    }
                    break;
                }
            }
        }
    }

    pub fn complete(&self) -> bool {
        self.needed.iter().all(|i| self.matched.contains_key(i))
    }

    pub fn pending(&self) -> Vec<usize> {
        self.tracked
            .iter()
            .copied()
            .filter(|i| !self.matched.contains_key(i))
            .collect()
    }

    /// Section 3's mismatch: (key, value) or None.
    pub fn mismatch(&self, recs: &[MRec], now: &View, at_fork: &View) -> Option<(Key, VV)> {
        let pend = self.pending();
        let pend_set: FxHashSet<usize> = pend.iter().copied().collect();
        let mut allowed: FxHashMap<Key, FxHashSet<Val>> = FxHashMap::default();
        let mut allowed_none: FxHashSet<Key> = FxHashSet::default();
        for &i in pend
            .iter()
            .chain(self.plan_pending.iter().filter(|q| !pend_set.contains(q)))
        {
            let r = &recs[i];
            for (exps, pre) in r.exps.iter().zip(&r.pres) {
                for (k, x) in exps {
                    if k.kind() == K_POS || k.kind() == K_ROOM {
                        let e = allowed.entry(*k).or_default();
                        for &v in &x.values {
                            if v == NONE {
                                allowed_none.insert(*k);
                            }
                            e.insert(v);
                        }
                    }
                }
                for (k, vs) in pre {
                    if k.kind() == K_POS {
                        allowed.entry(*k).or_default().extend(vs.iter().copied());
                    }
                }
            }
        }
        let mut checked: FxHashSet<Key> = FxHashSet::default();
        for &i in &pend {
            let r = &recs[i];
            for pre in &r.pres {
                checked.extend(pre.iter().map(|p| p.0).filter(|k| k.kind() == K_POS));
            }
            if let Some(u) = r.unit {
                checked.insert(u.pos());
            }
            if r.exps
                .iter()
                .zip(&r.pres)
                .any(|(e, p)| e.iter().any(|x| x.0.kind() == K_ROOM) || p.iter().any(|x| x.0.kind() == K_ROOM))
            {
                checked.insert(Key::pack(K_ROOM, 0, 0, 0));
            }
        }
        let mut ks: Vec<Key> = checked.into_iter().collect();
        sort_keys(&mut ks);
        for k in ks {
            let v = now.get(k);
            if v.is_none() || v == at_fork.get(k) {
                continue;
            }
            if let VV::V(x) = v {
                if allowed.get(&k).is_some_and(|a| a.contains(x)) {
                    continue;
                }
            }
            return Some((k, v.clone()));
        }
        None
    }

    /// The needed records still pending, each with its checked final writes that do not hold (B1's read).
    pub fn unmet(&self, recs: &[MRec], now: &View, limit: usize) -> Py {
        let mut out = Vec::new();
        for i in self.pending() {
            if !self.needed.contains(&i) {
                continue;
            }
            let fin = &self.final_[&i][0];
            let miss: Vec<Py> = fin
                .iter()
                .filter(|(k, x)| !x.holds(now.get(*k)))
                .take(limit)
                .map(|(k, x)| {
                    Py::Tuple(vec![
                        Py::Str(k.text()),
                        Py::List(txt(&x.values).into_iter().map(Py::Str).collect()),
                        now.get(*k).py(),
                    ])
                })
                .collect();
            out.push(Py::Tuple(vec![Py::Str(recs[i].name.clone()), Py::List(miss)]));
        }
        out.truncate(limit);
        Py::List(out)
    }

    /// Section 3's spent test: the needed records pending that nothing alive can run.
    pub fn dead_needed(&self, recs: &[MRec], now: &View) -> Vec<String> {
        self.pending()
            .into_iter()
            .filter(|i| {
                self.needed.contains(i)
                    && (recs[*i].kind == Kind::Input || recs[*i].unit.is_some_and(|u| now.val(u.pos()) == NOWHERE))
            })
            .map(|i| recs[i].name.clone())
            .collect()
    }
}

/// window.EndRule (Lemma 7)
#[derive(Default)]
pub struct EndRule {
    pub open: bool,
}

impl EndRule {
    pub fn closed(&mut self, ready: bool, pre_now: bool) -> bool {
        if self.open {
            return !pre_now;
        }
        self.open = ready && pre_now;
        false
    }
}

/// window.strict_stop (section 9.3)
pub fn strict_stop(best_q: u64, best_deviation: bool, qf: u64, b_qf: u64, b_dev_qf: u64) -> bool {
    if !best_deviation {
        return qf + b_qf >= best_q;
    }
    qf + b_qf > best_q && qf + b_dev_qf >= best_q
}

/// window.click_gate
pub fn click_gate(st: &Status) -> bool {
    st.up > 0 && st.cs == 0 && st.room != 0
}

/// window.path_shows (Lemma 9)
pub fn path_shows(pos_exps: &[Vec<(Key, Expect)>], now: &View) -> bool {
    pos_exps.iter().any(|pe| pe.iter().all(|(k, x)| x.holds(now.get(*k))))
}

//! replay.py's per-step functions as the window search uses them: the step's input (`Input`, `step_input`,
//! `verb_point`, `to_move`), read and write sets (`reads_writes`), the predicted writes (`Expect`, `expectations`,
//! `after`, `predict`), the match (`match_`, `unplanned`), the replay checks (`eval_check`), the checkpoint extras
//! (`extra_of`), and the harness's step record (`StepLog`) and stop (`Stop`). Also `History`, `verdict`, `classify`
//! and `ran_on` (the one-step harness's rules), ported for completeness.

use crate::pyj::{Py, J};
use crate::reading::{Check, Rec};
use crate::val::*;
use fmi_core::*;
use fmi_core::{Event, Input};

pub const INPUT_LIMIT: u64 = 600;
pub const WAIT_LIMIT: u64 = 3600;
pub const VAR_EGO: usize = 1;
pub const VAR_SENTENCE_SCRIPT: usize = 33;
pub const EXECUTOR_EGO: i32 = 1;
pub const HISTORY_CAP: usize = 12;
pub const RETRY_REASONS: [&str; 8] = [
    "position",
    "timer",
    "random draw",
    "actor state",
    "message state",
    "loop exit by time",
    "wait over by time",
    "camera position",
];

/// replay.Stop: the harness stops (an engine that ended, a memory floor).
#[derive(Debug, Clone)]
pub struct Stop(pub String);

impl std::fmt::Display for Stop {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// replay.Input
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InputStep {
    /// "click", "line", "key" or "wait"
    pub kind: &'static str,
    pub verb: Option<i64>,
    pub a: Option<i64>,
    pub b: Option<i64>,
    pub key: Option<String>,
}

/// replay.step_input
pub fn step_input(rec: &Rec) -> Result<InputStep, String> {
    match rec.kind.as_str() {
        "sentence" | "walk_to" => Ok(InputStep {
            kind: "click",
            verb: rec.src_int(1),
            a: rec.src_int(2),
            b: rec.src_int(3),
            key: None,
        }),
        "dialog_line" => Ok(InputStep {
            kind: "line",
            verb: rec.src_int(1),
            a: None,
            b: None,
            key: None,
        }),
        "skip_cutscene" => Ok(InputStep {
            kind: "key",
            verb: None,
            a: None,
            b: None,
            key: Some("esc".into()),
        }),
        "event" => Ok(InputStep {
            kind: "wait",
            verb: None,
            a: None,
            b: None,
            key: None,
        }),
        k => Err(format!("no input for a record of kind {k:?}")),
    }
}

/// replay._input_text
pub fn input_text(inp: &InputStep) -> String {
    match inp.kind {
        "click" => {
            let mut s = format!("click verb {} on {}", opt(inp.verb), opt(inp.a));
            if let Some(b) = inp.b {
                s += &format!(" with {b}");
            }
            s
        }
        "line" => format!("click dialog line {}", opt(inp.verb)),
        "key" => format!("key {}", inp.key.clone().unwrap_or_default()),
        _ => "wait".into(),
    }
}

fn opt(v: Option<i64>) -> String {
    v.map(|x| x.to_string()).unwrap_or_else(|| "None".into())
}

/// window.input_clicks (Lemma 3's n): 2 for a sentence or walk with a second object, else 1.
pub fn input_clicks(rec: &Rec) -> u64 {
    if (rec.kind == "sentence" || rec.kind == "walk_to") && rec.src.len() == 4 && rec.src[3].truthy() {
        2
    } else {
        1
    }
}

pub const VERB_OFFSETS: [(i32, i32); 4] = [(2, 2), (4, 3), (1, 1), (0, 0)];

/// replay.verb_point
pub fn verb_point(verb: i64, state: &Obs, at: &mut dyn FnMut(i32, i32) -> EResult<At>) -> EResult<Option<(i32, i32)>> {
    let mine: Vec<&Verb> = state.verbs.iter().filter(|v| v.id as i64 == verb).collect();
    if mine.len() != 1 {
        return Ok(None);
    }
    let v = mine[0];
    for (dx, dy) in VERB_OFFSETS {
        let p = (v.x + dx, v.y + dy);
        if at(p.0, p.1)?.verb_slot == v.slot {
            return Ok(Some(p));
        }
    }
    Ok(None)
}

/// The points `verb_point` tests for `verb`, in its order (none unless the verb is listed once).
pub fn verb_candidates(verb: i64, state: &Obs) -> Vec<(i32, i32)> {
    let mine: Vec<&Verb> = state.verbs.iter().filter(|v| v.id as i64 == verb).collect();
    if mine.len() != 1 {
        return vec![];
    }
    VERB_OFFSETS.iter().map(|(dx, dy)| (mine[0].x + dx, mine[0].y + dy)).collect()
}

/// The Executor's Move as replay.to_move makes it: a click (verb on A, and B) or a key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Move {
    pub name: String,
    pub key: Option<String>,
    pub verb: Option<i64>,
    pub obj: Option<i64>,
    pub obj2: Option<i64>,
    /// a floor click (the approach's internal moves): virtual room coordinates
    pub point: Option<(i64, i64)>,
    pub max_frames: u64,
}

/// replay.to_move
pub fn to_move(inp: &InputStep, name: &str) -> Result<Option<Move>, String> {
    match inp.kind {
        "click" => Ok(Some(Move {
            name: name.into(),
            key: None,
            verb: inp.verb,
            obj: inp.a,
            obj2: inp.b,
            point: None,
            max_frames: INPUT_LIMIT,
        })),
        "key" => Ok(Some(Move {
            name: name.into(),
            key: inp.key.clone(),
            verb: None,
            obj: None,
            obj2: None,
            point: None,
            max_frames: INPUT_LIMIT,
        })),
        "line" => Err(format!(
            "{name}: a dialog line is clicked by its verb number (Harness._click_verb), not as a Move"
        )),
        _ => Ok(None),
    }
}

/// replay.reads_writes on a task action: (reads, writes).
pub fn reads_writes(a: &Action) -> (Vec<Key>, Vec<Key>) {
    let mut r: Vec<Key> = a.pre.iter().map(|p| p.0).collect();
    for e in &a.effects {
        r.extend(e.when.iter().map(|w| w.0));
        if matches!(e.v, EffVal::Add(_)) {
            r.push(e.key);
        }
    }
    r.sort();
    r.dedup();
    let mut w: Vec<Key> = a.effects.iter().map(|e| e.key).collect();
    w.sort();
    w.dedup();
    (r, w)
}

/// window.rec_reads_writes on a record: (reads, writes).
pub fn rec_reads_writes(rec: &Rec) -> (Vec<Key>, Vec<Key>) {
    let mut r: Vec<Key> = rec.pre.iter().map(|p| p.0).collect();
    for (k, e) in &rec.effects {
        match e {
            RecEff::Add(_) => r.push(*k),
            RecEff::When(c, _) | RecEff::Copy(c) => r.push(*c),
            _ => {}
        }
    }
    r.sort();
    r.dedup();
    let mut w: Vec<Key> = rec.effects.iter().map(|e| e.0).collect();
    w.sort();
    w.dedup();
    (r, w)
}

/// replay.holds: the keys of `conds` that `state` does not meet (unobserved keys skipped).
pub fn holds(conds: &[(Key, Vec<Val>)], state: &View, unobserved: &Prefixes) -> Vec<Key> {
    conds
        .iter()
        .filter(|(k, s)| !unobserved.has(*k) && !in_set(state.get(*k), s))
        .map(|c| c.0)
        .collect()
}

/// `holds(...)` is empty: every condition met.
pub fn holds_all(conds: &[(Key, Vec<Val>)], state: &View, unobserved: &Prefixes) -> bool {
    conds
        .iter()
        .all(|(k, s)| unobserved.has(*k) || in_set(state.get(*k), s))
}

/// `v in s` for a set of task values (Python's None in a set holding None is in it).
#[inline]
pub fn in_set(v: &VV, s: &[Val]) -> bool {
    match v {
        VV::V(x) => s.contains(x),
        VV::Set(_) => false,
    }
}

/// replay.Expect
#[derive(Clone, Debug, PartialEq)]
pub struct Expect {
    pub key: Key,
    /// the values that match (a frozenset; may hold Python's None as NONE), sorted
    pub values: Vec<Val>,
    /// "set", "add", "guess", "when" or "copy"
    pub kind: &'static str,
    /// a guess's value the plan chose
    pub chosen: Option<Val>,
    pub changes: bool,
}

impl Expect {
    pub fn holds(&self, v: &VV) -> bool {
        in_set(v, &self.values)
    }
}

pub type Exps = Vec<(Key, Expect)>;

pub fn exp_get(e: &Exps, k: Key) -> Option<&Expect> {
    e.iter().find(|x| x.0 == k).map(|x| &x.1)
}

/// plan_fd.Step.guessed: (key, value, when [(key, values)]).
pub type Guessed = Vec<(Key, Val, Vec<(Key, Vec<Val>)>)>;

fn set1(v: Val) -> Vec<Val> {
    vec![v]
}

fn vv_val(v: &VV) -> Val {
    match v {
        VV::V(x) => *x,
        VV::Set(_) => NONE, // (a set never stands in a task state but for args:, which is unobserved here)
    }
}

/// replay.expectations
pub fn expectations(rec: &Rec, before: &View, doms: &Domains, guessed: &Guessed, unobserved: &Prefixes) -> Exps {
    let mut chosen: Vec<(Key, Val)> = Vec::new();
    for (k, v, when) in guessed {
        if when.iter().all(|(c, vs)| in_set(before.get(*c), vs)) {
            if let Some(p) = chosen.iter_mut().find(|p| p.0 == *k) {
                p.1 = *v;
            } else {
                chosen.push((*k, *v));
            }
        }
    }
    let mut out = Vec::with_capacity(rec.effects.len());
    for (k, e) in &rec.effects {
        if unobserved.has(*k) {
            continue;
        }
        let b0 = vv_val(before.get(*k));
        let (mut vals, kind): (Vec<Val>, &'static str) = match e {
            RecEff::Set(v) => (set1(*v), "set"),
            RecEff::Guess(vs) => (vs.clone(), "guess"),
            RecEff::Add(d) => {
                let vals = if b0 != NONE {
                    doms.get(*k).map(|dom| dom_add(dom, b0, *d)).unwrap_or_default()
                } else {
                    vec![]
                };
                let kind = if vals.len() == 1 { "add" } else { "guess" };
                (vals, kind)
            }
            RecEff::When(c, v) => (if before.val(*c) == 1 { set1(*v) } else { set1(b0) }, "when"),
            RecEff::Copy(c) => (set1(vv_val(before.get(*c))), "copy"),
        };
        vals.sort();
        vals.dedup();
        let changes = !vals.contains(&b0);
        out.push((
            *k,
            Expect {
                key: *k,
                values: vals,
                kind,
                chosen: chosen.iter().find(|p| p.0 == *k).map(|p| p.1),
                changes,
            },
        ));
    }
    out
}

/// replay.after: the state with every predicted write of one value applied.
pub fn after(state: &View, exps: &Exps) -> View {
    let mut out = state.clone();
    for (k, x) in exps {
        if x.values.len() == 1 {
            out.setv(*k, x.values[0]);
        }
    }
    out
}

/// replay.predict: the plan's prediction after the record (its set writes, its conditional writes whose condition
/// holds; any other write of a predicted key makes it unknown).
pub fn predict(pred: &View, rec: &Rec, before: &View, predicted: &Prefixes) -> View {
    let mut out = pred.clone();
    for (k, e) in &rec.effects {
        if !predicted.has(*k) {
            continue;
        }
        match e {
            RecEff::Set(v) => out.setv(*k, *v),
            RecEff::When(c, v) => {
                if before.val(*c) == 1 {
                    out.setv(*k, *v)
                }
            }
            _ => {
                out.m.remove(k);
            }
        }
    }
    out
}

/// replay.Mismatch
#[derive(Clone, Debug, PartialEq)]
pub struct Mismatch {
    pub key: Key,
    pub predicted: Vec<String>,
    pub observed: VV,
    pub kind: &'static str,
}

impl Mismatch {
    pub fn j(&self) -> J {
        J::Obj(vec![
            ("key".into(), J::Str(self.key.text())),
            (
                "predicted".into(),
                J::Arr(self.predicted.iter().map(|s| J::Str(s.clone())).collect()),
            ),
            ("observed".into(), self.observed.j()),
            ("kind".into(), J::Str(self.kind.into())),
        ])
    }
}

/// replay.match_: the predicted writes `now` does not show (choice: a guessed value inside its values but not the
/// plan's).
pub fn match_(exps: &Exps, now: &View) -> Vec<Mismatch> {
    let mut out = Vec::new();
    for (k, x) in exps {
        let v = now.get(*k);
        if !x.holds(v) {
            out.push(Mismatch {
                key: *k,
                predicted: txt(&x.values),
                observed: v.clone(),
                kind: if x.kind == "guess" { "guess" } else { "write" },
            });
        } else if x.kind == "guess" && x.chosen.is_some() && *v != VV::V(x.chosen.unwrap()) {
            out.push(Mismatch {
                key: *k,
                predicted: txt(&[x.chosen.unwrap()]),
                observed: v.clone(),
                kind: "choice",
            });
        }
    }
    out
}

/// replay.unplanned: tracked facts that differ from `before` to `after` with no predicted write, in key text order.
pub fn unplanned(before: &View, after: &View, predicted_keys: &rustc_hash::FxHashSet<Key>) -> Vec<Mismatch> {
    let mut keys: Vec<Key> = before.m.keys().chain(after.m.keys()).copied().collect();
    keys.sort();
    keys.dedup();
    sort_keys(&mut keys);
    keys.into_iter()
        .filter(|k| !predicted_keys.contains(k) && before.get(*k) != after.get(*k))
        .map(|k| Mismatch {
            key: k,
            predicted: vec![before.get(k).str()],
            observed: after.get(k).clone(),
            kind: "unplanned",
        })
        .collect()
}

/// replay._str: an int or None as it is, anything else its str.
pub fn vstr_j(v: &VV) -> J {
    match v {
        VV::V(x) if is_int(*x) => J::Int(*x),
        VV::V(NONE) => J::Null,
        v => J::Str(v.str()),
    }
}

/// replay.extra_of: (tick, rng, sorted actors [id, x, y]).
pub fn extra_of(s: &Obs, st: &Status) -> (u64, u64, Vec<(i32, i32, i32)>) {
    let mut actors: Vec<(i32, i32, i32)> = s.actors.iter().map(|a| (a.id, a.x, a.y)).collect();
    actors.sort();
    (st.tick, st.rng, actors)
}

pub fn checkpoint(frame: u64, facts: Facts, s: &Obs, st: &Status) -> Checkpoint {
    let (tick, rng, actors) = extra_of(s, st);
    Checkpoint {
        frame,
        facts,
        tick,
        rng,
        actors,
    }
}

// --- the replay checks -------------------------------------------------------------------------------------------

/// replay._what: the what of a check's why (`?(reason: what)`).
pub fn what_of(why: &str) -> String {
    let s = why.strip_prefix("?(").unwrap_or(why);
    match s.split_once(':') {
        Some((_, rest)) => {
            let t = rest.trim();
            let t = if t.is_empty() {
                t
            } else {
                &t[..t.len() - t.chars().last().unwrap().len_utf8()]
            };
            t.trim().to_string()
        }
        None => String::new(),
    }
}

/// reader._why: the reason of a why.
pub fn why_reason(s: &str) -> String {
    let s = s.strip_prefix("?(").unwrap_or(s);
    s.split(':').next().unwrap_or("").trim_end_matches(')').to_string()
}

/// replay.check_getter: (getter, actor) when VAR_RESULT comes from getActorX/Y/Room on one actor.
pub fn check_getter(c: &Check) -> Option<(String, String)> {
    let w = what_of(&c.why);
    for g in ["getActorX", "getActorY", "getActorRoom"] {
        if let Some(rest) = w.strip_prefix(g) {
            let inner = rest.strip_prefix('(')?.strip_suffix(')')?;
            if inner == "VAR_EGO" || (!inner.is_empty() && inner.bytes().all(|b| b.is_ascii_digit())) {
                return Some((g.to_string(), inner.to_string()));
            }
            return None;
        }
    }
    None
}

/// replay._CMP: (left, op, right), the first " op " from the left.
pub fn split_cmp(cond: &str) -> Option<(&str, &str, &str)> {
    if cond.contains('\n') {
        return None;
    }
    let b = cond.as_bytes();
    for i in 1..b.len() {
        if b[i] != b' ' {
            continue;
        }
        for op in ["==", "!=", "<=", ">=", "<", ">"] {
            let rest = &cond[i + 1..];
            if let Some(r) = rest.strip_prefix(op) {
                if let Some(r) = r.strip_prefix(' ') {
                    if !r.is_empty() {
                        return Some((&cond[..i], op, r));
                    }
                }
            }
        }
    }
    None
}

fn is_int_text(t: &str) -> bool {
    let d = t.strip_prefix('-').unwrap_or(t);
    !d.is_empty() && d.bytes().all(|b| b.is_ascii_digit())
}

fn bracket(t: &str, name: &str) -> Option<usize> {
    let inner = t.strip_prefix(name)?.strip_prefix('[')?.strip_suffix(']')?;
    if !inner.is_empty() && inner.bytes().all(|b| b.is_ascii_digit()) {
        inner.parse().ok()
    } else {
        None
    }
}

/// The record a check is evaluated for: its kind and source (B1's decide passes an event).
pub struct CheckRec<'a> {
    pub kind: &'a str,
    pub src: &'a [J],
}

/// replay.eval_check: True, False, or None when the dump does not show the operands.
pub fn eval_check(c: &Check, rec: &CheckRec, state: &Obs) -> Option<bool> {
    let side = c.side?;
    let vs = &state.vars;
    let mut locs: Option<[i64; 3]> = None;
    if (rec.kind == "sentence" || rec.kind == "walk_to") && rec.src.len() == 4 && rec.src[0].str() == Some("sentence") {
        let n = vs.get(VAR_SENTENCE_SCRIPT).copied().unwrap_or(0);
        if c.cite.split(':').next() == Some(format!("global/script-{n:03}.txt").as_str()) {
            locs = Some([
                rec.src[1].int().unwrap_or(0),
                rec.src[2].int().unwrap_or(0),
                rec.src[3].int().unwrap_or(0),
            ]);
        }
    }
    let getter = check_getter(c);
    let w = what_of(&c.why);
    let read: Option<&str> = w
        .strip_prefix("var:")
        .filter(|d| !d.is_empty() && d.bytes().all(|b| b.is_ascii_digit()));
    let val = |t: &str| -> Option<i64> {
        let t = t.trim();
        if is_int_text(t) {
            return t.parse().ok();
        }
        if let Some(n) = bracket(t, "Var") {
            locs?;
            let digits = t.strip_prefix("Var[").and_then(|x| x.strip_suffix(']')).unwrap_or("");
            if read.is_some_and(|r| r != digits) {
                return None; // the reader read another variable there (an indirect read)
            }
            return vs.get(n).map(|&x| x as i64);
        }
        if let Some(n) = bracket(t, "Local") {
            return locs.and_then(|l| l.get(n).copied());
        }
        if t == "VAR_EGO" {
            return vs.get(VAR_EGO).map(|&x| x as i64);
        }
        if t == "VAR_RESULT" {
            if let Some((g, who)) = &getter {
                let id = if who == "VAR_EGO" {
                    vs.get(VAR_EGO).copied().unwrap_or(0)
                } else {
                    who.parse().unwrap_or(-1)
                };
                let a = state.actors.iter().find(|a| a.id == id)?;
                return Some(match g.as_str() {
                    "getActorX" => a.x as i64,
                    "getActorY" => a.y as i64,
                    _ => state.room as i64,
                });
            }
        }
        None
    };
    let got = match split_cmp(&c.cond) {
        Some((l, op, r)) => {
            let (lv, rv) = (val(l)?, val(r)?);
            match op {
                "==" => lv == rv,
                "!=" => lv != rv,
                "<=" => lv <= rv,
                ">=" => lv >= rv,
                "<" => lv < rv,
                _ => lv > rv,
            }
        }
        None => val(&c.cond)? != 0,
    };
    Some(got == side)
}

// --- the one-step harness's rules (ported for completeness; the window search does not use them) --------------------

/// replay.History: every change to the tracked facts during a step.
pub struct History {
    pub start: View,
    pub last: View,
    pub frame0: u64,
    pub changes: Vec<(Key, Vec<(u64, VV)>)>,
}

impl History {
    pub fn new(start: &View, frame: u64) -> History {
        History {
            start: start.clone(),
            last: start.clone(),
            frame0: frame,
            changes: Vec::new(),
        }
    }
    pub fn note(&mut self, frame: u64, now: &View) {
        let mut keys: Vec<Key> = self.last.m.keys().chain(now.m.keys()).copied().collect();
        keys.sort();
        keys.dedup();
        for k in keys {
            let v = now.get(k);
            if v != self.last.get(k) {
                match self.changes.iter_mut().find(|c| c.0 == k) {
                    Some(c) => c.1.push((frame, v.clone())),
                    None => self.changes.push((k, vec![(frame, v.clone())])),
                }
            }
        }
        self.last = now.clone();
    }
    pub fn frames(&self) -> Vec<(u64, View)> {
        let mut by: std::collections::BTreeMap<u64, Vec<(Key, VV)>> = Default::default();
        for (k, ch) in &self.changes {
            for (f, v) in ch {
                by.entry(*f).or_default().push((*k, v.clone()));
            }
        }
        let mut cur = self.start.clone();
        let mut out = vec![(self.frame0, cur.clone())];
        for (f, kv) in by {
            for (k, v) in kv {
                cur.set(k, v);
            }
            out.push((f, cur.clone()));
        }
        out
    }
}

/// replay.verdict: (first, after) for a step whose path started and then differs.
pub fn verdict(
    rec: &Rec,
    mismatches: &[Mismatch],
    evals: Option<&[Option<bool>]>,
) -> (&'static str, Option<&'static str>) {
    if !mismatches.iter().any(|m| m.kind == "write") {
        return ("replan", None);
    }
    let none = vec![None; rec.checks.len()];
    let ev = evals.unwrap_or(&none);
    let failed: Vec<&Check> = rec
        .checks
        .iter()
        .zip(ev)
        .filter(|(_, ok)| **ok == Some(false))
        .map(|x| x.0)
        .collect();
    if !failed.is_empty() {
        return if failed
            .iter()
            .any(|c| RETRY_REASONS.contains(&why_reason(&c.why).as_str()))
        {
            ("retry", Some("replan"))
        } else {
            ("replan", None)
        };
    }
    if rec
        .checks
        .iter()
        .zip(ev)
        .filter(|(_, ok)| ok.is_none())
        .any(|(c, _)| RETRY_REASONS.contains(&why_reason(&c.why).as_str()))
    {
        return ("retry", Some("ban"));
    }
    ("ban", None)
}

/// replay.classify
pub fn classify(rec: &Rec, mismatches: &[Mismatch]) -> &'static str {
    verdict(rec, mismatches, None).0
}

/// replay.ran_on, on the plan's next event steps `ahead` ((step, its action, its guessed)): the steps of the run
/// that explains every write mismatch, each with its own missed writes; None when it does not.
pub fn ran_on(
    ahead: &[(String, &Action, &Rec, &Guessed)],
    doms: &Domains,
    exps: &Exps,
    mismatches: &[Mismatch],
    before: &View,
    hist: &History,
    unobserved: &Prefixes,
) -> Option<Vec<(String, Vec<Mismatch>)>> {
    let mut pending: Vec<Key> = mismatches.iter().filter(|m| m.kind == "write").map(|m| m.key).collect();
    if pending.is_empty() {
        return None;
    }
    let frames = hist.frames();
    let mut state = after(before, exps);
    let mut pos = 0usize;
    let mut chain = Vec::new();
    for (name, a, rec, guessed) in ahead {
        if !holds(&a.pre, &state, unobserved).is_empty() {
            return None;
        }
        let x = expectations(rec, &state, doms, guessed, unobserved);
        let need: Vec<&Expect> = pending.iter().filter_map(|k| exp_get(&x, *k)).collect();
        let ch: Vec<&Expect> = x.iter().map(|e| &e.1).filter(|e| e.changes).collect();
        let hit = if !need.is_empty() {
            (pos..frames.len()).find(|&n| need.iter().all(|e| e.holds(frames[n].1.get(e.key))))
        } else if !ch.is_empty() {
            (pos..frames.len()).find(|&n| ch.iter().any(|e| e.holds(frames[n].1.get(e.key))))
        } else {
            None
        };
        let hit = hit?;
        pos = hit;
        pending.retain(|k| !need.iter().any(|e| e.key == *k));
        let missed = ch
            .iter()
            .filter(|e| !(pos..frames.len()).any(|n| e.holds(frames[n].1.get(e.key))))
            .map(|e| Mismatch {
                key: e.key,
                predicted: txt(&e.values),
                observed: frames.last().unwrap().1.get(e.key).clone(),
                kind: "write",
            })
            .collect();
        chain.push((name.clone(), missed));
        state = after(&state, &x);
        if pending.is_empty() {
            return Some(chain);
        }
    }
    None
}

/// replay.StepLog: one step of the harness's record.
#[derive(Clone, Debug, Default)]
pub struct StepLog {
    pub plan: u32,
    pub index: u32,
    pub name: String,
    pub kind: String,
    pub input: String,
    pub start: u64,
    pub issue: Option<u64>,
    pub end: Option<u64>,
    pub outcome: String,
    pub then: String,
    pub tries: Vec<J>,
    pub mismatches: Vec<J>,
    pub unplanned: Vec<J>,
    pub checks: Vec<J>,
    pub inputs: Vec<J>,
    pub note: String,
    pub seconds: f64,
}

impl StepLog {
    pub fn add(&mut self, note: &str) {
        if self.note.is_empty() {
            self.note = note.into();
        } else {
            self.note = format!("{}; {note}", self.note);
        }
    }
    pub fn j(&self) -> J {
        J::Obj(vec![
            ("plan".into(), J::Int(self.plan as i64)),
            ("index".into(), J::Int(self.index as i64)),
            ("name".into(), J::Str(self.name.clone())),
            ("kind".into(), J::Str(self.kind.clone())),
            ("input".into(), J::Str(self.input.clone())),
            ("start".into(), J::Int(self.start as i64)),
            ("issue".into(), self.issue.map(|x| J::Int(x as i64)).unwrap_or(J::Null)),
            ("end".into(), self.end.map(|x| J::Int(x as i64)).unwrap_or(J::Null)),
            ("outcome".into(), J::Str(self.outcome.clone())),
            ("then".into(), J::Str(self.then.clone())),
            ("tries".into(), J::Arr(self.tries.clone())),
            ("mismatches".into(), J::Arr(self.mismatches.clone())),
            ("unplanned".into(), J::Arr(self.unplanned.clone())),
            ("checks".into(), J::Arr(self.checks.clone())),
            ("inputs".into(), J::Arr(self.inputs.clone())),
            ("note".into(), J::Str(self.note.clone())),
            ("seconds".into(), J::Float(self.seconds)),
        ])
    }
}

/// A Py tuple of a key and a value list etc., for why texts.
pub fn py_key(k: Key) -> Py {
    Py::Str(k.text())
}

/// An input event as the C8 schedule writes it (`{"frame": f, "click": [x, y]}`).
pub fn event_j(e: &Event) -> J {
    let mut o = vec![("frame".to_string(), J::Int(e.frame as i64))];
    match &e.what {
        Input::Click(x, y) => o.push(("click".into(), J::Arr(vec![J::Int(*x as i64), J::Int(*y as i64)]))),
        Input::RClick(x, y) => o.push(("rclick".into(), J::Arr(vec![J::Int(*x as i64), J::Int(*y as i64)]))),
        Input::Move(x, y) => o.push(("move".into(), J::Arr(vec![J::Int(*x as i64), J::Int(*y as i64)]))),
        Input::Key(k) => o.push(("key".into(), J::Str(k.clone()))),
        Input::Skips { text, cutscenes, spare } => {
            let mut s = vec![
                ("text".to_string(), J::Int(*text as i64)),
                ("cutscenes".to_string(), J::Int(*cutscenes as i64)),
            ];
            if let Some(sp) = spare {
                s.push(("spare".into(), J::Arr(sp.iter().map(|&n| J::Int(n as i64)).collect())));
            }
            o.push(("skips".into(), J::Obj(s)));
        }
    }
    J::Obj(o)
}

/// An event from its C8 JSON.
pub fn event_of(j: &J) -> Option<Event> {
    let frame = j.i("frame")? as u64;
    let xy = |k: &str| -> Option<(i32, i32)> {
        let a = j.a(k);
        Some((a.first()?.int()? as i32, a.get(1)?.int()? as i32))
    };
    let what = if j.get("click").is_some() {
        let (x, y) = xy("click")?;
        Input::Click(x, y)
    } else if j.get("rclick").is_some() {
        let (x, y) = xy("rclick")?;
        Input::RClick(x, y)
    } else if j.get("move").is_some() {
        let (x, y) = xy("move")?;
        Input::Move(x, y)
    } else if let Some(k) = j.s("key") {
        Input::Key(k.to_string())
    } else if let Some(s) = j.get("skips") {
        Input::Skips {
            text: s.get("text").map(J::truthy).unwrap_or(false),
            cutscenes: s.get("cutscenes").map(J::truthy).unwrap_or(false),
            spare: s
                .get("spare")
                .map(|a| a.arr().iter().filter_map(|x| x.int().map(|n| n as i32)).collect()),
        }
    } else {
        return None;
    };
    Some(Event { frame, what })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conditions_split_as_python() {
        assert_eq!(split_cmp("Var[103] == Local[2]"), Some(("Var[103]", "==", "Local[2]")));
        assert_eq!(split_cmp("a <= b"), Some(("a", "<=", "b")));
        assert_eq!(split_cmp("VAR_RESULT"), None);
        assert_eq!(what_of("?(not a fact: var:103)"), "var:103");
        assert_eq!(why_reason("?(position: getActorX(1))"), "position");
        let c = Check::of(
            &crate::pyj::parse(r#"["x.txt:1 [0000]", "VAR_RESULT > 5", true, "?(position: getActorX(VAR_EGO))"]"#)
                .unwrap(),
        );
        assert_eq!(check_getter(&c), Some(("getActorX".into(), "VAR_EGO".into())));
    }

    #[test]
    fn checks_on_a_dump() {
        let c = Check::of(
            &crate::pyj::parse(
                r#"["global/script-002.txt:8 [0000]", "Var[103] == Local[2]", true, "?(not a fact: var:103)"]"#,
            )
            .unwrap(),
        );
        let mut s = Obs {
            vars: vec![0; 200],
            ..Default::default()
        };
        s.vars[VAR_SENTENCE_SCRIPT] = 2;
        s.vars[103] = 428;
        let src = crate::pyj::parse(r#"["sentence", 2, 426, 428]"#).unwrap();
        let r = CheckRec {
            kind: "sentence",
            src: src.arr(),
        };
        assert_eq!(eval_check(&c, &r, &s), Some(true));
        s.vars[103] = 7;
        assert_eq!(eval_check(&c, &r, &s), Some(false));
        s.vars[VAR_SENTENCE_SCRIPT] = 3; // another script's code: Var[] not checked
        assert_eq!(eval_check(&c, &r, &s), None);
    }
}

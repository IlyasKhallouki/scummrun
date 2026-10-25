//! The planner's task from the records: reader.py `build_task`, relevance.py `relevant` and `restrict`, reader.py
//! `prune`, `task_start` and `replan_start`, over task.py's `Task`, `Action` and `Effect`. task.py's `write_sas` is
//! taskcore's (`--fold off --reduce none`, byte for byte); the task reaches it through the packed format (packed.rs).

use crate::builder::Builder;
use crate::domain::{Domain, Label, Labels};
use crate::facts::Facts;
use crate::py::Counter;
use crate::record::{Eff, Record};
use crate::table::{pos_key, FactTable, ELSEWHERE, NOWHERE};
use rustc_hash::{FxHashMap, FxHashSet};
use std::collections::{BTreeMap, BTreeSet};

/// task.py `Effect`: one of `value`, `add`, `values`, under `when` (all hold before the action).
#[derive(Clone, Debug, PartialEq)]
pub struct TEffect {
    pub key: String,
    pub value: Option<Label>,
    pub add: Option<i64>,
    pub values: Option<Labels>,
    pub when: Vec<(String, Labels)>,
}

impl TEffect {
    fn set(key: &str, v: Label, when: Vec<(String, Labels)>) -> TEffect {
        TEffect { key: key.into(), value: Some(v), add: None, values: None, when }
    }
}

/// task.py `Action`.
#[derive(Clone, Debug, PartialEq)]
pub struct Action {
    pub name: String,
    /// the precondition in Python's dict order
    pub pre: Vec<(String, Labels)>,
    pub effects: Vec<TEffect>,
    /// the record's index (`what`)
    pub what: usize,
    pub cost: i64,
}

impl Action {
    pub fn pre_get(&self, k: &str) -> Option<&Labels> {
        self.pre.iter().find(|p| p.0 == k).map(|p| &p.1)
    }
}

/// task.py `Task`: facts in Python's dict order.
#[derive(Clone, Debug)]
pub struct Task {
    pub facts: Vec<(String, Domain)>,
    pub index: FxHashMap<String, usize>,
    pub actions: Vec<Action>,
    pub goal: Vec<(String, Labels)>,
    /// records folded into an action with the same label, precondition and effects
    pub folded: usize,
}

impl Task {
    pub fn new(facts: Vec<(String, Domain)>, actions: Vec<Action>, goal: Vec<(String, Labels)>) -> Task {
        let index = facts.iter().enumerate().map(|(i, (k, _))| (k.clone(), i)).collect();
        Task { facts, index, actions, goal, folded: 0 }
    }
    pub fn fact(&self, k: &str) -> &Domain {
        &self.facts[self.index[k]].1
    }
    pub fn has(&self, k: &str) -> bool {
        self.index.contains_key(k)
    }
}

/// An insertion-ordered fact table being built (`facts.setdefault`).
struct FactsBuild<'t> {
    table: &'t FactTable,
    facts: Vec<(String, Option<Domain>)>,
    index: FxHashMap<String, usize>,
}

impl<'t> FactsBuild<'t> {
    fn setdefault(&mut self, k: &str) -> Option<Domain> {
        if let Some(&i) = self.index.get(k) {
            return self.facts[i].1.clone();
        }
        let d = self.table.domain(k).map(|d| d.into_owned());
        self.index.insert(k.to_string(), self.facts.len());
        self.facts.push((k.to_string(), d.clone()));
        d
    }
    fn set(&mut self, k: &str, d: Option<Domain>) {
        match self.index.get(k) {
            Some(&i) => self.facts[i].1 = d,
            None => {
                self.index.insert(k.to_string(), self.facts.len());
                self.facts.push((k.to_string(), d));
            }
        }
    }
}

/// build_task's fold key of a record: (label, precondition, effects), in a canonical text (sets sorted).
pub fn fold_key(r: &Record) -> String {
    fold_text(r, None)
}

/// A set's values as the fold key writes them: texts, sorted.
fn set_text(v: &[Label]) -> String {
    let mut s = String::new();
    push_set_text(&mut s, v);
    s
}

/// `set_text` written into `s`: the values' texts (an int's decimal digits, "above", "below") sorted as strings and
/// joined by commas, made in one buffer per set (a set of a precondition can hold hundreds of values).
fn push_set_text(s: &mut String, v: &[Label]) {
    let mut items: Vec<([u8; 24], u8)> = Vec::with_capacity(v.len());
    for l in v {
        let mut buf = [0u8; 24];
        let n = match l {
            Label::Int(n) => {
                use std::io::Write;
                let mut c = std::io::Cursor::new(&mut buf[..]);
                write!(c, "{n}").unwrap();
                c.position() as usize
            }
            other => {
                let t = other.text();
                buf[..t.len()].copy_from_slice(t.as_bytes());
                t.len()
            }
        };
        items.push((buf, n as u8));
    }
    items.sort_unstable_by(|a, b| a.0[..a.1 as usize].cmp(&b.0[..b.1 as usize]));
    for (i, (b, n)) in items.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push_str(std::str::from_utf8(&b[..*n as usize]).unwrap());
    }
}

/// The fold key; with `slots`, the parts `_read_once`'s rewrite of folded lists can change (an `args:` key's
/// precondition, an `args:` key's set) are written as a mark and pushed to `slots` in key order.
fn fold_text(r: &Record, mut slots: Option<&mut ArgSlots>) -> String {
    let mut s = String::with_capacity(256);
    s.push_str(&r.label);
    s.push('\u{1}');
    for (k, v) in &r.pre {
        s.push_str(k);
        s.push('=');
        match slots.as_deref_mut() {
            Some(sl) if k.starts_with("args:") => {
                s.push('\u{3}');
                sl.pre.push(v.clone());
            }
            _ => push_set_text(&mut s, v),
        }
        s.push('\u{2}');
    }
    s.push('\u{1}');
    // the keys are a dict's, so the effects sort by key alone, with or without the marks
    let mut effs: Vec<&(String, Eff)> = r.effects.iter().collect();
    effs.sort_by(|a, b| a.0.cmp(&b.0));
    for (k, e) in effs {
        s.push_str(k);
        s.push('=');
        match (slots.as_deref_mut(), e) {
            (Some(sl), Eff::Set(v @ Label::Int(_))) if k.starts_with("args:") => {
                s.push('\u{3}');
                sl.eff.push(*v);
            }
            _ => s.push_str(&e.key_text()),
        }
        s.push('\u{2}');
    }
    s
}

fn hash16(bytes: &[u8]) -> [u8; 16] {
    let d = sha1_smol::Sha1::from(bytes).digest().bytes();
    let mut out = [0u8; 16];
    out.copy_from_slice(&d[..16]);
    out
}

/// A 16-byte hash of a fold key (the streaming writer keeps one per distinct record).
pub fn fold_hash(r: &Record) -> [u8; 16] {
    hash16(fold_key(r).as_bytes())
}

/// The `args:` values of a record's fold key that `_read_once` rewrites when SIG_LIMIT folds their list into 0
/// after the record was made: the precondition sets of `args:` keys and the values the record sets them to.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ArgSlots {
    pub pre: Vec<Labels>,
    pub eff: Vec<Label>,
}

/// A record's fold key in two parts (packed.rs `RecordStream`): a 16-byte hash of the key with the `args:` values
/// marked, and those values. Two records fold after the rewrite (build_task on the rewritten records) exactly when
/// their `base` is equal and their slots are equal after the rewrite: [`FoldParts::hash_after`].
#[derive(Clone, Debug)]
pub struct FoldParts {
    pub base: [u8; 16],
    /// None: the record has no `args:` values in its key
    pub slots: Option<Box<ArgSlots>>,
}

pub fn fold_parts(r: &Record) -> FoldParts {
    let mut sl = ArgSlots::default();
    let base = hash16(fold_text(r, Some(&mut sl)).as_bytes());
    let empty = sl.pre.is_empty() && sl.eff.is_empty();
    FoldParts { base, slots: (!empty).then(|| Box::new(sl)) }
}

impl FoldParts {
    /// The fold key's hash once the lists in `folded` are 0 (reader.py `_read_once`: an `args:` set to a folded
    /// list sets 0; an `args:` precondition with a folded list in it is {0}). With `folded` empty, the key's own.
    pub fn hash_after(&self, folded: &BTreeSet<i64>) -> [u8; 16] {
        let Some(sl) = &self.slots else { return self.base };
        let is_folded = |l: &Label| matches!(l, Label::Int(x) if folded.contains(x));
        let mut s: Vec<u8> = self.base.to_vec();
        for v in &sl.pre {
            let t = if v.iter().any(is_folded) { "0".to_string() } else { set_text(v) };
            s.extend_from_slice(t.as_bytes());
            s.push(2);
        }
        s.push(1);
        for v in &sl.eff {
            let t = if is_folded(v) { "0".to_string() } else { v.text() };
            s.extend_from_slice(t.as_bytes());
            s.push(2);
        }
        hash16(&s)
    }
}

/// One record as build_task makes its action: (precondition, effects), and whether a step needed room (counted).
pub fn record_action(table: &FactTable, r: &Record, counts: &mut Counter) -> (Vec<(String, Labels)>, Vec<TEffect>) {
    let mut pre: Vec<(String, Labels)> = r.pre.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
    let mut effects = Vec::new();
    for (k, e) in &r.effects {
        match e {
            Eff::Set(v) => effects.push(TEffect::set(k, *v, vec![])),
            Eff::Guess(vs) => effects.push(TEffect { key: k.clone(), value: None, add: None, values: Some(vs.clone()), when: vec![] }),
            Eff::When(src, v) => effects.push(TEffect::set(k, Label::Int(*v), vec![(src.clone(), vec![Label::Int(1)])])),
            Eff::Copy(src) => {
                for u in [0, 1] {
                    effects.push(TEffect::set(k, Label::Int(u), vec![(src.clone(), vec![Label::Int(u)])]));
                }
            }
            Eff::Add(d) => {
                let dom = table.domain(k).unwrap();
                let have: Labels = pre.iter().find(|p| &p.0 == k).map(|p| p.1.clone()).unwrap_or_else(|| dom.values.clone());
                let ok: Labels = have.iter().copied().filter(|&v| !dom.add(v, *d).is_empty()).collect();
                if ok != have {
                    match pre.iter_mut().find(|p| &p.0 == k) {
                        Some(p) => p.1 = ok.clone(),
                        None => pre.push((k.clone(), ok.clone())),
                    }
                    counts.inc("steps that need room in their domain (a precondition)");
                }
                effects.push(TEffect { key: k.clone(), value: None, add: Some(*d), values: None, when: vec![(k.clone(), ok)] });
            }
        }
    }
    (pre, effects)
}

/// reader.py `build_task`: one action per record, records of one label with the same precondition and effects
/// folded into the first.
pub fn build_task(b: &mut Builder, records: &[Record], goal: &[(String, Labels)]) -> Task {
    let mut fb = FactsBuild { table: &b.table, facts: Vec::new(), index: FxHashMap::default() };
    for (k, _) in goal {
        let d = fb.table.domain(k).map(|d| d.into_owned());
        fb.set(k, d);
    }
    let mut seen: FxHashSet<String> = FxHashSet::default();
    let mut folded = 0;
    let mut actions = Vec::new();
    let mut counts = Counter::default();
    for (i, r) in records.iter().enumerate() {
        if !seen.insert(fold_key(r)) {
            folded += 1;
            continue;
        }
        for (k, e) in &r.effects {
            fb.setdefault(k);
            if let Eff::When(src, _) | Eff::Copy(src) = e {
                fb.setdefault(src);
            }
        }
        let (pre, effects) = record_action(fb.table, r, &mut counts);
        for k in r.pre.keys() {
            fb.setdefault(k);
        }
        actions.push(Action { name: format!("{}#{i}", r.label), pre, effects, what: i, cost: 1 });
    }
    let args: Vec<String> = fb.facts.iter().filter(|(k, _)| k.starts_with("args:")).map(|(k, _)| k.clone()).collect();
    for k in args {
        let d = fb.table.domain(&k).map(|d| d.into_owned());
        fb.set(&k, d);
    }
    let facts = fb.facts.into_iter().map(|(k, d)| (k.clone(), d.unwrap_or_else(|| panic!("no domain for {k}")))).collect();
    b.counts.update(&counts);
    let mut t = Task::new(facts, actions, goal.to_vec());
    t.folded = folded;
    t
}

/// relevance.py `Relevance`.
#[derive(Clone, Debug, Default)]
pub struct Relevance {
    pub actions: Vec<usize>,
    pub values: BTreeMap<String, BTreeSet<Option<Label>>>,
    pub whole: BTreeSet<String>,
    pub counts: Counter,
}

impl Relevance {
    pub fn facts(&self) -> BTreeSet<String> {
        self.values.keys().cloned().chain(self.whole.iter().cloned()).collect()
    }
}

/// relevance.py `_writes`: the values an effect can write (None where a label is not in the domain).
fn writes_of(task: &Task, e: &TEffect) -> BTreeSet<Option<Label>> {
    let dom = task.fact(&e.key);
    if let Some(vs) = &e.values {
        return vs.iter().map(|&v| dom.label_of(v)).collect();
    }
    if let Some(d) = e.add {
        let own = e.when.iter().find(|w| w.0 == e.key).map(|w| &w.1);
        return dom
            .values
            .iter()
            .filter(|u| own.is_none_or(|o| o.contains(u)))
            .flat_map(|&u| dom.add(u, d))
            .map(Some)
            .collect();
    }
    [dom.label_of(e.value.unwrap())].into_iter().collect()
}

/// relevance.py `relevant`. Facts are numbered for the fixpoint (`needed` and `whole` by number); the result is
/// relevance.py's.
pub fn relevant(task: &Task) -> Relevance {
    let mut ids: FxHashMap<&str, usize> = FxHashMap::default();
    let mut names: Vec<&str> = Vec::new();
    let id = |k: &'_ str, ids: &mut FxHashMap<&'_ str, usize>| -> usize { ids.get(k).copied().unwrap_or(usize::MAX) };
    for (k, _) in &task.facts {
        ids.insert(k.as_str(), names.len());
        names.push(k.as_str());
    }
    let nf = names.len();
    let mut needed: Vec<BTreeSet<Option<Label>>> = vec![BTreeSet::new(); nf];
    let mut has_needed: Vec<bool> = vec![false; nf]; // a key `needed` holds (defaultdict access), maybe empty
    let mut whole: Vec<bool> = vec![false; nf];
    for (k, s) in &task.goal {
        let dom = task.fact(k);
        let i = id(k, &mut ids);
        has_needed[i] = true;
        needed[i].extend(s.iter().map(|&v| dom.label_of(v)));
    }
    struct E {
        key: usize,
        vals: Vec<Option<Label>>,
        reads: Vec<usize>,
    }
    let acts: Vec<(Vec<E>, Vec<(usize, Vec<Option<Label>>)>)> = task
        .actions
        .iter()
        .map(|a| {
            let effs = a
                .effects
                .iter()
                .map(|e| {
                    let mut reads: Vec<usize> = e.when.iter().map(|w| id(&w.0, &mut ids)).collect();
                    if e.add.is_some() {
                        reads.push(id(&e.key, &mut ids));
                    }
                    E { key: id(&e.key, &mut ids), vals: writes_of(task, e).into_iter().collect(), reads }
                })
                .collect();
            let pre = a
                .pre
                .iter()
                .map(|(k, s)| {
                    let dom = task.fact(k);
                    (id(k, &mut ids), s.iter().map(|&v| dom.label_of(v)).collect())
                })
                .collect();
            (effs, pre)
        })
        .collect();
    let mut kept: Vec<bool> = vec![false; task.actions.len()];
    let mut rounds = 0;
    let mut changed = true;
    while changed {
        changed = false;
        rounds += 1;
        for (i, (effs, pre)) in acts.iter().enumerate() {
            if !kept[i] {
                let hit = effs.iter().any(|e| whole[e.key] || e.vals.iter().any(|v| needed[e.key].contains(v)));
                if !hit {
                    continue;
                }
                kept[i] = true;
                changed = true;
                for (k, vals) in pre {
                    has_needed[*k] = true;
                    needed[*k].extend(vals.iter().copied());
                }
            }
            for e in effs {
                if whole[e.key] || !needed[e.key].is_empty() {
                    if e.reads.iter().any(|&r| !whole[r]) {
                        for &r in &e.reads {
                            whole[r] = true;
                        }
                        changed = true;
                    }
                }
            }
        }
    }
    let values: BTreeMap<String, BTreeSet<Option<Label>>> = (0..nf)
        .filter(|&i| has_needed[i] && !needed[i].is_empty())
        .map(|i| (names[i].to_string(), needed[i].clone()))
        .collect();
    let whole: BTreeSet<String> = (0..nf).filter(|&i| whole[i]).map(|i| names[i].to_string()).collect();
    let actions: Vec<usize> = (0..task.actions.len()).filter(|&i| kept[i]).collect();
    let mut rel = Relevance { actions, values, whole, counts: Counter::default() };
    let mut mentioned: BTreeSet<&str> = task.goal.iter().map(|g| g.0.as_str()).collect();
    for a in &task.actions {
        mentioned.extend(a.pre.iter().map(|p| p.0.as_str()));
        for e in &a.effects {
            mentioned.insert(&e.key);
            mentioned.extend(e.when.iter().map(|w| w.0.as_str()));
        }
    }
    let nfacts = rel.facts().len() as i64;
    let nvalues: i64 = rel.values.iter().filter(|(k, _)| !rel.whole.contains(*k)).map(|(_, v)| v.len() as i64).sum();
    rel.counts.set("actions", task.actions.len() as i64);
    rel.counts.set("actions kept", rel.actions.len() as i64);
    rel.counts.set("facts named", mentioned.len() as i64);
    rel.counts.set("facts needed", nfacts);
    rel.counts.set("facts needed whole", rel.whole.len() as i64);
    rel.counts.set("values needed", nvalues);
    rel.counts.set("rounds", rounds);
    rel
}

/// reader.py `prune`: relevance's actions, and in them no effect on a fact no kept precondition, effect condition
/// or goal reads.
pub fn prune(task: &Task, rel: &Relevance) -> Task {
    let needed = rel.facts();
    let actions: Vec<Action> = rel
        .actions
        .iter()
        .map(|&i| {
            let a = &task.actions[i];
            Action { effects: a.effects.iter().filter(|e| needed.contains(&e.key)).cloned().collect(), ..a.clone() }
        })
        .collect();
    let in_pre: FxHashSet<&str> = actions.iter().flat_map(|a| a.pre.iter().map(|p| p.0.as_str())).collect();
    let facts: Vec<(String, Domain)> =
        task.facts.iter().filter(|(k, _)| needed.contains(k) || in_pre.contains(k.as_str())).cloned().collect();
    let mut t = Task::new(facts, actions, task.goal.clone());
    t.folded = task.folded;
    t
}

/// reader.py `task_start`: the start as the task reads it, with `pos:` and `escpos:` (`keys`: the task's facts; a
/// unit the task names that the oracle's list lacks is NOWHERE). With the switch on (`table.elsewhere`), a unit whose
/// slot is at a place none of the scope's points, and at none of them, is ELSEWHERE.
pub fn task_start<'k>(b: &mut Builder, start: &Facts, keys: impl IntoIterator<Item = &'k String>) -> Facts {
    let mut out = start.clone();
    for k in keys {
        if pos_key(k).is_some() {
            out.insert(k.clone(), NOWHERE);
        }
    }
    for (kind, pos) in [("at", "pos"), ("esc", "escpos")] {
        let names: Vec<(String, Vec<i64>)> =
            b.table.points[kind].iter().map(|(n, ws)| (n.clone(), ws.iter().copied().collect())).collect();
        for (name, ws) in names {
            let ws: Vec<i64> = ws.into_iter().filter(|w| start.get(&format!("{kind}:{name}:{w}")) == Some(1)).collect();
            if ws.len() > 1 {
                b.counts.inc(format!("{kind}: more than one place at the start for one unit (the first taken)"));
            }
            out.insert(format!("{pos}:{name}"), ws.iter().copied().min().unwrap_or(NOWHERE));
        }
    }
    if b.table.elsewhere {
        // the switch (PORT-NOTES.md, "ELSEWHERE"): a slot at a place none of the scope's points, and at none of them,
        // starts at ELSEWHERE, not NOWHERE
        let mut away: BTreeSet<String> = BTreeSet::new();
        for (k, v) in start.iter() {
            if v != 1 {
                continue;
            }
            let (kind, pos) = if k.starts_with("at:") {
                ("at", "pos")
            } else if k.starts_with("esc:") {
                ("esc", "escpos")
            } else {
                continue;
            };
            let Some((name, w)) = k[kind.len() + 1..].rsplit_once(':') else { continue };
            let Ok(w) = w.parse::<i64>() else { continue };
            let key = format!("{pos}:{name}");
            if !b.table.is_point(kind, name, w) && out.get(&key) == Some(NOWHERE) {
                away.insert(key);
            }
        }
        for key in away {
            out.insert(key, ELSEWHERE);
            b.counts.inc("slots at the start at a place none of the scope's points (ELSEWHERE)");
        }
    }
    out
}

/// reader.py `replan_start`: `task_start` of the engine's state, with each `args:<unit>` copied from the plan's
/// prediction when the slot is where the plan put it, else 0.
pub fn replan_start<'k>(
    b: &mut Builder,
    observed: &Facts,
    predicted: &Facts,
    keys: impl IntoIterator<Item = &'k String> + Clone,
) -> (Facts, Counter) {
    let mut out = task_start(b, observed, keys.clone());
    let mut counts = Counter::default();
    for k in keys.into_iter().filter(|k| k.starts_with("args:")) {
        let name = &k[5..];
        let pos = out.get(&format!("pos:{name}")).unwrap_or(NOWHERE);
        if pos != NOWHERE && predicted.get(&format!("pos:{name}")) == Some(pos) && predicted.get0(k) != 0 {
            out.insert(k.clone(), predicted.get0(k));
            counts.inc("args copied from the plan (the slot is where the plan put it)");
        } else {
            out.insert(k.clone(), 0);
            if pos != NOWHERE {
                counts.inc("live slots with unknown locals (args 0)");
            }
        }
    }
    (out, counts)
}

/// The keys build_task's facts hold, without building the task (export_records.py `kitchen`): the goal's, and
/// every record's precondition, effect and condition keys.
pub fn record_keys(records: &[Record], goal: &[(String, Labels)]) -> Vec<String> {
    let mut keys: BTreeSet<String> = goal.iter().map(|g| g.0.clone()).collect();
    for r in records {
        keys.extend(r.pre.keys().cloned());
        for (k, e) in &r.effects {
            keys.insert(k.clone());
            if let Eff::When(src, _) | Eff::Copy(src) = e {
                keys.insert(src.clone());
            }
        }
    }
    keys.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `set_text` is the old one's text: the values' texts sorted as strings.
    #[test]
    fn set_text_sorts_texts() {
        let v = vec![Label::Below, Label::Int(-3), Label::Int(0), Label::Int(9), Label::Int(10), Label::Int(123456789012), Label::Above];
        let mut want: Vec<String> = v.iter().map(|l| l.text()).collect();
        want.sort();
        assert_eq!(set_text(&v), want.join(","));
        assert_eq!(set_text(&[]), "");
        assert_eq!(set_text(&[Label::Int(0)]), "0");
    }
    use crate::record::What;

    fn rec(pre_args: &[i64], eff_args: i64, other: i64) -> Record {
        let mut pre = crate::record::Pre::new();
        pre.insert("args:local:5".into(), pre_args.iter().map(|&x| Label::Int(x)).collect());
        pre.insert("var:3".into(), vec![Label::Int(other)]);
        Record {
            kind: "event".into(),
            label: "event(local:5@1)".into(),
            pre,
            effects: vec![("var:9".into(), Eff::Set(Label::Int(1))), ("args:local:7".into(), Eff::Set(Label::Int(eff_args)))],
            checks: vec![],
            end: Some("end".into()),
            notes: vec![],
            next_events: vec![],
            source: What::Verb(1),
            either: vec![],
            engine_pre: vec![],
            unknown: vec![],
        }
    }

    /// `_read_once`'s rewrite of a folded list, applied to a record by hand.
    fn rewritten(r: &Record, folded: &BTreeSet<i64>) -> Record {
        let f = |l: &Label| matches!(l, Label::Int(x) if folded.contains(x));
        let mut r = r.clone();
        for (k, e) in r.effects.iter_mut() {
            if k.starts_with("args:") && matches!(e, Eff::Set(v) if f(v)) {
                *e = Eff::Set(Label::Int(0));
            }
        }
        for (k, s) in r.pre.iter_mut() {
            if k.starts_with("args:") && s.iter().any(f) {
                *s = vec![Label::Int(0)];
            }
        }
        r
    }

    /// Two records fold after the rewrite exactly when build_task's key of the rewritten records is equal.
    #[test]
    fn fold_parts_match_the_key_after_the_rewrite() {
        let recs = [rec(&[11], 21, 0), rec(&[12], 22, 0), rec(&[11, 13], 21, 0), rec(&[0], 0, 0), rec(&[11], 21, 1), rec(&[14], 21, 0)];
        for folded in [vec![], vec![11], vec![11, 12, 21, 22], vec![11, 12, 13, 14, 21, 22]] {
            let folded: BTreeSet<i64> = folded.into_iter().collect();
            for a in &recs {
                for b in &recs {
                    let want = fold_key(&rewritten(a, &folded)) == fold_key(&rewritten(b, &folded));
                    let got = fold_parts(a).hash_after(&folded) == fold_parts(b).hash_after(&folded);
                    assert_eq!(got, want, "{folded:?}: {:?} / {:?}", a.pre, b.pre);
                }
            }
        }
        // with nothing folded, the parts' hash tells records apart as the key does
        assert_ne!(fold_parts(&recs[0]).hash_after(&BTreeSet::new()), fold_parts(&recs[1]).hash_after(&BTreeSet::new()));
    }
}

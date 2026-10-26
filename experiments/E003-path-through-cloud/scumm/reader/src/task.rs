//! The planner's task from the records: reader.py `build_task`, relevance.py `relevant` and `restrict`, reader.py
//! `prune`, `task_start` and `replan_start`, over task.py's `Task`, `Action` and `Effect`. task.py's `write_sas` is
//! taskcore's (`--fold off --reduce none`, byte for byte); the task reaches it through the packed format (packed.rs).

use crate::builder::{Builder, SlotFlags, SlotLocals};
use crate::domain::{Domain, Label, Labels};
use crate::facts::Facts;
use crate::py::{Counter, Py};
use crate::record::{Eff, Record, Val};
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
            Eff::Cases(cs) => {
                for (when, vals) in cs {
                    match vals.as_slice() {
                        [v] => effects.push(TEffect::set(k, *v, when.clone())),
                        _ => effects.push(TEffect { key: k.clone(), value: None, add: None, values: Some(vals.clone()), when: when.clone() }),
                    }
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
            for src in e.sources() {
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
    let mut due: Vec<String> = Vec::new();
    for k in keys {
        if pos_key(k).is_some() {
            out.insert(k.clone(), NOWHERE);
        } else if b.scope.switches.actor_rooms && k.starts_with("actorroom:") && !start.contains(k) {
            // F9: an actor's room the start does not give is not known (table::NOT_KNOWN), not room 0
            out.insert(k.clone(), crate::table::NOT_KNOWN);
        } else if b.scope.switches.input_script && k == "mouse" && !start.contains(k) {
            // F1: where the mouse is, when the start does not give it, is not known (inputs.rs NOWHERE)
            out.insert(k.clone(), crate::inputs::NOWHERE);
        }
        if k.starts_with(crate::table::DUE) {
            due.push(k.clone());
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
    // fix round 1, switch due_first: each due: fact from the start's position, slot, freeze and test facts
    if let Some(d) = &b.due {
        let mut vals = Vec::new();
        for k in &due {
            let get = |f: &str| -> Option<Label> {
                let v = out.get0(f);
                Some(b.table.label(f, v).unwrap_or(Label::Int(v)))
            };
            vals.push((k.clone(), d.start_value(k, &get).unwrap_or(0)));
        }
        for (k, v) in vals {
            out.insert(k, v);
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

/// F10: a list of locals the reading built for a unit at a wait point: the locals of a record's next event whose
/// `args:<unit>` the record still sets to the list's number (a list SIG_LIMIT folded into 0 is no list).
#[derive(Clone, Debug, PartialEq)]
pub struct BuiltList {
    pub sig: i64,
    /// Local[n] -> value, as the event holds them (a local the list lacks is 0: its args are known)
    pub locals: Vec<(i64, Val)>,
    pub override_: i64,
    pub cut_data: Vec<i64>,
    /// the event's slot flags, which the number covers under the switch --live-locals (`Builder::list_number`)
    pub recursive: Py,
    pub resistant: Py,
    pub owned: Py,
}

impl BuiltList {
    fn of(e: &crate::record::Event) -> BuiltList {
        BuiltList {
            sig: e.sig,
            locals: e.locals.clone(),
            override_: e.override_,
            cut_data: e.cut_data.clone(),
            recursive: e.recursive.clone(),
            resistant: e.resistant.clone(),
            owned: e.owned.clone(),
        }
    }
}

/// F10 under the switch --live-locals: what a match compares besides the override and the cutscene data: the locals
/// the slot's unit reads (None: all of them) and the slot's flags from the dump.
#[derive(Clone, Copy, Debug)]
pub struct LiveMatch<'a> {
    pub reads: Option<&'a BTreeSet<i32>>,
    pub flags: SlotFlags,
}

/// F10: what a match of `slot`'s dumped locals compares: None off the switch --live-locals (every local, no flags);
/// under it the slot's unit's read locals and the dump's flags. A slot without flags under the switch stops here: a
/// list's number covers them, so no list could be told to be the slot's.
pub fn live_match<'a>(b: &'a Builder, slot: &SlotLocals) -> Option<LiveMatch<'a>> {
    let ll = b.table.live_locals.as_ref()?;
    let flags = slot.flags.unwrap_or_else(|| {
        panic!(
            "F10 under --live-locals: the dump gives slot {} no flags (resistant, recursive), and a list's number \
             covers them; dump the slots with a build whose SPEEDRUN_DUMP_LOCALS lists them",
            slot.name
        )
    });
    let unit = Builder::unit_of_name(&slot.name, b.game.resource(slot.room));
    Some(LiveMatch { reads: unit.and_then(|u| ll.get(u)), flags })
}

/// (unit name, wait point) -> the lists built there, each once, in the records' order.
pub type BuiltLists = BTreeMap<(String, i64), Vec<BuiltList>>;

/// F10: the lists of locals the records built (`BuiltList`), by unit name and wait point. Esc events share their
/// slot's list and are not listed apart.
pub fn built_lists(records: &[Record]) -> BuiltLists {
    let mut out = BuiltLists::new();
    for r in records {
        for e in r.next_events.iter().filter(|e| e.sig != 0 && !e.abort) {
            let set = r.effects.iter().any(|(k, x)| k.strip_prefix("args:") == Some(e.name.as_str()) && *x == Eff::Set(Label::Int(e.sig)));
            if !set {
                continue;
            }
            let v = out.entry((e.name.clone(), e.w)).or_default();
            if !v.iter().any(|l| l.sig == e.sig) {
                v.push(BuiltList::of(e));
            }
        }
    }
    out
}

/// F10: the lists of the events read from dumped locals (`Builder.slot_events`), by unit name and wait point, but
/// a list SIG_LIMIT folded. They are no record's next event, so `built_lists` lacks them.
pub fn slot_lists(b: &Builder) -> BuiltLists {
    let mut out = BuiltLists::new();
    for e in b.slot_events.iter().filter(|e| e.sig != 0 && !b.folded.contains(&e.sig)) {
        let v = out.entry((e.name.clone(), e.w)).or_default();
        if !v.iter().any(|l| l.sig == e.sig) {
            v.push(BuiltList::of(e));
        }
    }
    out
}

/// F10: whether a slot's dumped locals are a built list's: each of the dump's locals equals the list's number (0
/// where the list lacks it), or the list's local is unknown and its value set, if it has one, holds the dumped value;
/// a local past the dump's end, the override count and the cutscene data must be equal too. Under the switch
/// --live-locals (`live`) only the locals the slot's unit reads are compared (the list holds no other), and the
/// list's flags must be the dump's as the number takes them (`locals::recursive` for recursive): the number covers
/// them.
pub fn list_matches(list: &BuiltList, slot: &SlotLocals, live: Option<&LiveMatch>) -> bool {
    if list.override_ != slot.override_ || list.cut_data != slot.cut_data {
        return false;
    }
    if let Some(lm) = live {
        let f = lm.flags;
        if list.recursive != crate::locals::recursive(&Py::Bool(f.recursive))
            || list.resistant != Py::Bool(f.resistant)
            || list.owned != Py::Bool(f.owned)
        {
            return false;
        }
    }
    if list.locals.iter().any(|(n, _)| *n < 0 || *n as usize >= slot.locals.len()) {
        return false;
    }
    let reads = live.and_then(|lm| lm.reads);
    slot.locals
        .iter()
        .enumerate()
        .filter(|(n, _)| reads.is_none_or(|r| r.contains(&(*n as i32))))
        .all(|(n, &x)| match list.locals.iter().find(|(i, _)| *i == n as i64) {
            None => x == 0,
            Some((_, Val::Int(v))) => *v == x,
            Some((_, Val::Unk(u))) => u.values.as_ref().map_or(true, |vs| vs.contains(&x)),
        })
}

/// F10: the list a slot's dumped locals match at its wait point: the one with the most locals known as numbers, the
/// first built among equals; None when none matches. `live`: `live_match`'s.
pub fn match_list<'a>(lists: &'a BuiltLists, slot: &SlotLocals, w: i64, live: Option<&LiveMatch>) -> Option<&'a BuiltList> {
    let known = |l: &BuiltList| l.locals.iter().filter(|(_, v)| matches!(v, Val::Int(_))).count();
    let mut best: Option<&BuiltList> = None;
    for l in lists.get(&(slot.name.clone(), w)).into_iter().flatten().filter(|l| list_matches(l, slot, live)) {
        if best.map_or(true, |b| known(l) > known(b)) {
            best = Some(l);
        }
    }
    best
}

/// reader.py `replan_start` with F10's dumped locals: `task_start` of the engine's state; then each `args:<unit>`
/// of a live slot (`pos:<unit>` not NOWHERE) is the number of the built list its dumped locals match
/// (`match_list`), whatever the plan predicted. A slot whose locals match no list gets 0 and is returned, for its
/// events to be read again from its dumped locals (`Builder.slot_locals`). A live slot the dump gives no locals for
/// (`slots` lacks it) keeps reader.py's rule: the plan's number when it is where the plan put it, else 0. A slot at
/// ELSEWHERE (the switch, `task_start`) gets 0: no list is built there. Under --live-locals a match compares the
/// slot's read locals and its flags (`live_match`; a dumped slot without flags stops here). Returns (start, counts,
/// the slots no list matches).
pub fn replan_start_locals<'k>(
    b: &mut Builder,
    observed: &Facts,
    predicted: &Facts,
    keys: impl IntoIterator<Item = &'k String> + Clone,
    slots: &BTreeMap<String, SlotLocals>,
    lists: &BuiltLists,
) -> (Facts, Counter, Vec<SlotLocals>) {
    let mut out = task_start(b, observed, keys.clone());
    let mut counts = Counter::default();
    let mut unmatched = Vec::new();
    for k in keys.into_iter().filter(|k| k.starts_with("args:")) {
        let name = &k[5..];
        let pos = out.get(&format!("pos:{name}")).unwrap_or(NOWHERE);
        if pos == NOWHERE {
            out.insert(k.clone(), 0);
            continue;
        }
        if pos == ELSEWHERE {
            out.insert(k.clone(), 0);
            counts.inc("live slots at ELSEWHERE (args 0)");
            continue;
        }
        let Some(slot) = slots.get(name) else {
            counts.inc("live slots with no locals in the dump");
            if predicted.get(&format!("pos:{name}")) == Some(pos) && predicted.get0(k) != 0 {
                out.insert(k.clone(), predicted.get0(k));
                counts.inc("args copied from the plan (the slot is where the plan put it)");
            } else {
                out.insert(k.clone(), 0);
                counts.inc("live slots with unknown locals (args 0)");
            }
            continue;
        };
        let live = live_match(b, slot);
        match match_list(lists, slot, pos, live.as_ref()) {
            Some(l) => {
                out.insert(k.clone(), l.sig);
                counts.inc("live slots whose dumped locals match a built list");
                if predicted.get(&format!("pos:{name}")) == Some(pos) && predicted.get0(k) != 0 && predicted.get0(k) != l.sig {
                    counts.inc("live slots whose dumped locals are not the plan's list");
                }
            }
            None => {
                out.insert(k.clone(), 0);
                counts.inc("live slots with unknown locals (args 0)");
                unmatched.push(SlotLocals { w: pos, ..slot.clone() });
            }
        }
    }
    (out, counts, unmatched)
}

/// The keys build_task's facts hold, without building the task (export_records.py `kitchen`): the goal's, and
/// every record's precondition, effect and condition keys.
pub fn record_keys(records: &[Record], goal: &[(String, Labels)]) -> Vec<String> {
    let mut keys: BTreeSet<String> = goal.iter().map(|g| g.0.clone()).collect();
    for r in records {
        keys.extend(r.pre.keys().cloned());
        for (k, e) in &r.effects {
            keys.insert(k.clone());
            keys.extend(e.sources().into_iter().map(String::from));
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

    fn slot(name: &str, w: i64, locals: &[i64]) -> SlotLocals {
        let mut l = locals.to_vec();
        l.resize(25, 0);
        SlotLocals { name: name.into(), room: 33, w, esc: None, locals: l, override_: 0, cut_data: vec![], owned: false, flags: None }
    }

    fn list(sig: i64, locals: Vec<(i64, Val)>) -> BuiltList {
        BuiltList { sig, locals, override_: 0, cut_data: vec![], recursive: Py::None, resistant: Py::Bool(false), owned: Py::Bool(false) }
    }

    /// F10's match: numbers equal, a missing local is 0, an unknown local takes what its set allows, the cutscene
    /// fields equal.
    #[test]
    fn dumped_locals_match_a_built_list() {
        use crate::record::Unknown;
        let s = slot("script:2", 736, &[2, 428, 0, 1]);
        assert!(list_matches(&list(1, vec![(0, Val::Int(2)), (1, Val::Int(428)), (3, Val::Int(1))]), &s, None));
        assert!(list_matches(&list(1, vec![(0, Val::Int(2)), (1, Val::Int(428)), (2, Val::Int(0)), (3, Val::Int(1))]), &s, None));
        assert!(!list_matches(&list(1, vec![(0, Val::Int(2)), (1, Val::Int(428))]), &s, None), "Local[3] is 1, the list's 0");
        assert!(!list_matches(&list(1, vec![(0, Val::Int(2)), (1, Val::Int(427)), (3, Val::Int(1))]), &s, None));
        let any = Val::Unk(Unknown::new("local", "Local[1] from a fact", None));
        let set = |v: Vec<i64>| Val::Unk(Unknown::new("local", "Local[1]", Some(v)));
        assert!(list_matches(&list(1, vec![(0, Val::Int(2)), (1, any), (3, Val::Int(1))]), &s, None));
        assert!(list_matches(&list(1, vec![(0, Val::Int(2)), (1, set(vec![427, 428])), (3, Val::Int(1))]), &s, None));
        assert!(!list_matches(&list(1, vec![(0, Val::Int(2)), (1, set(vec![427])), (3, Val::Int(1))]), &s, None));
        assert!(!list_matches(&list(1, vec![(30, Val::Int(0))]), &slot("script:2", 736, &[]), None), "a local past the dump");
        let mut c = list(1, vec![]);
        c.override_ = 1;
        c.cut_data = vec![5];
        let mut s2 = slot("script:2", 736, &[]);
        assert!(!list_matches(&c, &s2, None));
        s2.override_ = 1;
        s2.cut_data = vec![5];
        assert!(list_matches(&c, &s2, None));
        // at the slot's wait point only; the list with more numbers wins
        let mut lists = BuiltLists::new();
        let loose = list(7, vec![(0, Val::Int(2)), (1, Val::Unk(Unknown::new("local", "x", None))), (3, Val::Int(1))]);
        let tight = list(8, vec![(0, Val::Int(2)), (1, Val::Int(428)), (3, Val::Int(1))]);
        lists.insert(("script:2".into(), 736), vec![loose.clone(), tight.clone()]);
        assert_eq!(match_list(&lists, &s, 736, None).map(|l| l.sig), Some(8));
        assert_eq!(match_list(&lists, &s, 740, None), None);
        lists.insert(("script:2".into(), 736), vec![loose]);
        assert_eq!(match_list(&lists, &s, 736, None).map(|l| l.sig), Some(7));
    }

    /// F10 under --live-locals: only the read locals are compared, and the list's flags must be the dump's as the
    /// number takes them (recursive false and unknown alike).
    #[test]
    fn dumped_locals_match_under_live_locals() {
        let s = slot("script:2", 736, &[2, 428, 0, 1]);
        let reads: BTreeSet<i32> = [0, 3].into_iter().collect();
        let flags = SlotFlags { resistant: false, recursive: false, owned: false };
        let lm = LiveMatch { reads: Some(&reads), flags };
        // the list holds only the read locals: Local[1] = 428 is not read, so it is not compared
        let l = list(1, vec![(0, Val::Int(2)), (3, Val::Int(1))]);
        assert!(!list_matches(&l, &s, None), "every local compared off the switch");
        assert!(list_matches(&l, &s, Some(&lm)));
        assert!(!list_matches(&list(1, vec![(0, Val::Int(2))]), &s, Some(&lm)), "Local[3] is read and is 1");
        // the flags: resistant and owned as they are, recursive as true or not
        let mut r = l.clone();
        r.resistant = Py::Bool(true);
        assert!(!list_matches(&r, &s, Some(&lm)));
        let lm_r = LiveMatch { reads: Some(&reads), flags: SlotFlags { resistant: true, ..flags } };
        assert!(list_matches(&r, &s, Some(&lm_r)));
        let mut o = l.clone();
        o.owned = Py::Bool(true);
        assert!(!list_matches(&o, &s, Some(&lm)));
        let lm_rec = LiveMatch { reads: Some(&reads), flags: SlotFlags { recursive: true, ..flags } };
        assert!(!list_matches(&l, &s, Some(&lm_rec)), "a recursive slot is not a list with recursive unknown");
        let mut t = l.clone();
        t.recursive = Py::Bool(true);
        assert!(list_matches(&t, &s, Some(&lm_rec)));
        assert!(!list_matches(&t, &s, Some(&lm)));
        // a unit whose reads are not known exactly: every local compared, the flags too
        let all = LiveMatch { reads: None, flags };
        assert!(!list_matches(&l, &s, Some(&all)));
        assert!(list_matches(&list(1, vec![(0, Val::Int(2)), (1, Val::Int(428)), (3, Val::Int(1))]), &s, Some(&all)));
    }

    /// `built_lists` keeps a next event's list only where its record still sets args:<unit> to it.
    #[test]
    fn built_lists_skip_folded_lists() {
        let ev = |sig: i64| crate::record::Event {
            name: "local:7".into(),
            unit: scumm_ir::UnitKey::Local(33, 7),
            w: 4,
            sig,
            locals: vec![(0, Val::Int(9))],
            args_known: true,
            recursive: crate::py::Py::None,
            resistant: crate::py::Py::None,
            owned: crate::py::Py::Bool(false),
            abort: false,
            override_: 0,
            cut_data: vec![],
        };
        let mut a = rec(&[0], 0, 0);
        a.effects = vec![("args:local:7".into(), Eff::Set(Label::Int(21)))];
        a.next_events = vec![ev(21)];
        let mut b = a.clone();
        b.effects = vec![("args:local:7".into(), Eff::Set(Label::Int(0)))];
        b.next_events = vec![ev(22)];
        let lists = built_lists(&[a.clone(), b, a]);
        assert_eq!(lists.len(), 1);
        assert_eq!(lists[&("local:7".to_string(), 4)].iter().map(|l| l.sig).collect::<Vec<_>>(), vec![21]);
    }
}

//! The switch --reach (experiments/E004-fast-exact/reach.md has the proof): forward reachability in the reader. A
//! monotone set of reached fact values, as taskcore's step (d) computes it, is kept while the reading runs; a job or a
//! fork's branch that needs a value not reached yet does not run, and runs when the value arrives. The written records
//! are those (d) keeps, each once. Off, none of this runs and the reading is byte for byte as before.

use crate::builder::{Builder, Job, Machine, RecordSink};
use crate::domain::{Label, Labels};
use crate::facts::Facts;
use crate::machine::{pkey_of, PKey};
use crate::memo::{Query, RunLog};
use crate::py::Counter;
use crate::record::{Event, PathOut, Pre, Record};
use crate::table::{pos_key, FactTable, NOWHERE};
use crate::task::{record_action, task_start};
use rustc_hash::{FxHashMap, FxHashSet};
use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};

/// What a run agrees with to follow a branch: facts and their value sets, sorted by fact.
pub type Guide = Vec<(PKey, Labels)>;

/// A branch held back: the path that forked (`guide`), and the fact and value set it waits for.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Held {
    pub guide: Guide,
    pub key: String,
    pub set: Labels,
}

pub fn meet(a: &[Label], b: &[Label]) -> bool {
    a.iter().any(|x| b.contains(x))
}

/// Whether a guide agrees with a path's precondition and the group it would take: every fact both name has a common
/// value. No guides (a job's first run): everything agrees.
pub fn agrees(guides: Option<&Vec<Guide>>, pre: &BTreeMap<PKey, Labels>, key: PKey, g: &[Label]) -> bool {
    let Some(gs) = guides else { return true };
    gs.iter().any(|guide| {
        guide.iter().all(|(k, s)| {
            if *k == key {
                meet(s, g)
            } else {
                pre.get(k).is_none_or(|t| meet(s, t))
            }
        })
    })
}

/// A guide made of a path's precondition and a group (`extra`), the job's own seed left out: the run has it anyway.
pub fn make_guide(pre: &BTreeMap<PKey, Labels>, seed: &BTreeMap<PKey, Labels>, extra: Option<(PKey, &Labels)>) -> Guide {
    let mut out: BTreeMap<PKey, Labels> = pre.iter().filter(|(k, v)| seed.get(*k) != Some(*v)).map(|(k, v)| (*k, v.clone())).collect();
    if let Some((k, g)) = extra {
        out.insert(k, g.clone());
    }
    out.into_iter().collect()
}

/// `at:<unit>:W` as (`pos:<unit>`, W), `esc:<unit>:W` as (`escpos:<unit>`, W); the facts a record writes for them.
fn place_of(key: &str) -> Option<(String, i64)> {
    let (rest, pos) = if let Some(r) = key.strip_prefix("at:") {
        (r, "pos")
    } else if let Some(r) = key.strip_prefix("esc:") {
        (r, "escpos")
    } else {
        return None;
    };
    let (name, w) = rest.rsplit_once(':')?;
    Some((format!("{pos}:{name}"), w.parse().ok()?))
}

/// The fact a wait on `key` wakes on.
fn watch_key(key: &str) -> String {
    match place_of(key) {
        Some((p, _)) => p,
        None => key.to_string(),
    }
}

#[derive(Default)]
struct Inner {
    /// the round being read (a batch's jobs all read this round's values)
    now: u32,
    /// fact -> value -> the round it arrived in
    vals: FxHashMap<String, FxHashMap<Label, u32>>,
    /// a rerun's guides, by its job's label, for this round
    guides: FxHashMap<String, Arc<Vec<Guide>>>,
}

/// The reached values, shared with the workers. Written only between batches.
#[derive(Default)]
pub struct Reached {
    inner: RwLock<Inner>,
}

/// A fact's start value when the start file does not name it: what `Writer::close` writes for it (`get0`).
fn default_of(table: &FactTable, key: &str) -> Option<Label> {
    if pos_key(key).is_some() {
        return Some(Label::Int(NOWHERE));
    }
    if key.starts_with("args:") {
        return Some(Label::Int(0));
    }
    let d = table.domain(key)?;
    d.label(0).filter(|l| d.contains(*l))
}

impl Reached {
    fn has(i: &Inner, table: &FactTable, key: &str, l: Label, upto: i64) -> bool {
        match i.vals.get(key) {
            Some(m) => m.get(&l).is_some_and(|&r| r as i64 <= upto),
            None => default_of(table, key) == Some(l),
        }
    }

    fn other(i: &Inner, table: &FactTable, key: &str, w: i64, upto: i64) -> bool {
        match i.vals.get(key) {
            Some(m) => m.iter().any(|(l, &r)| *l != Label::Int(w) && r as i64 <= upto),
            None => default_of(table, key).is_some_and(|l| l != Label::Int(w)),
        }
    }

    fn meets_at(i: &Inner, table: &FactTable, key: &str, set: &[Label], upto: i64) -> bool {
        if let Some((pos, w)) = place_of(key) {
            return set.iter().any(|l| match l {
                Label::Int(1) => Self::has(i, table, &pos, Label::Int(w), upto),
                Label::Int(0) => Self::other(i, table, &pos, w, upto),
                _ => false,
            });
        }
        set.iter().any(|l| Self::has(i, table, key, *l, upto))
    }

    /// Some value of `set` is reached in the round being read.
    pub fn meets(&self, table: &FactTable, key: &str, set: &[Label]) -> bool {
        let i = self.inner.read().unwrap();
        let t = i.now as i64;
        Self::meets_at(&i, table, key, set, t)
    }

    /// The first set of `pre` that no reached value meets. The questions asked go to `log` (memo.rs).
    pub fn first_unmet<'a, I>(&self, table: &FactTable, pre: I, log: Option<&mut RunLog>) -> Option<(String, Labels)>
    where
        I: IntoIterator<Item = (&'a String, &'a Labels)>,
    {
        let i = self.inner.read().unwrap();
        let t = i.now as i64;
        let mut log = log;
        for (k, s) in pre {
            let answer = Self::meets_at(&i, table, k, s, t);
            if let Some(l) = log.as_deref_mut() {
                l.queries.push(Query { key: k.clone(), set: s.clone(), before: false, answer });
            }
            if !answer {
                return Some((k.clone(), s.clone()));
            }
        }
        None
    }

    /// Whether every set of `pre` met the values of the round before: the record was written then.
    pub fn was_met(&self, table: &FactTable, pre: &[(String, Labels)], log: Option<&mut RunLog>) -> bool {
        let i = self.inner.read().unwrap();
        let t = i.now as i64 - 1;
        let mut log = log;
        for (k, s) in pre {
            let answer = Self::meets_at(&i, table, k, s, t);
            if let Some(l) = log.as_deref_mut() {
                l.queries.push(Query { key: k.clone(), set: s.clone(), before: true, answer });
            }
            if !answer {
                return false;
            }
        }
        true
    }

    /// Every question gets the answer it got (memo.rs).
    pub fn answers_hold(&self, table: &FactTable, queries: &[Query]) -> bool {
        let i = self.inner.read().unwrap();
        let now = i.now as i64;
        queries.iter().all(|q| Self::meets_at(&i, table, &q.key, &q.set, if q.before { now - 1 } else { now }) == q.answer)
    }

    pub fn guides_of(&self, label: &str) -> Option<Arc<Vec<Guide>>> {
        self.inner.read().unwrap().guides.get(label).cloned()
    }

    /// The reached values of a fact, in the domain's order.
    pub fn values(&self, table: &FactTable, key: &str) -> Labels {
        let i = self.inner.read().unwrap();
        let mut v: Labels = match i.vals.get(key) {
            Some(m) => m.keys().copied().collect(),
            None => default_of(table, key).into_iter().collect(),
        };
        v.sort();
        v
    }

    /// `NAME.reached`'s lines: one per fact, its key and its reached values (reach.md, "A replan start").
    pub fn lines<'k>(&self, table: &FactTable, keys: impl IntoIterator<Item = &'k String>) -> Vec<String> {
        keys.into_iter()
            .map(|k| {
                let vals: Vec<String> = self.values(table, k).iter().map(|l| l.text()).collect();
                format!("{k}\t{}", vals.join("\t"))
            })
            .collect()
    }
}

/// A start file in taskcore's --start-file format: `key<TAB>value`, a value's text; a value that is no integer or is
/// empty is left out (a collapsed value is read from the reading's domain by its own start check).
pub fn start_file(path: &str) -> Facts {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{path}: {e}"));
    Facts::from_items(text.lines().filter_map(|l| {
        let (k, v) = l.split_once('\t')?;
        Some((k.to_string(), v.parse::<i64>().ok()?))
    }))
}

/// A job's whole run (guide None) or a branch of it, waiting for a set to meet the reached values.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct Wait {
    job: u32,
    guide: Option<Guide>,
    key: String,
    set: Labels,
}

/// An effect whose conditions do not hold yet.
struct PendingEff {
    when: Vec<(String, Labels)>,
    key: String,
    vals: Labels,
}

/// The rounds' state on the reading's thread, for one pass. It stays after the pass: `Builder::reach_extend`.
#[derive(Default)]
pub struct ReachState {
    /// the jobs that held something back (their labels are their identity), and their numbers
    jobs: Vec<Job>,
    ids: FxHashMap<String, u32>,
    waits: FxHashMap<String, Vec<Arc<Wait>>>,
    wait_set: FxHashSet<Arc<Wait>>,
    ready_jobs: Vec<u32>,
    ready_guides: BTreeMap<u32, Vec<Guide>>,
    fresh: Vec<(String, Label)>,
    effects: FxHashMap<String, Vec<PendingEff>>,
    todo: Vec<Event>,
    pub rounds: i64,
    pub max_waits: usize,
}

impl ReachState {
    fn id_of(&mut self, job: &Job) -> u32 {
        if let Some(&i) = self.ids.get(&job.label) {
            return i;
        }
        let i = self.jobs.len() as u32;
        self.jobs.push(job.clone());
        self.ids.insert(job.label.clone(), i);
        i
    }

    fn add_wait(&mut self, w: Wait) {
        let w = Arc::new(w);
        if self.wait_set.insert(w.clone()) {
            self.waits.entry(watch_key(&w.key)).or_default().push(w);
        }
    }
}

impl Builder<'_> {
    fn reach_arc(&self) -> Arc<Reached> {
        self.table.reach.clone().expect("the switch --reach")
    }

    /// Stage 1 of a path's end: the path's precondition as its record will have it; false: it does not meet the
    /// reached values, and is held on the first set that fails, so no record is made of it.
    pub(crate) fn reach_path(&mut self, job: &Job, p: &PathOut) -> bool {
        let reach = self.reach_arc();
        let mut pre = Pre::new();
        for (k, s) in &p.pre {
            if let Some(dom) = self.table.domain(k) {
                if *s != dom.values || k.starts_with("args:") {
                    pre.insert(k.clone(), s.clone());
                }
            }
        }
        let (pre, _) = self.positions(pre, Vec::new());
        match reach.first_unmet(&self.table, pre.iter(), self.memo_log.as_mut()) {
            None => true,
            Some((key, set)) => {
                self.reach_hold_path(job, p, key, set);
                false
            }
        }
    }

    fn reach_hold_path(&mut self, job: &Job, p: &PathOut, key: String, set: Labels) {
        let raw: BTreeMap<PKey, Labels> = p.pre.iter().map(|(k, v)| (pkey_of(k), v.clone())).collect();
        let seed: BTreeMap<PKey, Labels> = job.pre.iter().map(|(k, v)| (pkey_of(k), v.clone())).collect();
        self.reach_out.push(Held { guide: make_guide(&raw, &seed, None), key, set });
    }

    /// Stage 2, on the record: its action's precondition (`record_action`: a step that needs room in its domain adds
    /// to it). Whether to write it: it meets the reached values, and in a rerun it did not meet them the round before.
    pub(crate) fn reach_record(&mut self, job: &Job, p: &PathOut, rec: &Record) -> bool {
        let reach = self.reach_arc();
        let mut c = Counter::default();
        let (pre, _) = record_action(&self.table, rec, &mut c);
        if let Some((key, set)) = reach.first_unmet(&self.table, pre.iter().map(|(k, s)| (k, s)), self.memo_log.as_mut()) {
            self.reach_hold_path(job, p, key, set);
            return false;
        }
        if reach.guides_of(&job.label).is_some() && reach.was_met(&self.table, &pre, self.memo_log.as_mut()) {
            self.counts.inc("reach: records of a rerun already written");
            return false;
        }
        true
    }

    /// `run_split`'s parts under the switch: the values a part can run with; the others are held.
    pub(crate) fn reach_parts(&mut self, job: &Job, pre: &Pre, key: &str, vals: Labels) -> Labels {
        let reach = self.reach_arc();
        let guides = reach.guides_of(&job.label);
        let raw: BTreeMap<PKey, Labels> = pre.iter().map(|(k, v)| (pkey_of(k), v.clone())).collect();
        let seed: BTreeMap<PKey, Labels> = job.pre.iter().map(|(k, v)| (pkey_of(k), v.clone())).collect();
        let pk = pkey_of(key);
        let mut keep = Labels::new();
        for v in vals {
            let g = vec![v];
            if !agrees(guides.as_deref(), &raw, pk, &g) {
                continue;
            }
            let answer = reach.meets(&self.table, key, &g);
            if let Some(l) = &mut self.memo_log {
                l.queries.push(Query { key: key.to_string(), set: g.clone(), before: false, answer });
            }
            if answer {
                keep.push(v);
            } else {
                self.reach_out.push(Held { guide: make_guide(&raw, &seed, Some((pk, &g))), key: key.to_string(), set: g });
            }
        }
        keep
    }

    /// The values of a pass's start, or of a later start (`reach_extend`).
    fn reach_seed_values(&mut self, start: &Facts) -> Vec<(String, Label)> {
        let keys: Vec<String> = if self.scope.switches.actor_rooms {
            (0..crate::scope::NUM_ACTORS).map(|a| format!("actorroom:{a}")).collect()
        } else {
            Vec::new()
        };
        let s0 = task_start(self, start, keys.iter());
        let mut out = Vec::new();
        for (k, v) in s0.iter() {
            if k.starts_with("at:") || k.starts_with("esc:") {
                continue;
            }
            if pos_key(k).is_some() || k.starts_with("args:") {
                out.push((k.clone(), Label::Int(v)));
            } else if let Some(d) = self.table.domain(k) {
                if let Some(l) = d.label(v).filter(|l| d.contains(*l)) {
                    out.push((k.clone(), l));
                }
            }
        }
        out
    }

    /// One of (d)'s effects: fired if its conditions hold, else it waits on the first that does not.
    fn reach_effect(&mut self, reach: &Reached, when: Vec<(String, Labels)>, key: &str, vals: Labels) {
        match reach.first_unmet(&self.table, when.iter().map(|(k, s)| (k, s)), None) {
            None => {
                let st = self.reach_state.as_mut().unwrap();
                st.fresh.extend(vals.into_iter().map(|v| (key.to_string(), v)));
            }
            Some((k, _)) => {
                let st = self.reach_state.as_mut().unwrap();
                st.effects.entry(k).or_default().push(PendingEff { when, key: key.to_string(), vals });
            }
        }
    }

    /// A written value as (d) takes it: `packed.rs` writes a value past a domain's bounds as the collapsed value, and
    /// taskcore's load gives a value the domain lacks no index (K_BAD, which reaches nothing).
    fn reach_value(&self, key: &str, l: Label) -> Option<Label> {
        if key.starts_with("args:") {
            return Some(l);
        }
        let d = self.table.domain(key)?;
        d.label_of(l).filter(|x| d.contains(*x))
    }

    /// (d)'s effects of a written record (`packed.rs` of taskcore: `add` as one effect per current value).
    fn reach_apply(&mut self, reach: &Reached, r: &Record) {
        let mut c = Counter::default();
        let (_, effects) = record_action(&self.table, r, &mut c);
        for e in effects {
            if let Some(d) = e.add {
                let dom = self.table.domain(&e.key).expect("a step's fact has a domain").into_owned();
                let size = dom.values.len();
                let own: Option<Labels> = e.when.iter().rev().find(|(f, s)| *f == e.key && s.iter().filter(|x| dom.contains(**x)).count() < size).map(|x| x.1.clone());
                let rest: Vec<(String, Labels)> = e.when.iter().filter(|(f, _)| *f != e.key).cloned().collect();
                for &u in &dom.values {
                    if own.as_ref().is_some_and(|o| !o.contains(&u)) {
                        continue;
                    }
                    let nxt = dom.add(u, d);
                    if nxt.is_empty() || (size == 1 && nxt == [u]) {
                        continue;
                    }
                    let mut w = rest.clone();
                    w.push((e.key.clone(), vec![u]));
                    self.reach_effect(reach, w, &e.key, nxt);
                }
                continue;
            }
            let vals: Labels = match (e.value, &e.values) {
                (Some(v), _) => vec![v],
                (None, Some(vs)) => vs.clone(),
                _ => continue,
            };
            let got: Vec<Option<Label>> = vals.iter().map(|&v| self.reach_value(&e.key, v)).collect();
            if got.iter().any(|x| x.is_none()) {
                continue;
            }
            self.reach_effect(reach, e.when.clone(), &e.key, got.into_iter().flatten().collect());
        }
    }

    /// What one job's run left: its held branches become waits; its records' effects are applied.
    fn reach_after_job(&mut self, reach: &Reached, job: &Job, recs: &[Record]) {
        let held = std::mem::take(&mut self.reach_out);
        if !held.is_empty() {
            let st = self.reach_state.as_mut().unwrap();
            let id = st.id_of(job);
            for h in held {
                st.add_wait(Wait { job: id, guide: Some(h.guide), key: h.key, set: h.set });
            }
        }
        for r in recs {
            self.reach_apply(reach, r);
        }
    }

    /// A job that does not meet the reached values waits on the first set that fails; true: it can run.
    fn reach_job_ready(&mut self, reach: &Reached, job: &Job) -> bool {
        match reach.first_unmet(&self.table, job.pre.iter(), None) {
            None => true,
            Some((key, set)) => {
                let st = self.reach_state.as_mut().unwrap();
                let id = st.id_of(job);
                st.add_wait(Wait { job: id, guide: None, key, set });
                false
            }
        }
    }

    /// The round ends: the new values go in together, (d)'s closure runs on what waited on them, and what they free
    /// waits for the next batch.
    fn reach_commit(&mut self, reach: &Reached) {
        let mut new_keys: FxHashSet<String> = FxHashSet::default();
        {
            let mut i = reach.inner.write().unwrap();
            i.now += 1;
            i.guides.clear();
        }
        loop {
            let fresh = std::mem::take(&mut self.reach_state.as_mut().unwrap().fresh);
            if fresh.is_empty() {
                break;
            }
            let mut woken: Vec<String> = Vec::new();
            {
                let mut i = reach.inner.write().unwrap();
                let now = i.now;
                for (k, l) in fresh {
                    if !i.vals.contains_key(&k) {
                        let m: FxHashMap<Label, u32> = default_of(&self.table, &k).into_iter().map(|d| (d, 0)).collect();
                        i.vals.insert(k.clone(), m);
                    }
                    let m = i.vals.get_mut(&k).unwrap();
                    if !m.contains_key(&l) {
                        m.insert(l, now);
                        new_keys.insert(k.clone());
                        woken.push(k);
                    }
                }
            }
            woken.sort();
            woken.dedup();
            for k in woken {
                let list = self.reach_state.as_mut().unwrap().effects.remove(&k).unwrap_or_default();
                for e in list {
                    self.reach_effect(reach, e.when, &e.key, e.vals);
                }
            }
        }
        let table = &self.table;
        let st = self.reach_state.as_mut().unwrap();
        let mut keys: Vec<String> = new_keys.into_iter().collect();
        keys.sort();
        for k in keys {
            let Some(list) = st.waits.remove(&k) else { continue };
            let mut kept = Vec::new();
            for w in list {
                if reach.meets(table, &w.key, &w.set) {
                    st.wait_set.remove(&w);
                    match &w.guide {
                        None => st.ready_jobs.push(w.job),
                        Some(g) => st.ready_guides.entry(w.job).or_default().push(g.clone()),
                    }
                } else {
                    kept.push(w);
                }
            }
            if !kept.is_empty() {
                st.waits.insert(k, kept);
            }
        }
        st.max_waits = st.max_waits.max(st.wait_set.len());
    }

    /// A list SIG_LIMIT folded: `RecordStream::finish` rewrites the numbers already written to 0, so `args:<unit>` = 0
    /// is what they reach.
    pub(crate) fn reach_folded(&mut self, name: &str) {
        if let Some(st) = &mut self.reach_state {
            st.fresh.push((format!("args:{name}"), Label::Int(0)));
        }
    }

    /// The next batch: the events the last batch's records named, the jobs freed, the reruns.
    fn reach_batch(&mut self, reach: &Reached, skip: Option<&dyn Fn(&Event) -> bool>) -> Vec<Job> {
        let mut out = Vec::new();
        let mut todo = std::mem::take(&mut self.reach_state.as_mut().unwrap().todo);
        let jobs = self.event_jobs(&mut todo, skip);
        for j in jobs {
            if self.reach_job_ready(reach, &j) {
                out.push(j);
            }
        }
        let ready = std::mem::take(&mut self.reach_state.as_mut().unwrap().ready_jobs);
        for id in ready {
            let j = self.reach_state.as_ref().unwrap().jobs[id as usize].clone();
            if self.reach_job_ready(reach, &j) {
                out.push(j);
            }
        }
        let guides = std::mem::take(&mut self.reach_state.as_mut().unwrap().ready_guides);
        let mut map = FxHashMap::default();
        for (id, mut gs) in guides {
            gs.sort();
            gs.dedup();
            let j = self.reach_state.as_ref().unwrap().jobs[id as usize].clone();
            map.insert(j.label.clone(), Arc::new(gs));
            out.push(j);
            self.counts.inc("reach: reruns");
        }
        reach.inner.write().unwrap().guides = map;
        out
    }

    /// The batches until none has a job.
    fn reach_rounds(
        &mut self,
        m: &mut dyn Machine,
        mut jobs: Vec<Job>,
        skip: Option<&dyn Fn(&Event) -> bool>,
        sink: &mut Option<&mut dyn RecordSink>,
    ) {
        let reach = self.reach_arc();
        while !jobs.is_empty() {
            let st = self.reach_state.as_mut().unwrap();
            st.rounds += 1;
            let what = if st.rounds == 1 { "inputs".to_string() } else { format!("event round {}", st.rounds - 1) };
            let t0 = std::time::Instant::now();
            let n0 = self.n_records;
            let n_jobs = jobs.len();
            if let Some(f) = &mut self.on_batch_start {
                f(&what, n_jobs, n_jobs);
            }
            let jobs_arc = Arc::new(jobs);
            let order = jobs_arc.clone();
            let mut i = 0;
            let mut nexts = crate::builder::NextEvents::default();
            let r2 = reach.clone();
            self.run_jobs_shared(m, jobs_arc, &mut |b: &mut Builder<'_>, recs: Vec<Record>| {
                b.reach_after_job(&r2, &order[i], &recs);
                i += 1;
                nexts.add(&recs);
                b.emit(recs, sink);
            });
            self.reach_state.as_mut().unwrap().todo.extend(nexts.events);
            *self.phase_secs.entry("runs").or_default() += t0.elapsed().as_secs_f64();
            let n = self.n_records - n0;
            self.tell(&what, n_jobs, n, t0.elapsed().as_secs_f64());
            self.reach_commit(&reach);
            jobs = self.reach_batch(&reach, skip);
        }
    }

    /// `read_once`'s rounds under the switch (reach.md, "The rule").
    pub(crate) fn read_once_reach(
        &mut self,
        m: &mut dyn Machine,
        start_events: bool,
        skip: Option<&dyn Fn(&Event) -> bool>,
        sink: &mut Option<&mut dyn RecordSink>,
    ) {
        let reach = self.reach_arc();
        *reach.inner.write().unwrap() = Inner::default();
        self.reach_state = Some(Box::default());
        // the start's values (and the dumped slots' lists, as the harness's start file has them) are in at round 0
        let starts = self.reach_starts.clone();
        let mut seeds = Vec::new();
        for (n, s) in starts.iter().enumerate() {
            seeds.extend(self.reach_seed_values(s).into_iter().map(|(k, l)| (n, k, l)));
        }
        let mut events: Vec<Event> = if start_events { self.start_events() } else { vec![] };
        if start_events && !self.slot_locals.is_empty() {
            let dumped = self.dumped_events();
            for e in dumped.iter().filter(|e| !e.abort) {
                seeds.push((0, format!("args:{}", e.name), Label::Int(e.sig)));
            }
            events.extend(dumped);
        }
        {
            let mut i = reach.inner.write().unwrap();
            for (n, k, l) in seeds {
                if !i.vals.contains_key(&k) {
                    // the first start names this fact, or an earlier one holds its default value
                    let m: FxHashMap<Label, u32> = if n > 0 { default_of(&self.table, &k).into_iter().map(|d| (d, 0)).collect() } else { FxHashMap::default() };
                    i.vals.insert(k.clone(), m);
                }
                i.vals.get_mut(&k).unwrap().insert(l, 0);
            }
        }
        let inputs = self.inputs();
        let mut jobs = Vec::new();
        for j in inputs {
            if self.reach_job_ready(&reach, &j) {
                jobs.push(j);
            }
        }
        // the inputs run first and the start's events after them, as `read_once` has it: an event's precondition on
        // `args:<unit>` depends on the lists known when it is made
        self.reach_state.as_mut().unwrap().todo = events;
        self.reach_rounds(m, jobs, skip, sink);
        let (rounds, waits, left) = {
            let st = self.reach_state.as_ref().unwrap();
            (st.rounds, st.max_waits, st.wait_set.len())
        };
        self.counts.set("event rounds", rounds - 1);
        self.counts.set("reach: most branches held at once", waits as i64);
        self.counts.set("reach: branches and jobs still held at the end", left as i64);
        let vals: i64 = reach.inner.read().unwrap().vals.values().map(|m| m.len() as i64).sum();
        self.counts.set("reach: values reached (facts written or seeded)", vals);
    }

    /// A later start: its values go in as a round, they free what waits on them, and the rounds go on (reach.md, "A
    /// replan start"). The records so far stay as written.
    pub fn reach_extend(&mut self, m: &mut dyn Machine, start: &Facts, skip: Option<&dyn Fn(&Event) -> bool>, mut sink: Option<&mut dyn RecordSink>) {
        let reach = self.reach_arc();
        let vals = self.reach_seed_values(start);
        self.reach_state.as_mut().expect("a pass read with the switch --reach").fresh.extend(vals);
        self.reach_commit(&reach);
        let jobs = self.reach_batch(&reach, skip);
        self.reach_rounds(m, jobs, skip, &mut sink);
    }
}



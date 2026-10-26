//! reader.py `Builder`: the inputs and events of a scope, the runs (on a [`Machine`]), the records made from their
//! paths, and the reading's passes (`read_all`). Everything here is reader.py's Builder, step for step; the machine
//! runs are the one thing it asks for ([`Machine`], PORT-NOTES.md).

use crate::domain::{Domain, Label, Labels};
use crate::domains;
use crate::facts::slot_name;
use crate::py::{repr_str, repr_tuple, Counter, Py, PyCounter};
use crate::record::*;
use crate::scope::{self, live_bound, made_verbs, Scope, CLASS_UNTOUCHABLE, NUM_ACTORS, OF_OWNER_ROOM, VAR_USERPUT};
use crate::table::{FactTable, Kind};
use rustc_hash::{FxHashMap, FxHashSet};
use scumm_ir::{known, Expr, Game, Stmt, UnitId, UnitKey};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

pub const SIG_LIMIT: usize = 1024;
pub const SPLIT_DEPTH: i64 = 2;
pub const SPLIT_BUDGET: i64 = 4;
pub const VAR_CUTSCENEEXIT_KEY: i32 = 24;
pub const VAR_VERB_SCRIPT: i32 = 32;
pub const VAR_SENTENCE_SCRIPT: i32 = 33;
pub const VAR_EGO: i32 = 1;
/// Path ends that are no action (reader.DROP_ENDS: model.INFEASIBLE and the reader's own).
pub const DROP_ENDS: [&str; 9] =
    ["spin", "nesting", "error", "limit", "unmodelled", "deferral unknown", "nothing ran", "left the scope", "no value"];
/// The Reader's counters the Builder reads to decide a split.
pub const FORKS_ON_READ: &str = "forks on read";
pub const FORKS_ON_NONFACTS: &str = "forks on non-fact tests";

/// One input or event: (kind, label, pins, precondition, what, the input's own replay checks).
#[derive(Clone, Debug, PartialEq)]
pub struct Job {
    pub kind: String,
    pub label: String,
    pub pins: Pins,
    pub pre: Pre,
    pub what: What,
    pub checks: Vec<Assumption>,
}

impl Job {
    pub fn py(&self) -> Py {
        Py::Tuple(vec![
            Py::str(&self.kind),
            Py::str(&self.label),
            pins_py(&self.pins),
            pre_py(&self.pre),
            self.what.py(),
            Py::List(self.checks.iter().map(|c| c.py()).collect()),
        ])
    }
}

/// The Reader's counters a run added to (reader.py `Reader.counts`, `forks`, `fork_keys`).
#[derive(Clone, Debug, Default)]
pub struct RdCounts {
    pub counts: Counter,
    pub forks: Counter,
    pub fork_keys: Counter,
}

impl RdCounts {
    pub fn update(&mut self, d: &RdCounts) {
        self.counts.update(&d.counts);
        self.forks.update(&d.forks);
        self.fork_keys.update(&d.fork_keys);
    }
    pub fn clear(&mut self) {
        self.counts.clear();
        self.forks.clear();
        self.fork_keys.clear();
    }
}

/// What one machine run gives the Builder (model.Run and the Reader's state after it).
#[derive(Clone, Debug, Default)]
pub struct RunOut {
    pub paths: Vec<PathOut>,
    /// paths the path limit ended (`Run.limit`)
    pub limit: i64,
    /// `sum(Run.value_caps.values())`
    pub value_caps: i64,
    /// the first fact the run forked on (`Reader.first_fork`)
    pub first_fork: Option<String>,
    /// the Reader's counters' increments during the run (keys added with 0 included)
    pub rd: RdCounts,
}

/// What the machine-dependent half provides: one run of an input or event (reader.py `Reader.run_sentence_sym`,
/// `run_click_verb`, `run_event`), with the scope, the fact table (domains and kinds) and the run's pins and
/// precondition.
pub trait Machine {
    fn run(&mut self, cx: &Ctx, label: &str, pins: &Pins, pre: &Pre, what: &What) -> RunOut;

    /// A whole batch of jobs (one `_run_jobs` call) run elsewhere (par.rs: on threads): the machine merges each
    /// job's [`JobOut`] into `b` ([`Builder::merge_job`]) and hands its index and records to `each`, in the jobs'
    /// order, and returns true. False (the default): the Builder runs the jobs itself, one after another.
    fn run_batch(&mut self, _b: &mut Builder<'_>, _jobs: &Arc<Vec<Job>>, _each: &mut dyn FnMut(&mut Builder<'_>, usize, Vec<Record>)) -> bool {
        false
    }

    /// The runs a split is about to make, in its order (`Builder.run`'s parts): a machine may start them early
    /// (par.rs) and give each one's result to the `run` call for it. The default does nothing.
    fn prefetch(&mut self, _cx: &Ctx, _label: &str, _runs: &[(Pins, Pre)], _what: &What) {}
}

/// What the Builder's counters and sets gained while one job ran (in a worker's Builder, par.rs), merged into the
/// reading's Builder in the jobs' order ([`Builder::merge_job`]): the same keys in the same order as when the jobs
/// run one after another in one Builder.
#[derive(Clone, Debug, Default)]
pub struct Deltas {
    pub counts: Counter,
    pub ends: PyCounter,
    pub drops: PyCounter,
    pub nonfact_checks: Counter,
    pub guesses: Counter,
    pub unknown_writes: Counter,
    pub unknown_sites: PyCounter,
    pub out_of_domain: BTreeMap<String, BTreeSet<i64>>,
    pub rd: RdCounts,
    pub sig_ids: BTreeMap<i64, BTreeSet<String>>,
    /// the `args:` values `record` added to the table (`table.args_vals`), in order
    pub args: Vec<(String, i64)>,
}

/// One job's records and what it added to the Builder (par.rs).
#[derive(Debug, Default)]
pub struct JobOut {
    pub records: Vec<Record>,
    pub deltas: Deltas,
    /// the pins of the job's last run (`Builder::last_pins`)
    pub last_pins: Pins,
    /// the job's wall seconds where it ran
    pub secs: f64,
}

/// Takes the records as the Builder makes them (packed.rs `RecordStream`): a batch after each round of runs, `first`
/// the index of its first record; `reset` when a pass starts over (its records are dropped).
pub trait RecordSink {
    fn batch(&mut self, first: usize, recs: &[Record], table: &FactTable);
    fn reset(&mut self);
    /// The records handed over, when the Builder keeps none (`keep_records` off): a sink may take them to another
    /// thread (par.rs `SinkThread`). The default lends them to `batch`.
    fn batch_owned(&mut self, first: usize, recs: Vec<Record>, table: &FactTable) {
        self.batch(first, &recs, table)
    }
}

/// The Builder's read-only state a machine run needs.
pub struct Ctx<'a> {
    pub game: &'a Game,
    pub scope: &'a Scope,
    pub table: &'a FactTable,
}

/// The next round's events as the records of a batch give them, in their order, each once: a later event with the
/// same (unit name, unit, wait point, list of locals, Esc) as an earlier one changes nothing in `_read_once`'s round
/// (its replan copy is the earlier one's, `_limit` gives it the same key, and `events_done` then skips it), so it is
/// not kept (at Part I's scale the inputs' records name millions of events, a few tens of thousands of them distinct).
#[derive(Default)]
struct NextEvents {
    events: Vec<Event>,
    seen: FxHashSet<(String, UnitKey, i64, i64, bool)>,
}

impl NextEvents {
    fn add(&mut self, recs: &[Record]) {
        for r in recs {
            for e in &r.next_events {
                if self.seen.insert((e.name.clone(), e.unit, e.w, e.sig, e.abort)) {
                    self.events.push(e.clone());
                }
            }
        }
    }
}

/// `Builder::on_job`: (the job, its records, the batch's `args:` values and folded lists at its start).
pub type JobHook = Box<dyn FnMut(&Job, &[Record], &BTreeMap<String, BTreeSet<i64>>, &BTreeSet<i64>)>;

/// The `_pin` values: labels, or raw ints for a fact with no domain.
type PinVals = Vec<Label>;

/// The reader's switches (fix round 1, fix-round1.md): each a rule beyond reader.py, off by default. With every one
/// off the reading is reader.py's, byte for byte.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Switches {
    /// F2: the planner rule that a due event goes first (due.rs)
    pub due_first: bool,
    /// F12: forks by value sets, the mouse as ranges, findObject and actorFromPos as value sets (machine.rs, and
    /// scumm-machine's switch of the same name)
    pub value_sets: bool,
}

impl Switches {
    /// Comma-separated switch names.
    pub fn parse(names: &str) -> Result<Switches, String> {
        let mut s = Switches::default();
        for n in names.split(',').map(str::trim).filter(|n| !n.is_empty()) {
            match n {
                "due_first" => s.due_first = true,
                "value_sets" => s.value_sets = true,
                _ => return Err(format!("unknown reader switch {n} (known: due_first, value_sets)")),
            }
        }
        Ok(s)
    }
    pub fn names(&self) -> Vec<&'static str> {
        let mut v = Vec::new();
        if self.due_first {
            v.push("due_first");
        }
        if self.value_sets {
            v.push("value_sets");
        }
        v
    }
}

/// reader.py `Builder`.
pub struct Builder<'g> {
    pub game: &'g Game,
    /// fix round 1's switches (all off: reader.py's reading)
    pub switches: Switches,
    /// switch due_first's due facts, after the reading (due.rs)
    pub due: Option<crate::due::Due>,
    pub scope: Scope,
    pub table: FactTable,
    pub sig_limit: usize,
    pub records: Vec<Record>,
    pub counts: Counter,
    pub ends: PyCounter,
    pub drops: PyCounter,
    pub nonfact_checks: Counter,
    pub guesses: Counter,
    pub unknown_writes: Counter,
    pub unknown_sites: PyCounter,
    pub out_of_domain: BTreeMap<String, BTreeSet<i64>>,
    pub rd: RdCounts,
    pub sig_ids: BTreeMap<i64, BTreeSet<String>>,
    pub sig_seen: FxHashMap<(String, i64), Vec<i64>>,
    pub folded: BTreeSet<i64>,
    pub events_done: FxHashSet<(String, UnitKey, i64, i64, bool)>,
    /// the pins of the last run (the Reader's oracle's, which `event_pins` lists slot keys from)
    pub last_pins: Pins,
    bverb_cache: FxHashMap<(i64, i64), BTreeSet<i64>>,
    unknown_cache: FxHashMap<(String, String), Option<Vec<String>>>,
    cites: Option<FxHashMap<String, (UnitId, u32)>>,
    ego_cache: Option<Vec<Label>>,
    /// kind and domain of the keys `effect` reads, for one pass (an `args:`, `pos:` or `escpos:` domain grows
    /// within a pass: not kept)
    fact_info: FxHashMap<String, std::rc::Rc<(Kind, Option<Domain>)>>,
    /// false: the records go to the sink and are not kept (`records` stays empty, so memory does not grow with
    /// the record count); the reading is the same, `n_records` counts them
    pub keep_records: bool,
    /// the pass's records so far (`len(self.records)`)
    pub n_records: usize,
    /// with `keep_records` off: the `args:` values the pass's records set (`_read_once`'s last loop reads them)
    args_set: BTreeMap<String, BTreeSet<i64>>,
    /// a worker's Builder (par.rs) logs the `args:` values `record` adds to the table here
    args_log: Option<Vec<(String, i64)>>,
    /// told after each batch of runs (reader.py `Builder.progress`): what, jobs, records, seconds
    pub progress: Option<Box<dyn FnMut(&str, usize, usize, f64)>>,
    /// told before each batch of runs: what, jobs, the events it was made from (the measurements)
    pub on_batch_start: Option<Box<dyn FnMut(&str, usize, usize)>>,
    /// wall seconds by phase of `read_once` (the measurements; not part of the reading)
    pub phase_secs: BTreeMap<&'static str, f64>,
    /// the values written outside their domains in each pass that widened domains (`out_of_domain` before
    /// `extend_domains`; the spot-check's dump; not part of the reading)
    pub widened: Vec<BTreeMap<String, BTreeSet<i64>>>,
    /// told each job's records as made, with the `args:` values and folded lists its batch started with (the
    /// spot-check's dump; not part of the reading)
    pub on_job: Option<JobHook>,
    /// `oracle_keys`' cache: the universe as a set, and its keys by prefix
    uni_keys: std::cell::RefCell<(FxHashSet<String>, FxHashMap<String, Vec<String>>)>,
    /// `args_domain`'s cache: key -> (its count of list numbers, its domain)
    args_doms: std::cell::RefCell<FxHashMap<String, (usize, Domain)>>,
}

/// reader.py `_why`: the reason of an assumption's `why` (model.Unknown's text).
pub fn why_of(s: &str) -> String {
    let s = s.strip_prefix("?(").unwrap_or(s);
    s.split(':').next().unwrap().trim_end_matches(')').to_string()
}

fn lab_set(v: impl IntoIterator<Item = Label>) -> Labels {
    let mut v: Labels = v.into_iter().collect();
    v.sort();
    v.dedup();
    v
}

impl<'g> Builder<'g> {
    pub fn new(game: &'g Game, scope: Scope, doms: domains::Domains) -> Builder<'g> {
        let table = FactTable::new(&scope, doms, game);
        Builder::with_table(game, scope, table)
    }

    /// A worker's Builder (par.rs): the reading's scope, a copy of its table and folded lists, empty counters. It
    /// runs jobs (`run_job`) and hands over what each added with [`Builder::take_deltas`].
    pub fn worker(game: &'g Game, scope: Scope, table: FactTable, folded: BTreeSet<i64>, sig_limit: usize) -> Builder<'g> {
        let mut b = Builder::with_table(game, scope, table);
        b.folded = folded;
        b.sig_limit = sig_limit;
        b.args_log = Some(Vec::new());
        b
    }

    /// A worker's Builder takes the reading's table and folded lists for a new batch: the whole table when a pass
    /// widened a domain (`base`; each key's kind and domain worked out again), and the `args:` values.
    pub fn sync_table(&mut self, base: Option<&FactTable>, args_vals: &BTreeMap<String, BTreeSet<i64>>, folded: &BTreeSet<i64>) {
        if let Some(t) = base {
            self.table = t.clone();
            self.fact_info.clear();
            self.ego_cache = None;
        }
        if self.table.args_vals != *args_vals {
            self.table.args_vals.clone_from(args_vals);
        }
        if self.folded != *folded {
            self.folded.clone_from(folded);
        }
    }

    /// What the counters and sets gained since the last call (a worker's Builder after a job); they are left empty.
    pub fn take_deltas(&mut self) -> Deltas {
        Deltas {
            counts: std::mem::take(&mut self.counts),
            ends: std::mem::take(&mut self.ends),
            drops: std::mem::take(&mut self.drops),
            nonfact_checks: std::mem::take(&mut self.nonfact_checks),
            guesses: std::mem::take(&mut self.guesses),
            unknown_writes: std::mem::take(&mut self.unknown_writes),
            unknown_sites: std::mem::take(&mut self.unknown_sites),
            out_of_domain: std::mem::take(&mut self.out_of_domain),
            rd: std::mem::take(&mut self.rd),
            sig_ids: std::mem::take(&mut self.sig_ids),
            args: self.args_log.as_mut().map(std::mem::take).unwrap_or_default(),
        }
    }

    /// A job run elsewhere (par.rs), merged as if it had run here: its counters added in their order, its sets
    /// joined, its `args:` values added to the table, its last run's pins the last pins.
    pub fn merge_job(&mut self, d: Deltas, last_pins: Pins) {
        self.counts.update(&d.counts);
        self.ends.update(&d.ends);
        self.drops.update(&d.drops);
        self.nonfact_checks.update(&d.nonfact_checks);
        self.guesses.update(&d.guesses);
        self.unknown_writes.update(&d.unknown_writes);
        self.unknown_sites.update(&d.unknown_sites);
        for (k, v) in d.out_of_domain {
            self.out_of_domain.entry(k).or_default().extend(v);
        }
        self.rd.update(&d.rd);
        for (sig, texts) in d.sig_ids {
            self.sig_ids.entry(sig).or_default().extend(texts);
        }
        for (k, s) in d.args {
            self.table.args_vals.entry(k).or_default().insert(s);
        }
        self.last_pins = last_pins;
    }

    fn with_table(game: &'g Game, scope: Scope, table: FactTable) -> Builder<'g> {
        Builder {
            game,
            switches: Switches::default(),
            due: None,
            scope,
            table,
            sig_limit: SIG_LIMIT,
            records: Vec::new(),
            counts: Counter::default(),
            ends: PyCounter::default(),
            drops: PyCounter::default(),
            nonfact_checks: Counter::default(),
            guesses: Counter::default(),
            unknown_writes: Counter::default(),
            unknown_sites: PyCounter::default(),
            out_of_domain: BTreeMap::new(),
            rd: RdCounts::default(),
            sig_ids: BTreeMap::new(),
            sig_seen: FxHashMap::default(),
            folded: BTreeSet::new(),
            events_done: FxHashSet::default(),
            last_pins: Pins::new(),
            bverb_cache: FxHashMap::default(),
            unknown_cache: FxHashMap::default(),
            cites: None,
            ego_cache: None,
            fact_info: FxHashMap::default(),
            keep_records: true,
            n_records: 0,
            args_set: BTreeMap::new(),
            args_log: None,
            progress: None,
            on_batch_start: None,
            phase_secs: BTreeMap::new(),
            uni_keys: Default::default(),
            args_doms: Default::default(),
            on_job: None,
            widened: Vec::new(),
        }
    }

    pub fn ctx(&self) -> Ctx<'_> {
        Ctx { game: self.game, scope: &self.scope, table: &self.table }
    }

    pub fn kind(&self, key: &str) -> Kind {
        self.table.kind(key, &self.scope)
    }

    pub fn domain(&self, key: &str) -> Option<Domain> {
        self.table.domain(key).map(|d| d.into_owned())
    }

    // --- engine conditions (actions.usable) and the inputs ----------------------------------------------------------

    /// `Builder.ego_values`: VAR_EGO's value read on an empty path, or its domain's values.
    pub fn ego_values(&mut self) -> Vec<Label> {
        if let Some(v) = &self.ego_cache {
            return v.clone();
        }
        let key = format!("var:{VAR_EGO}");
        let dom = self.domain(&key);
        let out = match self.kind(&key) {
            Kind::Const => vec![Label::Int(self.table.start.get0(&key))],
            Kind::Fact => {
                let d = dom.clone().unwrap();
                assert_eq!(d.values.len(), 1, "VAR_EGO forks on read: reader.py's ego_values raises Fork here");
                match self.table.value(&key, d.values[0]) {
                    Val::Int(n) => vec![Label::Int(n)],
                    _ => d.values.clone(),
                }
            }
            Kind::NonFact => dom.map_or(Vec::new(), |d| d.values.clone()),
        };
        self.ego_cache = Some(out.clone());
        out
    }

    /// `table.domain` of an `args:` key, kept while its list numbers do not change (within a pass they only grow,
    /// so their count tells; `reset` forgets them).
    fn args_domain(&self, key: &str) -> Domain {
        let n = self.table.args_vals.get(key).map_or(0, |s| s.len());
        let mut c = self.args_doms.borrow_mut();
        if let Some((m, d)) = c.get(key) {
            if *m == n {
                return d.clone();
            }
        }
        let d = self.table.domain(key).unwrap().into_owned();
        c.insert(key.to_string(), (n, d.clone()));
        d
    }

    /// `Builder._pin`: an engine condition (a constant checked against the start; a fact into the precondition; a
    /// single value also a pin).
    pub fn pin(&self, pins: &mut Pins, pre: &mut Pre, key: &str, values: &[Label]) -> bool {
        let mut values: PinVals = lab_set(values.iter().copied());
        if self.kind(key) == Kind::Const {
            return values.contains(&Label::Int(self.table.start.get0(key)));
        }
        let dom = if key.starts_with("args:") { Some(std::borrow::Cow::Owned(self.args_domain(key))) } else { self.table.domain(key) };
        if let Some(dom) = &dom {
            values = lab_set(values.iter().filter_map(|&v| dom.label_of(v)));
            if values.is_empty() {
                return false;
            }
            if values != dom.values {
                let have = pre.get(key).cloned().unwrap_or_else(|| values.clone());
                pre.insert(key.to_string(), values.iter().copied().filter(|v| have.contains(v)).collect());
            }
        }
        if values.len() == 1 {
            let v = values[0];
            pins.insert(
                key.to_string(),
                if dom.is_some() {
                    self.table.value(key, v)
                } else {
                    match v {
                        Label::Int(n) => Val::Int(n),
                        _ => unreachable!(),
                    }
                },
            );
        }
        true
    }

    fn userput_on(&self) -> Labels {
        let dom = self.domain(&format!("var:{VAR_USERPUT}")).unwrap();
        dom.values.iter().copied().filter(|v| *v == Label::Above || matches!(v, Label::Int(n) if *n > 0)).collect()
    }

    /// `Builder._sentence_base`: (variant, ok, pins, precondition) per engine variant ("" and "~frozen").
    pub fn sentence_base(&self, n: i64) -> Vec<(&'static str, bool, Pins, Pre)> {
        let key = self.game.script_key(n as i32).to_string();
        let mut out = Vec::new();
        for (variant, live, frozen) in [("", 0, 0), ("~frozen", 1, 1)] {
            let (mut pins, mut pre) = (Pins::new(), Pre::new());
            let mut ok = self.pin(&mut pins, &mut pre, &format!("var:{VAR_SENTENCE_SCRIPT}"), &[Label::Int(n)]);
            for (k, v) in [
                (key.clone(), live),
                (format!("frozen:{}", slot_name(&key)), frozen),
                (format!("objscript:{n}"), 0),
                (format!("frozen:obj:{n}"), 0),
            ] {
                ok = ok && self.pin(&mut pins, &mut pre, &k, &[Label::Int(v)]);
            }
            ok = ok && self.pin(&mut pins, &mut pre, &format!("var:{VAR_USERPUT}"), &self.userput_on());
            out.push((variant, ok, pins, pre));
        }
        out
    }

    /// `Builder.placements`: where object A can be clicked: (place, conditions in order, replay checks).
    pub fn placements(&mut self, a: i64) -> Vec<(&'static str, Vec<(String, Labels)>, Vec<Assumption>)> {
        if a < NUM_ACTORS {
            return vec![(
                "actor",
                vec![(format!("class:{a}:{CLASS_UNTOUCHABLE}"), vec![Label::Int(0)])],
                vec![Assumption::new(
                    "actor.cpp:2115-2147",
                    format!("actor {a} drawn in the room under the click"),
                    true,
                    Unknown::new(R_ACTOR, format!("getActorFromPos({a})"), None).show(),
                )],
            )];
        }
        let mut out = Vec::new();
        let home = *self.game.object_room.get(&(a as i32)).expect("an object of the scope has a room");
        let rdom = self.domain("room");
        let mut rooms: Labels = rdom
            .map(|d| d.values.iter().copied().filter(|r| matches!(r, Label::Int(n) if self.game.resource(*n as i32) == Some(home))).collect())
            .unwrap_or_default();
        if rooms.is_empty() {
            rooms = vec![Label::Int(home as i64)];
        }
        let parent = self.game.object_parent.get(&(a as i32)).copied().unwrap_or(0);
        let checks = if parent != 0 {
            vec![Assumption::new(
                "object.cpp:567-590",
                format!("object {a}'s parent {parent} in its parent state"),
                true,
                Unknown::new(R_HELD, "the parent state is not in the index", None).show(),
            )]
        } else {
            vec![]
        };
        out.push((
            "room",
            vec![
                ("room".to_string(), rooms),
                (format!("owner:{a}"), vec![Label::Int(OF_OWNER_ROOM)]),
                (format!("class:{a}:{CLASS_UNTOUCHABLE}"), vec![Label::Int(0)]),
            ],
            checks,
        ));
        let dom = self.domain(&format!("owner:{a}"));
        let ego: Vec<i64> = self.ego_values().iter().filter_map(|v| v.int()).collect();
        if let Some(dom) = dom {
            if ego.iter().any(|&v| dom.values.contains(&Label::Int(v))) {
                out.push(("inventory", vec![(format!("owner:{a}"), lab_set(ego.iter().map(|&v| Label::Int(v))))], vec![]));
            }
        }
        out
    }

    /// `Builder._b_verbs`: the verbs V of the startObject(B, V) calls the sentence script `n` reaches with `verb`.
    pub fn b_verbs(&mut self, n: i64, verb: i64) -> BTreeSet<i64> {
        if let Some(v) = self.bverb_cache.get(&(n, verb)) {
            return v.clone();
        }
        let game = self.game;
        let ast = &game.ast;
        let mut out = BTreeSet::new();
        let u = if n < game.first_local as i64 { game.id(UnitKey::Global(n as i32)) } else { None };
        if let Some(u) = u {
            let ins = game.ins(u);
            let bound = !ins.iter().any(|x| matches!(x.st, Some(Stmt::Assign(a)) if ast.expr(a.lhs) == Expr::Local(0)));
            let env: domains::Env = if bound { [(0, vec![verb])].into_iter().collect() } else { Default::default() };
            for i in live_bound(game, u, &env) {
                if let Some(Stmt::Start { name, script, obj: Some(o), .. }) = ins[i as usize].st {
                    if name == known::START_OBJECT && ast.expr(o) == Expr::Local(2) {
                        if let Some(v) = ast.constant(script) {
                            out.insert(v as i64);
                        }
                    }
                }
            }
        }
        self.bverb_cache.insert((n, verb), out.clone());
        out
    }

    fn has_entry(&self, o: i64, v: i64) -> bool {
        self.game.id(UnitKey::Obj(o as i32)).is_some_and(|u| self.game.unit(u).entries.iter().any(|e| e.0 as i64 == v))
    }

    /// `Builder.b_values`: B for (verb, A).
    pub fn b_values(&mut self, a: i64, verb: i64, ns: &[i64]) -> Vec<i64> {
        let game = self.game;
        let ast = &game.ast;
        let mut consts: BTreeSet<i64> = BTreeSet::new();
        let u = game.id(UnitKey::Obj(a as i32));
        let idx = game.entry(a as i32, verb as i32);
        if let (Some(u), Some(idx)) = (u, idx) {
            let mut caps = 0;
            let ins = game.ins(u);
            for i in domains::live(game, u, &domains::settings_env(), Some(&[idx]), &mut caps) {
                if let Some(Stmt::Jump { cond: Some(c), .. }) = ins[i as usize].st {
                    if let Expr::Cmp { a: x, b: y, .. } = ast.expr(c) {
                        for (l, r) in [(x, y), (y, x)] {
                            if ast.expr(l) == Expr::Local(0) {
                                if let Some(v) = ast.constant(r) {
                                    consts.insert(v as i64);
                                }
                            }
                        }
                    }
                }
            }
        }
        let mut handlers: BTreeSet<i64> = BTreeSet::new();
        for &n in ns {
            for v in self.b_verbs(n, verb) {
                for &o in &self.scope.objects {
                    if o != a && game.entry(o as i32, v as i32).is_some() && self.has_entry(o, v) {
                        handlers.insert(o);
                    }
                }
            }
        }
        if consts.is_empty() && handlers.is_empty() {
            return vec![];
        }
        let inside: Vec<i64> =
            consts.iter().copied().filter(|b| self.scope.objects.contains(b) || (0 < *b && *b < NUM_ACTORS)).collect();
        self.counts.add("B constants outside the scope (not built)", (consts.len() - inside.len()) as i64);
        let inside_set: BTreeSet<i64> = inside.iter().copied().collect();
        self.counts.add("B objects whose own entry the sentence script starts", handlers.difference(&inside_set).count() as i64);
        self.counts.add("B actors (reachability a replay check)", inside.iter().filter(|&&b| b < NUM_ACTORS).count() as i64);
        let all: BTreeSet<i64> = consts.union(&handlers).copied().collect();
        let other = self.scope.other_b(game, a, &all);
        if other.is_some() {
            self.counts.inc("B = other (a stand-in object)");
        }
        let mut out: Vec<i64> = inside_set.union(&handlers).copied().collect();
        out.extend(other);
        out
    }

    pub fn sentence_scripts(&self) -> Vec<i64> {
        let key = format!("var:{VAR_SENTENCE_SCRIPT}");
        let vals = match self.domain(&key) {
            Some(d) => d.values,
            None => vec![Label::Int(self.table.start.get0(&key))],
        };
        vals.into_iter().filter_map(|v| v.int()).filter(|&v| v != 0).collect()
    }

    /// `Builder.inputs`: every player input of the scope, in reader.py's order.
    pub fn inputs(&mut self) -> Vec<Job> {
        let mut out = Vec::new();
        let ns = self.sentence_scripts();
        let objects = self.scope.objects.clone();
        let verbs = self.scope.verbs.clone();
        for &n in &ns {
            for (variant, ok, base_pins, base_pre) in self.sentence_base(n) {
                if !ok {
                    self.counts.inc(format!("sentence scripts whose base conditions never hold{variant}"));
                    continue;
                }
                for &a in &objects {
                    for (place, conds, checks) in self.placements(a) {
                        let mut vs: Vec<(i64, bool)> = verbs.iter().map(|&v| (v, false)).collect();
                        if place == "room" {
                            if let Some(w) = self.scope.walk_verb {
                                vs.push((w, true));
                            }
                        }
                        for (verb, walk) in vs {
                            let (mut pins, mut pre) = (base_pins.clone(), base_pre.clone());
                            let mut ok = conds.iter().all(|(k, v)| self.pin(&mut pins, &mut pre, k, v));
                            if !walk {
                                ok = ok && self.pin(&mut pins, &mut pre, &format!("verb:{verb}"), &[Label::Int(1)]);
                            }
                            if !ok {
                                self.counts.inc("inputs whose engine conditions never hold (constants)");
                                continue;
                            }
                            let kind = if walk { "walk_to" } else { "sentence" };
                            let mut bs: Vec<Option<i64>> = vec![None];
                            if !walk {
                                bs.extend(self.b_values(a, verb, &ns).into_iter().map(Some));
                            }
                            for b in bs {
                                let mut label = if walk {
                                    format!("walk_to({a})")
                                } else {
                                    match b {
                                        Some(b) if b != 0 => format!("sentence({verb},{a},{b})"),
                                        _ => format!("sentence({verb},{a})"),
                                    }
                                };
                                if place == "inventory" {
                                    label.push_str("[inv]");
                                }
                                label.push_str(variant);
                                if !variant.is_empty() {
                                    self.counts.inc(format!("sentence inputs{variant}"));
                                }
                                match b {
                                    Some(b) => {
                                        let (mut pb, mut prb) = (pins.clone(), pre.clone());
                                        let all = self.placements(b);
                                        let room: Vec<_> = all.iter().filter(|x| x.0 != "inventory").cloned().collect();
                                        let pl = if room.is_empty() { all } else { room };
                                        let okb = pl[0].1.iter().all(|(k, v)| self.pin(&mut pb, &mut prb, k, v));
                                        if !okb {
                                            continue;
                                        }
                                        let mut ch = checks.clone();
                                        ch.extend(pl[0].2.iter().cloned());
                                        out.push(Job {
                                            kind: kind.into(),
                                            label,
                                            pins: pb,
                                            pre: prb,
                                            what: What::Sentence { verb, a, b: Some(b) },
                                            checks: ch,
                                        });
                                    }
                                    None => out.push(Job {
                                        kind: kind.into(),
                                        label,
                                        pins: pins.clone(),
                                        pre: pre.clone(),
                                        what: What::Sentence { verb, a, b: None },
                                        checks: checks.clone(),
                                    }),
                                }
                            }
                        }
                    }
                }
            }
        }
        let key32 = format!("var:{VAR_VERB_SCRIPT}");
        let vs: Vec<i64> = match self.domain(&key32) {
            Some(d) => d.values,
            None => vec![Label::Int(self.table.start.get0(&key32))],
        }
        .into_iter()
        .filter_map(|v| v.int())
        .filter(|&v| v != 0)
        .collect();
        for &v in &self.scope.dialog_verbs.clone() {
            for &n in &vs {
                for &room in &self.scope.rooms.clone() {
                    let (mut pins, mut pre) = (Pins::new(), Pre::new());
                    let ok = self.pin(&mut pins, &mut pre, &key32, &[Label::Int(n)])
                        && self.pin(&mut pins, &mut pre, &format!("verb:{v}"), &[Label::Int(1)])
                        && self.pin(&mut pins, &mut pre, "room", &[Label::Int(room as i64)])
                        && self.pin(&mut pins, &mut pre, &format!("var:{VAR_USERPUT}"), &self.userput_on());
                    if ok {
                        out.push(Job {
                            kind: "dialog_line".into(),
                            label: format!("dialog_line({v})[{n}@{room}]"),
                            pins,
                            pre,
                            what: What::Verb(v),
                            checks: vec![],
                        });
                    }
                }
            }
        }
        out
    }

    // --- events ------------------------------------------------------------------------------------------------------

    /// `Builder.start_events`: the slots live at the start, with unknown locals (args 0).
    pub fn start_events(&mut self) -> Vec<Event> {
        let mut out = Vec::new();
        let st = self.table.start.clone();
        let res = self.game.resource(st.get0("room") as i32);
        let mut keys: Vec<(&String, i64)> = st.iter().collect();
        keys.sort_by(|a, b| a.0.cmp(b.0));
        for (k, v) in keys {
            if !k.starts_with("at:") || v != 1 {
                continue;
            }
            let (name, w) = k[3..].rsplit_once(':').unwrap();
            let w: i64 = w.parse().unwrap();
            let unit = if let Some(n) = name.strip_prefix("script:") {
                Some(UnitKey::Global(n.parse().unwrap()))
            } else if let Some(n) = name.strip_prefix("local:") {
                res.map(|r| UnitKey::Local(r, n.parse().unwrap()))
            } else if let Some(o) = name.strip_prefix("obj:") {
                Some(UnitKey::Obj(o.parse().unwrap()))
            } else {
                // (name, room): any other name is no unit of the scope
                res.and_then(|r| match name {
                    "entry" => Some(UnitKey::Entry(r)),
                    "exit" => Some(UnitKey::Exit(r)),
                    _ => None,
                })
            };
            let Some(unit) = unit.filter(|u| self.scope.has_unit(*u)) else {
                self.counts.inc("slots live at the start outside the scope (no events)");
                continue;
            };
            let owned = match unit {
                UnitKey::Obj(o) => st.get0(&format!("owner:{o}")) != OF_OWNER_ROOM,
                _ => false,
            };
            let res_flag = match self.game.lifecycle_key(unit) {
                Some(f) => self.game.resistant(f, res),
                None => Some(false),
            };
            out.push(Event {
                name: name.to_string(),
                unit,
                w,
                sig: 0,
                locals: vec![],
                args_known: false,
                recursive: Py::None,
                resistant: Py::opt_bool(res_flag),
                owned: Py::Bool(owned),
                abort: false,
                override_: 0,
                cut_data: vec![],
            });
        }
        out
    }

    /// `Builder._sig`: the number of a list of locals (48 bits of its text's SHA-1; 0: not known).
    pub fn sig(&mut self, name: &str, locals: &[(i64, Val)], ov: i64, cd: &[CutData], args_known: bool) -> i64 {
        if !args_known {
            return 0;
        }
        let mut pairs: Vec<(String, String)> = locals.iter().map(|(i, v)| (i.to_string(), v.show())).collect();
        pairs.push(("override".into(), ov.to_string()));
        pairs.push(("cut_data".into(), cut_tuple_str(cd)));
        pairs.sort();
        let inner: Vec<String> = pairs.iter().map(|(a, b)| repr_tuple(&[repr_str(a), repr_str(b)])).collect();
        let text = repr_tuple(&[repr_str(name), repr_tuple(&inner)]);
        let hex = sha1_smol::Sha1::from(text.as_bytes()).digest().to_string();
        let mut sig = i64::from_str_radix(&hex[..12], 16).unwrap();
        if sig == 0 {
            sig = 1;
        }
        self.sig_ids.entry(sig).or_default().insert(text);
        sig
    }

    /// The slot keys of a unit the oracle lists (`SymOracle.keys(prefix)`): the scope's universe, then the last
    /// run's pins that the universe lacks, sorted. The universe's part is worked out once per prefix (the scope
    /// does not change).
    fn oracle_keys(&self, prefix: &str) -> Vec<String> {
        let mut cache = self.uni_keys.borrow_mut();
        let (uni, by_prefix) = &mut *cache;
        if uni.is_empty() {
            uni.extend(self.scope.universe.iter().cloned());
        }
        let mut out: Vec<String> = by_prefix
            .entry(prefix.to_string())
            .or_insert_with(|| self.scope.universe.iter().filter(|k| k.starts_with(prefix)).cloned().collect())
            .clone();
        out.extend(self.last_pins.keys().filter(|k| k.starts_with(prefix) && !uni.contains(*k)).cloned());
        out
    }

    /// `Builder.event_pins`.
    pub fn event_pins(&self, ev: &Event) -> (bool, Pins, Pre) {
        let (mut pins, mut pre) = (Pins::new(), Pre::new());
        let mut ok = true;
        if let Some(key) = self.game.lifecycle_key(ev.unit) {
            ok = self.pin(&mut pins, &mut pre, &key.to_string(), &[Label::Int(1)]);
        }
        let exit_key = format!("var:{VAR_CUTSCENEEXIT_KEY}");
        if ev.abort {
            ok = ok && self.pin(&mut pins, &mut pre, &format!("esc:{}:{}", ev.name, ev.w), &[Label::Int(1)]);
            let dom = self.domain(&exit_key);
            if self.kind(&exit_key) == Kind::Const {
                ok = ok && self.table.start.get0(&exit_key) != 0;
            } else if let Some(dom) = dom {
                let vals: Labels = dom.values.iter().copied().filter(|v| *v != Label::Int(0)).collect();
                ok = ok && self.pin(&mut pins, &mut pre, &exit_key, &vals);
            }
        } else {
            ok = ok && self.pin(&mut pins, &mut pre, &format!("at:{}:{}", ev.name, ev.w), &[Label::Int(1)]);
            ok = ok && self.pin(&mut pins, &mut pre, &format!("frozen:{}", ev.name), &[Label::Int(0)]);
            let own = format!("at:{}:{}", ev.name, ev.w);
            for k in self.oracle_keys(&format!("at:{}:", ev.name)) {
                if k != own {
                    pins.insert(k, Val::Int(0));
                }
            }
        }
        ok = ok && self.pin(&mut pins, &mut pre, &format!("args:{}", ev.name), &[Label::Int(ev.sig)]);
        if let UnitKey::Local(r, _) | UnitKey::Entry(r) | UnitKey::Exit(r) = ev.unit {
            let rooms: Labels = self
                .scope
                .rooms
                .iter()
                .filter(|&&x| self.game.resource(x) == Some(r))
                .map(|&x| Label::Int(x as i64))
                .collect();
            ok = ok && self.pin(&mut pins, &mut pre, "room", &rooms);
        }
        (ok, pins, pre)
    }

    // --- one run -------------------------------------------------------------------------------------------------------

    /// `Builder.run` for one input or event (the root call: its budget is made here).
    pub fn run_job(&mut self, m: &mut dyn Machine, job: &Job) -> Vec<Record> {
        self.counts.inc(format!("runs: {}", job.kind));
        let mut budget = (SPLIT_BUDGET, 0i64, job.pre.keys().cloned().collect::<BTreeSet<String>>());
        self.run(m, job, &job.pins, &job.pre, &mut budget)
    }

    fn run(
        &mut self,
        m: &mut dyn Machine,
        job: &Job,
        pins: &Pins,
        pre: &Pre,
        budget: &mut (i64, i64, BTreeSet<String>),
    ) -> Vec<Record> {
        let facts0 = self.rd.counts.get(FORKS_ON_READ);
        let nonfacts0 = self.rd.counts.get(FORKS_ON_NONFACTS);
        let run = m.run(&self.ctx(), &job.label, pins, pre, &job.what);
        self.last_pins = pins.clone();
        self.rd.update(&run.rd);
        self.counts.add("value sets past model.MAX_VALUES (made plain unknowns)", run.value_caps);
        let facts = self.rd.counts.get(FORKS_ON_READ) - facts0;
        let nonfacts = self.rd.counts.get(FORKS_ON_NONFACTS) - nonfacts0;
        if run.limit != 0 && run.first_fork.is_some() && facts < nonfacts {
            self.counts.inc("runs past the path limit not split (more forks on non-facts than on facts)");
        } else if run.limit != 0 && run.first_fork.is_some() {
            let key = run.first_fork.clone().unwrap();
            let dom = self.domain(&key).unwrap();
            let vals: Labels = match pre.get(&key) {
                Some(s) => dom.values.iter().copied().filter(|v| s.contains(v)).collect(),
                None => dom.values.clone(),
            };
            if vals.len() > 1 && budget.1 < SPLIT_DEPTH && budget.0 >= vals.len() as i64 {
                budget.0 -= vals.len() as i64;
                budget.1 += 1;
                self.counts.inc("runs past the path limit split on their first fork on a read");
                let parts: Vec<(Pins, Pre)> = vals
                    .iter()
                    .map(|&v| {
                        let (mut p2, mut pre2) = (pins.clone(), pre.clone());
                        pre2.insert(key.clone(), vec![v]);
                        p2.insert(key.clone(), self.table.value(&key, v));
                        (p2, pre2)
                    })
                    .collect();
                m.prefetch(&self.ctx(), &job.label, &parts, &job.what);
                let mut out = Vec::new();
                for (p2, pre2) in &parts {
                    out.extend(self.run(m, job, p2, pre2, budget));
                }
                budget.1 -= 1;
                return out;
            }
            self.counts.inc("runs past the path limit not split (depth or budget used up, or no fork on a read)");
        }
        self.counts.add("paths ended by the path limit", run.limit);
        let mut out = Vec::new();
        for p in &run.paths {
            self.ends.add(Py::Tuple(vec![Py::str(&job.kind), end_py(&p.end)]), 1);
            if p.end.as_deref().is_some_and(|e| DROP_ENDS.contains(&e)) || !p.sym {
                let base = job.label.split('#').next().unwrap().split('@').next().unwrap();
                let lab = format!("{base}{}", if job.label.contains('@') { ")" } else { "" });
                self.drops.add(Py::Tuple(vec![Py::str(lab), end_py(&p.end)]), 1);
                continue;
            }
            let Some(mut rec) = self.record(&job.kind, &job.label, p, &job.what) else {
                self.counts.inc(format!("no-op paths (no effect): {}", job.kind));
                continue;
            };
            for c in &job.checks {
                rec.checks.push(c.clone());
                self.nonfact_checks.inc(why_of(&c.why));
            }
            let own: BTreeSet<String> = match &job.what {
                What::Event(e) => [format!("pos:{}", e.name), format!("escpos:{}", e.name)].into_iter().collect(),
                _ => BTreeSet::new(),
            };
            rec.engine_pre = rec.pre.keys().filter(|k| budget.2.contains(*k) || own.contains(*k)).cloned().collect();
            out.push(rec);
        }
        out
    }

    /// `Builder.record`: the record of one path, or None when it changes nothing.
    pub fn record(&mut self, kind: &str, label: &str, p: &PathOut, what: &What) -> Option<Record> {
        let mut pre = Pre::new();
        for (k, s) in &p.pre {
            if let Some(dom) = self.table.domain(k) {
                if *s != dom.values {
                    pre.insert(k.clone(), s.clone());
                }
            }
        }
        let mut effects: Vec<(String, Eff)> = Vec::new();
        for (k, v) in &p.writes {
            if let Some(e) = self.effect(k, v, p) {
                effects.push((k.clone(), e));
            }
        }
        let mut nexts = Vec::new();
        for (name, kept) in &p.kept {
            if p.writes.get(&format!("at:{}:{}", name, kept.w)) != Some(&Val::Int(1)) {
                continue;
            }
            let locs: Vec<(i64, Val)> = kept
                .locals
                .iter()
                .map(|(i, v)| {
                    let fact = matches!(v, Val::Unk(u) if u.values.is_some() && u.reason == R_FACT);
                    (*i, if fact { Val::Unk(Unknown::new(R_LOCAL, format!("Local[{i}] from a fact"), None)) } else { v.clone() })
                })
                .collect();
            let cd: Vec<CutData> = kept.cut_data.iter().map(cut_text).collect();
            let mut sig = self.sig(name, &locs, kept.override_, &cd, kept.known);
            if self.folded.contains(&sig) {
                sig = 0;
            }
            let ev = Event {
                name: name.clone(),
                unit: kept.unit,
                w: kept.w,
                sig,
                locals: locs,
                args_known: kept.known && sig != 0,
                recursive: kept.recursive.clone(),
                resistant: kept.resistant.clone(),
                owned: kept.owned.clone(),
                abort: false,
                override_: if sig != 0 { kept.override_ } else { 0 },
                cut_data: if sig != 0 {
                    cd.iter().filter_map(|c| if let CutData::Int(n) = c { Some(*n) } else { None }).collect()
                } else {
                    vec![]
                },
            };
            let args = format!("args:{name}");
            match effects.iter_mut().find(|e| e.0 == args) {
                Some(e) => e.1 = Eff::Set(Label::Int(sig)),
                None => effects.push((args.clone(), Eff::Set(Label::Int(sig)))),
            }
            if let Some(log) = &mut self.args_log {
                log.push((args.clone(), sig));
            }
            self.table.args_vals.entry(args).or_default().insert(sig);
            let esc = format!("esc:{name}:");
            let escs: Vec<i64> = p
                .writes
                .iter()
                .filter(|(k, v)| k.starts_with(&esc) && **v == Val::Int(1))
                .map(|(k, _)| k.rsplit(':').next().unwrap().parse().unwrap())
                .collect();
            nexts.push(ev.clone());
            for w in escs {
                nexts.push(Event { w, abort: true, ..ev.clone() });
            }
        }
        let (pre, effects) = self.positions(pre, effects);
        let mut unknown = Vec::new();
        for w in &p.unknown_writes {
            let t = w.items();
            let head = t[0].as_str().to_string();
            let cite = t[1].as_str().to_string();
            self.unknown_writes.inc(&head);
            self.unknown_sites.add(Py::Tuple(vec![Py::str(&head), Py::str(&cite)]), 1);
            unknown.push(w.clone());
            let which = if self.unknown_targets(&head, &cite).is_none() { "no candidate list" } else { "with a candidate list" };
            self.counts.inc(format!("unknown-target writes left out and listed on the record ({which}): {head}"));
        }
        let effects: Vec<(String, Eff)> = effects
            .into_iter()
            .filter(|(k, e)| !matches!(e, Eff::Set(v) if pre.get(k).is_some_and(|s| s.len() == 1 && s[0] == *v)))
            .collect();
        if effects.is_empty() {
            return None;
        }
        let mut sides: FxHashMap<(&str, &str), BTreeSet<String>> = FxHashMap::default();
        for a in &p.assumptions {
            sides.entry((&a.cite, &a.cond)).or_default().insert(a.side_str());
        }
        let mut checks: Vec<Assumption> = Vec::new();
        let mut either: Vec<(String, String)> = Vec::new();
        for a in &p.assumptions {
            if sides[&(a.cite.as_str(), a.cond.as_str())].len() == 1 {
                if !checks.contains(a) {
                    checks.push(a.clone());
                    self.nonfact_checks.inc(why_of(&a.why));
                }
            } else if !either.iter().any(|e| e.0 == a.cite && e.1 == a.cond) {
                either.push((a.cite.clone(), a.cond.clone()));
                self.counts.inc("conditions with both sides on one record (listed apart, not checked)");
            }
        }
        let n = p.notes.len();
        Some(Record {
            kind: kind.into(),
            label: label.into(),
            pre,
            effects,
            checks,
            end: p.end.clone(),
            notes: p.notes[n.saturating_sub(5)..].to_vec(),
            next_events: nexts,
            source: what.clone(),
            either,
            engine_pre: vec![],
            unknown,
        })
    }

    /// `Builder.unknown_targets`: the facts a write whose target does not fold may hit, or None.
    pub fn unknown_targets(&mut self, head: &str, cite: &str) -> Option<Vec<String>> {
        let ck = (head.to_string(), cite.to_string());
        if let Some(v) = self.unknown_cache.get(&ck) {
            return v.clone();
        }
        let game = self.game;
        let ast = &game.ast;
        if self.cites.is_none() {
            let mut m = FxHashMap::default();
            for u in game.ids() {
                for i in 0..game.ins(u).len() as u32 {
                    m.insert(game.cite(u, i), (u, i));
                }
            }
            self.cites = Some(m);
        }
        let st = self.cites.as_ref().unwrap().get(cite).and_then(|&(u, i)| game.ins(u)[i as usize].st);
        let objects = &self.scope.objects;
        let out: Option<Vec<String>> = match head {
            "VerbOps" | "saveVerbs" | "restoreVerbs" => {
                let changes = head != "VerbOps"
                    || matches!(st, Some(Stmt::Op { subops, .. }) if ast.subs(subops).iter().any(|s|
                        [known::NEW, known::ON, known::OFF, known::DIM, known::DELETE].contains(&s.name)));
                Some(if changes { made_verbs(game).keys().map(|v| format!("verb:{v}")).collect() } else { vec![] })
            }
            "drawObject" | "setState" => Some(objects.iter().map(|o| format!("state:{o}")).collect()),
            "setOwnerOf" => Some(objects.iter().map(|o| format!("owner:{o}")).collect()),
            "pickupObject" => {
                let mut v: Vec<String> =
                    objects.iter().flat_map(|o| ["owner", "state"].iter().map(move |k| format!("{k}:{o}"))).collect();
                v.extend(objects.iter().map(|o| format!("class:{o}:{CLASS_UNTOUCHABLE}")));
                Some(v)
            }
            "setClass" => match st {
                Some(Stmt::Op { args, .. }) if ast.kids(args).len() > 1 && matches!(ast.expr(ast.kids(args)[1]), Expr::List(_)) => {
                    let items = domains::list_items(ast, ast.kids(args)[1]);
                    let cls: Vec<i64> = items.iter().filter_map(|&c| ast.constant(c)).map(|c| (c & 0x7F) as i64).collect();
                    if cls.len() == items.len() && !cls.contains(&0) {
                        Some(objects.iter().flat_map(|o| cls.iter().map(move |c| format!("class:{o}:{c}"))).collect())
                    } else {
                        None
                    }
                }
                _ => None,
            },
            "startObject" => match st {
                Some(Stmt::Start { script, .. }) if ast.constant(script).is_some() => {
                    let v = ast.constant(script).unwrap();
                    let mut objs: Vec<i32> = game
                        .units
                        .iter()
                        .filter_map(|u| match u.key {
                            UnitKey::Obj(o) if u.entries.iter().any(|e| e.0 == v) => Some(o),
                            _ => None,
                        })
                        .collect();
                    objs.sort_unstable();
                    let mut out: Vec<String> = objs.iter().map(|o| format!("objscript:{o}")).collect();
                    out.extend(scope::entry_writes(game, v));
                    Some(out)
                }
                _ => None,
            },
            _ => None,
        };
        self.unknown_cache.insert(ck, out.clone());
        out
    }

    /// `Builder.positions`: `at:<unit>:W` and `esc:<unit>:W` as one fact per unit, `pos:<unit>` and `escpos:<unit>`.
    /// A written place is not checked against pos:'s domain (reader.py's own behaviour, a finding: PORT-NOTES.md).
    pub fn positions(&mut self, mut pre: Pre, mut effects: Vec<(String, Eff)>) -> (Pre, Vec<(String, Eff)>) {
        for (kind, pos) in [("at", "pos"), ("esc", "escpos")] {
            let prefix = format!("{kind}:");
            let mut ones: BTreeMap<String, Vec<i64>> = BTreeMap::new();
            let mut zeros: BTreeSet<String> = BTreeSet::new();
            let mut kept = Vec::with_capacity(effects.len());
            for (k, e) in effects {
                if !k.starts_with(&prefix) {
                    kept.push((k, e));
                    continue;
                }
                let (name, w) = k[prefix.len()..].rsplit_once(':').unwrap();
                if w == "*" {
                    zeros.insert(name.to_string());
                } else if e == Eff::Set(Label::Int(1)) {
                    ones.entry(name.to_string()).or_default().push(w.parse().unwrap());
                } else if e == Eff::Set(Label::Int(0)) {
                    zeros.insert(name.to_string());
                } else {
                    self.counts.inc(format!("{kind}: writes not 0 or 1 (left out)"));
                }
            }
            effects = kept;
            let names: BTreeSet<String> = ones.keys().cloned().chain(zeros.iter().cloned()).collect();
            for name in names {
                let e = match ones.get(&name) {
                    Some(ws) if ws.len() > 1 => {
                        self.counts.inc(format!("{kind}: more than one place written for one unit (a guess)"));
                        Eff::Guess(lab_set(ws.iter().map(|&w| Label::Int(w))))
                    }
                    Some(ws) => Eff::Set(Label::Int(ws[0])),
                    None => Eff::Set(Label::Int(crate::table::NOWHERE)),
                };
                let key = format!("{pos}:{name}");
                match effects.iter_mut().find(|x| x.0 == key) {
                    Some(x) => x.1 = e,
                    None => effects.push((key, e)),
                }
            }
            let mut need: BTreeMap<String, Vec<(i64, Labels)>> = BTreeMap::new();
            let keys: Vec<String> = pre.keys().filter(|k| k.starts_with(&prefix)).cloned().collect();
            for k in keys {
                let s = pre.remove(&k).unwrap();
                let (name, w) = k[prefix.len()..].rsplit_once(':').unwrap();
                let e = need.entry(name.to_string()).or_default();
                let w: i64 = w.parse().unwrap();
                match e.iter_mut().find(|x| x.0 == w) {
                    Some(x) => x.1 = s,
                    None => e.push((w, s)),
                }
            }
            for (name, ws) in need {
                let key = format!("{pos}:{name}");
                let mut ok: BTreeSet<Label> = self.table.domain(&key).unwrap().values.iter().copied().collect();
                for (w, s) in ws {
                    if s == [Label::Int(1)] {
                        ok.retain(|x| *x == Label::Int(w));
                    } else if s == [Label::Int(0)] {
                        ok.remove(&Label::Int(w));
                    }
                }
                pre.insert(key, ok.into_iter().collect());
            }
        }
        (pre, effects)
    }

    /// `Reader._stepped`: a counter's value from domain value `base` after steps adding up to `d`.
    pub fn stepped(&mut self, key: &str, base: Label, d: i64) -> Val {
        if let Label::Int(b) = base {
            return Val::Int(b + d);
        }
        self.rd.counts.inc("steps on a collapsed value (a guess)");
        let vals = self.table.domain(key).unwrap().add(base, d);
        if vals.len() > 1 {
            self.table.lazy(key, Some(&vals))
        } else {
            self.table.value(key, vals[0])
        }
    }

    /// `Builder.effect`: the effect of the path's last write `v` to `key`, or None (not a fact, or no value).
    /// `Same` is reader.py's SAME (the fact keeps the value the precondition gives it).
    pub fn effect(&mut self, key: &str, v: &Val, p: &PathOut) -> Option<Eff> {
        match self.effect_or_same(key, v, p) {
            EffOut::Eff(e) => Some(e),
            EffOut::Same | EffOut::None => None,
        }
    }

    /// `table.kind(key)` and `table.domain(key)`, kept for the pass (`fact_info`).
    fn fact_info(&mut self, key: &str) -> std::rc::Rc<(Kind, Option<Domain>)> {
        if let Some(i) = self.fact_info.get(key) {
            return i.clone();
        }
        let i = std::rc::Rc::new((self.kind(key), self.domain(key)));
        if !(key.starts_with("args:") || key.starts_with("pos:") || key.starts_with("escpos:")) {
            self.fact_info.insert(key.to_string(), i.clone());
        }
        i
    }

    fn effect_or_same(&mut self, key: &str, v: &Val, p: &PathOut) -> EffOut {
        let info = self.fact_info(key);
        let Some(dom) = info.1.as_ref().filter(|_| info.0 != Kind::NonFact) else {
            self.counts.inc("writes to non-facts (left out of the task)");
            return EffOut::None;
        };
        if let Val::Unk(u) = v {
            if u.class != UClass::Unknown {
                let src = u.what.clone();
                let s: Option<Label> = if self.fact_info(&src).0 == Kind::Const {
                    Some(Label::Int(self.table.start.get0(&src)))
                } else {
                    p.pre.get(&src).filter(|s| s.len() == 1).map(|s| s[0])
                };
                let first = u.values.as_ref().unwrap()[0];
                if let Some(s) = s {
                    if u.class == UClass::Cond {
                        return if s == Label::Int(1) { EffOut::Eff(Eff::Set(Label::Int(first))) } else { EffOut::Same };
                    }
                    return if src == key { EffOut::Same } else { EffOut::Eff(Eff::Set(s)) };
                }
                if src == key {
                    return EffOut::Same;
                }
                return if u.class == UClass::Cond { EffOut::Eff(Eff::When(src, first)) } else { EffOut::Eff(Eff::Copy(src)) };
            }
        }
        if v.is_step() {
            let d = v.step_of();
            let base = p.pre.get(key);
            if d == 0 {
                return EffOut::Same;
            }
            if let Some(b) = base.filter(|b| b.len() == 1) {
                let nv = self.stepped(key, b[0], d);
                return self.effect_or_same(key, &nv, p);
            }
            return EffOut::Eff(Eff::Add(d));
        }
        match v {
            Val::Int(n) => match dom.label(*n) {
                None => {
                    self.out_of_domain.entry(key.to_string()).or_default().insert(*n);
                    EffOut::None
                }
                Some(l) => EffOut::Eff(Eff::Set(l)),
            },
            Val::Unk(u) => {
                if u.reason == R_COLLAPSED && u.values.as_ref().is_some_and(|v| v.len() == 1) {
                    return match dom.label(u.values.as_ref().unwrap()[0]) {
                        Some(l) => EffOut::Eff(Eff::Set(l)),
                        // reader.py writes ("set", None) here; a collapsed value is always its domain's own
                        None => panic!("a collapsed value outside its domain: {key}"),
                    };
                }
                if let Some(vals) = &u.values {
                    let mut labs: BTreeSet<Label> = BTreeSet::new();
                    for &x in vals {
                        match dom.label(x) {
                            Some(l) => {
                                labs.insert(l);
                            }
                            None => {
                                self.out_of_domain.entry(key.to_string()).or_default().insert(x);
                            }
                        }
                    }
                    let labs: Labels = labs.into_iter().collect();
                    let have = p.pre.get(key).cloned().unwrap_or_else(|| dom.values.clone());
                    if u.reason == R_FACT && u.what == key && labs == have {
                        return EffOut::Same;
                    }
                    self.guesses.inc(&u.reason);
                    return match labs.len() {
                        0 => EffOut::None,
                        1 => EffOut::Eff(Eff::Set(labs[0])),
                        _ => EffOut::Eff(Eff::Guess(labs)),
                    };
                }
                self.guesses.inc(&u.reason);
                EffOut::Eff(Eff::Guess(dom.values.clone()))
            }
        }
    }

    // --- everything ----------------------------------------------------------------------------------------------------

    /// `Builder.read_all`: every input, then events until no new one appears, in passes (a value written outside
    /// its domain widens the domain and the reading runs again).
    pub fn read_all(&mut self, m: &mut dyn Machine, start_events: bool, skip: Option<&dyn Fn(&Event) -> bool>, passes: usize) -> usize {
        self.read_all_into(m, start_events, skip, passes, None)
    }

    /// `read_all`, handing the records to `sink` as they are made, job by job in the jobs' order (and telling it
    /// when a pass starts over).
    pub fn read_all_into(
        &mut self,
        m: &mut dyn Machine,
        start_events: bool,
        skip: Option<&dyn Fn(&Event) -> bool>,
        passes: usize,
        mut sink: Option<&mut dyn RecordSink>,
    ) -> usize {
        self.table.value_sets = self.switches.value_sets;
        for n in 0..passes {
            if n > 0 {
                if let Some(s) = &mut sink {
                    s.reset();
                }
            }
            self.read_once(m, start_events, skip, &mut sink);
            self.counts.set(format!("pass {}: records", n + 1), self.n_records as i64);
            if self.out_of_domain.is_empty() || n == passes - 1 {
                break;
            }
            self.widened.push(self.out_of_domain.clone());
            let added = self.extend_domains();
            self.counts.set(format!("pass {}: domain values added for the next pass", n + 1), added);
            self.reset();
        }
        self.counts.set("records", self.n_records as i64);
        if self.switches.due_first {
            crate::due::apply(self);
        }
        self.n_records
    }

    /// `Builder._reset`: forget a pass (the "pass N" counts stay).
    pub fn reset(&mut self) {
        let keep: Vec<(String, i64)> = self.counts.0.iter().filter(|(k, _)| k.starts_with("pass ")).map(|(k, v)| (k.clone(), *v)).collect();
        self.records.clear();
        self.events_done.clear();
        self.sig_ids.clear();
        self.counts.clear();
        self.ends.clear();
        self.drops.clear();
        self.nonfact_checks.clear();
        self.guesses.clear();
        self.rd.clear();
        self.unknown_writes.clear();
        self.unknown_sites.clear();
        for (k, v) in keep {
            self.counts.add(k, v);
        }
        self.out_of_domain.clear();
        self.table.args_vals.clear();
        self.args_doms.borrow_mut().clear();
        self.sig_seen.clear();
        self.folded.clear();
        self.ego_cache = None;
        self.fact_info.clear();
        self.n_records = 0;
        self.args_set.clear();
    }

    /// `Builder.extend_domains`: add the written values a domain lacked; returns how many.
    pub fn extend_domains(&mut self) -> i64 {
        let mut added = 0;
        for (key, vals) in self.out_of_domain.clone() {
            let dom = self.domain(&key).unwrap();
            let mut ints: BTreeSet<i64> = dom.values.iter().filter_map(|v| v.int()).collect();
            ints.extend(vals);
            self.fact_info.clear();
            let above = dom.values.contains(&Label::Above);
            let below = dom.values.contains(&Label::Below);
            if above {
                ints.insert(dom.high.unwrap() + 1);
            }
            if below {
                ints.insert(dom.low.unwrap() - 1);
            }
            let new = Domain::of(ints, if below { dom.low } else { None }, if above { dom.high } else { None });
            added += new.values.len() as i64 - dom.values.len() as i64;
            self.table.extra.insert(key, new);
        }
        // reader.py reads VAR_EGO afresh on every call: its domain may have grown
        self.ego_cache = None;
        added
    }

    fn read_once(
        &mut self,
        m: &mut dyn Machine,
        start_events: bool,
        skip: Option<&dyn Fn(&Event) -> bool>,
        sink: &mut Option<&mut dyn RecordSink>,
    ) {
        self.sig_seen.clear();
        self.folded.clear();
        self.records.clear();
        self.n_records = 0;
        self.args_set.clear();
        let t0 = std::time::Instant::now();
        let jobs = self.inputs();
        *self.phase_secs.entry("inputs()").or_default() += t0.elapsed().as_secs_f64();
        let tr = std::time::Instant::now();
        let n_jobs = jobs.len();
        if let Some(f) = &mut self.on_batch_start {
            f("inputs", n_jobs, 0);
        }
        let mut nexts = NextEvents::default();
        self.run_jobs_shared(m, Arc::new(jobs), &mut |b: &mut Builder<'_>, recs: Vec<Record>| {
            nexts.add(&recs);
            b.emit(recs, sink);
        });
        let jobs_len = n_jobs;
        let n = self.n_records;
        *self.phase_secs.entry("runs").or_default() += tr.elapsed().as_secs_f64();
        self.tell("inputs", jobs_len, n, t0.elapsed().as_secs_f64());
        let mut todo: Vec<Event> = if start_events { self.start_events() } else { vec![] };
        todo.extend(nexts.events);
        let mut rounds = 0;
        while !todo.is_empty() {
            rounds += 1;
            let te = std::time::Instant::now();
            let mut jobs = Vec::new();
            let mut added: FxHashSet<(String, UnitKey, i64, bool)> =
                todo.iter().filter(|e| e.sig == 0).map(|e| (e.name.clone(), e.unit, e.w, e.abort)).collect();
            let n0 = todo.len();
            for i in 0..n0 {
                let ev = &todo[i];
                let k = (ev.name.clone(), ev.unit, ev.w, ev.abort);
                if ev.sig != 0 && !added.contains(&k) && !self.events_done.contains(&(ev.name.clone(), ev.unit, ev.w, 0, ev.abort)) {
                    added.insert(k);
                    let copy = ev.unknown_locals(true);
                    todo.push(copy);
                    self.counts.inc("events with unknown locals (args 0) added for replans");
                }
            }
            *self.phase_secs.entry("events: replans").or_default() += te.elapsed().as_secs_f64();
            let mut tp = 0.0;
            for ev in &todo {
                let ev = self.limit(ev);
                let k = (ev.name.clone(), ev.unit, ev.w, ev.sig, ev.abort);
                if self.events_done.contains(&k) {
                    continue;
                }
                self.events_done.insert(k);
                if skip.is_some_and(|f| f(&ev)) {
                    self.counts.inc("events left out by the caller");
                    continue;
                }
                let tq = std::time::Instant::now();
                let (ok, pins, pre) = self.event_pins(&ev);
                tp += tq.elapsed().as_secs_f64();
                if !ok {
                    self.counts.inc("events whose conditions never hold (constants)");
                    continue;
                }
                let kind = if ev.abort { "skip_cutscene" } else { "event" };
                jobs.push(Job { kind: kind.into(), label: ev.label(), pins, pre, what: What::Event(ev), checks: vec![] });
            }
            *self.phase_secs.entry("events").or_default() += te.elapsed().as_secs_f64();
            *self.phase_secs.entry("events: event_pins").or_default() += tp;
            let t0 = std::time::Instant::now();
            let n0 = self.n_records;
            let n_jobs = jobs.len();
            let what = format!("event round {rounds}");
            if let Some(f) = &mut self.on_batch_start {
                f(&what, n_jobs, todo.len());
            }
            drop(todo);
            let mut nexts = NextEvents::default();
            self.run_jobs_shared(m, Arc::new(jobs), &mut |b: &mut Builder<'_>, recs: Vec<Record>| {
                nexts.add(&recs);
                b.emit(recs, sink);
            });
            todo = nexts.events;
            *self.phase_secs.entry("runs").or_default() += t0.elapsed().as_secs_f64();
            let n = self.n_records - n0;
            self.tell(&what, n_jobs, n, t0.elapsed().as_secs_f64());
        }
        self.counts.set("event rounds", rounds);
        let tw = std::time::Instant::now();
        let folded = self.folded.clone();
        for r in &mut self.records {
            for (k, e) in r.effects.iter_mut() {
                if k.starts_with("args:") {
                    if let Eff::Set(Label::Int(s)) = e {
                        if folded.contains(s) {
                            *e = Eff::Set(Label::Int(0));
                        }
                    }
                }
            }
            for (k, s) in r.pre.iter_mut() {
                if k.starts_with("args:") && s.iter().any(|v| matches!(v, Label::Int(x) if folded.contains(x))) {
                    *s = vec![Label::Int(0)];
                }
            }
        }
        if self.keep_records {
            self.add_args_vals_from_records(None);
        } else {
            // the records are gone: their `args:` values as the rewrite above leaves them
            for (k, vals) in std::mem::take(&mut self.args_set) {
                let e = self.table.args_vals.entry(k).or_default();
                for s in vals {
                    e.insert(if folded.contains(&s) { 0 } else { s });
                }
            }
        }
        let collisions: i64 = self.sig_ids.values().map(|t| t.len() as i64 - 1).sum();
        self.counts.set("lists of locals sharing a number (hash collisions)", collisions);
        *self.phase_secs.entry("end of pass").or_default() += tw.elapsed().as_secs_f64();
    }

    /// One job's records, as made: to the sink, then kept (or, with `keep_records` off, only counted).
    fn emit(&mut self, recs: Vec<Record>, sink: &mut Option<&mut dyn RecordSink>) {
        if recs.is_empty() {
            return;
        }
        let t = std::time::Instant::now();
        let first = self.n_records;
        self.n_records += recs.len();
        if self.keep_records {
            if let Some(s) = sink {
                s.batch(first, &recs, &self.table);
            }
            self.records.extend(recs);
        } else {
            for r in &recs {
                for (k, e) in &r.effects {
                    if let (true, Eff::Set(Label::Int(s))) = (k.starts_with("args:"), e) {
                        self.args_set.entry(k.clone()).or_default().insert(*s);
                    }
                }
            }
            if let Some(s) = sink {
                s.batch_owned(first, recs, &self.table);
            }
        }
        *self.phase_secs.entry("emit").or_default() += t.elapsed().as_secs_f64();
    }

    /// The progress callback (reader.py `Builder.progress`): what ran, its jobs, its records, its seconds.
    fn tell(&mut self, what: &str, jobs: usize, records: usize, secs: f64) {
        if let Some(p) = &mut self.progress {
            p(what, jobs, records, secs);
        }
    }

    fn add_args_vals_from_records(&mut self, from: Option<usize>) {
        for r in &self.records[from.unwrap_or(0)..] {
            for (k, e) in &r.effects {
                if k.starts_with("args:") {
                    if let Eff::Set(Label::Int(s)) = e {
                        self.table.args_vals.entry(k.clone()).or_default().insert(*s);
                    }
                }
            }
        }
    }

    /// `Builder._limit`: past SIG_LIMIT a wait point's further lists of locals are folded into 0.
    pub fn limit(&mut self, ev: &Event) -> Event {
        if ev.sig == 0 {
            return ev.clone();
        }
        let seen = self.sig_seen.entry((ev.name.clone(), ev.w)).or_default();
        if seen.contains(&ev.sig) {
            return ev.clone();
        }
        if !self.folded.contains(&ev.sig) && seen.len() < self.sig_limit {
            seen.push(ev.sig);
            return ev.clone();
        }
        if !self.folded.contains(&ev.sig) {
            self.folded.insert(ev.sig);
            self.counts.inc("lists of locals past SIG_LIMIT (taken as unknown)");
        }
        ev.unknown_locals(false)
    }

    /// `Builder._run_jobs`: the event jobs' lists of locals registered first, the records' after. Each job's
    /// records go to `each` in the jobs' order, as made: here, one job after another, or where the machine runs
    /// the batch (`Machine::run_batch`, par.rs).
    pub fn run_jobs_each(&mut self, m: &mut dyn Machine, jobs: &[Job], each: &mut dyn FnMut(&mut Builder<'_>, Vec<Record>)) {
        self.run_jobs_shared(m, Arc::new(jobs.to_vec()), each)
    }

    /// `run_jobs_each` on jobs a machine can share with its threads.
    pub fn run_jobs_shared(&mut self, m: &mut dyn Machine, jobs: Arc<Vec<Job>>, each: &mut dyn FnMut(&mut Builder<'_>, Vec<Record>)) {
        for job in jobs.iter() {
            if let What::Event(e) = &job.what {
                if e.sig != 0 {
                    self.table.args_vals.entry(format!("args:{}", e.name)).or_default().insert(e.sig);
                }
            }
        }
        let start = self.on_job.is_some().then(|| (self.table.args_vals.clone(), self.folded.clone()));
        let mut args: Vec<(String, i64)> = Vec::new();
        {
            let mut each2 = |b: &mut Builder<'_>, i: usize, recs: Vec<Record>| {
                if let (Some(h), Some((a, f))) = (b.on_job.as_mut(), &start) {
                    h(&jobs[i], &recs, a, f);
                }
                for r in &recs {
                    for (k, e) in &r.effects {
                        if let (true, Eff::Set(Label::Int(s))) = (k.starts_with("args:"), e) {
                            args.push((k.clone(), *s));
                        }
                    }
                }
                each(b, recs);
            };
            if !m.run_batch(self, &jobs, &mut each2) {
                for (i, job) in jobs.iter().enumerate() {
                    let recs = self.run_job(m, job);
                    each2(self, i, recs);
                }
            }
        }
        for (k, s) in args {
            self.table.args_vals.entry(k).or_default().insert(s);
        }
    }

    /// `run_jobs_each`, the records returned.
    pub fn run_jobs(&mut self, m: &mut dyn Machine, jobs: &[Job]) -> Vec<Record> {
        let mut out = Vec::new();
        self.run_jobs_each(m, jobs, &mut |_: &mut Builder<'_>, recs: Vec<Record>| out.extend(recs));
        out
    }

    // --- golden.py's reader file ------------------------------------------------------------------------------------

    /// The "reading" line's scope part (golden.py `write_reading`), with `info` (name, start, workers, sources...).
    pub fn reading_py(&self, info: &[(String, Py)]) -> Py {
        let mut keys: Vec<UnitKey> = self.scope.units.iter().map(|&u| self.game.unit(u).key).collect();
        keys.sort_by_key(|&k| crate::facts::unit_text(k));
        let mut fields: Vec<(String, Py)> = info.to_vec();
        let mut put = |k: &str, v: Py| fields.push((k.to_string(), v));
        put("rooms", Py::List(self.scope.rooms.iter().map(|&r| Py::Int(r as i64)).collect()));
        put("units", Py::List(keys.into_iter().map(crate::facts::unit_py).collect()));
        put("objects", Py::List(self.scope.objects.iter().map(|&o| Py::Int(o)).collect()));
        put("verbs", Py::List(self.scope.verbs.iter().map(|&o| Py::Int(o)).collect()));
        put("dialog_verbs", Py::List(self.scope.dialog_verbs.iter().map(|&o| Py::Int(o)).collect()));
        put("walk_verb", Py::opt_int(self.scope.walk_verb));
        put("universe", Py::strs(&self.scope.universe));
        put("scope_counts", self.scope.counts.py());
        put("n_records", Py::Int(if self.keep_records { self.records.len() } else { self.n_records } as i64));
        Py::Dict(fields)
    }

    /// The "builder" line (golden.py `BUILDER_COUNTERS` and the Reader's).
    pub fn counters_py(&self) -> Py {
        Py::Dict(vec![
            ("counts".into(), self.counts.py()),
            ("ends".into(), self.ends.py()),
            ("drops".into(), self.drops.py()),
            ("nonfact_checks".into(), self.nonfact_checks.py()),
            ("guesses".into(), self.guesses.py()),
            ("unknown_writes".into(), self.unknown_writes.py()),
            ("unknown_sites".into(), self.unknown_sites.py()),
            (
                "out_of_domain".into(),
                Py::Dict(
                    self.out_of_domain
                        .iter()
                        .map(|(k, v)| (k.clone(), Py::Set(v.iter().map(|&x| Py::Int(x)).collect())))
                        .collect(),
                ),
            ),
            ("rd.counts".into(), self.rd.counts.py()),
            ("rd.forks".into(), self.rd.forks.py()),
            ("rd.fork_keys".into(), self.rd.fork_keys.py()),
        ])
    }

    /// The whole reader file, line by line (golden.py `write_reading`).
    pub fn reading_lines(&self, info: &[(String, Py)]) -> impl Iterator<Item = String> + '_ {
        let head = Py::dict(vec![("kind", Py::str("reading")), ("reading", self.reading_py(info))]).text();
        let recs = self.records.iter().enumerate().map(|(i, r)| {
            Py::dict(vec![("kind", Py::str("record")), ("index", Py::Int(i as i64)), ("record", r.py())]).text()
        });
        let tail = Py::dict(vec![("kind", Py::str("builder")), ("builder", self.counters_py())]).text();
        std::iter::once(head).chain(recs).chain(std::iter::once(tail))
    }
}

enum EffOut {
    Eff(Eff),
    Same,
    None,
}


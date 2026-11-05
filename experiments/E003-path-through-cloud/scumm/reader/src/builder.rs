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

/// E014's switch `FMI_EGO_VALUES=1`: VAR_EGO may have several values in a scope (`Builder::ego_values`).
fn ego_values_switch() -> bool {
    std::env::var("FMI_EGO_VALUES").is_ok_and(|v| v == "1")
}

/// E014's switch `FMI_READ_CUT`: under split_limit, a run past the path limit with no fact left to split on, or with
/// paths the step limit ended, is read short in place of stopping the reading, its cut paths dropped and counted.
/// `1`: such a run gives no record (the machine hands over none of its paths, as for a run split_limit splits);
/// `paths`: the paths it finished make its records (slower: on the interactive demo's room 49 alone the reading ran
/// past 10 minutes against 30 s, its finished paths starting events that meet the same loop again). Unset: the reading
/// stops there, as before.
pub(crate) fn read_cut_switch() -> bool {
    read_cut_mode() != 0
}

/// `FMI_READ_CUT`'s mode: 0 unset, 1 the run gives no record, 2 (`paths`) its finished paths make records.
pub(crate) fn read_cut_mode() -> u8 {
    static ON: std::sync::OnceLock<u8> = std::sync::OnceLock::new();
    *ON.get_or_init(|| match std::env::var("FMI_READ_CUT").as_deref() {
        Ok("1") => 1,
        Ok("paths") => 2,
        _ => 0,
    })
}

/// E014's switch `FMI_READ_WIDEN=1` (plan.md, amendment (6)): under split_limit, a run past the path limit is run
/// again once with its loops widened (scumm-machine's `widen.rs`); if that run ends under the limit it is the run, and
/// nothing is split. A run the limit does not cut is never run again. Unset: as before.
pub(crate) fn read_widen_switch() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var("FMI_READ_WIDEN").is_ok_and(|v| v == "1"))
}

/// E014 amendment (8)'s switch `FMI_READ_SOUND=1`: the ops whose only effect is on the sound or the screen
/// (soundKludge, setRoomShadow, debug) have no tracked effect (scumm-machine's `heads::effect_with`), and
/// VAR_SOUNDRESULT, which the engine's sound queue writes outside the scripts, is no fact (`domains::engine_why`).
/// Unset: as before (each such op ends its path "unmodelled", dropped).
pub fn read_sound_switch() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var("FMI_READ_SOUND").is_ok_and(|v| v == "1"))
}

/// Path ends that are no action (reader.DROP_ENDS: model.INFEASIBLE and the reader's own).
pub const DROP_ENDS: [&str; 9] =
    ["spin", "nesting", "error", "limit", "unmodelled", "deferral unknown", "nothing ran", "left the scope", "no value"];

/// Whether a path's end is no action: DROP_ENDS, or FMI_READ_WIDEN's "widened" (a loop came round again in a state
/// its widened iteration holds; scumm-machine's `widen.rs`), which no path ends with when the switch is off.
pub fn dropped_end(e: &str) -> bool {
    DROP_ENDS.contains(&e) || e == "widened"
}
/// The Reader's counters the Builder reads to decide a split.
/// The switch grow_limit: the largest path limit a run is read again with before the reading stops.
pub const GROW_MAX: usize = 16_384;
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
    /// the switch --reach: the branches this run held back (reach.rs)
    pub held: Vec<crate::reach::Held>,
    /// the switch pass2_reuse, in pass 1: the facts the machine half consulted, and its questions (memo.rs)
    pub touched: Vec<String>,
    pub queries: Vec<crate::memo::Query>,
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

    /// The switch grow_limit: the path limit of the runs that follow, 0 for the machine's own. The default does
    /// nothing.
    fn set_path_limit(&mut self, _n: usize) {}

    /// E014's switch FMI_READ_WIDEN: the runs that follow widen their loops (scumm-machine's `widen.rs`). The default
    /// does nothing.
    fn set_widen(&mut self, _on: bool) {}
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
    pub outside_points: BTreeMap<String, BTreeMap<i64, u64>>,
    pub rd: RdCounts,
    pub sig_ids: BTreeMap<i64, BTreeSet<String>>,
    /// the `args:` values `record` added to the table (`table.args_vals`), in order
    pub args: Vec<(String, i64)>,
    /// the switch --reach: the branches the job held back
    pub held: Vec<crate::reach::Held>,
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
    /// the switch --prep: what the reading's thread and the sink work out of the records, worked out here
    pub prep: Option<JobPrep>,
}

impl JobOut {
    /// The job's records, also those that stayed with its worker (`Lite`).
    pub fn n_records(&self) -> usize {
        self.records.len() + self.prep.as_ref().and_then(|p| p.lite.as_ref()).map_or(0, |l| l.n)
    }
}

/// The switch --prep (E011 round 2): what the reading's thread (`reach_after_job`) and the sink work out of a job's
/// records, worked out by the worker that ran the job (par.rs), so that their own threads only take it in the jobs'
/// order. Each part is a function of the records and of what stays fixed while the job's batch runs (the table's
/// domains, the reached values), made by the same code the reading's thread or the sink would run.
#[derive(Default)]
pub struct JobPrep {
    /// per record, the (d) effects with their first unmet condition (reach.rs `reach_prep`)
    pub reach: Option<Vec<crate::reach::ReachEff>>,
    /// the sink's part, one per record (the sink's own type: `PrepFn`)
    pub sink: Option<Box<dyn std::any::Any + Send>>,
    /// all that the reading's thread reads of the records besides the parts above, when the records stay with the
    /// worker (`Builder::prep_lite`; `JobOut::records` is then empty)
    pub lite: Option<Lite>,
}

/// What the reading's thread reads of a job's records (`run_jobs_shared`, `NextEvents::add`, `emit`) when they stay
/// with the worker that made them: their count, their `args:` sets in order, their next events (each (name, unit,
/// w, sig, abort) once, in order).
#[derive(Debug)]
pub struct Lite {
    pub n: usize,
    pub args: Vec<(String, i64)>,
    pub next_events: Vec<Event>,
}

impl Lite {
    pub fn of(recs: &[Record]) -> Lite {
        let mut args = Vec::new();
        for r in recs {
            for (k, e) in &r.effects {
                if let (true, Eff::Set(Label::Int(s))) = (k.starts_with("args:"), e) {
                    args.push((k.clone(), *s));
                }
            }
        }
        let mut nexts = NextEvents::default();
        nexts.add(recs);
        Lite { n: recs.len(), args, next_events: nexts.events }
    }
}

impl std::fmt::Debug for JobPrep {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "JobPrep {{ reach: {:?}, sink: {}, lite: {:?} }}", self.reach.as_ref().map(|r| r.len()), self.sink.is_some(), self.lite.as_ref().map(|l| l.n))
    }
}

/// The sink's part of the switch --prep: made from a job's records and the worker's table.
pub type PrepFn = dyn Fn(&[Record], &FactTable) -> Box<dyn std::any::Any + Send> + Send + Sync;

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
    /// `batch_owned` with the sink's part of the switch --prep (`JobPrep::sink`), made where the job ran. The default
    /// leaves it.
    fn batch_prepared(&mut self, first: usize, recs: Vec<Record>, table: &FactTable, _prep: Box<dyn std::any::Any + Send>) {
        self.batch_owned(first, recs, table)
    }
}

/// The Builder's read-only state a machine run needs.
pub struct Ctx<'a> {
    pub game: &'a Game,
    pub scope: &'a Scope,
    pub table: &'a FactTable,
    /// the switch --reach: the identity of the job being run (reach.rs `job_key`), by which its guides are found;
    /// None: no guides
    pub job_key: Option<&'a str>,
}

/// The next round's events as the records of a batch give them, in their order, each once: a later event with the
/// same (unit name, unit, wait point, list of locals, Esc) as an earlier one changes nothing in `_read_once`'s round
/// (its replan copy is the earlier one's, `_limit` gives it the same key, and `events_done` then skips it), so it is
/// not kept (at Part I's scale the inputs' records name millions of events, a few tens of thousands of them distinct).
#[derive(Default)]
pub(crate) struct NextEvents {
    pub(crate) events: Vec<Event>,
    seen: FxHashSet<(String, UnitKey, i64, i64, bool)>,
}

impl NextEvents {
    pub(crate) fn add(&mut self, recs: &[Record]) {
        for r in recs {
            for e in &r.next_events {
                if self.seen.insert((e.name.clone(), e.unit, e.w, e.sig, e.abort)) {
                    self.events.push(e.clone());
                }
            }
        }
    }

    /// `add` of a job's records whose next events a worker took (`Lite`): the same events, each once, in order.
    pub(crate) fn add_events(&mut self, evs: Vec<Event>) {
        for e in evs {
            if self.seen.insert((e.name.clone(), e.unit, e.w, e.sig, e.abort)) {
                self.events.push(e);
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
    /// F5: scumm-machine's draw_clears; the scope's and the domains' writes of it (Scope::of_with, domains::build_with)
    pub draw_clears: bool,
    /// F9: scumm-machine's actor_rooms; the scope's writes and the table's domains of it (Scope::of_with, table.rs)
    pub actor_rooms: bool,
    /// A run past the path limit is split on its first fork on a read, again and again, until no part passes the
    /// limit; a run that passes it with no fact left to split on, or a path the step limit ended, stops the reading
    /// (`Builder::run_split`). Off: reader.py's rule, which leaves a run unsplit when it forked more on non-facts,
    /// past SPLIT_DEPTH levels or SPLIT_BUDGET parts, and drops the paths the limit ended.
    pub split_limit: bool,
    /// Forward reachability (reach.rs, experiments/E004-fast-exact/reach.md): a job or a fork's branch that needs a
    /// value not reached from the start does not run, and runs when the value arrives. Needs split_limit.
    pub reach: bool,
    /// F1c (fix-round1.md): a sentence or a walk with a room's input script that is not plain starts at that script,
    /// from the sentence variables as constants (inputs.rs; machine.rs `f1c_click`; `Builder::settled_jobs`)
    pub input_settled: bool,
    /// Pass 2 reruns only what a widened value touches (memo.rs, experiments/E004-fast-exact/pass2-reuse.md); the output
    /// is byte for byte the same.
    pub pass2_reuse: bool,
    /// A run past the path limit with no fact left to split on is read again whole with the limit 4 times larger,
    /// up to GROW_MAX, before the reading stops (`Builder::run_split`). Needs split_limit.
    pub grow_limit: bool,
    /// A path the step limit ended becomes a havoc record (havoc.rs, experiments/E004-fast-exact/havoc-cut.md) in
    /// place of stopping the reading. Needs split_limit.
    pub havoc_cut: bool,
    /// Jobs that differ only in an input argument their run never read share one run (machine.rs `SharedRuns`,
    /// experiments/E004-fast-exact/shared-runs.md); the output is byte for byte the same.
    pub shared_runs: bool,
    /// h² over the pairs of the verb script and the room (pairs.rs, experiments/E004-fast-exact/reach-pairs.md): a job
    /// whose seed has a set on both runs only when a reached pair meets them. Needs --reach.
    pub reach_pairs: bool,
    /// The rough reading (experiments/E004-fast-exact/rough-reading.md): every job is read with the machine's step
    /// limit N in place of STEP_LIMIT, so a path past N steps ends there and havoc_cut makes it a havoc record. 0 (the
    /// default): the machine's own limit. Needs havoc_cut. By name `step_limit=N`; reader-scope's `--step-limit N`.
    pub step_limit: u32,
    /// havoc_cut's closure names, for a start whose operand does not fold, the units the engine can run for it: an
    /// object's verb entry, a variable's values (havoc.rs `closure`, experiments/E004-fast-exact/havoc-cut.md, "The
    /// switch havoc_starts"), in place of every unit. Needs havoc_cut.
    pub havoc_starts: bool,
    /// havoc_cut's closure names, for a room change, the exit code of the rooms it can leave and the entry code of the
    /// rooms it can load (havoc.rs `closure`, experiments/E005-narrow-havoc/havoc-rooms.md), in place of every scope
    /// room. Needs havoc_cut.
    pub havoc_rooms: bool,
    /// havoc_cut's closure names, for a start by a local or an indexed variable, the scripts of the values a static
    /// analysis of the game's code gives it (args.rs, experiments/E005-narrow-havoc/havoc-args.md), in place of every
    /// unit. Needs havoc_cut.
    pub havoc_args: bool,
    /// A WaitFor op whose test reads engine state no fact holds both blocks and passes in the same run
    /// (experiments/E004-fast-exact/wait-both.md; machine.rs `wait_both`)
    pub wait_both: bool,
    /// With --frame-costs: an event at a WaitFor op that can pass at once costs 0 frames (costs.rs,
    /// experiments/E006-passed-wait/plan.md); the records are the same
    pub wait_pass_cost: bool,
}

impl Switches {
    /// Comma-separated switch names.
    pub fn parse(names: &str) -> Result<Switches, String> {
        let mut s = Switches::default();
        for n in names.split(',').map(str::trim).filter(|n| !n.is_empty()) {
            match n {
                "due_first" => s.due_first = true,
                "value_sets" => s.value_sets = true,
                "draw_clears" => s.draw_clears = true,
                "actor_rooms" => s.actor_rooms = true,
                "split_limit" => s.split_limit = true,
                "reach" => s.reach = true,
                "pass2_reuse" => s.pass2_reuse = true,
                "input_settled" => s.input_settled = true,
                "grow_limit" => s.grow_limit = true,
                "havoc_cut" => s.havoc_cut = true,
                "shared_runs" => s.shared_runs = true,
                "reach_pairs" => s.reach_pairs = true,
                "havoc_starts" => s.havoc_starts = true,
                "havoc_rooms" => s.havoc_rooms = true,
                "havoc_args" => s.havoc_args = true,
                "wait_both" => s.wait_both = true,
                "wait_pass_cost" => s.wait_pass_cost = true,
                _ if n.starts_with("step_limit=") => {
                    s.step_limit = n["step_limit=".len()..].parse().map_err(|_| format!("reader switch {n}: step_limit=N with N a number"))?
                }
                _ => return Err(format!("unknown reader switch {n} (known: due_first, value_sets, draw_clears, actor_rooms, split_limit, reach, pass2_reuse, input_settled, grow_limit, havoc_cut, shared_runs, reach_pairs, havoc_starts, havoc_rooms, havoc_args, wait_both, wait_pass_cost, step_limit=N)")),
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
        if self.draw_clears {
            v.push("draw_clears");
        }
        if self.actor_rooms {
            v.push("actor_rooms");
        }
        if self.split_limit {
            v.push("split_limit");
        }
        if self.reach {
            v.push("reach");
        }
        if self.pass2_reuse {
            v.push("pass2_reuse");
        }
        if self.input_settled {
            v.push("input_settled");
        }
        if self.grow_limit {
            v.push("grow_limit");
        }
        if self.havoc_cut {
            v.push("havoc_cut");
        }
        if self.shared_runs {
            v.push("shared_runs");
        }
        if self.reach_pairs {
            v.push("reach_pairs");
        }
        if self.havoc_starts {
            v.push("havoc_starts");
        }
        if self.havoc_rooms {
            v.push("havoc_rooms");
        }
        if self.havoc_args {
            v.push("havoc_args");
        }
        if self.wait_both {
            v.push("wait_both");
        }
        if self.wait_pass_cost {
            v.push("wait_pass_cost");
        }
        v
    }
    /// The machine's switches the scope and the domains take (Scope::of_with, domains::build_with): F5's and F9's.
    /// value_sets reaches the Reader's machine through the table (FactTable::value_sets, Reader::run_full).
    pub fn scope(&self) -> scumm_machine::Switches {
        scumm_machine::Switches { draw_clears: self.draw_clears, actor_rooms: self.actor_rooms, ..Default::default() }
    }
}

/// Fix round 1, F10: the flags of a slot live in the engine, as the bridge dumps them (SPEEDRUN_DUMP_LOCALS):
/// ScriptSlot's freezeResistant and recursive (script.h:91), and whether a room change leaves it alive, which is its
/// `where` (an object's slot in the inventory, WIO_INVENTORY; killScriptsAndResources, script.cpp:1083-1106) and the
/// model's owned flag (scumm-machine `room_slot`). Under the switch --live-locals a list's number covers them
/// (`Builder::list_number`), so a dumped slot's list is numbered and matched with them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SlotFlags {
    pub resistant: bool,
    pub recursive: bool,
    pub owned: bool,
}

/// Fix round 1, F10: a slot live in the engine, with the locals the bridge dumped (SPEEDRUN_DUMP_LOCALS:
/// vm.localvar, script.h:136; its cutsceneOverride and the cutSceneData of the levels it began, script.cpp:1640).
/// The reading builds this slot's events from these locals (`Builder::dumped_events`), as `record` builds the
/// events of a slot a path left waiting with known locals.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlotLocals {
    /// the unit's name as facts.py names it: script:N, local:N, obj:N, entry, exit
    pub name: String,
    /// the room the engine is in (a local script or the room's entry and exit code is that room's)
    pub room: i32,
    /// where the slot waits (facts.py `at:<unit>:W`)
    pub w: i64,
    /// the override armed in this slot at the top cutscene level (facts.py `esc:<unit>:W`)
    pub esc: Option<i64>,
    /// Local[0..NUM_SCRIPT_LOCAL]
    pub locals: Vec<i64>,
    /// cutscenes the slot began and has not ended (ScriptSlot.cutsceneOverride; Frame.override)
    pub override_: i64,
    /// their cutSceneData, the bottom level first (Path.cut_data's order)
    pub cut_data: Vec<i64>,
    /// an object's script whose object is not in the room (`start_events`' owned)
    pub owned: bool,
    /// the slot's flags from the dump (None: a dump without them); needed under the switch --live-locals
    pub flags: Option<SlotFlags>,
}

impl SlotLocals {
    /// The locals as a list of a known slot holds them: Local[n] -> value, the zeros left out (with known args a
    /// local the list lacks reads 0, model.py `Machine.local`).
    pub fn list(&self) -> Vec<(i64, Val)> {
        self.locals.iter().enumerate().filter(|(_, &v)| v != 0).map(|(i, &v)| (i as i64, Val::Int(v))).collect()
    }
}

/// reader.py `Builder`.
pub struct Builder<'g> {
    pub game: &'g Game,
    /// fix round 1's switches (all off: reader.py's reading)
    pub switches: Switches,
    /// the switch grow_limit: the path limit of the run being read again whole, 0 for the machine's own
    pub path_limit_now: usize,
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
    /// the places written to a slot that are none of the scope's points for its name, in this pass: `pos:<unit>` or
    /// `escpos:<unit>` -> place -> records (the scope-1 finding, PORT-NOTES.md "ELSEWHERE"; listed whether the
    /// switch is on or off; not part of the reading)
    pub outside_points: BTreeMap<String, BTreeMap<i64, u64>>,
    pub rd: RdCounts,
    pub sig_ids: BTreeMap<i64, BTreeSet<String>>,
    pub sig_seen: FxHashMap<(String, i64), Vec<i64>>,
    pub folded: BTreeSet<i64>,
    pub events_done: FxHashSet<(String, UnitKey, i64, i64, bool)>,
    /// the pins of the last run (the Reader's oracle's, which `event_pins` lists slot keys from)
    pub last_pins: Pins,
    bverb_cache: FxHashMap<(i64, i64), BTreeSet<i64>>,
    unknown_cache: FxHashMap<(String, String), Option<Vec<String>>>,
    /// `sig`'s lists numbered so far, by the hash of their inputs
    sig_cache: FxHashMap<u64, Vec<SigMade>>,
    /// `unknown_targets_listed`'s answers, by the hash of (head, cite)
    unknown_listed: FxHashMap<u64, Vec<(String, String, bool)>>,
    cites: Option<FxHashMap<String, (UnitId, u32)>>,
    ego_cache: Option<Vec<Label>>,
    /// kind and domain of the keys `effect` reads, for one pass (an `args:`, `pos:` or `escpos:` domain grows
    /// within a pass: not kept)
    fact_info: FxHashMap<String, std::rc::Rc<(Kind, Option<Domain>)>>,
    /// false: the records go to the sink and are not kept (`records` stays empty, so memory does not grow with
    /// the record count); the reading is the same, `n_records` counts them
    /// the switch `--frame-costs` (costs.rs): each record's cost in engine frames; None: every record costs 1
    pub frame_costs: Option<std::sync::Arc<crate::costs::FrameCosts>>,
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
    /// `pin_kind`'s cache
    pin_kinds: std::cell::RefCell<FxHashMap<String, Kind>>,
    /// `args_domain`'s cache: key -> (its count of list numbers, its domain)
    args_doms: std::cell::RefCell<FxHashMap<String, (usize, Domain)>>,
    /// F10: slots live in the engine with their dumped locals; their events are read with the start's (empty: the
    /// reading is reader.py's)
    pub slot_locals: Vec<SlotLocals>,
    /// F10: the events `dumped_events` made in the last pass (not the Esc ones), with their lists' numbers
    pub slot_events: Vec<Event>,
    /// the switch --reach: the starts the reached values are seeded from
    pub reach_starts: Vec<crate::facts::Facts>,
    /// the switch --reach: the rounds' state (reach.rs)
    pub(crate) reach_state: Option<Box<crate::reach::ReachState>>,
    /// the switch --reach: the branches the job being run held back
    pub(crate) reach_out: Vec<crate::reach::Held>,
    /// the switch --reach: the job being run, as reach.rs keys it (`job_key`)
    pub(crate) cur_job_key: Option<Arc<str>>,
    /// the switch pass2_reuse: what the job being run depends on, in pass 1 (memo.rs); the switch --memo: in every
    /// job that runs
    pub(crate) memo_log: Option<crate::memo::RunLog>,
    /// the switch --memo (experiments/E011-speed/memo.md): the store's root folder; None off
    pub memo_dir: Option<std::path::PathBuf>,
    /// the switch --memo in a resident process (run.rs `Resident`): the store kept in memory from reading to reading,
    /// with `memo_dir` (if any) its disk store; None off
    pub memo_mem: Option<Arc<crate::memo::MemStore>>,
    /// the switch --memo-units-set (E012 resident, memo.md's amendment of 2026-11-03): the store's key holds the
    /// scope's units as a set (their keys sorted), not in `Scope::units`' order; off: the key as before
    pub memo_units_set: bool,
    /// the switch --memo: each consulted key's kind, domain and constant start in this table (`Builder::key_info`)
    pub(crate) memo_infos: FxHashMap<String, (u8, Option<Domain>, Option<i64>)>,
    /// the switch --prep: the job being taken in (par.rs `run_batch`), its part worked out where it ran
    pub(crate) job_prep: Option<JobPrep>,
    /// the switch --prep (E011 round 2): the workers work out what the reading's thread and the sink would of each
    /// job's records (`JobPrep`); the reading is byte for byte the same. Off by default.
    pub prep: bool,
    /// the switch --prep: the sink's part (`PrepFn`), None for none
    pub prep_sink: Option<Arc<PrepFn>>,
    /// the switch --prep: the records stay with the worker (`Lite`); for a sink that reads only its part (`PrepFn`),
    /// with no records kept and no `on_job`
    pub prep_lite: bool,
}

/// An event's list of locals apart from the slot's flags, for `Builder::limit` under the switch --live-locals: a hash
/// of its unit name, locals, override count and cutscene data, as its number has them but for the flags. A function
/// of the list of all locals (the restriction to the read locals is one), so a wait point has no more of these than
/// lists of all locals.
pub fn flag_free(ev: &Event) -> i64 {
    use std::hash::{Hash, Hasher};
    let mut h = rustc_hash::FxHasher::default();
    ev.name.hash(&mut h);
    ev.locals.hash(&mut h);
    ev.override_.hash(&mut h);
    ev.cut_data.hash(&mut h);
    h.finish() as i64
}

/// A list `Builder::sig` numbered: its inputs, its number and its text.
struct SigMade {
    name: String,
    locals: Vec<(i64, Val)>,
    ov: i64,
    cd: Vec<CutData>,
    flags: Vec<(String, String)>,
    sig: i64,
    text: String,
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
            self.memo_infos.clear();
            self.pin_kinds.borrow_mut().clear();
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
            outside_points: std::mem::take(&mut self.outside_points),
            rd: std::mem::take(&mut self.rd),
            sig_ids: std::mem::take(&mut self.sig_ids),
            args: self.args_log.as_mut().map(std::mem::take).unwrap_or_default(),
            held: std::mem::take(&mut self.reach_out),
        }
    }

    /// What `take_deltas` took, back on the counters (a worker's Builder, whose counters are empty between jobs).
    pub(crate) fn set_deltas(&mut self, d: Deltas) {
        self.counts = d.counts;
        self.ends = d.ends;
        self.drops = d.drops;
        self.nonfact_checks = d.nonfact_checks;
        self.guesses = d.guesses;
        self.unknown_writes = d.unknown_writes;
        self.unknown_sites = d.unknown_sites;
        self.out_of_domain = d.out_of_domain;
        self.outside_points = d.outside_points;
        self.rd = d.rd;
        self.sig_ids = d.sig_ids;
        self.args_log = Some(d.args);
        self.reach_out = d.held;
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
        for (k, m) in d.outside_points {
            let e = self.outside_points.entry(k).or_default();
            for (w, n) in m {
                *e.entry(w).or_default() += n;
            }
        }
        self.rd.update(&d.rd);
        for (sig, texts) in d.sig_ids {
            self.sig_ids.entry(sig).or_default().extend(texts);
        }
        for (k, s) in d.args {
            self.table.args_vals.entry(k).or_default().insert(s);
        }
        self.reach_out.extend(d.held);
        self.last_pins = last_pins;
    }

    fn with_table(game: &'g Game, scope: Scope, table: FactTable) -> Builder<'g> {
        Builder {
            game,
            switches: Switches::default(),
            path_limit_now: 0,
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
            outside_points: BTreeMap::new(),
            rd: RdCounts::default(),
            sig_ids: BTreeMap::new(),
            sig_seen: FxHashMap::default(),
            folded: BTreeSet::new(),
            events_done: FxHashSet::default(),
            last_pins: Pins::new(),
            bverb_cache: FxHashMap::default(),
            unknown_cache: FxHashMap::default(),
            sig_cache: FxHashMap::default(),
            unknown_listed: FxHashMap::default(),
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
            pin_kinds: Default::default(),
            on_job: None,
            widened: Vec::new(),
            slot_locals: Vec::new(),
            slot_events: Vec::new(),
            frame_costs: None,
            reach_starts: Vec::new(),
            reach_state: None,
            reach_out: Vec::new(),
            cur_job_key: None,
            memo_log: None,
            memo_dir: None,
            memo_mem: None,
            memo_units_set: false,
            memo_infos: FxHashMap::default(),
            job_prep: None,
            prep: false,
            prep_sink: None,
            prep_lite: false,
        }
    }

    pub fn ctx(&self) -> Ctx<'_> {
        Ctx { game: self.game, scope: &self.scope, table: &self.table, job_key: self.cur_job_key.as_deref() }
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
            // E014 (FMI_EGO_VALUES=1): a game whose scripts give VAR_EGO several values in the scope. The inventory's
            // condition (owner = the ego) then takes every value of the domain: a superset of the engine's own test,
            // which the run decides (unset: one value, as MI's scopes always have)
            Kind::Fact if ego_values_switch() && dom.as_ref().is_some_and(|d| d.values.len() > 1) => {
                let d = dom.clone().unwrap();
                d.values
                    .iter()
                    .map(|&l| match self.table.value(&key, l) {
                        Val::Int(n) => Label::Int(n),
                        _ => l,
                    })
                    .collect()
            }
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

    /// `kind(key)`, kept for the table (a key's kind changes only with the table: `sync_table`, `reset` and
    /// `extend_domains` forget them).
    fn pin_kind(&self, key: &str) -> Kind {
        if let Some(&k) = self.pin_kinds.borrow().get(key) {
            return k;
        }
        let k = self.kind(key);
        self.pin_kinds.borrow_mut().insert(key.to_string(), k);
        k
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
        if self.pin_kind(key) == Kind::Const {
            return values.contains(&Label::Int(self.table.start.get0(key)));
        }
        let dom = if key.starts_with("args:") { Some(std::borrow::Cow::Owned(self.args_domain(key))) } else { self.table.domain(key) };
        if let Some(dom) = &dom {
            values = lab_set(values.iter().filter_map(|&v| dom.label_of(v)));
            if values.is_empty() {
                return false;
            }
            if values != dom.values || (key.starts_with("args:") && self.table.reach.is_some()) {
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
                for &o in self.scope.objects.iter().chain(&self.scope.held) {
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
            consts.iter().copied().filter(|b| self.scope.objects.contains(b) || self.scope.held.contains(b) || (0 < *b && *b < NUM_ACTORS)).collect();
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

    /// F1c: the switch's reading of the scripts (inputs.rs), once.
    pub fn settled_rule(&mut self) -> Option<std::sync::Arc<crate::inputs::SettledRule>> {
        if self.table.settled.is_none() {
            let rule = crate::inputs::settled_rule(self.game, &self.scope.units, &self.table.start);
            self.table.settled = rule.map(std::sync::Arc::new);
        }
        self.table.settled.clone()
    }

    /// F1c rule 2 and 3 (fix-round1.md): each sentence or walk on room objects and actors, per value of VAR_VERB_SCRIPT:
    /// a plain value keeps today's job with var:32 pinned; any other value n gets a job labelled [n] that needs the
    /// hover live, not frozen and at its wait, and its guards, and carries `SETTLED_PIN`. An input with an object in the
    /// inventory stays as it is.
    fn settled_jobs(&mut self, jobs: Vec<Job>) -> Vec<Job> {
        use crate::inputs::SETTLED_PIN;
        let Some(rule) = self.settled_rule() else {
            self.counts.inc("F1c: no plain input script or no hover (the switch changes nothing)");
            return jobs;
        };
        let key32 = format!("var:{VAR_VERB_SCRIPT}");
        let values: Vec<i64> = match self.domain(&key32) {
            Some(d) if self.kind(&key32) != Kind::Const => d.values.iter().filter_map(|v| v.int()).filter(|&v| v != 0).collect(),
            _ => vec![self.table.start.get0(&key32)],
        };
        let hover_key = self.game.lifecycle_key(rule.hover).map(|k| k.to_string());
        let hover = slot_name(hover_key.as_deref().unwrap_or(""));
        let mut base = (Pins::new(), Pre::new());
        let mut ok = true;
        if let Some(k) = &hover_key {
            ok = self.pin(&mut base.0, &mut base.1, k, &[Label::Int(1)]);
        }
        ok = ok && self.pin(&mut base.0, &mut base.1, &format!("frozen:{hover}"), &[Label::Int(0)]);
        ok = ok && self.pin(&mut base.0, &mut base.1, &format!("at:{hover}:{}", rule.wait), &[Label::Int(1)]);
        for g in &rule.guards {
            let vals: Labels = match self.domain(&g.key) {
                Some(d) => {
                    let value = |l: &Label| match *l {
                        Label::Int(n) => n,
                        Label::Above => d.high.unwrap() + 1,
                        Label::Below => d.low.unwrap() - 1,
                    };
                    d.values.iter().filter(|l| g.holds(value(l))).copied().collect()
                }
                None => vec![Label::Int(self.table.start.get0(&g.key))].into_iter().filter(|l| matches!(l, Label::Int(n) if g.holds(*n))).collect(),
            };
            ok = ok && !vals.is_empty() && self.pin(&mut base.0, &mut base.1, &g.key, &vals);
        }
        let mut out = Vec::with_capacity(jobs.len() * 2);
        for job in jobs {
            let What::Sentence { a, .. } = job.what else { unreachable!("today's sentence jobs") };
            if job.label.contains("[inv]") {
                out.push(job);
                continue;
            }
            for &n in &values {
                let mut j = job.clone();
                let n_label = Label::Int(n);
                if !self.pin(&mut j.pins, &mut j.pre, &key32, &[n_label]) {
                    continue;
                }
                if rule.plain.contains(&n) {
                    out.push(j);
                    continue;
                }
                if !ok {
                    self.counts.inc("F1c: inputs whose hover or guards never hold (constants)");
                    continue;
                }
                // a local input script: the job's rooms it can run in
                let mut rooms: Labels = self
                    .scope
                    .rooms
                    .iter()
                    .filter(|&&r| rule.runs_in(self.game, n, self.game.resource(r)))
                    .map(|&r| Label::Int(r as i64))
                    .collect();
                if a >= NUM_ACTORS {
                    let home = self.game.object_room.get(&(a as i32)).copied();
                    rooms.retain(|r| matches!(r, Label::Int(x) if self.game.resource(*x as i32) == home));
                }
                if rooms.is_empty() {
                    self.counts.inc("F1c: inputs whose input script no room of theirs installs");
                    continue;
                }
                if (n as i32) >= self.game.first_local && !self.pin(&mut j.pins, &mut j.pre, "room", &rooms) {
                    continue;
                }
                if j.pre.get("room").is_some_and(|s| s.is_empty()) {
                    self.counts.inc("F1c: inputs whose input script no room of theirs installs");
                    continue;
                }
                let mut fine = true;
                for (k, v) in &base.1 {
                    fine = fine && self.pin(&mut j.pins, &mut j.pre, k, v);
                }
                for (k, v) in &base.0 {
                    if !j.pre.contains_key(k) {
                        j.pins.insert(k.clone(), v.clone());
                    }
                }
                if !fine {
                    continue;
                }
                j.pins.insert(SETTLED_PIN.to_string(), Val::Int(n));
                j.label = match j.label.strip_suffix("~frozen") {
                    Some(l) => format!("{l}[{n}]~frozen"),
                    None => format!("{}[{n}]", j.label),
                };
                self.counts.inc("F1c: inputs that start at the input script");
                out.push(j);
            }
        }
        out
    }

    /// `Builder.inputs`: every player input of the scope, in reader.py's order.
    pub fn inputs(&mut self) -> Vec<Job> {
        let mut out = Vec::new();
        let ns = self.sentence_scripts();
        let objects: Vec<i64> = self.scope.objects.iter().chain(&self.scope.held).copied().collect();
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
        if self.switches.input_settled {
            out = self.settled_jobs(out);
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

    /// The unit a slot's name (facts.py's) names, in the room resource `res`.
    pub fn unit_of_name(name: &str, res: Option<i32>) -> Option<UnitKey> {
        if let Some(n) = name.strip_prefix("script:") {
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
        }
    }

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
            let unit = Self::unit_of_name(name, res);
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

    /// A kept slot's list of locals as its event keys it, and the list's number: what `record` makes of a slot a path
    /// left waiting, and what any other maker of a slot's event must use (fix round 1's F10, PORT-NOTES.md "Live
    /// locals"). Off: the list as it is, numbered by `sig` (reader.py's `_sig`), the recursive flag as it is. With the
    /// switch --live-locals: only the locals unit `unit`'s code reads, numbered with the slot's flags (resistant, owned,
    /// and recursive only as "true or not", `locals::recursive`). Returns (locals, number, recursive flag).
    #[allow(clippy::too_many_arguments)]
    pub fn list_number(
        &mut self,
        name: &str,
        unit: UnitKey,
        locals: Vec<(i64, Val)>,
        ov: i64,
        cd: &[CutData],
        known: bool,
        recursive: &Py,
        resistant: &Py,
        owned: &Py,
    ) -> (Vec<(i64, Val)>, i64, Py) {
        let (locals, flags, recursive) = match &self.table.live_locals {
            Some(ll) => {
                let (kept, dropped) = ll.keep(unit, locals);
                if dropped {
                    self.counts.inc("lists of locals with locals their unit never reads (left out: live locals)");
                }
                let recursive = crate::locals::recursive(recursive);
                let flags = vec![("recursive", recursive.text()), ("resistant", resistant.text()), ("owned", owned.text())];
                (kept, flags, recursive)
            }
            None => (locals, Vec::new(), recursive.clone()),
        };
        let sig = self.sig(name, &locals, ov, cd, known, &flags);
        (locals, sig, recursive)
    }

    /// F10: the events of the slots in `slot_locals`, with their dumped locals known (`args_known`), numbered as
    /// `record` numbers a list (`list_number`), and an Esc event where the slot's override is armed. A slot outside
    /// the scope gets none (counted). Off --live-locals the slot's flags are the start's (`start_events`: the R flag
    /// from the unit's starts, recursive unknown, owned from the object's owner), as the reading had them before the
    /// dump had flags. Under --live-locals a list's number covers the flags, so they are the dump's (`SlotFlags`), and
    /// a slot without them stops the reading: no number would be known for it.
    pub fn dumped_events(&mut self) -> Vec<Event> {
        let mut out = Vec::new();
        self.slot_events.clear();
        for s in self.slot_locals.clone() {
            let res = self.game.resource(s.room);
            let Some(unit) = Self::unit_of_name(&s.name, res).filter(|u| self.scope.has_unit(*u)) else {
                self.counts.inc("F10: dumped slots outside the scope (no events)");
                continue;
            };
            let cd: Vec<CutData> = s.cut_data.iter().map(|&c| CutData::Int(c)).collect();
            let (recursive, resistant, owned) = if self.table.live_locals.is_some() {
                let f = s.flags.unwrap_or_else(|| {
                    panic!(
                        "F10 under --live-locals: the dump gives slot {} no flags (resistant, recursive), and a list's \
                         number covers them; dump the slots with a build whose SPEEDRUN_DUMP_LOCALS lists them",
                        s.name
                    )
                });
                (Py::Bool(f.recursive), Py::Bool(f.resistant), Py::Bool(f.owned))
            } else {
                let res_flag = match self.game.lifecycle_key(unit) {
                    Some(f) => self.game.resistant(f, res),
                    None => Some(false),
                };
                (Py::None, Py::opt_bool(res_flag), Py::Bool(s.owned))
            };
            // numbered as `record` numbers a list (`list_number`)
            let (locals, sig, recursive) =
                self.list_number(&s.name, unit, s.list(), s.override_, &cd, true, &recursive, &resistant, &owned);
            // the domain of args:<unit> holds the list's number before its events are pinned (as `record` adds it)
            self.table.args_vals.entry(format!("args:{}", s.name)).or_default().insert(sig);
            let ev = Event {
                name: s.name.clone(),
                unit,
                w: s.w,
                sig,
                locals,
                args_known: true,
                recursive,
                resistant,
                owned,
                abort: false,
                override_: s.override_,
                cut_data: s.cut_data.clone(),
            };
            self.counts.inc("F10: dumped slots read from their locals");
            let esc = s.esc.map(|w| Event { w, abort: true, ..ev.clone() });
            self.slot_events.push(ev.clone());
            out.push(ev);
            out.extend(esc);
        }
        out
    }

    /// `Builder._sig`: the number of a list of locals (48 bits of its text's SHA-1; 0: not known). `flags`: more
    /// (name, text) pairs in the text, given exactly when the switch --live-locals is on (`list_number` gives them;
    /// a call without them under the switch would number a list as no record does, so it stops here).
    pub fn sig(&mut self, name: &str, locals: &[(i64, Val)], ov: i64, cd: &[CutData], args_known: bool, flags: &[(&str, String)]) -> i64 {
        assert_eq!(
            flags.is_empty(),
            self.table.live_locals.is_none(),
            "Builder::sig: a list's flags are in its number exactly when the switch --live-locals is on; number a slot's list with Builder::list_number"
        );
        if !args_known {
            return 0;
        }
        // a list numbered before: its number and text again (`sig` is a function of these inputs), found by their
        // hash and compared whole
        let h = {
            use std::hash::{Hash, Hasher};
            let mut h = rustc_hash::FxHasher::default();
            (name, locals, ov, cd, flags).hash(&mut h);
            h.finish()
        };
        let known = self.sig_cache.get(&h).and_then(|v| {
            v.iter().find(|e| e.name == name && e.locals == locals && e.ov == ov && e.cd == cd && e.flags.iter().map(|(a, b)| (a.as_str(), b.as_str())).eq(flags.iter().map(|(a, b)| (*a, b.as_str()))))
        });
        if let Some(e) = known {
            let set = self.sig_ids.entry(e.sig).or_default();
            if !set.contains(&e.text) {
                set.insert(e.text.clone());
            }
            return e.sig;
        }
        self.sig_made(name, locals, ov, cd, flags)
    }

    /// `sig`, made: the text, its hash, the number; kept in `sig_cache`.
    fn sig_made(&mut self, name: &str, locals: &[(i64, Val)], ov: i64, cd: &[CutData], flags: &[(&str, String)]) -> i64 {
        let mut pairs: Vec<(String, String)> = locals.iter().map(|(i, v)| (i.to_string(), v.show())).collect();
        pairs.push(("override".into(), ov.to_string()));
        pairs.push(("cut_data".into(), cut_tuple_str(cd)));
        pairs.extend(flags.iter().map(|(k, v)| (k.to_string(), v.clone())));
        pairs.sort();
        let inner: Vec<String> = pairs.iter().map(|(a, b)| repr_tuple(&[repr_str(a), repr_str(b)])).collect();
        let text = repr_tuple(&[repr_str(name), repr_tuple(&inner)]);
        let hex = sha1_smol::Sha1::from(text.as_bytes()).digest().to_string();
        let mut sig = i64::from_str_radix(&hex[..12], 16).unwrap();
        if sig == 0 {
            sig = 1;
        }
        let h = {
            use std::hash::{Hash, Hasher};
            let mut h = rustc_hash::FxHasher::default();
            (name, locals, ov, cd, flags).hash(&mut h);
            h.finish()
        };
        self.sig_cache.entry(h).or_default().push(SigMade {
            name: name.to_string(),
            locals: locals.to_vec(),
            ov,
            cd: cd.to_vec(),
            flags: flags.iter().map(|(a, b)| (a.to_string(), b.clone())).collect(),
            sig,
            text: text.clone(),
        });
        self.sig_ids.entry(sig).or_default().insert(text);
        sig
    }

    /// The slot keys of a unit the oracle lists (`SymOracle.keys(prefix)`), each handed to `f`: the scope's universe,
    /// then the last run's pins that the universe lacks, sorted. The universe's part is worked out once per prefix
    /// (the scope does not change).
    fn oracle_keys_each(&self, prefix: &str, f: &mut dyn FnMut(&String)) {
        let mut cache = self.uni_keys.borrow_mut();
        let (uni, by_prefix) = &mut *cache;
        if uni.is_empty() {
            uni.extend(self.scope.universe.iter().cloned());
        }
        if !by_prefix.contains_key(prefix) {
            by_prefix.insert(prefix.to_string(), self.scope.universe.iter().filter(|k| k.starts_with(prefix)).cloned().collect());
        }
        for k in &by_prefix[prefix] {
            f(k);
        }
        for k in self.last_pins.keys().filter(|k| k.starts_with(prefix) && !uni.contains(*k)) {
            f(k);
        }
    }

    /// `Builder.event_pins`.
    pub fn event_pins(&self, ev: &Event) -> (bool, Pins, Pre) {
        let (mut pins, mut pre) = (Pins::new(), Pre::new());
        let mut ok = true;
        if let Some(key) = self.game.lifecycle_key(ev.unit) {
            ok = self.pin(&mut pins, &mut pre, &key.to_string(), &[Label::Int(1)]);
        }
        if ev.abort {
            let exit_key = format!("var:{VAR_CUTSCENEEXIT_KEY}");
            ok = ok && self.pin(&mut pins, &mut pre, &format!("esc:{}:{}", ev.name, ev.w), &[Label::Int(1)]);
            let dom = self.domain(&exit_key);
            if self.kind(&exit_key) == Kind::Const {
                ok = ok && self.table.start.get0(&exit_key) != 0;
            } else if let Some(dom) = dom {
                let vals: Labels = dom.values.iter().copied().filter(|v| *v != Label::Int(0)).collect();
                ok = ok && self.pin(&mut pins, &mut pre, &exit_key, &vals);
            }
        } else {
            let prefix = format!("at:{}:", ev.name);
            let own = format!("{prefix}{}", ev.w);
            ok = ok && self.pin(&mut pins, &mut pre, &own, &[Label::Int(1)]);
            ok = ok && self.pin(&mut pins, &mut pre, &format!("frozen:{}", ev.name), &[Label::Int(0)]);
            self.oracle_keys_each(&prefix, &mut |k: &String| {
                if *k != own {
                    pins.insert(k.clone(), Val::Int(0));
                }
            });
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
        // the switch --reach: a job's guides are found by the whole job (reach.rs `job_key`), not by its label, which
        // rooms share
        // (the key is made only when some rerun has guides this batch: with none, no key finds any)
        self.cur_job_key = self.table.reach.as_ref().filter(|r| r.has_guides()).map(|_| Arc::from(crate::reach::job_key(job)));
        if self.args_log.is_some() {
            if let Some(memo) = self.table.memo.clone() {
                return self.run_job_memo(&memo, m, job);
            }
            if let Some(cm) = self.table.cross.clone() {
                return self.run_job_cross(&cm, m, job);
            }
        }
        self.run_job_real(m, job)
    }

    pub(crate) fn run_job_real(&mut self, m: &mut dyn Machine, job: &Job) -> Vec<Record> {
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
        let t_run = std::time::Instant::now();
        let mut run = m.run(&self.ctx(), &job.label, pins, pre, &job.what);
        crate::par::trace_run(&job.label, t_run.elapsed().as_secs_f64(), &run);
        self.last_pins = pins.clone();
        self.book_run(&run);
        let facts = self.rd.counts.get(FORKS_ON_READ) - facts0;
        let nonfacts = self.rd.counts.get(FORKS_ON_NONFACTS) - nonfacts0;
        if run.limit != 0 && self.table.split_limit && read_widen_switch() {
            // E014 (FMI_READ_WIDEN): the run again with its loops widened; ended under the limit, it is the run
            let t_w = std::time::Instant::now();
            m.set_widen(true);
            let w = m.run(&self.ctx(), &job.label, pins, pre, &job.what);
            m.set_widen(false);
            crate::par::trace_widen(&job.label, t_w.elapsed().as_secs_f64(), run.limit, &w);
            self.book_run(&w);
            if w.limit == 0 {
                self.counts.inc("FMI_READ_WIDEN: runs past the path limit read whole with their loops widened");
                run = w;
            } else {
                self.counts.inc("FMI_READ_WIDEN: runs past the path limit still past it with their loops widened (split as before)");
            }
        }
        if run.limit == 0 {
            self.reach_out.extend(run.held);
        }
        if self.table.split_limit {
            if run.limit != 0 && !(read_cut_switch() && !self.splits(&run.first_fork, pre)) {
                return self.run_split(m, job, pins, pre, budget, run.first_fork.clone(), run.limit);
            }
            if run.limit != 0 {
                // E014 (FMI_READ_CUT): no fact left to split on and no larger limit to read it with: the run is read
                // short (mode 1: no record; mode `paths`: its finished paths make its records, and the paths the limit
                // ended are dropped, DROP_ENDS), counted; the reading goes on
                self.counts.inc("FMI_READ_CUT: runs past the path limit with no fact left to split on, read short");
            }
            // scumm-machine's step limit ends a path "limit" too; in a run the path limit cut nothing (limit 0)
            // every such end is the step limit's, and no split lifts it
            let n = run.paths.iter().filter(|p| p.end.as_deref() == Some("limit")).count();
            if n != 0 && !self.table.havoc_cut && !read_cut_switch() {
                panic!("split_limit: {} has {n} paths the step limit ended; the reading stops", job.label);
            }
            if n != 0 && !self.table.havoc_cut {
                // E014 (FMI_READ_CUT=1): the paths the step limit ended are dropped, counted
                self.counts.add("FMI_READ_CUT: paths the step limit ended, dropped", n as i64);
            }
        } else if run.limit != 0 && run.first_fork.is_some() && facts < nonfacts {
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
            // the switch havoc_cut: a path the step limit ended (machine.rs hands it over with where it stood)
            let havoc = self.table.havoc_cut && p.cut.is_some();
            if !havoc && (p.end.as_deref().is_some_and(dropped_end) || !p.sym) {
                let base = job.label.split('#').next().unwrap().split('@').next().unwrap();
                let lab = format!("{base}{}", if job.label.contains('@') { ")" } else { "" });
                self.drops.add(Py::Tuple(vec![Py::str(lab), end_py(&p.end)]), 1);
                continue;
            }
            // --reach: the path's precondition as its record will have it, made once (`reach_path`, `record_with`)
            let mut placed: Option<Pre> = None;
            if self.table.reach.is_some() {
                match self.reach_path(job, p) {
                    Some(pre) => placed = Some(pre),
                    None => continue,
                }
            }
            let rec = if havoc {
                Some(self.havoc_record(&job.kind, &job.label, p, &job.what))
            } else {
                self.record_with(&job.kind, &job.label, p, &job.what, placed)
            };
            let Some(mut rec) = rec else {
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
            if self.table.reach.is_some() && !self.reach_record(job, p, &rec) {
                continue;
            }
            out.push(rec);
        }
        out
    }

    /// A run's counters and, for the job memo, what it touched and asked (`run`; FMI_READ_WIDEN books both runs).
    fn book_run(&mut self, run: &RunOut) {
        self.rd.update(&run.rd);
        self.counts.add("value sets past model.MAX_VALUES (made plain unknowns)", run.value_caps);
        if let Some(l) = &mut self.memo_log {
            l.touched.extend(run.touched.iter().cloned());
            l.queries.extend(run.queries.iter().cloned());
            for p in &run.paths {
                l.touched.extend(p.pre.keys().cloned());
                l.touched.extend(p.writes.keys().cloned());
            }
        }
    }

    /// E014 (FMI_READ_CUT): whether `run_split` can lift a run past the path limit: its first fork on a read has two
    /// or more values left, or grow_limit can read it whole with a larger limit.
    fn splits(&self, first_fork: &Option<String>, pre: &Pre) -> bool {
        let n = match first_fork {
            Some(key) => match (self.domain(key), pre.get(key)) {
                (Some(dom), Some(s)) => dom.values.iter().filter(|v| s.contains(v)).count(),
                (Some(dom), None) => dom.values.len(),
                (None, _) => 0,
            },
            None => 0,
        };
        n >= 2 || (self.table.grow_limit && self.path_limit_now.max(crate::machine::PATH_LIMIT) < GROW_MAX)
    }

    /// The switch split_limit: a run past the path limit is run again once per value of its first fork on a read
    /// (the values its precondition leaves), that fact pinned to the value, and each part that passes the limit is
    /// split again, with no cap on depth or parts and whatever its forks on non-facts. In any state the engine runs
    /// one path of the run, and the parts' preconditions divide the run's states by the pinned fact's values: they
    /// do not overlap and together cover them. So the parts' records cover every state the run can start in, and
    /// no path the limit ended is dropped. A part that passes the limit with no fact left to split on stops the
    /// reading (a panic naming the job).
    #[allow(clippy::too_many_arguments)]
    fn run_split(
        &mut self,
        m: &mut dyn Machine,
        job: &Job,
        pins: &Pins,
        pre: &Pre,
        budget: &mut (i64, i64, BTreeSet<String>),
        first_fork: Option<String>,
        limit: i64,
    ) -> Vec<Record> {
        let vals: Labels = match &first_fork {
            Some(key) => {
                let dom = self.domain(key).unwrap_or_else(|| panic!("split_limit: {}'s first fork {key} has no domain", job.label));
                match pre.get(key) {
                    Some(s) => dom.values.iter().copied().filter(|v| s.contains(v)).collect(),
                    None => dom.values.clone(),
                }
            }
            None => Labels::new(),
        };
        if vals.len() < 2 && self.table.grow_limit && self.path_limit_now.max(crate::machine::PATH_LIMIT) < GROW_MAX {
            // grow_limit: the run read whole with a larger limit. The limit only ends paths, so a run it cuts
            // nothing of is the whole run, and its records cover every state the run can start in. The parts of a
            // split under the larger limit run here, in order, so the reading does not depend on the workers.
            let prev = self.path_limit_now;
            let next = prev.max(crate::machine::PATH_LIMIT) * 4;
            self.counts.inc(format!("grow_limit: runs read again with the path limit at {next}"));
            crate::par::trace_grow(&job.label, budget.1 as usize, next, limit);
            self.path_limit_now = next;
            m.set_path_limit(next);
            let out = self.run(m, job, pins, pre, budget);
            self.path_limit_now = prev;
            m.set_path_limit(prev);
            return out;
        }
        if vals.len() < 2 {
            panic!(
                "split_limit: {} passed the path limit ({limit} paths cut) with no fact left to split on (first fork {first_fork:?}, {} splits deep); the reading stops",
                job.label, budget.1
            );
        }
        let key = first_fork.unwrap();
        let n_dom = vals.len();
        let vals = if self.table.reach.is_some() { self.reach_parts(job, pre, &key, vals) } else { vals };
        crate::par::trace_split(&job.label, budget.1 as usize, &key, n_dom, vals.len(), limit);
        budget.1 += 1;
        self.counts.inc("runs past the path limit split on their first fork on a read");
        self.counts.inc(format!("split_limit: runs split at depth {}", budget.1));
        self.counts.add("split_limit: parts run", vals.len() as i64);
        let parts: Vec<(Pins, Pre)> = vals
            .iter()
            .map(|&v| {
                let (mut p2, mut pre2) = (pins.clone(), pre.clone());
                pre2.insert(key.clone(), vec![v]);
                p2.insert(key.clone(), self.table.value(&key, v));
                (p2, pre2)
            })
            .collect();
        if self.path_limit_now == 0 {
            m.prefetch(&self.ctx(), &job.label, &parts, &job.what);
        }
        let mut out = Vec::new();
        for (p2, pre2) in &parts {
            out.extend(self.run(m, job, p2, pre2, budget));
        }
        budget.1 -= 1;
        out
    }

    /// `Builder.record`: the record of one path, or None when it changes nothing.
    pub fn record(&mut self, kind: &str, label: &str, p: &PathOut, what: &What) -> Option<Record> {
        self.record_with(kind, label, p, what, None)
    }

    /// `record`, with the path's precondition already made and placed (`positions`) when given (`reach_path` makes
    /// it under --reach: the same keys and sets as here, and `positions` leaves a placed precondition as it is).
    pub(crate) fn record_with(&mut self, kind: &str, label: &str, p: &PathOut, what: &What, placed: Option<Pre>) -> Option<Record> {
        let pre = match placed {
            Some(pre) => pre,
            None => {
                let mut pre = Pre::new();
                for (k, s) in &p.pre {
                    if let Some(dom) = self.table.domain(k) {
                        if *s != dom.values || (k.starts_with("args:") && self.table.reach.is_some()) {
                            pre.insert(k.clone(), s.clone());
                        }
                    }
                }
                pre
            }
        };
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
            // the switch --live-locals (PORT-NOTES.md, "Live locals"): only the locals the unit's code reads, and the
            // slot's flags in the list's number (lists that differed in unread locals may differ there), its recursive
            // flag only as "true or not", all the model and the engine read of it
            let cd: Vec<CutData> = kept.cut_data.iter().map(cut_text).collect();
            let (locs, mut sig, recursive) =
                self.list_number(name, kept.unit, locs, kept.override_, &cd, kept.known, &kept.recursive, &kept.resistant, &kept.owned);
            if let Some(l) = &mut self.memo_log {
                l.sigs.push((sig, self.folded.contains(&sig)));
            }
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
                recursive,
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
            let which = if self.unknown_targets_listed(&head, &cite) { "with a candidate list" } else { "no candidate list" };
            self.counts.inc(format!("unknown-target writes left out and listed on the record ({which}): {head}"));
        }
        let effects: Vec<(String, Eff)> = effects
            .into_iter()
            .filter(|(k, e)| !matches!(e, Eff::Set(v) if pre.get(k).is_some_and(|s| s.len() == 1 && s[0] == *v)))
            .collect();
        if effects.is_empty() {
            return None;
        }
        let mut sides: FxHashMap<(&str, &str), BTreeSet<std::borrow::Cow<str>>> = FxHashMap::default();
        for a in &p.assumptions {
            sides.entry((&a.cite, &a.cond)).or_default().insert(a.side_text());
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

    /// Whether `unknown_targets` gives a list (kept by the pair's hash, the list not copied).
    pub fn unknown_targets_listed(&mut self, head: &str, cite: &str) -> bool {
        use std::hash::{Hash, Hasher};
        let mut h = rustc_hash::FxHasher::default();
        (head, cite).hash(&mut h);
        let h = h.finish();
        if let Some(e) = self.unknown_listed.get(&h).and_then(|v| v.iter().find(|e| e.0 == head && e.1 == cite)) {
            return e.2;
        }
        let listed = self.unknown_targets(head, cite).is_some();
        self.unknown_listed.entry(h).or_default().push((head.to_string(), cite.to_string(), listed));
        listed
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
    /// A written place is not checked against pos:'s domain (reader.py's own behaviour, a finding: PORT-NOTES.md),
    /// but listed in `outside_points`. With the switch on (`table.elsewhere`) such a place is written as ELSEWHERE,
    /// and no precondition set holds ELSEWHERE (PORT-NOTES.md, "ELSEWHERE").
    pub fn positions(&mut self, mut pre: Pre, mut effects: Vec<(String, Eff)>) -> (Pre, Vec<(String, Eff)>) {
        for (kind, pos, prefix) in [("at", "pos", "at:"), ("esc", "escpos", "esc:")] {
            // the unit's writes apart, the others in their order
            let (mine, kept): (Vec<(String, Eff)>, Vec<(String, Eff)>) = effects.into_iter().partition(|(k, _)| k.starts_with(prefix));
            effects = kept;
            if !mine.is_empty() {
                let mut ones: BTreeMap<&str, Vec<i64>> = BTreeMap::new();
                let mut zeros: BTreeSet<&str> = BTreeSet::new();
                for (k, e) in &mine {
                    let (name, w) = k[prefix.len()..].rsplit_once(':').unwrap();
                    if w == "*" {
                        zeros.insert(name);
                    } else if *e == Eff::Set(Label::Int(1)) {
                        ones.entry(name).or_default().push(w.parse().unwrap());
                    } else if *e == Eff::Set(Label::Int(0)) {
                        zeros.insert(name);
                    } else {
                        self.counts.inc(format!("{kind}: writes not 0 or 1 (left out)"));
                    }
                }
                let names: BTreeSet<&str> = ones.keys().copied().chain(zeros.iter().copied()).collect();
                for name in names {
                    let key = format!("{pos}:{name}");
                    for &w in ones.get(name).into_iter().flatten() {
                        if !self.table.is_point(kind, name, w) {
                            *self.outside_points.entry(key.clone()).or_default().entry(w).or_default() += 1;
                            if self.table.elsewhere {
                                self.counts.inc(format!("{kind}: a place none of the scope's points written as ELSEWHERE"));
                            }
                        }
                    }
                    let e = match ones.get(name) {
                        Some(ws) if ws.len() > 1 => {
                            self.counts.inc(format!("{kind}: more than one place written for one unit (a guess)"));
                            let labs = lab_set(ws.iter().map(|&w| Label::Int(self.table.place(kind, name, w))));
                            if labs.len() == 1 {
                                Eff::Set(labs[0])
                            } else {
                                Eff::Guess(labs)
                            }
                        }
                        Some(ws) => Eff::Set(Label::Int(self.table.place(kind, name, ws[0]))),
                        None => Eff::Set(Label::Int(crate::table::NOWHERE)),
                    };
                    match effects.iter_mut().find(|x| x.0 == key) {
                        Some(x) => x.1 = e,
                        None => effects.push((key, e)),
                    }
                }
            }
            // the precondition's keys of the prefix, which sort together
            let keys: Vec<String> = pre
                .range::<str, _>((std::ops::Bound::Included(prefix), std::ops::Bound::Unbounded))
                .take_while(|(k, _)| k.starts_with(prefix))
                .map(|(k, _)| k.clone())
                .collect();
            if keys.is_empty() {
                continue;
            }
            let mut need: BTreeMap<&str, Vec<(i64, Labels)>> = BTreeMap::new();
            for k in &keys {
                let s = pre.remove(k).unwrap();
                let (name, w) = k[prefix.len()..].rsplit_once(':').unwrap();
                let e = need.entry(name).or_default();
                let w: i64 = w.parse().unwrap();
                match e.iter_mut().find(|x| x.0 == w) {
                    Some(x) => x.1 = s,
                    None => e.push((w, s)),
                }
            }
            for (name, ws) in need {
                let key = format!("{pos}:{name}");
                // the domain's values, sorted and distinct: kept so as each test leaves them
                let mut ok: Labels = self.table.domain(&key).unwrap().values.clone();
                for (w, s) in ws {
                    if s == [Label::Int(1)] {
                        ok.retain(|x| *x == Label::Int(w));
                    } else if s == [Label::Int(0)] {
                        ok.retain(|x| *x != Label::Int(w));
                    }
                }
                if self.table.elsewhere {
                    ok.retain(|x| *x != Label::Int(crate::table::ELSEWHERE));
                }
                pre.insert(key, ok);
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
                if let Some(l) = &mut self.memo_log {
                    // its kind (and a constant's start) is read here (memo.md, item 4)
                    l.touched.insert(src.clone());
                }
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
        self.table.split_limit = self.switches.split_limit;
        self.table.grow_limit = self.switches.grow_limit;
        assert!(!self.table.grow_limit || self.table.split_limit, "grow_limit needs split_limit");
        self.table.havoc_cut = self.switches.havoc_cut;
        assert!(!self.table.havoc_cut || self.table.split_limit, "havoc_cut needs split_limit");
        self.table.step_limit = self.switches.step_limit;
        assert!(self.table.step_limit == 0 || self.table.havoc_cut, "step_limit needs havoc_cut");
        self.table.havoc_starts = self.switches.havoc_starts;
        assert!(!self.table.havoc_starts || self.table.havoc_cut, "havoc_starts needs havoc_cut");
        self.table.havoc_rooms = self.switches.havoc_rooms;
        assert!(!self.table.havoc_rooms || self.table.havoc_cut, "havoc_rooms needs havoc_cut");
        assert!(!self.switches.havoc_args || self.table.havoc_cut, "havoc_args needs havoc_cut");
        self.havoc_args_analysis();
        assert!(!self.switches.reach_pairs || self.switches.reach, "reach_pairs needs --reach");
        self.table.wait_both = self.switches.wait_both;
        if self.switches.reach {
            assert!(self.switches.split_limit, "--reach needs --split-limit: the paths the limit ends depend on what else runs");
            assert!(!self.switches.due_first, "--reach reads no due: facts");
            self.table.reach = Some(Arc::new(crate::reach::Reached::default()));
            if self.reach_starts.is_empty() {
                self.reach_starts.push(self.scope.start.clone());
            }
        }
        if self.switches.input_settled {
            self.settled_rule();
        }
        if self.switches.pass2_reuse {
            self.table.memo = Some(Arc::new(crate::memo::RunMemo::default()));
        }
        if self.switches.shared_runs && self.table.shared.is_none() {
            self.table.shared = Some(Arc::new(crate::shared::SharedRuns::default()));
        }
        if self.memo_dir.is_some() || self.memo_mem.is_some() {
            let root = self.memo_dir.clone();
            self.open_memo(root.as_deref());
        }
        for n in 0..passes {
            if n > 0 {
                if let Some(s) = &mut sink {
                    s.reset();
                }
            }
            self.read_once(m, start_events, skip, &mut sink);
            if let Some(cm) = &self.table.cross {
                // the runs this pass stored join the index: the next pass may take them (memo.md)
                cm.index_own();
            }
            self.counts.set(format!("pass {}: records", n + 1), self.n_records as i64);
            if self.out_of_domain.is_empty() || n == passes - 1 {
                break;
            }
            self.widened.push(self.out_of_domain.clone());
            let added = self.extend_domains();
            if let Some(memo) = &self.table.memo {
                memo.start_reuse(self.widened.last().unwrap().keys().cloned());
            }
            self.counts.set(format!("pass {}: domain values added for the next pass", n + 1), added);
            self.reset();
            // havoc_args reads the variables by their domains, which the next pass widened
            self.havoc_args_analysis();
        }
        self.counts.set("records", self.n_records as i64);
        if let Some(memo) = &self.table.memo {
            memo.clear();
        }
        if let Some(cm) = &self.table.cross {
            let n = cm.finish().unwrap_or_else(|e| panic!("--memo: the store file: {e}"));
            eprintln!("[memo] {} runs stored in {}", n, cm.dir.display());
        }
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
        self.outside_points.clear();
        self.table.args_vals.clear();
        self.args_doms.borrow_mut().clear();
        self.sig_seen.clear();
        self.folded.clear();
        self.ego_cache = None;
        self.fact_info.clear();
        self.memo_infos.clear();
        self.pin_kinds.borrow_mut().clear();
        self.n_records = 0;
        self.args_set.clear();
    }

    /// The switch havoc_args: the analysis of the game's code (args.rs) with this pass's domains, and its counts; with
    /// the switch off, none.
    pub fn havoc_args_analysis(&mut self) {
        if !self.switches.havoc_args {
            self.table.havoc_args = None;
            return;
        }
        let t = std::time::Instant::now();
        let a = crate::args::HavocArgs::of(self.game, &self.scope, &self.table);
        self.counts.set("havoc_args: units no launch reaches", a.unreached as i64);
        self.counts.set("havoc_args: starts whose values are not known (they may start any unit)", a.wild.len() as i64);
        self.counts.set("havoc_args: rounds of the analysis", a.rounds as i64);
        self.counts.set("havoc_args: analysis ms", t.elapsed().as_millis() as i64);
        self.table.havoc_args = Some(Arc::new(a));
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
        self.pin_kinds.borrow_mut().clear();
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
        if self.table.reach.is_some() {
            self.read_once_reach(m, start_events, skip, sink);
            self.end_of_pass();
            return;
        }
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
            b.next_events_of(&recs, &mut nexts);
            b.emit(recs, sink);
        });
        let jobs_len = n_jobs;
        let n = self.n_records;
        *self.phase_secs.entry("runs").or_default() += tr.elapsed().as_secs_f64();
        self.tell("inputs", jobs_len, n, t0.elapsed().as_secs_f64());
        let mut todo: Vec<Event> = if start_events { self.start_events() } else { vec![] };
        if start_events && !self.slot_locals.is_empty() {
            let dumped = self.dumped_events();
            todo.extend(dumped);
        }
        todo.extend(nexts.events);
        let mut rounds = 0;
        while !todo.is_empty() {
            rounds += 1;
            let jobs = self.event_jobs(&mut todo, skip);
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
                b.next_events_of(&recs, &mut nexts);
                b.emit(recs, sink);
            });
            todo = nexts.events;
            *self.phase_secs.entry("runs").or_default() += t0.elapsed().as_secs_f64();
            let n = self.n_records - n0;
            self.tell(&what, n_jobs, n, t0.elapsed().as_secs_f64());
        }
        self.counts.set("event rounds", rounds);
        self.end_of_pass();
    }

    /// The jobs of one round's events: each event's copy with unknown locals, its list past SIG_LIMIT folded, those
    /// not yet run, and their pins. `todo` gains the copies.
    pub(crate) fn event_jobs(&mut self, todo: &mut Vec<Event>, skip: Option<&dyn Fn(&Event) -> bool>) -> Vec<Job> {
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
        for ev0 in todo.iter() {
            // `limit`, the event made only when it is not done already
            let folds = self.limit_folds(ev0);
            if folds {
                self.reach_folded(&ev0.name);
            }
            let k = (ev0.name.clone(), ev0.unit, ev0.w, if folds { 0 } else { ev0.sig }, ev0.abort);
            if self.events_done.contains(&k) {
                continue;
            }
            self.events_done.insert(k);
            let ev = if folds { ev0.unknown_locals(false) } else { ev0.clone() };
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
        jobs
    }

    /// `_read_once`'s last loop: the folded lists' numbers rewritten, the `args:` values, the collisions.
    fn end_of_pass(&mut self) {
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
    pub(crate) fn emit(&mut self, recs: Vec<Record>, sink: &mut Option<&mut dyn RecordSink>) {
        if let Some(lite) = self.job_prep.as_mut().and_then(|p| p.lite.take()) {
            self.emit_lite(lite, sink);
            return;
        }
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
                match self.job_prep.as_mut().and_then(|p| p.sink.take()) {
                    Some(prep) => s.batch_prepared(first, recs, &self.table, prep),
                    None => s.batch_owned(first, recs, &self.table),
                }
            }
        }
        *self.phase_secs.entry("emit").or_default() += t.elapsed().as_secs_f64();
    }

    /// `emit` of a job whose records stayed with the worker (`Lite`): the same count, `args:` values and sink.
    fn emit_lite(&mut self, lite: Lite, sink: &mut Option<&mut dyn RecordSink>) {
        if lite.n == 0 {
            return;
        }
        assert!(!self.keep_records, "--prep: the records stay with the worker only when none are kept");
        let t = std::time::Instant::now();
        let first = self.n_records;
        self.n_records += lite.n;
        for (k, s) in lite.args {
            self.args_set.entry(k).or_default().insert(s);
        }
        if let Some(s) = sink {
            let prep = self.job_prep.as_mut().and_then(|p| p.sink.take()).expect("--prep: the sink's part with the records' count");
            s.batch_prepared(first, Vec::new(), &self.table, prep);
        }
        *self.phase_secs.entry("emit").or_default() += t.elapsed().as_secs_f64();
    }

    /// `NextEvents::add` of a job's records, or of what the worker took of them (`Lite`).
    pub(crate) fn next_events_of(&mut self, recs: &[Record], nexts: &mut NextEvents) {
        match self.job_prep.as_mut().and_then(|p| p.lite.as_mut()) {
            Some(l) => nexts.add_events(std::mem::take(&mut l.next_events)),
            None => nexts.add(recs),
        }
    }

    /// The progress callback (reader.py `Builder.progress`): what ran, its jobs, its records, its seconds.
    pub(crate) fn tell(&mut self, what: &str, jobs: usize, records: usize, secs: f64) {
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

    /// `Builder._limit`: past SIG_LIMIT a wait point's further lists of locals are folded into 0. With the switch
    /// --live-locals the limit counts lists apart from the slot's flags ([`flag_free`]): a list's flag variants take one
    /// place, so the switch folds a wait point's lists only where their lists of all locals pass SIG_LIMIT too
    /// (PORT-NOTES.md, "Live locals", point 6).
    pub fn limit(&mut self, ev: &Event) -> Event {
        if self.limit_folds(ev) {
            ev.unknown_locals(false)
        } else {
            ev.clone()
        }
    }

    /// `limit`'s decision, with its side effects: true when the event's list is folded (`limit` gives
    /// `unknown_locals(false)`), false when it stays as it is.
    fn limit_folds(&mut self, ev: &Event) -> bool {
        if ev.sig == 0 {
            return false;
        }
        let key = if self.table.live_locals.is_some() { flag_free(ev) } else { ev.sig };
        let seen = match self.sig_seen.get_mut(&(ev.name.clone(), ev.w)) {
            Some(s) => s,
            None => self.sig_seen.entry((ev.name.clone(), ev.w)).or_default(),
        };
        if seen.contains(&key) {
            return false;
        }
        if !self.folded.contains(&ev.sig) && seen.len() < self.sig_limit {
            seen.push(key);
            return false;
        }
        if !self.folded.contains(&ev.sig) {
            self.folded.insert(ev.sig);
            self.counts.inc("lists of locals past SIG_LIMIT (taken as unknown)");
        }
        true
    }

    /// `Builder._run_jobs`: the event jobs' lists of locals registered first, the records' after. Each job's
    /// records go to `each` in the jobs' order, as made: here, one job after another, or where the machine runs
    /// the batch (`Machine::run_batch`, par.rs).
    pub fn run_jobs_each(&mut self, m: &mut dyn Machine, jobs: &[Job], each: &mut dyn FnMut(&mut Builder<'_>, Vec<Record>)) {
        self.run_jobs_shared(m, Arc::new(jobs.to_vec()), each)
    }

    /// `run_jobs_each` on jobs a machine can share with its threads.
    pub fn run_jobs_shared(&mut self, m: &mut dyn Machine, jobs: Arc<Vec<Job>>, each: &mut dyn FnMut(&mut Builder<'_>, Vec<Record>)) {
        if let Some(sh) = &self.table.shared {
            // the switch shared_runs: a run is shared within its batch only
            sh.next_batch();
        }
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
                match b.job_prep.as_ref().and_then(|p| p.lite.as_ref()) {
                    // the switch --prep: the records stayed with the worker, which took their `args:` sets
                    Some(l) => args.extend(l.args.iter().cloned()),
                    None => {
                        for r in &recs {
                            for (k, e) in &r.effects {
                                if let (true, Eff::Set(Label::Int(s))) = (k.starts_with("args:"), e) {
                                    args.push((k.clone(), *s));
                                }
                            }
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
        if let Some(fc) = &self.frame_costs {
            // the switch --frame-costs: its parameters (costs.rs); absent with the switch off
            put("frame_costs", fc.py());
        }
        if let Some(ll) = &self.table.live_locals {
            // the switch --live-locals: every unit's read locals (None: all), keyed by its slot's name in the engine's
            // state (`local:N@R`, `entry@R`, `exit@R` for room R's code); absent with the switch off
            put("live_locals", Py::dict(vec![("switch", Py::str("--live-locals")), ("slots", ll.slots_py(self.game))]));
        }
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


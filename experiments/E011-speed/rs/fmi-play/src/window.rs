//! window.py's strict path (window-search.md sections 1-9) and E010's play when ready: `Search::play_plan` plays one
//! plan's windows; `window_strict` steps the parent of one window and forks its children; `child_strict` checks one
//! child with B1's strict matcher.
//!
//! **The children's order.** The reference runs its children on three threads and lets each read the window's best
//! and first-event table as they stand when it asks, so its records depend on thread timing (three recorded runs of
//! E009's step test (a) differ in pruned children, duplicates and the parent's last frame). Here every such read is
//! answered as if the children had run one at a time in fork order, while they run in parallel: a child asks whether
//! an earlier child's run is the same (Lemma 5) once every earlier child's first event is known; it is pruned at a
//! frame when an earlier child ended (success, goal, deviation) at a Q no later than its own, waiting only while an
//! earlier child still running could end there; the parent stops at a frame when the best of the children forked
//! before it gives section 9.3's stop, waiting only while one of them could still change that. The window's choice is
//! the reference's (the least end key over every end point; pruning never removes a better end point), and every
//! record is one the reference's semantics gives under that order.

use crate::code::{Code, RunR};
use crate::executor::{self, Br, Executor, FrameObs, PlayErr};
use crate::pyj::{self, cut, Py, J};
use crate::reading::{ArgsView, FastView, PlayReading, ReadingData};
use crate::recs::*;
use crate::replay::{self, InputStep, StepLog};
use crate::strict::{self, BranchLog, Fail, InMatch, LogDec, Plan, Track, Verdict};
use crate::val::*;
use fmi_core::*;
use rustc_hash::{FxHashMap, FxHashSet};
use std::collections::{BTreeMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Instant;

pub const WINDOW_FRAMES: u64 = 3600;
pub const CHILD_LIMIT: u64 = 3600;
pub const DELTA_CAP: i64 = 15;
pub const MEM_FLOOR_KB: u64 = 3 * 1024 * 1024;
pub const CHILD_ROWS: usize = 4000;
pub const LEMMA9: &str = "another path of the click, no change a later step reads";
pub const END_POINTS: [&str; 3] = ["success", "goal", "deviation"];

/// window.WindowConfig as E009 runs it (and E010's ready). Any other combination is refused by name.
#[derive(Clone, Debug)]
pub struct Config {
    pub workers: usize,
    pub delta_lo: i64,
    pub window_frames: u64,
    pub child_limit: u64,
    pub mem_floor_kb: u64,
    pub strict: bool,
    pub repeat_stop: u32,
    pub cache: bool,
    pub fork_each: u32,
    pub lite: bool,
    pub split_wait: bool,
    pub ready: bool,
    pub ready_tries: u32,
    /// the harness's approach stones (step.py: stone_fork = Controller.fork)
    pub stones: bool,
    pub unobserved_actor_rooms: bool,
}

impl Config {
    /// step.py's E009 wiring: WindowConfig(workers=3, delta_lo, strict, cache, fork_each=7, lite, split_wait).
    pub fn e009(delta_lo: i64) -> Config {
        Config {
            workers: 3,
            delta_lo,
            window_frames: WINDOW_FRAMES,
            child_limit: CHILD_LIMIT,
            mem_floor_kb: MEM_FLOOR_KB,
            strict: true,
            repeat_stop: 2,
            cache: true,
            fork_each: 7,
            lite: true,
            split_wait: true,
            ready: false,
            ready_tries: 294,
            stones: true,
            unobserved_actor_rooms: true,
        }
    }
    /// E010's play when ready on top of E009's wiring.
    pub fn e010_ready(delta_lo: i64) -> Config {
        Config {
            ready: true,
            ..Config::e009(delta_lo)
        }
    }
    /// Only E009's and E010's settings are ported: anything else is refused, by name.
    pub fn check(&self) -> Result<(), String> {
        let mut bad = Vec::new();
        if !self.strict {
            bad.push("strict off (the window search without B1's matcher)");
        }
        if !self.cache {
            bad.push("cache off");
        }
        if !self.lite {
            bad.push("lite off");
        }
        if !self.split_wait {
            bad.push("split_wait off");
        }
        if self.fork_each != 7 && self.fork_each != 0 {
            bad.push("fork_each other than 7");
        }
        if !self.stones {
            bad.push("approach stones off");
        }
        if !self.unobserved_actor_rooms {
            bad.push("unobserved_actor_rooms off");
        }
        if bad.is_empty() {
            Ok(())
        } else {
            Err(format!(
                "fmi-play ports E009's and E010's window settings only; refused: {}",
                bad.join(", ")
            ))
        }
    }
    pub fn j(&self) -> J {
        crate::jobj! {
            "workers" => self.workers, "delta_lo" => self.delta_lo, "window_frames" => self.window_frames,
            "child_limit" => self.child_limit, "mem_floor_kb" => self.mem_floor_kb, "strict" => self.strict,
            "repeat_stop" => self.repeat_stop, "cache" => self.cache, "fork_each" => self.fork_each,
            "lite" => self.lite, "split_wait" => self.split_wait, "ready" => self.ready,
            "ready_tries" => self.ready_tries,
        }
    }
}

/// This process's CPU seconds (user and system, every thread).
pub fn process_cpu_s() -> f64 {
    let mut u: libc::rusage = unsafe { std::mem::zeroed() };
    unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut u) };
    let t = |v: libc::timeval| v.tv_sec as f64 + v.tv_usec as f64 / 1e6;
    t(u.ru_utime) + t(u.ru_stime)
}

/// FMI_CHECK_VIEW=1: every view read from a dump is checked against the view of its facts_of.
pub fn check_views() -> bool {
    static C: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *C.get_or_init(|| std::env::var("FMI_CHECK_VIEW").is_ok_and(|v| v == "1"))
}

/// window.delta_lo_of
pub fn delta_lo_of(dmax: Option<i64>) -> i64 {
    match dmax {
        Some(d) if (1..DELTA_CAP).contains(&d) => d,
        _ => 1,
    }
}

pub fn mem_available_kb() -> u64 {
    std::fs::read_to_string("/proc/meminfo")
        .ok()
        .and_then(|t| {
            t.lines()
                .find(|l| l.starts_with("MemAvailable:"))
                .and_then(|l| l.split_whitespace().nth(1).and_then(|x| x.parse().ok()))
        })
        .unwrap_or(1 << 40)
}

// --- a child -------------------------------------------------------------------------------------------------------

/// A click's outcome (Child.took): undecided ("n/a"), the plan's record (with or without a change), another of its
/// records (its name), or none.
#[derive(Clone, Debug, PartialEq)]
pub enum Took {
    NA,
    Plan,
    PlanNoChange,
    Alt(String),
    No,
}

impl Took {
    pub fn j(&self) -> J {
        match self {
            Took::NA => J::Str("n/a".into()),
            Took::Plan => J::Str("plan".into()),
            Took::PlanNoChange => J::Str("plan, no change".into()),
            Took::Alt(s) => J::Str(s.clone()),
            Took::No => J::Null,
        }
    }
}

/// window.Child
pub struct Child {
    pub g: u64,
    pub q0: u64,
    pub early: bool,
    pub done: Vec<usize>,
    pub at_fork: Arc<View>,
    pub full_at_fork: Option<View>,
    pub outcome: String,
    pub first: Option<u64>,
    pub issue: Option<u64>,
    pub end: Option<u64>,
    pub q_end: Option<u64>,
    pub frames: u64,
    pub why: Option<Py>,
    pub events: Vec<Event>,
    pub matched: Vec<(usize, (u64, usize))>,
    pub snapshot: Option<Box<dyn Engine>>,
    pub live: Option<Box<dyn Engine>>,
    pub checkpoints: Vec<Checkpoint>,
    pub arrival: Option<u64>,
    pub unplanned: Vec<J>,
    pub choices: Vec<(usize, Vec<(Key, VV)>)>,
    pub alt: Option<String>,
    pub dev_stop: Option<u64>,
    pub took: Took,
    pub track: Option<Track>,
    pub track_issue: Option<Track>,
    pub fail: Option<Fail>,
    pub choice_fail: Option<Fail>,
    pub d0: Option<Arc<Obs>>,
    pub outside: Vec<(Key, Option<i64>)>,
    /// E010: where the input code stopped when it issued no input
    pub reached: Option<u64>,
    pub cpu_s: f64,
    pub stone_trials: u32,
    pub split_runs: u64,
    pub split_child: bool,
    pub lemma9: u32,
    pub bound_violation: Option<J>,
    pub delta_violation: Option<J>,
}

impl Child {
    pub fn blank() -> Child {
        Child::new(0, 0, false, vec![], Arc::new(View::new()))
    }
    pub fn new(g: u64, q0: u64, early: bool, done: Vec<usize>, at_fork: Arc<View>) -> Child {
        Child {
            g,
            q0,
            early,
            done,
            at_fork,
            full_at_fork: None,
            outcome: String::new(),
            first: None,
            issue: None,
            end: None,
            q_end: None,
            frames: 0,
            why: None,
            events: vec![],
            matched: vec![],
            snapshot: None,
            live: None,
            checkpoints: vec![],
            arrival: None,
            unplanned: vec![],
            choices: vec![],
            alt: None,
            dev_stop: None,
            took: Took::NA,
            track: None,
            track_issue: None,
            fail: None,
            choice_fail: None,
            d0: None,
            outside: vec![],
            reached: None,
            cpu_s: 0.0,
            stone_trials: 0,
            split_runs: 0,
            split_child: false,
            lemma9: 0,
            bound_violation: None,
            delta_violation: None,
        }
    }
    /// Child.row
    pub fn row(&self) -> J {
        let o = |v: Option<u64>| v.map(|x| J::Int(x as i64)).unwrap_or(J::Null);
        J::Arr(vec![
            J::Int(self.g as i64),
            o(self.first),
            o(self.issue),
            J::Str(self.outcome.clone()),
            o(self.end),
            o(self.q_end),
            J::Int(self.frames as i64),
            self.why.as_ref().map(|w| J::Str(cut(&w.str(), 80))).unwrap_or(J::Null),
        ])
    }
    /// window.end_key
    pub fn end_key(&self) -> (u64, bool, u64) {
        (self.q_end.unwrap_or(u64::MAX), self.outcome == "deviation", self.g)
    }
    pub fn is_end_point(&self) -> bool {
        END_POINTS.contains(&self.outcome.as_str())
    }
    fn set(&mut self, outcome: &str, why: Option<Py>) {
        self.outcome = outcome.into();
        self.why = why;
    }
}

// --- the board: the children's shared reads in fork order ----------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum FirstSt {
    Unknown,
    None_,
    At(u64),
}

#[derive(Clone, Copy, Debug)]
struct Slot {
    q_cur: u64,
    first: FirstSt,
    /// done: Some(end key) for an end point, None otherwise
    done: Option<Option<(u64, bool, u64)>>,
    cancel: bool,
}

/// The children's progress and ends, read in fork order (module docstring).
pub struct Board {
    st: Mutex<Vec<Slot>>,
    cv: Condvar,
    waiters: AtomicU64,
}

impl Board {
    fn new() -> Board {
        Board {
            st: Mutex::new(Vec::new()),
            cv: Condvar::new(),
            waiters: AtomicU64::new(0),
        }
    }
    fn add(&self, q: u64) -> usize {
        let mut s = self.st.lock().unwrap();
        s.push(Slot {
            q_cur: q,
            first: FirstSt::Unknown,
            done: None,
            cancel: false,
        });
        s.len() - 1
    }
    fn notify(&self) {
        if self.waiters.load(Ordering::Acquire) > 0 {
            self.cv.notify_all();
        }
    }
    fn progress(&self, c: usize, q: u64) {
        let mut s = self.st.lock().unwrap();
        if s[c].q_cur < q {
            s[c].q_cur = q;
        }
        drop(s);
        self.notify();
    }
    fn set_first(&self, c: usize, f: Option<u64>) {
        let mut s = self.st.lock().unwrap();
        if s[c].first == FirstSt::Unknown {
            s[c].first = match f {
                Some(x) => FirstSt::At(x),
                None => FirstSt::None_,
            };
        }
        drop(s);
        self.notify();
    }
    fn finish(&self, c: usize, end: Option<(u64, bool, u64)>, q: Option<u64>) {
        let mut s = self.st.lock().unwrap();
        if s[c].first == FirstSt::Unknown {
            s[c].first = FirstSt::None_;
        }
        if let Some(q) = q {
            s[c].q_cur = s[c].q_cur.max(q);
        }
        s[c].done = Some(end);
        drop(s);
        self.cv.notify_all();
    }
    fn wait_until<T>(&self, mut f: impl FnMut(&[Slot]) -> Option<T>) -> T {
        let mut s = self.st.lock().unwrap();
        loop {
            if let Some(x) = f(&s) {
                return x;
            }
            self.waiters.fetch_add(1, Ordering::AcqRel);
            s = self.cv.wait_timeout(s, std::time::Duration::from_millis(20)).unwrap().0;
            self.waiters.fetch_sub(1, Ordering::AcqRel);
        }
    }
    /// Lemma 5 in fork order: an earlier child's first input event lands on `first` too.
    fn duplicate(&self, c: usize, first: u64) -> bool {
        self.wait_until(|s| {
            if s[..c].iter().any(|x| x.first == FirstSt::Unknown) {
                return None;
            }
            Some(s[..c].iter().any(|x| x.first == FirstSt::At(first)))
        })
    }
    /// The prune in fork order: an earlier child ended (an end point) at a Q no later than `qf`.
    fn pruned(&self, c: usize, qf: u64) -> bool {
        self.wait_until(|s| {
            if s[..c].iter().any(|x| x.done.and_then(|d| d).is_some_and(|e| e.0 <= qf)) {
                return Some(true);
            }
            if s[..c].iter().any(|x| x.done.is_none() && x.q_cur <= qf) {
                return None;
            }
            Some(false)
        })
    }
    /// The parent's best at a frame: the least end key of the children forked before it, once none of those still
    /// running could end at a Q of `bound` or less.
    fn best_before(&self, n: usize, bound: u64) -> Option<(u64, bool, u64)> {
        self.wait_until(|s| {
            if s[..n].iter().any(|x| x.done.is_none() && x.q_cur <= bound) {
                return None;
            }
            Some(s[..n].iter().filter_map(|x| x.done.and_then(|d| d)).min())
        })
    }
    /// `best_before` without waiting: None while a child forked before could still change it.
    fn best_before_now(&self, n: usize, bound: u64) -> Option<Option<(u64, bool, u64)>> {
        let s = self.st.lock().unwrap();
        if s[..n].iter().any(|x| x.done.is_none() && x.q_cur <= bound) {
            return None;
        }
        Some(s[..n].iter().filter_map(|x| x.done.and_then(|d| d)).min())
    }
    fn cancel(&self, c: usize) {
        self.st.lock().unwrap()[c].cancel = true;
    }
    fn cancelled(&self, c: usize) -> bool {
        self.st.lock().unwrap()[c].cancel
    }
}

// --- the window's shared context -----------------------------------------------------------------------------------

pub struct Alt {
    pub name: String,
    pub idx: u32,
    pub units: Vec<Unit>,
    pub linked: FxHashSet<Key>,
}

/// window_strict's `w`: what the children read.
pub struct W<'a> {
    pub cfg: &'a Config,
    pub rd: &'a PlayReading<'a>,
    pub code: &'a Code,
    pub sp: &'a Plan,
    pub recs: &'a [MRec],
    pub tracked: Vec<usize>,
    pub needed: FxHashSet<usize>,
    pub needed_order: Vec<usize>,
    pub first: Option<usize>,
    pub inp: Option<InputStep>,
    pub name: String,
    pub goal: &'a [(Key, Vec<Val>)],
    pub unobserved: &'a Prefixes,
    pub goal_writers: FxHashSet<usize>,
    pub plan_pending: Vec<usize>,
    pub issued: FxHashSet<usize>,
    pub before_input: FxHashSet<usize>,
    pub alts: Vec<Alt>,
    pub in_units: Vec<Vec<Unit>>,
    pub b_qf: u64,
    pub b_dev_qf: u64,
    pub explain_units: Vec<(i32, i32)>,
    pub chk_units: FxHashSet<Unit>,
    pub predicted_keys: FxHashSet<Key>,
    pub lite_objects: Vec<u32>,
    pub tracked_i: FxHashSet<usize>,
    pub early_recs: FxHashSet<usize>,
    pub fv: FastView<'a>,
    pub av: ArgsView<'a>,
    pub full_fv: FastView<'a>,
    pub board: Board,
    pub ready: bool,
    pub ready_failed_firsts: Mutex<FxHashSet<u64>>,
    /// the test oracle: per child, recorded answers to its reads of the best (test 2)
    pub oracle: Option<&'a dyn ChildOracle>,
    pub prof: Prof,
    pub hit_cache: Option<&'a executor::HitCache>,
}

/// Where an observed child frame's wall goes (summed over the window's children), and the input code's.
#[derive(Default)]
pub struct Prof {
    pub frames: AtomicU64,
    pub ns: [AtomicU64; 7],
    pub plays: AtomicU64,
    pub play_ns: AtomicU64,
    pub play_frames: AtomicU64,
    /// state asked, state kept, at asked, at kept
    pub play_hits: [AtomicU64; 4],
    pub forks: AtomicU64,
    pub fork_ns: AtomicU64,
    /// stone trials: count, wall, frames, hit tests
    pub stone: [AtomicU64; 4],
    /// the input code's thread CPU ns: all, in run, in obs, in at, in the stone trials' fork and quit
    pub play_cpu: [AtomicU64; 5],
    /// a child's whole thread CPU ns
    pub child_cpu: AtomicU64,
}

pub const PROF_PARTS: [&str; 7] = [
    "run",
    "runs",
    "explain_log",
    "add_entries",
    "state",
    "facts+view",
    "match",
];

impl Prof {
    fn add(&self, d: &[std::time::Duration; 7]) {
        self.frames.fetch_add(1, Ordering::Relaxed);
        for (a, x) in self.ns.iter().zip(d) {
            a.fetch_add(x.as_nanos() as u64, Ordering::Relaxed);
        }
    }
    fn play(&self, d: std::time::Duration, frames: u64, hits: [u64; 4]) {
        self.plays.fetch_add(1, Ordering::Relaxed);
        self.play_ns.fetch_add(d.as_nanos() as u64, Ordering::Relaxed);
        self.play_frames.fetch_add(frames, Ordering::Relaxed);
        for (a, h) in self.play_hits.iter().zip(hits) {
            a.fetch_add(h, Ordering::Relaxed);
        }
    }
    fn play_cpu(&self, all: u64, parts: [u64; 4]) {
        self.play_cpu[0].fetch_add(all, Ordering::Relaxed);
        for (a, x) in self.play_cpu[1..].iter().zip(parts) {
            a.fetch_add(x, Ordering::Relaxed);
        }
    }
    pub fn cpu_line(&self) -> String {
        let g = |a: &AtomicU64| a.load(Ordering::Relaxed) as f64 / 1e9;
        format!(
            "thread CPU s: children {:.2}; input code {:.2} (engine calls: run {:.2}, obs {:.2}, at {:.2}, fork+quit {:.2})",
            g(&self.child_cpu),
            g(&self.play_cpu[0]),
            g(&self.play_cpu[1]),
            g(&self.play_cpu[2]),
            g(&self.play_cpu[3]),
            g(&self.play_cpu[4])
        )
    }
    fn stones(&self, n: u64, ns: u64, frames: u64, at: u64) {
        for (a, x) in self.stone.iter().zip([n, ns, frames, at]) {
            a.fetch_add(x, Ordering::Relaxed);
        }
    }
    fn fork(&self, d: std::time::Duration) {
        self.forks.fetch_add(1, Ordering::Relaxed);
        self.fork_ns.fetch_add(d.as_nanos() as u64, Ordering::Relaxed);
    }
    pub fn play_line(&self) -> String {
        let n = self.plays.load(Ordering::Relaxed).max(1) as f64;
        let fr = self.play_frames.load(Ordering::Relaxed);
        let h: Vec<u64> = self.play_hits.iter().map(|a| a.load(Ordering::Relaxed)).collect();
        format!(
            "input code: {} children, {:.1} ms and {:.1} frames each ({:.3} ms a frame), {} dumps asked and {} kept, {} hit tests asked and {} kept; snapshots {} at {:.3} ms; stone trials {} in {:.2} s, {} frames, {} hit tests",
            self.plays.load(Ordering::Relaxed),
            self.play_ns.load(Ordering::Relaxed) as f64 / n / 1e6,
            fr as f64 / n,
            self.play_ns.load(Ordering::Relaxed) as f64 / (fr.max(1) as f64) / 1e6,
            h[0],
            h[1],
            h[2],
            h[3],
            self.forks.load(Ordering::Relaxed),
            self.fork_ns.load(Ordering::Relaxed) as f64 / (self.forks.load(Ordering::Relaxed).max(1) as f64) / 1e6,
            self.stone[0].load(Ordering::Relaxed),
            self.stone[1].load(Ordering::Relaxed) as f64 / 1e9,
            self.stone[2].load(Ordering::Relaxed),
            self.stone[3].load(Ordering::Relaxed)
        )
    }
    pub fn line(&self) -> String {
        let n = self.frames.load(Ordering::Relaxed).max(1) as f64;
        let parts: Vec<String> = PROF_PARTS
            .iter()
            .zip(&self.ns)
            .map(|(p, a)| format!("{p} {:.3}", a.load(Ordering::Relaxed) as f64 / n / 1e6))
            .collect();
        let tot: u64 = self.ns.iter().map(|a| a.load(Ordering::Relaxed)).sum();
        format!(
            "{} observed child frames, {:.3} ms each ({})",
            self.frames.load(Ordering::Relaxed),
            tot as f64 / n / 1e6,
            parts.join(", ")
        )
    }
}

/// Recorded answers to a child's reads of the window's shared state (fmi-play's differential test 2).
pub trait ChildOracle: Sync {
    /// whether the child forked at `g` is pruned at frame `f` with Q `qf`; None: answer by the board
    fn pruned(&self, g: u64, f: u64, qf: u64) -> Option<bool>;
    /// whether the child forked at `g`, its first event at `first`, is a duplicate; None: by the board
    fn duplicate(&self, g: u64, first: u64) -> Option<bool>;
    /// whether the parent stops at frame `g` by section 9.3's rule; None: by the board
    fn parent_stop(&self, g: u64) -> Option<bool>;
}

impl<'a> W<'a> {
    /// the window's view of a dump: FastView's values and ArgsView's lists (window_strict's `sview`)
    pub fn sview(&self, s: &Obs) -> Result<View, String> {
        let mut v = self.fv.view_obs(s);
        if check_views() {
            let r = self.fv.view(&facts_of(s));
            assert_eq!(
                v, r,
                "FMI_CHECK_VIEW: the dump's view differs from the view of facts_of"
            );
        }
        self.av.apply(s, &mut v)?;
        Ok(v)
    }
    fn holds_goal(&self, now: &View) -> bool {
        replay::holds_all(self.goal, now, self.unobserved)
    }
}

// --- one child (section 3 with section 9) ----------------------------------------------------------------------------

struct Obsv<'a, 'w> {
    w: &'w W<'a>,
    rows_all: Vec<RunR>,
    log: BranchLog,
    frames_seen: BTreeMap<u64, (Arc<View>, Arc<Obs>)>,
    err: Option<String>,
    board_c: usize,
}

impl<'a, 'w> FrameObs for Obsv<'a, 'w> {
    fn on_frame(&mut self, e: &mut dyn Engine, s: &Arc<Obs>) -> EResult<()> {
        let rows = e.runs()?;
        self.rows_all.extend(strict::runs_of(&rows.runs));
        let ents = e.explain_log()?;
        self.log.kept.add_entries(&strict::ents_of(&ents.entries));
        let f = e.status().frame;
        let v = match self.w.sview(s) {
            Ok(v) => v,
            Err(m) => {
                self.err = Some(m.clone());
                return Err(EngineError::Bridge(m));
            }
        };
        self.frames_seen.insert(f, (Arc::new(v), s.clone()));
        self.w.board.progress(self.board_c, e.status().qf);
        Ok(())
    }
}

/// window.Search._sig
fn sig_of(f: &Fail) -> (String, Option<String>, String) {
    f.sig()
}

struct CState {
    cands: Vec<(bool, String, u32, Vec<Unit>, FxHashSet<Key>)>, // (plan?, name or path index, idx, units, linked)
    needs: Vec<Vec<Unit>>,
    own: Vec<Vec<(Unit, RunR)>>,
    chk_own: Vec<(Unit, RunR)>,
    decided: Vec<Option<InMatch>>,
    matched_writes: Vec<(i64, Vec<(Key, Val)>)>,
    held: Vec<(u64, RunR, Verdict, Arc<View>)>,
    run_ctx: FxHashMap<
        i64,
        (
            u64,
            (Option<Arc<View>>, Option<Arc<Obs>>),
            (Option<Arc<View>>, Option<Arc<Obs>>),
            Vec<RunR>,
        ),
    >,
}

/// One child's whole check: `child_strict` and the closures of window._child_strict.
struct CRun<'a, 'w> {
    w: &'w W<'a>,
    ch: Child,
    track: Track,
    log: BranchLog,
    frames_seen: BTreeMap<u64, (Arc<View>, Arc<Obs>)>,
    rk: BTreeMap<u64, Vec<RunR>>,
    s_issue: u64,
    v0: Arc<View>,
    d0: Option<Arc<Obs>>,
    cs: CState,
    first_i: usize,
}

impl<'a, 'w> CRun<'a, 'w> {
    fn fs(&self, f: u64) -> (Option<Arc<View>>, Option<Arc<Obs>>) {
        match self.frames_seen.get(&f) {
            Some((v, d)) => (Some(v.clone()), Some(d.clone())),
            None => (None, None),
        }
    }

    fn fail(&mut self, f: Fail) {
        if self.ch.fail.is_none() {
            self.ch.fail = Some(f);
        }
    }

    fn apply_run(&mut self, f: u64, r: &RunR, v: &Verdict, view: &View, dump: Option<Arc<Obs>>) {
        let w = self.w;
        let dump = dump.or_else(|| self.fs(f).1);
        let res = self.track.apply(w.sp, w.recs, w.rd, f, r, v, view, dump.as_deref());
        for (ri, how) in res {
            if how == "matched" {
                let path = self.track.m(ri).unwrap().1;
                if let Some(idx) = w.sp.idx[&ri][path] {
                    let wr: Vec<(Key, Val)> =
                        w.rd.b1(idx)
                            .eff
                            .iter()
                            .filter_map(|(k, e)| {
                                if let crate::reading::B1Eff::Set(v) = e {
                                    Some((*k, *v))
                                } else {
                                    None
                                }
                            })
                            .collect();
                    self.cs.matched_writes.push((r.seq, wr));
                }
            }
        }
    }

    fn flush(&mut self) {
        if self.ch.took != Took::NA && !self.cs.held.is_empty() {
            let held = std::mem::take(&mut self.cs.held);
            for (f, pr, pv, va) in held {
                if self.ch.took != Took::No {
                    self.apply_run(f, &pr, &pv, &va, None);
                }
            }
        }
    }

    fn decide(&mut self, n: usize, f: u64, va: Arc<View>, runs_f: &[RunR]) {
        let w = self.w;
        let seqs: Vec<i64> = self.cs.own[n].iter().map(|o| o.1.seq).collect();
        let (lo, hi) = (seqs.iter().min().copied(), seqs.iter().max().copied());
        let mut applied: Vec<(Key, Val)> = Vec::new();
        for (sq, wr) in &self.cs.matched_writes {
            if lo.is_none_or(|lo| *sq < lo) {
                for &(k, v) in wr {
                    match applied.iter_mut().find(|p| p.0 == k) {
                        Some(p) => p.1 = v,
                        None => applied.push((k, v)),
                    }
                }
            }
        }
        let mut later: Vec<RunR> = runs_f
            .iter()
            .filter(|r| hi.is_some_and(|h| r.seq > h))
            .copied()
            .collect();
        if self.log.parter.is_some() {
            let mut l2 = Vec::new();
            for r in later {
                let ps = if r.depth == 0 { self.log.parts(w.code, &r) } else { None };
                match ps {
                    Some(ps) => l2.extend(ps),
                    None => l2.push(r),
                }
            }
            later = l2;
        }
        let chk_own = self.cs.chk_own.clone();
        let code = w.code;
        let mut on_log = |chk: &crate::reading::Check, log: &mut BranchLog| -> LogDec {
            let cu = strict::cite_unit(&chk.cite);
            let mut r_ = cu.and_then(|cu| chk_own.iter().find(|o| o.0 == cu.0).map(|o| o.1));
            if let Some(r) = r_ {
                if log.parter.is_some() {
                    if let Some(ps) = log.parts(code, &r) {
                        r_ = Some(ps[0]);
                    }
                }
            }
            match r_ {
                Some(r) => strict::decide_on_log(code, &log.kept.ex, &[&r], chk),
                None => LogDec::NoLog,
            }
        };
        let dk = self.fs(f).1;
        let own = self.cs.own[n].clone();
        let idx = self.cs.cands[n].2;
        let mut res = strict::input_match(
            w.rd,
            w.sp,
            code,
            idx,
            &self.v0,
            self.d0.as_deref(),
            &applied,
            &own,
            &va,
            &later,
            &mut on_log,
            dk.as_deref(),
            &mut self.log,
        );
        res.frame = f;
        res.view = Some(va);
        self.cs.decided[n] = Some(res);
    }

    fn apply_split(&mut self, res: &InMatch, idx: u32) {
        let w = self.w;
        let eff = w.rd.b1(idx).eff.clone();
        let mut items: Vec<(Unit, Vec<RunR>)> = res.split.clone();
        items.sort_by_key(|x| x.1[0].seq);
        for (_u, ps) in items {
            let seq0 = ps[0].seq / crate::code::SEQ_SCALE * crate::code::SEQ_SCALE;
            let Some(ctx) = self.cs.run_ctx.get(&seq0).cloned() else {
                continue;
            };
            if ps.len() < 2 {
                continue;
            }
            let (k_, (vb, db), (va, da), runs_k) = ctx;
            let (Some(vb), Some(va)) = (vb, va) else { continue };
            let prior: Vec<RunR> = runs_k.iter().filter(|x| x.seq < seq0).copied().collect();
            let mut open: FxHashSet<Key> = if prior.is_empty() {
                FxHashSet::default()
            } else {
                strict::open_keys_of(&prior, &vb, &va)
            };
            let prior_seqs: FxHashSet<i64> = prior.iter().map(|x| x.seq).collect();
            let mut state = (*vb).clone();
            for (sq, wr) in &self.cs.matched_writes {
                if prior_seqs.contains(&(sq / crate::code::SEQ_SCALE * crate::code::SEQ_SCALE)) {
                    for &(k, v) in wr {
                        state.setv(k, v);
                    }
                }
            }
            for (k, e) in &eff {
                match e {
                    crate::reading::B1Eff::Set(v) => state.setv(*k, *v),
                    _ => {
                        open.insert(*k);
                    }
                }
            }
            let later: Vec<RunR> = strict::plan_runs(w.sp, w.code, &runs_k, &mut self.log)
                .into_iter()
                .filter(|x| x.seq > seq0)
                .collect();
            let vs = strict::part_verdicts(
                w.sp,
                w.code,
                &ps[1..],
                &state,
                &va,
                &later,
                db.as_deref(),
                &self.log.kept.ex,
                &open,
            );
            for (pr, pv) in vs {
                self.apply_run(k_, &pr, &pv, &va, da.clone());
            }
        }
        self.ch.split_child = true;
    }

    fn settle(&mut self, fin: bool) {
        if self.ch.took != Took::NA {
            return;
        }
        let plan_c: Vec<usize> = (0..self.cs.cands.len()).filter(|&n| self.cs.cands[n].0).collect();
        let ok = plan_c
            .iter()
            .copied()
            .find(|&n| self.cs.decided[n].as_ref().is_some_and(|d| d.ok));
        if let Some(ok) = ok {
            let res = self.cs.decided[ok].clone().unwrap();
            let path: usize = self.cs.cands[ok].1.parse().unwrap_or(0);
            self.track.set_matched(self.first_i, (self.s_issue, path, res.last_seq));
            self.track
                .unit_seq
                .insert(self.first_i, self.cs.own[ok].iter().map(|o| (o.0, o.1.seq)).collect());
            self.track.first_seq.insert(self.first_i, res.first_seq);
            self.ch.outside = res.outside.clone();
            self.ch.took = if res.took { Took::Plan } else { Took::PlanNoChange };
            if !res.split.is_empty() {
                let idx = self.cs.cands[ok].2;
                self.apply_split(&res, idx);
            }
            self.ch.track_issue = Some(self.track.clone());
            return;
        }
        if !fin && plan_c.iter().any(|&n| self.cs.decided[n].is_none()) {
            return;
        }
        let alt_c: Vec<usize> = (0..self.cs.cands.len()).filter(|&n| !self.cs.cands[n].0).collect();
        for &n in &alt_c {
            let Some(rj) = &self.cs.decided[n] else { continue };
            let linked = &self.cs.cands[n].4;
            let view = rj.view.clone().unwrap_or_default();
            if rj.ok && rj.took && linked.iter().any(|k| view.get(*k) != self.v0.get(*k)) {
                let name = self.cs.cands[n].1.clone();
                self.ch.alt = Some(name.clone());
                self.ch.took = Took::Alt(name);
                self.ch.track_issue = Some(self.track.clone());
                let f = rj.frame;
                self.input_fail(f, "no run");
                return;
            }
        }
        if fin || alt_c.iter().all(|&n| self.cs.decided[n].is_some()) {
            self.ch.took = Took::No;
        }
    }

    fn frame_step(&mut self, f: u64) {
        let w = self.w;
        let (vb, db) = self.fs(f.wrapping_sub(1));
        let (va, da) = self.fs(f);
        let runs_f: Vec<RunR> = self.rk.get(&f).cloned().unwrap_or_default();
        let mut vs: BTreeMap<i64, Vec<(RunR, Verdict)>> = BTreeMap::new();
        if let (Some(vb), Some(va)) = (&vb, &va) {
            for (r, v) in self
                .track
                .verdicts(w.sp, w.code, &runs_f, vb, va, db.as_deref(), &mut self.log)
            {
                let whole = r.seq / crate::code::SEQ_SCALE * crate::code::SEQ_SCALE;
                vs.entry(whole).or_default().push((r, v));
            }
        }
        for r in &runs_f {
            if r.depth >= 1 && r.frame >= self.s_issue && self.ch.took == Took::NA {
                for n in 0..self.cs.cands.len() {
                    if let Some(u) = r.unit {
                        if self.cs.cands[n].3.contains(&u) && !self.cs.own[n].iter().any(|o| o.0 == u) {
                            self.cs.own[n].push((u, *r));
                            if self.log.parter.is_some() {
                                self.cs.run_ctx.entry(r.seq).or_insert_with(|| {
                                    (f, (vb.clone(), db.clone()), (va.clone(), da.clone()), runs_f.clone())
                                });
                            }
                        }
                    }
                }
                if let Some(u) = r.unit {
                    if w.chk_units.contains(&u) && !self.cs.chk_own.iter().any(|o| o.0 == u) {
                        self.cs.chk_own.push((u, *r));
                        self.log.kept.pinned.insert(r.seq);
                    }
                }
            }
            if let Some(list) = vs.get(&r.seq).cloned() {
                if self.ch.took == Took::NA {
                    if let Some(va) = &va {
                        for n in 0..self.cs.cands.len() {
                            let own = &self.cs.own[n];
                            if self.cs.decided[n].is_none()
                                && !own.is_empty()
                                && self.cs.needs[n].iter().all(|u| own.iter().any(|o| o.0 == *u))
                                && own.iter().map(|o| o.1.seq).max().unwrap() < r.seq
                            {
                                self.decide(n, f, va.clone(), &runs_f);
                            }
                        }
                        self.settle(false);
                        self.flush();
                    }
                }
                let first_own = self.cs.own.iter().flat_map(|o| o.iter().map(|x| x.1.seq)).min();
                for (pr, pv) in list {
                    if self.ch.took == Took::NA && first_own.is_some_and(|fo| pr.seq > fo) {
                        self.cs.held.push((f, pr, pv, va.clone().unwrap_or_default()));
                    } else {
                        let view = va.clone().unwrap_or_default();
                        self.apply_run(f, &pr, &pv, &view, None);
                    }
                }
            }
        }
        if self.ch.took == Took::NA && f > self.s_issue {
            if let Some(va) = &va {
                for n in 0..self.cs.cands.len() {
                    let own = &self.cs.own[n];
                    if self.cs.decided[n].is_none() && self.cs.needs[n].iter().all(|u| own.iter().any(|o| o.0 == *u)) {
                        self.decide(n, f, va.clone(), &runs_f);
                    }
                }
                self.settle(false);
            }
        }
        self.flush();
    }

    fn input_fail(&mut self, f: u64, why_default: &str) {
        let w = self.w;
        let first_c = (0..self.cs.cands.len()).find(|&n| self.cs.cands[n].0);
        let mut res: Option<InMatch> = first_c.and_then(|n| self.cs.decided[n].clone());
        if res.is_none() {
            if let Some(fc) = first_c {
                let miss: Vec<Unit> = self.cs.needs[fc]
                    .iter()
                    .copied()
                    .filter(|u| !self.cs.own[fc].iter().any(|o| o.0 == *u))
                    .collect();
                res = Some(InMatch {
                    why: Some(match miss.first() {
                        Some(u) => format!("no run pos:{}", u.text()),
                        None => why_default.to_string(),
                    }),
                    frame: f,
                    ..Default::default()
                });
            }
        }
        let Some(res) = res else { return };
        let why = res.why.clone().unwrap_or_default();
        let first = &w.recs[self.first_i];
        let asks = w.sp.idx[&self.first_i].first().copied().flatten().and_then(|i| {
            let tail = why.split_once(' ').map(|x| x.1).unwrap_or(&why);
            w.rd.b1(i).pre(Key::parse(tail)).cloned()
        });
        let mat = self.log.kept.freeze(
            &[(self.ch.g, self.v0.clone())],
            &self.d0.iter().map(|d| (self.ch.g, d.clone())).collect::<Vec<_>>(),
        );
        let mut fl = Fail::new("input", Some(first.name.clone()), res.frame, why, res.run, mat);
        fl.want = res.want;
        fl.engine = res.engine.clone();
        fl.own = first_c
            .map(|n| self.cs.own[n].iter().map(|o| o.1).collect())
            .unwrap_or_default();
        fl.asks = asks;
        self.fail(fl);
    }

    fn not_run_fail(&mut self, f: u64) {
        let w = self.w;
        let r = w.tracked.iter().copied().find(|&i| {
            w.needed.contains(&i)
                && !self.track.is_matched(i)
                && !self.track.is_refuted(i)
                && w.recs[i].kind == Kind::Event
        });
        let Some(r) = r else { return };
        let mut fl = Fail::new(
            "not run",
            Some(w.recs[r].name.clone()),
            f,
            "its run did not happen".into(),
            None,
            self.log.kept.freeze(&[], &[]),
        );
        fl.unit = w.recs[r].unit;
        self.fail(fl);
    }
}

fn err_why(e: &EngineError) -> Py {
    match e {
        EngineError::Bridge(m) => Py::s(format!("ControlError: {m}")),
        EngineError::Io(m) => Py::s(format!("OSError: {m}")),
        EngineError::Ended(st) => Py::s(format!("Stop: engine ended: {}", st.reason)),
    }
}

/// window.Search._child_strict for one child forked at `ch.g` (`c`: its index in fork order).
pub fn child_strict<'a>(w: &W<'a>, c: usize, cb: Box<dyn Engine>, ch: Child) -> Child {
    let t = executor::thread_ns();
    let out = child_strict_(w, c, cb, ch);
    w.prof.child_cpu.fetch_add(executor::thread_ns() - t, Ordering::Relaxed);
    out
}

fn child_strict_<'a>(w: &W<'a>, c: usize, mut cb: Box<dyn Engine>, mut ch: Child) -> Child {
    let mut snap: Option<Box<dyn Engine>> = None;
    let out = (|| -> Result<(), EngineError> {
        let first_i = w.first.unwrap();
        if w.board.cancelled(c) {
            ch.outcome = "cancelled".into();
            return Ok(());
        }
        cb.explain(Some(&ExplainArm {
            from: ch.g + 1,
            to: ch.g + w.cfg.child_limit + w.cfg.window_frames + 2,
            units: w.explain_units.clone(),
        }))?;
        let track = ch.track.clone().unwrap_or_default();
        let observe_play = w
            .early_recs
            .iter()
            .any(|&i| !track.is_matched(i) && !track.is_refuted(i));
        let mut obsv = Obsv {
            w,
            rows_all: vec![],
            log: BranchLog::new(w.cfg.split_wait),
            frames_seen: BTreeMap::new(),
            err: None,
            board_c: c,
        };
        // the input code
        let mut ex = Executor::new(w.cfg.stones);
        let play_res;
        let first_before;
        {
            let mut br = Br::new(&mut *cb, if w.cfg.cache { ch.d0.clone() } else { None });
            br.watch_first = true;
            br.hc = w.hit_cache;
            if observe_play {
                br.obs = Some(&mut obsv);
            }
            let (t0, f0, c0) = (Instant::now(), br.frame(), executor::thread_ns());
            play_res = executor::play_input(&mut ex, &mut br, w.inp.as_ref().unwrap(), &w.name);
            first_before = br.first_before.take();
            w.prof.play(t0.elapsed(), br.frame().saturating_sub(f0), br.hits);
            w.prof.play_cpu(executor::thread_ns() - c0, br.cpu);
        }
        ch.stone_trials += ex.stone_trials;
        w.prof
            .stones(ex.stone_trials as u64, ex.stone_ns, ex.stone_frames, ex.stone_at);
        ch.cpu_s += ex.stone_cpu;
        w.board.progress(c, cb.status().qf);
        if let Some(m) = obsv.err.take() {
            return Err(EngineError::Bridge(m));
        }
        match play_res {
            Err(PlayErr::Route(m)) => {
                ch.set("interface", Some(Py::s(m)));
                if w.ready {
                    let ins: Vec<u64> = ex
                        .movie
                        .iter()
                        .filter(|e| executor::is_input(e))
                        .map(|e| e.frame)
                        .collect();
                    ch.first = ins.iter().min().copied();
                    ch.reached = Some(cb.status().frame);
                }
                return Ok(());
            }
            Err(PlayErr::Engine(e)) => return Err(e),
            Ok(()) => {}
        }
        let ins: Vec<u64> = ex
            .movie
            .iter()
            .filter(|e| executor::is_input(e))
            .map(|e| e.frame)
            .collect();
        ch.events = ex.movie.clone();
        if ins.is_empty() {
            ch.set("interface", Some(Py::s("no input event")));
            ch.reached = Some(cb.status().frame);
            return Ok(());
        }
        let (first, issue) = (*ins.iter().min().unwrap(), *ins.iter().max().unwrap());
        ch.first = Some(first);
        ch.issue = Some(issue);
        let dup = if w.ready {
            ready_duplicate(w, c, first)
        } else {
            match w.oracle.and_then(|o| o.duplicate(ch.g, first)) {
                Some(d) => d,
                None => w.board.duplicate(c, first),
            }
        };
        w.board.set_first(c, Some(first));
        if dup {
            ch.outcome = "duplicate".into();
            return Ok(());
        }
        if w.board.cancelled(c) {
            ch.outcome = "cancelled".into();
            return Ok(());
        }
        {
            let mut br = Br::new(&mut *cb, None);
            if observe_play {
                br.obs = Some(&mut obsv);
            }
            if br.frame() != issue {
                match br.run_to(issue) {
                    Ok(_) => {}
                    Err(EngineError::Ended(st)) => {
                        ch.set("ended", Some(Py::s(st.reason.clone())));
                        return Ok(());
                    }
                    Err(e) => return Err(e),
                }
            }
        }
        let s_issue = issue;
        let mut log = std::mem::take(&mut obsv.log);
        let mut rows_all = std::mem::take(&mut obsv.rows_all);
        let mut frames_seen = std::mem::take(&mut obsv.frames_seen);
        let observe = |cb: &mut Box<dyn Engine>,
                       log: &mut BranchLog,
                       rows_all: &mut Vec<RunR>,
                       frames_seen: &mut BTreeMap<u64, (Arc<View>, Arc<Obs>)>,
                       full: bool|
         -> Result<(), EngineError> {
            let rows = cb.runs()?;
            rows_all.extend(strict::runs_of(&rows.runs));
            log.kept.add_entries(&strict::ents_of(&cb.explain_log()?.entries));
            if full {
                let s = Arc::new(cb.obs(&ObsReq::default())?);
                let v = w.sview(&s).map_err(EngineError::Bridge)?;
                frames_seen.insert(cb.status().frame, (Arc::new(v), s));
            }
            Ok(())
        };
        if !frames_seen.contains_key(&s_issue) {
            observe(&mut cb, &mut log, &mut rows_all, &mut frames_seen, true)?;
        } else {
            observe(&mut cb, &mut log, &mut rows_all, &mut frames_seen, false)?;
        }
        let st_issue = cb.status().clone();
        let d_issue = frames_seen[&s_issue].1.clone();
        let tf = Instant::now();
        snap = Some(cb.fork()?);
        w.prof.fork(tf.elapsed());
        match cb.run(&RunSpec {
            frame: Some(s_issue + 1),
            ..Default::default()
        }) {
            Ok(_) => {}
            Err(EngineError::Ended(st)) => {
                ch.set("ended", Some(Py::s(st.reason.clone())));
                return Ok(());
            }
            Err(e) => return Err(e),
        }
        w.board.progress(c, cb.status().qf);
        observe(&mut cb, &mut log, &mut rows_all, &mut frames_seen, true)?;
        let mut rk = strict::by_frame(&rows_all);
        let at_fork = ch.at_fork.clone();
        if let Some(d0) = &ch.d0 {
            frames_seen.entry(ch.g).or_insert_with(|| (at_fork.clone(), d0.clone()));
        }
        let fs_snapshot: Vec<u64> = rk.keys().copied().collect();
        for f in fs_snapshot {
            let (v, d) = match frames_seen.get(&f) {
                Some((v, d)) => (Some(v.clone()), Some(d.clone())),
                None => (None, None),
            };
            log.kept.frame(f, rk[&f].clone(), d, v);
        }
        let (mut v0, mut d0) = (ch.at_fork.clone(), ch.d0.clone());
        if let Some((fbf, fbd)) = &first_before {
            if *fbf != ch.g {
                let v = Arc::new(w.sview(fbd).map_err(EngineError::Bridge)?);
                v0 = v.clone();
                d0 = Some(fbd.clone());
                frames_seen.entry(*fbf).or_insert_with(|| (v, fbd.clone()));
            }
        }
        // 9.2: the click's record and its other records
        let mut cands: Vec<(bool, String, u32, Vec<Unit>, FxHashSet<Key>)> = Vec::new();
        for (nn, idx) in w.sp.idx[&first_i].iter().enumerate() {
            if let Some(idx) = idx {
                cands.push((true, nn.to_string(), *idx, w.in_units[nn].clone(), FxHashSet::default()));
            }
        }
        for a in &w.alts {
            cands.push((false, a.name.clone(), a.idx, a.units.clone(), a.linked.clone()));
        }
        let needs: Vec<Vec<Unit>> = cands
            .iter()
            .map(|c| {
                w.rd.b1(c.2)
                    .eff
                    .iter()
                    .filter_map(|(k, e)| match e {
                        crate::reading::B1Eff::Set(v) if k.kind() == K_POS && is_int(*v) && *v >= 0 => {
                            Some(Unit::of_key(*k))
                        }
                        _ => None,
                    })
                    .collect()
            })
            .collect();
        let nc = cands.len();
        let mut cr = CRun {
            w,
            ch: std::mem::replace(&mut ch, Child::blank()),
            track,
            log,
            frames_seen,
            rk: BTreeMap::new(),
            s_issue,
            v0,
            d0,
            cs: CState {
                cands,
                needs,
                own: vec![vec![]; nc],
                chk_own: vec![],
                decided: vec![None; nc],
                matched_writes: vec![],
                held: vec![],
                run_ctx: FxHashMap::default(),
            },
            first_i,
        };
        let inner = (|| -> Result<(), EngineError> {
            std::mem::swap(&mut cr.rk, &mut rk);
            let steps: Vec<u64> = cr
                .rk
                .keys()
                .copied()
                .filter(|&f| cr.ch.g < f && f <= s_issue + 1)
                .collect();
            for f in steps {
                cr.frame_step(f);
            }
            let mut moved = false;
            let (mut now, mut s) = {
                let (v, d) = &cr.frames_seen[&(s_issue + 1)];
                ((**v).clone(), d.clone())
            };
            let mut m = Matcher::new(w.recs, &w.tracked, &w.needed, Some(first_i), &cr.ch.done, &w.issued);
            m.plan_pending = w.plan_pending.clone();
            let mut ended_engine = false;
            loop {
                let st = cb.status().clone();
                let f = st.frame;
                cr.ch.frames = f - cr.ch.g;
                if w.board.cancelled(c) {
                    cr.ch.outcome = "cancelled".into();
                    break;
                }
                if cr.ch.took == Took::No {
                    cr.ch.set("spent", Some(Py::s("Lemma 9′")));
                    cr.ch.lemma9 += 1;
                    cr.input_fail(f, "no run");
                    break;
                }
                if w.cfg.delta_lo > 1 && (st.v19 as i64).max(1) < w.cfg.delta_lo {
                    cr.ch.set("delta", Some(Py::Int(st.v19 as i64)));
                    cr.ch.delta_violation = Some(J::Arr(vec![J::Int(f as i64), J::Int(st.v19 as i64)]));
                    break;
                }
                if cr.ch.choice_fail.is_none() {
                    if let Some((i, ch_)) = cr.track.choices.iter().find(|x| w.tracked_i.contains(&x.0)).cloned() {
                        let (fr, _path, seq) = *cr.track.m(i).unwrap();
                        let run_ = cr.rk.values().flat_map(|rs| rs.iter()).find(|r| r.seq == seq).copied();
                        cr.ch.choice_fail = Some(Fail::new(
                            "choice",
                            Some(w.recs[i].name.clone()),
                            fr,
                            format!("effect {}", ch_[0].0.text()),
                            run_,
                            cr.log.kept.freeze(&[], &[]),
                        ));
                    }
                }
                if st.mv.unwrap_or(0) != 0 {
                    moved = true;
                } else if moved && cr.ch.arrival.is_none() {
                    cr.ch.arrival = Some(f);
                }
                let goal = w.holds_goal(&now);
                let credited = !w.goal_writers.is_empty() && w.goal_writers.iter().all(|i| cr.track.is_matched(*i));
                if goal && !credited && !(cr.ch.took == Took::NA && w.goal_writers.contains(&first_i)) {
                    let mut gws: Vec<usize> = w.goal_writers.iter().copied().collect();
                    gws.sort();
                    cr.ch.outcome = "goal, not from the plan".into();
                    cr.ch.why = Some(Py::List(
                        gws.iter()
                            .map(|&i| {
                                Py::Tuple(vec![
                                    Py::s(w.recs[i].name.clone()),
                                    Py::s(if cr.track.is_matched(i) {
                                        "matched".to_string()
                                    } else if let Some(r) = cr.track.r(i) {
                                        format!("refuted: {}", r.1)
                                    } else {
                                        "its run did not happen".to_string()
                                    }),
                                ])
                            })
                            .collect(),
                    ));
                    let gi = gws.iter().copied().find(|&i| cr.track.is_refuted(i));
                    if let Some(gi) = gi {
                        let (fr, why_, run_) = cr.track.r(gi).unwrap().clone();
                        let mat = cr.log.kept.freeze(&[], &[]);
                        cr.fail(Fail::new(
                            "refuted",
                            Some(w.recs[gi].name.clone()),
                            fr,
                            why_,
                            Some(run_),
                            mat,
                        ));
                    } else if cr.ch.took == Took::NA
                        || (0..cr.cs.cands.len())
                            .any(|n| cr.cs.cands[n].0 && cr.cs.decided[n].as_ref().is_some_and(|d| !d.ok))
                    {
                        cr.input_fail(f, "no run");
                    } else {
                        cr.not_run_fail(f);
                    }
                    break;
                }
                if (goal && credited) || w.needed.iter().all(|i| cr.track.is_matched(*i)) {
                    cr.ch.outcome = if goal { "goal".into() } else { "success".into() };
                    break;
                }
                if let Some(&i) = w.needed_order.iter().find(|&&i| cr.track.is_refuted(i)) {
                    let (fr, why_, run_) = cr.track.r(i).unwrap().clone();
                    let name = w
                        .tracked
                        .iter()
                        .find(|&&r| r == i)
                        .map(|&r| w.recs[r].name.clone())
                        .unwrap_or_default();
                    cr.ch.set(
                        "refuted",
                        Some(Py::Tuple(vec![Py::s(name.clone()), Py::s(why_.clone())])),
                    );
                    let mat = cr.log.kept.freeze(&[], &[]);
                    cr.fail(Fail::new("refuted", Some(name), fr, why_, Some(run_), mat));
                    break;
                }
                m.matched = cr.track.matched.iter().map(|(i, v)| (*i, (Some(v.0), v.1))).collect();
                if let Some((k, v)) = m.mismatch(w.recs, &now, &cr.ch.at_fork) {
                    cr.ch.set("mismatch", Some(Py::Tuple(vec![Py::s(k.text()), v.py()])));
                    let unit = if k.kind() == K_POS { Some(Unit::of_key(k)) } else { None };
                    let run_ = unit.and_then(|u| {
                        cr.rk
                            .get(&f)
                            .and_then(|rs| rs.iter().rev().find(|r| r.unit == Some(u)).copied())
                    });
                    let mat = cr.log.kept.freeze(&[], &[]);
                    cr.fail(Fail::new("mismatch", None, f, format!("end {}", k.text()), run_, mat));
                    break;
                }
                let pr = if w.ready {
                    false
                } else {
                    match w.oracle.and_then(|o| o.pruned(cr.ch.g, f, st.qf)) {
                        Some(p) => p,
                        None => w.board.pruned(c, st.qf),
                    }
                };
                if pr {
                    cr.ch.outcome = "pruned".into();
                    break;
                }
                if f > s_issue + 1 && st.idle.is_some() {
                    if cr.ch.took == Took::NA {
                        cr.settle(true);
                        cr.flush();
                    }
                    if cr.ch.took == Took::No {
                        cr.ch.set("spent", Some(Py::s("Lemma 9′: no run by the idle stop")));
                        cr.ch.lemma9 += 1;
                        cr.input_fail(f, "no run");
                        break;
                    }
                    let dead = m.dead_needed(w.recs, &now);
                    if !dead.is_empty() {
                        let why = m.unmet(w.recs, &now, 4);
                        cr.ch.set("spent", Some(why));
                        cr.not_run_fail(f);
                        break;
                    }
                }
                if f - s_issue >= w.cfg.child_limit {
                    let why = m.unmet(w.recs, &now, 4);
                    cr.ch.set("limit", Some(why));
                    if cr.ch.took == Took::NA {
                        cr.settle(true);
                        cr.flush();
                    }
                    if cr.ch.took == Took::No {
                        cr.input_fail(f, "no run");
                    } else {
                        cr.not_run_fail(f);
                    }
                    break;
                }
                let t0 = Instant::now();
                match cb.run(&RunSpec {
                    frame: Some(f + 1),
                    ..Default::default()
                }) {
                    Ok(_) => {}
                    Err(EngineError::Ended(st)) => {
                        cr.ch.set("ended", Some(Py::s(st.reason.clone())));
                        ended_engine = true;
                        break;
                    }
                    Err(e) => return Err(e),
                }
                w.board.progress(c, cb.status().qf);
                let t1 = Instant::now();
                let rows = cb.runs()?;
                let t2 = Instant::now();
                let ents = cb.explain_log()?;
                let t3 = Instant::now();
                cr.log.kept.add_entries(&strict::ents_of(&ents.entries));
                let req = if w.cfg.lite {
                    ObsReq {
                        objects: Some(w.lite_objects.clone()),
                        names: false,
                        exact: false,
                    }
                } else {
                    ObsReq::default()
                };
                let t4 = Instant::now();
                s = Arc::new(cb.obs(&req)?);
                let t5 = Instant::now();
                now = w.sview(&s).map_err(EngineError::Bridge)?;
                let t6 = Instant::now();
                let nf = f + 1;
                let r_new = strict::by_frame(&strict::runs_of(&rows.runs))
                    .remove(&nf)
                    .unwrap_or_default();
                cr.rk.insert(nf, r_new.clone());
                let nowa = Arc::new(now.clone());
                cr.frames_seen.insert(nf, (nowa.clone(), s.clone()));
                let g = cr.ch.g;
                cr.frames_seen.retain(|&x, _| !(x + 1 < f && x != g));
                cr.rk.retain(|&x, _| x + 2 >= f);
                cr.log.kept.frame(nf, r_new, Some(s.clone()), Some(nowa));
                cr.frame_step(nf);
                let t7 = Instant::now();
                w.prof
                    .add(&[t1 - t0, t2 - t1, t3 - t2, t4 - t3, t5 - t4, t6 - t5, t7 - t6]);
            }
            let _ = ended_engine;
            cr.ch.end = Some(cb.status().frame);
            cr.ch.q_end = Some(cb.status().qf);
            cr.ch.matched = cr.track.matched.iter().map(|(i, v)| (*i, (v.0, v.1))).collect();
            cr.ch.choices = cr
                .track
                .choices
                .iter()
                .filter(|x| w.tracked_i.contains(&x.0))
                .cloned()
                .collect();
            let took_effect = cr.ch.took == Took::Plan
                || (cr.ch.alt.is_some() && cr.ch.took == Took::Alt(cr.ch.alt.clone().unwrap()));
            if ["spent", "mismatch", "limit", "refuted"].contains(&cr.ch.outcome.as_str())
                && w.before_input.iter().all(|i| cr.track.is_matched(*i))
                && took_effect
            {
                cr.ch.outcome = "deviation".into();
                let dev_stop = cb.status().frame;
                cr.ch.dev_stop = Some(dev_stop);
                while cb.status().frame - dev_stop < w.cfg.window_frames {
                    if click_gate(cb.status()) {
                        break;
                    }
                    if w.board.cancelled(c) {
                        cr.ch.outcome = "cancelled".into();
                        break;
                    }
                    let qf = cb.status().qf;
                    let pr = if w.ready {
                        false
                    } else {
                        match w.oracle.and_then(|o| o.pruned(cr.ch.g, cb.status().frame, qf)) {
                            Some(p) => p,
                            None => w.board.pruned(c, qf),
                        }
                    };
                    if pr {
                        cr.ch.outcome = "pruned".into();
                        break;
                    }
                    let nf = cb.status().frame + 1;
                    match cb.run(&RunSpec {
                        frame: Some(nf),
                        ..Default::default()
                    }) {
                        Ok(_) => {}
                        Err(EngineError::Ended(st)) => {
                            cr.ch.set("ended", Some(Py::s(st.reason.clone())));
                            break;
                        }
                        Err(e) => return Err(e),
                    }
                    w.board.progress(c, cb.status().qf);
                }
                if cr.ch.outcome == "deviation" && cb.status().frame != dev_stop {
                    cb.runs()?;
                    cb.explain_log()?;
                    s = Arc::new(cb.obs(&ObsReq::default())?);
                }
                cr.ch.end = Some(cb.status().frame);
                cr.ch.q_end = Some(cb.status().qf);
            }
            if cr.ch.is_end_point() {
                if w.cfg.lite {
                    s = Arc::new(cb.obs(&ObsReq::default())?);
                }
                let obs = facts_of(&s);
                cr.ch.checkpoints = vec![
                    replay::checkpoint(s_issue, facts_of(&d_issue), &d_issue, &st_issue),
                    replay::checkpoint(cb.status().frame, obs.clone(), &s, cb.status()),
                ];
                if cr.ch.full_at_fork.is_none() {
                    if let Some(d0) = &cr.ch.d0 {
                        cr.ch.full_at_fork = Some(w.full_fv.view(&facts_of(d0)));
                    }
                }
                let full = w.full_fv.view(&obs);
                let faf = cr.ch.full_at_fork.clone().unwrap_or_default();
                cr.ch.unplanned = replay::unplanned(&faf, &full, &w.predicted_keys)
                .into_iter()
                .take(40)
                .map(|x| {
                    crate::jobj! {"key" => x.key.text(), "start" => replay::vstr_j(faf.get(x.key)), "changes" => J::Arr(vec![]),
                    "count" => 1i64, "end" => replay::vstr_j(&x.observed), "net" => true}
                })
                .collect();
                let floor = if cr.ch.outcome != "deviation" {
                    w.b_qf
                } else {
                    w.b_dev_qf
                };
                if cr.ch.q_end.unwrap() < cr.ch.q0 + floor {
                    cr.ch.bound_violation = Some(J::Arr(vec![
                        J::Int(cr.ch.g as i64),
                        J::Int(cr.ch.q0 as i64),
                        J::Int(cr.ch.q_end.unwrap() as i64),
                        J::Int(floor as i64),
                        J::Str(cr.ch.outcome.clone()),
                    ]));
                }
                // the branches an end point keeps: its snapshot (the last input's frame) and its live branch
                cr.ch.snapshot = snap.take();
            }
            Ok(())
        })();
        cr.ch.track = Some(cr.track.clone());
        cr.ch.split_runs = cr.log.splits();
        let CRun { ch: chx, .. } = cr;
        ch = chx;
        inner
    })();
    if let Err(e) = out {
        ch.set("error", Some(err_why(&e)));
    }
    ch.cpu_s += cb.cpu_s();
    if ch.is_end_point() {
        ch.live = Some(cb);
    } else {
        cb.quit();
    }
    if let Some(s) = snap {
        ch.cpu_s += s.cpu_s();
        s.quit();
    }
    ch
}

/// E010: a try whose first event lands on the first event of a try resolved as failed is the same run (Lemma 5);
/// in fork order, as the ready rule resolves them.
fn ready_duplicate(w: &W, c: usize, first: u64) -> bool {
    let _ = c;
    w.ready_failed_firsts.lock().unwrap().contains(&first)
}

// --- the window ------------------------------------------------------------------------------------------------------

/// What window_strict returns.
pub struct WinRes {
    pub rec: J,
    /// every child's fork frame and row, in fork order (before the best's fork frame is moved to its run's earliest)
    pub rows: Vec<(u64, J)>,
    pub best: Option<Child>,
    pub track: Track,
    pub cause: Option<J>,
    pub parent_matched: Vec<(usize, (u64, usize))>,
    pub parent: Option<Box<dyn Engine>>,
}

/// E010's _Ready: a window's tries in fork order.
struct Ready {
    workers: usize,
    max_failed: u32,
    limit: usize,
    cursor: usize,
    next: Option<u64>,
    failed: u32,
    chosen: Option<usize>,
    stopped: bool,
    order: Vec<J>,
    cancelled: u32,
    parallel_from: Option<u64>,
}

impl Ready {
    fn may_fork(&self, g: u64) -> bool {
        self.chosen.is_none() && !self.stopped && self.next.is_none_or(|n| g >= n)
    }
}

/// A frame where section 9.3's stop is still open: the parent goes on past it, and the window ends there if the
/// children forked before it give the stop (the parent's record as it stood there is kept to go back to).
struct StopCand {
    g: u64,
    qf: u64,
    n: usize,
    status: Status,
    lens: (usize, usize, usize, usize),
    splits: u64,
    gate_shut: u64,
    open: J,
}

/// The first open stop frame the children decide (in frame order); `wait`: wait until each is decided.
fn resolve_stops(
    board: &Board,
    cands: &mut VecDeque<StopCand>,
    b_qf: u64,
    b_dev_qf: u64,
    wait: bool,
) -> Option<(StopCand, bool)> {
    while let Some(c) = cands.front() {
        let bound = c.qf + b_qf;
        let got = if wait {
            Some(board.best_before(c.n, bound))
        } else {
            board.best_before_now(c.n, bound)
        };
        let Some(best) = got else { return None };
        let c = cands.pop_front().unwrap();
        if let Some((bq, bdev, _)) = best {
            if strict_stop(bq, bdev, c.qf, b_qf, b_dev_qf) {
                cands.clear();
                return Some((c, bdev));
            }
        }
    }
    None
}

fn r_ok<T>(r: &Result<T, String>) -> bool {
    r.is_ok()
}

/// The parent's frame: the window's facts at it.
struct PFrame {
    st: Status,
    s: Arc<Obs>,
    now: View,
}

impl Search {
    /// window.Search.window_strict: one window from the parent's frame (sections 5 and 9). The parent is the
    /// harness's branch; it is left at the window's end.
    #[allow(clippy::too_many_arguments)]
    pub fn window_strict(
        &mut self,
        rd: &PlayReading,
        code: &Code,
        sp: &Plan,
        recs: &[MRec],
        members: &[usize],
        tracked: &[usize],
        needed: &FxHashSet<usize>,
        parent: &mut Box<dyn Engine>,
        goal: &[(Key, Vec<Val>)],
        kept_now: &View,
        plan_no: u32,
        k: usize,
        unobserved: &Prefixes,
    ) -> Result<WinRes, String> {
        let cfg = self.cfg.clone();
        let t0 = Instant::now();
        let (cpu_self0, cpu_parent0) = (process_cpu_s(), parent.cpu_s());
        let first_i = members[0];
        let first = &recs[first_i];
        let has_input = first.kind == Kind::Input;
        let inp = if has_input {
            Some(replay::step_input(&first.whats[0])?)
        } else {
            None
        };
        // the click's other records (strict alternatives)
        let mut alts: Vec<Alt> = Vec::new();
        if has_input && inp.as_ref().unwrap().kind == "click" {
            let own: FxHashSet<&str> = first.paths.iter().map(|p| p.name.as_str()).collect();
            let src = &first.whats[0].src_text;
            for &i in rd.by_source().get(src).map(|v| v.as_slice()).unwrap_or(&[]) {
                let a = rd.action(i);
                if own.contains(a.name.as_str()) {
                    continue;
                }
                let eff = &rd.b1(i).eff;
                let keys: Vec<Key> = eff.iter().map(|e| e.0).filter(|k| k.kind() != K_POS).collect();
                let linked = later_reads(recs, first.i, &keys);
                alts.push(Alt {
                    name: a.name.clone(),
                    idx: i,
                    units: eff
                        .iter()
                        .filter(|e| e.0.kind() == K_POS)
                        .map(|e| Unit::of_key(e.0))
                        .collect(),
                    linked,
                });
            }
        }
        let mut reads: FxHashSet<Key> = FxHashSet::default();
        for &j in members {
            reads.extend(recs[j].reads.iter().copied());
        }
        let mut extra: FxHashSet<Key> = FxHashSet::default();
        for a in &alts {
            let r = rd.b1(a.idx);
            extra.extend(r.pre.iter().map(|p| p.0));
            extra.extend(r.eff.iter().map(|p| p.0));
        }
        let mut keys: FxHashSet<Key> = FxHashSet::default();
        for &i in tracked {
            keys.extend(recs[i].writes.iter().copied());
            keys.extend(recs[i].reads.iter().copied());
            if let Some(u) = recs[i].unit {
                keys.insert(u.pos());
            }
        }
        keys.extend(reads.iter().copied());
        keys.extend(goal.iter().map(|g| g.0));
        keys.insert(Key::pack(K_ROOM, 0, 0, 0));
        keys.extend(extra.iter().copied());
        let mut args_units: Vec<Unit> = keys
            .iter()
            .filter(|k| k.kind() == K_ARGS)
            .map(|k| Unit::of_key(*k))
            .collect();
        args_units.sort_by_cached_key(|u| u.text());
        args_units.dedup();
        for u in &args_units {
            keys.insert(u.pos());
        }
        let doms: Vec<(Key, &Domain)> = keys
            .iter()
            .filter_map(|k| rd.dom(*k).map(|d| (*k, d)))
            .filter(|(k, _)| !unobserved.has(*k))
            .collect();
        let dom_keys: FxHashSet<Key> = doms.iter().map(|d| d.0).collect();
        let fv = FastView::new(rd, doms.iter().copied(), unobserved);
        let av = ArgsView::new(rd, args_units.clone());
        let full_fv = FastView::new(rd, rd.doms.0.iter().map(|(k, d)| (*k, d)), unobserved);
        let observed_reads = reads.iter().filter(|k| dom_keys.contains(k)).count();
        let before_input: FxHashSet<usize> = tracked
            .iter()
            .copied()
            .filter(|&i| has_input && first.has_pred(i))
            .collect();
        let mut input_reads: Vec<Key> = if has_input {
            first.reads.iter().copied().filter(|k| dom_keys.contains(k)).collect()
        } else {
            vec![]
        };
        sort_keys(&mut input_reads);
        let issued: FxHashSet<usize> = tracked
            .iter()
            .copied()
            .filter(|&i| recs[i].kind == Kind::Input && recs[i].macro_ < k)
            .collect();
        let n_clicks = if has_input {
            replay::input_clicks(&first.whats[0])
        } else {
            0
        };
        let (chain, chain_names) = if has_input {
            chain_frames(recs, members, cfg.split_wait)
        } else {
            (0, vec![])
        };
        let b_frames = n_clicks as i64 + chain;
        let b_qf = (4 * cfg.delta_lo * b_frames) as u64;
        let b_dev_qf = (4 * cfg.delta_lo * n_clicks as i64) as u64;
        let goal_writers: FxHashSet<usize> = tracked
            .iter()
            .copied()
            .filter(|i| self.goal_writers.contains(i))
            .collect();
        let mut units: FxHashSet<Unit> = tracked.iter().filter_map(|&i| recs[i].unit).collect();
        let mut in_units: Vec<Vec<Unit>> = Vec::new();
        if has_input {
            for idx in &sp.idx[&first_i] {
                let us: Vec<Unit> = match idx {
                    Some(i) => rd
                        .b1(*i)
                        .eff
                        .iter()
                        .filter(|e| e.0.kind() == K_POS)
                        .map(|e| Unit::of_key(e.0))
                        .collect(),
                    None => vec![],
                };
                units.extend(us.iter().copied());
                in_units.push(us);
            }
        }
        for a in &alts {
            units.extend(a.units.iter().copied());
        }
        let mut chk_units: FxHashSet<Unit> = FxHashSet::default();
        if has_input {
            let idxs: Vec<u32> = sp.idx[&first_i]
                .iter()
                .flatten()
                .copied()
                .chain(alts.iter().map(|a| a.idx))
                .collect();
            for idx in idxs {
                for c_ in rd.b1(idx).checks.iter() {
                    if let Some(cu) = strict::cite_unit(&c_.cite) {
                        chk_units.insert(cu.0);
                    }
                }
            }
        }
        units.extend(chk_units.iter().copied());
        let mut us: Vec<Unit> = units.into_iter().collect();
        us.sort_by_cached_key(|u| u.text());
        let explain_units: Vec<(i32, i32)> = us.iter().flat_map(|&u| strict::unit_where(u)).collect();
        let mut lite: Vec<u32> = dom_keys
            .iter()
            .filter(|k| matches!(k.kind(), K_OWNER | K_STATE | K_CLASS))
            .map(|k| k.a() as u32)
            .chain(args_units.iter().filter(|u| u.0 == U_OBJ).map(|u| u.1 as u32))
            .collect();
        lite.sort();
        lite.dedup();
        let mut pm = Matcher::new(recs, tracked, needed, None, &[], &issued);
        pm.plan_pending = recs.iter().filter(|r| !r.done).map(|r| r.i).collect();
        let predicted_keys: FxHashSet<Key> = tracked
            .iter()
            .flat_map(|&i| recs[i].exps.iter().flat_map(|e| e.iter().map(|x| x.0)))
            .collect();
        let early_recs: FxHashSet<usize> = tracked
            .iter()
            .copied()
            .filter(|&i| !(has_input && recs[i].has_pred(first_i)) && i != first_i)
            .collect();
        let mut needed_order: Vec<usize> = needed.iter().copied().collect();
        needed_order.sort();
        let w = W {
            cfg: &cfg,
            rd,
            code,
            sp,
            recs,
            tracked: tracked.to_vec(),
            needed: needed.clone(),
            needed_order,
            first: if has_input { Some(first_i) } else { None },
            inp: inp.clone(),
            name: first.name.clone(),
            goal,
            unobserved,
            goal_writers: goal_writers.clone(),
            plan_pending: pm.plan_pending.clone(),
            issued: issued.clone(),
            before_input: before_input.clone(),
            alts,
            in_units,
            b_qf,
            b_dev_qf,
            explain_units,
            chk_units,
            predicted_keys,
            lite_objects: lite,
            tracked_i: tracked.iter().copied().collect(),
            early_recs,
            fv,
            av,
            full_fv,
            board: Board::new(),
            ready: cfg.ready && has_input,
            ready_failed_firsts: Mutex::new(FxHashSet::default()),
            oracle: self.oracle,
            prof: Prof::default(),
            hit_cache: self.hit_cache.as_deref(),
        };
        let mut rec = J::Obj(vec![]);
        let rr = |rec: &mut J, k: &str, v: J| rec.set(k, v);
        rr(&mut rec, "plan", J::Int(plan_no as i64));
        rr(&mut rec, "macro", J::Int(k as i64));
        rr(
            &mut rec,
            "input",
            if has_input { J::Str(first.name.clone()) } else { J::Null },
        );
        rr(&mut rec, "strict", J::Bool(true));
        rr(
            &mut rec,
            "records",
            J::Arr(members.iter().map(|&j| J::Str(recs[j].name.clone())).collect()),
        );
        rr(
            &mut rec,
            "needed",
            J::Arr(
                tracked
                    .iter()
                    .filter(|i| needed.contains(i))
                    .map(|&i| J::Str(recs[i].name.clone()))
                    .collect(),
            ),
        );
        rr(
            &mut rec,
            "tracked",
            J::Arr(tracked.iter().map(|&i| J::Str(recs[i].name.clone())).collect()),
        );
        rr(&mut rec, "start", J::Int(parent.status().frame as i64));
        rr(&mut rec, "start_qf", J::Int(parent.status().qf as i64));
        rr(&mut rec, "b_frames", J::Int(b_frames));
        rr(&mut rec, "b_qf", J::Int(b_qf as i64));
        rr(&mut rec, "n_clicks", J::Int(n_clicks as i64));
        rr(&mut rec, "chain", J::Arr(chain_names.into_iter().map(J::Str).collect()));
        rr(&mut rec, "delta_lo", J::Int(cfg.delta_lo));
        rr(&mut rec, "observed_reads", J::Int(observed_reads as i64));
        rr(&mut rec, "before_input", J::Int(before_input.len() as i64));
        rr(&mut rec, "open", J::Null);
        rr(&mut rec, "alternatives", J::Int(w.alts.len() as i64));
        rr(&mut rec, "args_units", J::Int(args_units.len() as i64));
        if cfg.split_wait {
            rr(&mut rec, "split_wait", J::Bool(true));
        }
        // the parent: segment on, explain on every unit
        let pf = parent.status().frame;
        parent.segment(true).map_err(|e| e.to_string())?;
        parent.runs().map_err(|e| e.to_string())?;
        parent
            .explain(Some(&ExplainArm {
                from: pf + 1,
                to: pf + cfg.window_frames + cfg.child_limit + 2,
                units: vec![],
            }))
            .map_err(|e| e.to_string())?;
        parent.explain_log().map_err(|e| e.to_string())?;
        let mut ptrack = self.strack.clone();
        let mut plog = BranchLog::new(cfg.split_wait);
        let s0 = Arc::new(parent.obs(&ObsReq::default()).map_err(|e| e.to_string())?);
        let now0 = w.sview(&s0)?;
        let mut pfr = PFrame {
            st: parent.status().clone(),
            s: s0.clone(),
            now: now0,
        };
        let mut prev_view = Arc::new(pfr.now.clone());
        let mut prev_dump = s0.clone();
        plog.kept.frame(pf, vec![], Some(s0), Some(prev_view.clone()));
        let mut end_rule = EndRule::default();
        let mut prev = View::new();
        let mut end_reason = String::new();
        let mut gate_shut = 0u64;
        let w0 = pf;
        let mut parent_fail: Option<Fail> = None;
        let mut pgv: Option<View> = None;
        let mut children: Vec<Option<Child>> = Vec::new();
        let mut child_g: Vec<u64> = Vec::new();
        let mut delta_violation: Option<J> = None;
        let mut ready = Ready {
            workers: cfg.workers,
            max_failed: cfg.ready_tries,
            limit: 1,
            cursor: 0,
            next: None,
            failed: 0,
            chosen: None,
            stopped: false,
            order: vec![],
            cancelled: 0,
            parallel_from: None,
        };
        let pool = Pool::new(cfg.workers.max(1));
        let mut cands: VecDeque<StopCand> = VecDeque::new();
        let mut stop_at: Option<(StopCand, bool)> = None;
        let wref = &w;
        let res: Result<(), String> = std::thread::scope(|scope| {
            let results: Arc<Mutex<Vec<(usize, Child)>>> = Arc::new(Mutex::new(Vec::new()));
            for _ in 0..cfg.workers.max(1) {
                let pool = &pool;
                let results = results.clone();
                scope.spawn(move || {
                    while let Some((c, eng, ch)) = pool.take() {
                        let out = child_strict(wref, c, eng, ch);
                        let endk = if out.is_end_point() { Some(out.end_key()) } else { None };
                        wref.board.finish(c, endk, out.q_end);
                        results.lock().unwrap().push((c, out));
                        pool.done();
                    }
                });
            }
            let collect = |children: &mut Vec<Option<Child>>| {
                let mut r = results.lock().unwrap();
                for (c, ch) in r.drain(..) {
                    children[c] = Some(ch);
                }
            };
            let out = (|| -> Result<(), String> {
                loop {
                    if !cands.is_empty() {
                        if let Some(s) = resolve_stops(&w.board, &mut cands, b_qf, b_dev_qf, false) {
                            stop_at = Some(s);
                            break;
                        }
                    }
                    let st = pfr.st.clone();
                    let g = st.frame;
                    let now = pfr.now.clone();
                    if cfg.delta_lo > 1 && (st.v19 as i64).max(1) < cfg.delta_lo {
                        delta_violation = Some(J::Arr(vec![J::Int(g as i64), J::Int(st.v19 as i64)]));
                        end_reason = "delta".into();
                        break;
                    }
                    if let Some(&i) = w.needed_order.iter().find(|&&i| ptrack.is_refuted(i)) {
                        let (f_, why_, run_) = ptrack.r(i).unwrap().clone();
                        parent_fail = Some(Fail::new(
                            "refuted",
                            Some(recs[i].name.clone()),
                            f_,
                            why_,
                            Some(run_),
                            plog.kept.freeze(&[], &[]),
                        ));
                        end_reason = "a needed record refuted in the parent".into();
                        break;
                    }
                    if w.holds_goal(&now) {
                        let credited = !goal_writers.is_empty() && goal_writers.iter().all(|i| ptrack.is_matched(*i));
                        end_reason = format!(
                            "goal in the parent{}",
                            if credited { "" } else { ", not from the plan" }
                        );
                        if !credited {
                            parent_fail = self.goal_writer_fail(code, &plog, g, pgv.as_ref(), &now, goal);
                        }
                        break;
                    }
                    pgv = Some(now.clone());
                    let mut pre_now = false;
                    if !has_input {
                        if needed.iter().all(|i| ptrack.is_matched(*i)) {
                            end_reason = "matched in the parent".into();
                            break;
                        }
                    } else {
                        let mut belief = now.clone();
                        for (kk, v) in &kept_now.m {
                            if kk.kind() != K_ARGS {
                                belief.set(*kk, v.clone());
                            }
                        }
                        pre_now = first
                            .paths
                            .iter()
                            .any(|a| replay::holds_all(&a.pre, &belief, unobserved));
                        let ready_ = before_input.iter().all(|i| ptrack.is_matched(*i));
                        if end_rule.closed(ready_, pre_now) {
                            end_reason = "the input's precondition closed in the parent after its window opened".into();
                            let ch: Vec<J> = input_reads
                                .iter()
                                .filter(|kk| prev.get(**kk) != now.get(**kk))
                                .take(8)
                                .map(|kk| J::Arr(vec![J::Str(kk.text()), prev.get(*kk).j(), now.get(*kk).j()]))
                                .collect();
                            rec.set("end_change", J::Arr(ch));
                            break;
                        }
                        if rec.get("open").is_some_and(|o| o.is_null()) && end_rule.open {
                            rec.set("open", J::Int(g as i64));
                        }
                        prev = now.clone();
                        if !w.ready {
                            let n = children.len();
                            match w.oracle.and_then(|o| o.parent_stop(g)) {
                                // (the test oracle: the recorded run's stop; the kind from the children here)
                                Some(true) => {
                                    let (_, bdev, _) = w.board.best_before(n, u64::MAX - 1).unwrap_or((0, false, 0));
                                    end_reason = format!(
                                        "stop: Q(g) + b reached the best {}",
                                        if bdev { "deviation" } else { "success" }
                                    );
                                    break;
                                }
                                Some(false) => {}
                                None => {
                                    cands.push_back(StopCand {
                                        g,
                                        qf: st.qf,
                                        n,
                                        status: st.clone(),
                                        lens: (
                                            ptrack.matched.len(),
                                            ptrack.refuted.len(),
                                            ptrack.choices.len(),
                                            ptrack.outside.len(),
                                        ),
                                        splits: plog.splits(),
                                        gate_shut,
                                        open: rec.get("open").cloned().unwrap_or(J::Null),
                                    });
                                    if let Some(s) = resolve_stops(&w.board, &mut cands, b_qf, b_dev_qf, false) {
                                        stop_at = Some(s);
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    if g - w0 >= cfg.window_frames {
                        end_reason = format!("{} frames", cfg.window_frames);
                        break;
                    }
                    if has_input {
                        let gate = self
                            .gate(inp.as_ref().unwrap(), &st, &pfr.s, parent)
                            .map_err(|e| e.to_string())?;
                        let mut turn = gate;
                        if w.ready {
                            self.resolve(&w, &mut ready, &mut children, &child_g, &collect);
                            while gate
                                && ready.may_fork(g)
                                && (child_g.len() - ready.cursor >= ready.limit || pool.running() >= cfg.workers)
                            {
                                std::thread::sleep(std::time::Duration::from_micros(200));
                                collect(&mut children);
                                self.resolve(&w, &mut ready, &mut children, &child_g, &collect);
                            }
                            if ready.chosen.is_some() {
                                end_reason = "ready: the first end point in the order of tries".into();
                                break;
                            }
                            turn = gate && ready.may_fork(g);
                        }
                        if turn {
                            while pool.running() + pool.queued() >= cfg.workers + 2 && stop_at.is_none() {
                                std::thread::sleep(std::time::Duration::from_micros(100));
                                stop_at = resolve_stops(&w.board, &mut cands, b_qf, b_dev_qf, false);
                            }
                            if stop_at.is_some() {
                                break;
                            }
                            while mem_available_kb() < cfg.mem_floor_kb {
                                if pool.running() + pool.queued() == 0 {
                                    return Err(format!(
                                        "memory: MemAvailable {} MB, under {} MB",
                                        mem_available_kb() / 1024,
                                        cfg.mem_floor_kb / 1024
                                    ));
                                }
                                std::thread::sleep(std::time::Duration::from_millis(50));
                            }
                            let cb = parent.fork().map_err(|e| e.to_string())?;
                            let mut ch = Child::new(
                                g,
                                st.qf,
                                !pre_now,
                                ptrack.matched.iter().map(|x| x.0).collect(),
                                Arc::new(now.clone()),
                            );
                            ch.track = Some(ptrack.clone());
                            ch.d0 = Some(pfr.s.clone());
                            let c = w.board.add(st.qf);
                            children.push(None);
                            child_g.push(g);
                            pool.put(c, cb, ch);
                        } else if !gate {
                            gate_shut += 1;
                        }
                    }
                    // the parent's next frame
                    match parent.run(&RunSpec {
                        frame: Some(g + 1),
                        ..Default::default()
                    }) {
                        Ok(_) => {}
                        Err(EngineError::Ended(st)) => {
                            end_reason = format!("engine ended: {}", st.reason);
                            break;
                        }
                        Err(e) => return Err(e.to_string()),
                    }
                    let rows = parent.runs().map_err(|e| e.to_string())?;
                    plog.kept.add_entries(&strict::ents_of(
                        &parent.explain_log().map_err(|e| e.to_string())?.entries,
                    ));
                    let s = Arc::new(parent.obs(&ObsReq::default()).map_err(|e| e.to_string())?);
                    let r_ = strict::by_frame(&strict::runs_of(&rows.runs))
                        .remove(&(g + 1))
                        .unwrap_or_default();
                    let nowv = w.sview(&s)?;
                    let vs = ptrack.verdicts(sp, code, &r_, &prev_view, &nowv, Some(&prev_dump), &mut plog);
                    for (run_, v_) in vs {
                        ptrack.apply(sp, recs, rd, g + 1, &run_, &v_, &nowv, Some(&s));
                    }
                    let nowa = Arc::new(nowv.clone());
                    plog.kept.frame(g + 1, r_, Some(s.clone()), Some(nowa.clone()));
                    prev_view = nowa;
                    prev_dump = s.clone();
                    pfr = PFrame {
                        st: parent.status().clone(),
                        s,
                        now: nowv,
                    };
                }
                Ok(())
            })();
            if r_ok(&out) && stop_at.is_none() && !cands.is_empty() {
                stop_at = resolve_stops(&w.board, &mut cands, b_qf, b_dev_qf, true);
            }
            if let Some((c, _)) = &stop_at {
                for (i, &gc) in child_g.iter().enumerate() {
                    if gc >= c.g {
                        w.board.cancel(i);
                    }
                }
            }
            pool.close();
            // the workers end when the queue is empty (the scope joins them)
            let r = out;
            while pool.running() + pool.queued() > 0 {
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            collect(&mut children);
            r
        });
        res?;
        let mut children: Vec<Child> = children.into_iter().map(|c| c.expect("every child returns")).collect();
        let mut pend = parent.status().clone();
        if let Some((c, bdev)) = stop_at {
            // section 9.3's stop at an earlier frame than the parent reached: the window ends there
            end_reason = format!(
                "stop: Q(g) + b reached the best {}",
                if bdev { "deviation" } else { "success" }
            );
            for ch in children.iter_mut().filter(|ch| ch.g >= c.g) {
                for e in [ch.snapshot.take(), ch.live.take()].into_iter().flatten() {
                    e.quit();
                }
            }
            children.retain(|ch| ch.g < c.g);
            ptrack.matched.truncate(c.lens.0);
            ptrack.refuted.truncate(c.lens.1);
            ptrack.choices.truncate(c.lens.2);
            ptrack.outside.truncate(c.lens.3);
            if let Some(p) = plog.parter.as_mut() {
                p.splits = c.splits;
            }
            gate_shut = c.gate_shut;
            rec.set("open", c.open.clone());
            if let J::Obj(o) = &mut rec {
                o.retain(|x| x.0 != "end_change");
            }
            parent_fail = None;
            delta_violation = None;
            pend = c.status.clone();
        }
        // the window's choice
        let best_i: Option<usize> = if w.ready {
            self.resolve_final(&w, &mut ready, &mut children)
        } else {
            (0..children.len())
                .filter(|&i| children[i].is_end_point())
                .min_by_key(|&i| children[i].end_key())
        };
        for (i, c) in children.iter_mut().enumerate() {
            if Some(i) != best_i {
                if let Some(e) = c.snapshot.take() {
                    e.quit();
                }
                if let Some(e) = c.live.take() {
                    e.quit();
                }
            }
        }
        // the counters the reference keeps in `w`, in fork order
        let mut failures: Vec<((String, Option<String>, String), u64)> = Vec::new();
        let mut failure_of: Vec<((String, Option<String>, String), Fail)> = Vec::new();
        let mut lemma9 = 0u64;
        let mut bound_violations: Vec<J> = Vec::new();
        let (mut split_children, mut split_runs) = (0u64, 0u64);
        for c in &children {
            if let Some(f) = &c.fail {
                let sig = sig_of(f);
                match failures.iter_mut().find(|x| x.0 == sig) {
                    Some(x) => x.1 += 1,
                    None => {
                        failures.push((sig.clone(), 1));
                        failure_of.push((sig, f.clone()));
                    }
                }
            }
            lemma9 += c.lemma9 as u64;
            if let Some(b) = &c.bound_violation {
                bound_violations.push(b.clone());
            }
            if let Some(d) = &c.delta_violation {
                delta_violation = Some(d.clone());
            }
            split_children += c.split_child as u64;
            split_runs += c.split_runs;
            self.cpu_child += c.cpu_s;
            self.stone_trials += c.stone_trials;
        }
        let mut cause: Option<J> = None;
        let fail: Option<Fail> = match best_i {
            Some(b) if children[b].outcome == "deviation" => children[b].fail.clone(),
            Some(_) => None,
            None => parent_fail.clone().or_else(|| {
                // Counter.most_common(1): the greatest count, the first among equals
                let top =
                    failures.iter().fold(
                        None::<&(_, u64)>,
                        |b, d| if b.is_none_or(|b| d.1 > b.1) { Some(d) } else { b },
                    );
                top.and_then(|t| failure_of.iter().find(|x| x.0 == t.0).map(|x| x.1.clone()))
            }),
        };
        if let Some(f) = &fail {
            cause = Some(strict::explain(code, rd, f));
        }
        let rows: Vec<(u64, J)> = children.iter().map(|c| (c.g, c.row())).collect();
        // the earliest fork of the same run (Lemma 5): window.py moves the best's fork frame there
        if let Some(b) = best_i {
            if let Some(f) = children[b].first {
                let g0 = children
                    .iter()
                    .filter(|c| c.first == Some(f) && c.outcome != "interface")
                    .map(|c| c.g)
                    .min();
                if let Some(g0) = g0 {
                    children[b].g = children[b].g.min(g0);
                }
            }
        }
        let mut outcomes: BTreeMap<String, u64> = BTreeMap::new();
        for c in &children {
            *outcomes.entry(c.outcome.clone()).or_default() += 1;
        }
        let mut other_paths: Vec<(String, u64)> = Vec::new();
        for c in &children {
            if let Some(a) = &c.alt {
                match other_paths.iter_mut().find(|x| x.0 == *a) {
                    Some(x) => x.1 += 1,
                    None => other_paths.push((a.clone(), 1)),
                }
            }
        }
        let mut mk: Vec<(String, u64)> = Vec::new();
        for c in &children {
            if c.outcome == "mismatch" {
                if let Some(Py::Tuple(t)) = &c.why {
                    let k0 = t[0].str();
                    match mk.iter_mut().find(|x| x.0 == k0) {
                        Some(x) => x.1 += 1,
                        None => mk.push((k0, 1)),
                    }
                }
            }
        }
        let most_common = |v: &[(String, u64)], n: usize| -> Vec<(String, u64)> {
            let mut idx: Vec<usize> = (0..v.len()).collect();
            idx.sort_by_key(|&i| std::cmp::Reverse(v[i].1));
            idx.into_iter().take(n).map(|i| v[i].clone()).collect()
        };
        let fail_mc = {
            let mut idx: Vec<usize> = (0..failures.len()).collect();
            idx.sort_by_key(|&i| std::cmp::Reverse(failures[i].1));
            idx.into_iter().take(8).map(|i| failures[i].clone()).collect::<Vec<_>>()
        };
        rec.set("end", J::Int(pend.frame as i64));
        rec.set("end_qf", J::Int(pend.qf as i64));
        rec.set("end_reason", J::Str(end_reason.clone()));
        rec.set("forks", J::Int(children.len() as i64));
        rec.set("gate_shut", J::Int(gate_shut as i64));
        rec.set(
            "outcomes",
            J::Obj(outcomes.iter().map(|(k, v)| (k.clone(), J::Int(*v as i64))).collect()),
        );
        rec.set("child_frames", J::Int(children.iter().map(|c| c.frames as i64).sum()));
        rec.set(
            "early_successes",
            J::Int(
                children
                    .iter()
                    .filter(|c| c.early && (c.outcome == "success" || c.outcome == "goal"))
                    .count() as i64,
            ),
        );
        rec.set("lemma9_stops", J::Int(lemma9 as i64));
        rec.set(
            "other_paths",
            J::Obj(
                other_paths
                    .iter()
                    .map(|(k, v)| (k.clone(), J::Int(*v as i64)))
                    .collect(),
            ),
        );
        rec.set("bound_violations", J::Arr(bound_violations));
        rec.set("delta_violation", delta_violation.clone().unwrap_or(J::Null));
        rec.set(
            "failures",
            J::Arr(
                fail_mc
                    .iter()
                    .map(|(sig, n)| {
                        J::Arr(vec![
                            J::Arr(vec![
                                J::Str(sig.0.clone()),
                                sig.1.clone().map(J::Str).unwrap_or(J::Null),
                                J::Str(sig.2.clone()),
                            ]),
                            J::Int(*n as i64),
                        ])
                    })
                    .collect(),
            ),
        );
        rec.set(
            "mismatch_keys",
            J::Arr(
                most_common(&mk, 8)
                    .into_iter()
                    .map(|(k, n)| J::Arr(vec![J::Str(k), J::Int(n as i64)]))
                    .collect(),
            ),
        );
        rec.set(
            "children",
            J::Arr(children.iter().take(CHILD_ROWS).map(Child::row).collect()),
        );
        rec.set("wall_s", J::Float((t0.elapsed().as_secs_f64() * 100.0).round() / 100.0));
        rec.set("python_cpu_s", J::Float(0.0));
        rec.set("cause", cause.clone().unwrap_or(J::Null));
        let by = |i: usize| recs.get(i).map(|r| r.name.clone()).unwrap_or_else(|| i.to_string());
        if let Some(b) = best_i {
            let best = &children[b];
            let bt = best.track.clone().unwrap_or_default();
            let matched: Vec<(String, J)> = bt
                .matched
                .iter()
                .filter(|(i, _)| !ptrack.is_matched(*i))
                .map(|(i, v)| (by(*i), J::Int(v.0 as i64)))
                .collect();
            rec.set(
                "best",
                crate::jobj! {
                    "g" => best.g, "first" => best.first, "issue" => best.issue, "end" => best.end, "q0" => best.q0,
                    "q_end" => best.q_end, "outcome" => best.outcome.clone(), "early" => best.early, "took" => best.took.j(),
                    "frames_issue_to_end" => best.end.unwrap_or(0) as i64 - best.issue.unwrap_or(0) as i64,
                    "arrival" => best.arrival,
                    "walk_frames" => best.arrival.map(|a| a as i64 - best.issue.unwrap_or(0) as i64),
                    "matched" => J::Obj(matched),
                    "unplanned" => best.unplanned.len(),
                },
            );
        }
        if cfg.split_wait {
            rec.set(
                "parts",
                crate::jobj! {"parent" => plog.splits(), "children" => split_children, "children_runs" => split_runs},
            );
        }
        if w.ready {
            rec.set(
                "ready",
                crate::jobj! {
                    "tries" => J::Arr(ready.order.clone()), "chosen" => best_i.map(|b| children[b].g),
                    "failed" => ready.failed, "bound" => ready.max_failed, "stopped" => ready.stopped,
                    "cancelled" => ready.cancelled, "parallel_from" => ready.parallel_from, "next" => ready.next,
                    "earliest" => false,
                },
            );
        }
        rec.set(
            "parent_matched",
            J::Obj(
                ptrack
                    .matched
                    .iter()
                    .map(|(i, v)| (by(*i), J::Int(v.0 as i64)))
                    .collect(),
            ),
        );
        rec.set(
            "parent_refuted",
            J::Obj(
                ptrack
                    .refuted
                    .iter()
                    .map(|(i, v)| (by(*i), J::Arr(vec![J::Int(v.0 as i64), J::Str(v.1.clone())])))
                    .collect(),
            ),
        );
        let best_ref = best_i.map(|b| &children[b]);
        rec.set(
            "outside_domain",
            self.outside(rd, &ptrack, best_ref, if has_input { Some(first_i) } else { None }, &by),
        );
        parent.explain(None).map_err(|e| e.to_string())?;
        self.say(&format!("  window {}: {}", k + 1, w.prof.line()));
        self.say(&format!(
            "  window {}: CPU s: this process {:.2}, the parent engine {:.2}, the children's engines {:.2} ({} children); wall {:.2} s",
            k + 1,
            process_cpu_s() - cpu_self0,
            parent.cpu_s() - cpu_parent0,
            children.iter().map(|c| c.cpu_s).sum::<f64>(),
            children.len(),
            t0.elapsed().as_secs_f64()
        ));
        self.say(&format!("  window {}: {}", k + 1, w.prof.play_line()));
        self.say(&format!("  window {}: {}", k + 1, w.prof.cpu_line()));
        if let Some(hc) = &self.hit_cache {
            self.say(&format!("  window {}: {}", k + 1, hc.line()));
        }
        let best = best_i.map(|b| children.swap_remove(b));
        let parent_matched = ptrack.matched.iter().map(|(i, v)| (*i, (v.0, v.1))).collect();
        Ok(WinRes {
            rec,
            rows,
            best,
            track: ptrack,
            cause,
            parent_matched,
            parent: None,
        })
    }

    /// E010 _Ready.resolve: the ended tries in fork order.
    fn resolve(
        &self,
        w: &W,
        rd: &mut Ready,
        children: &mut [Option<Child>],
        child_g: &[u64],
        collect: &dyn Fn(&mut Vec<Option<Child>>),
    ) {
        let _ = collect;
        while rd.cursor < child_g.len() && rd.chosen.is_none() && !rd.stopped {
            let c = rd.cursor;
            if rd.next.is_some_and(|n| child_g[c] < n) {
                w.board.cancel(c);
                rd.cancelled += 1;
                rd.cursor += 1;
                continue;
            }
            let Some(ch) = &children[c] else { return };
            rd.cursor += 1;
            rd.order.push(J::Arr(vec![
                J::Int(ch.g as i64),
                ch.first.map(|x| J::Int(x as i64)).unwrap_or(J::Null),
                J::Str(ch.outcome.clone()),
            ]));
            if ch.is_end_point() {
                rd.chosen = Some(c);
                break;
            }
            let nxt = match ch.first {
                None => (ch.g + 1).max(ch.reached.unwrap_or(ch.g + 1)),
                Some(f1) => {
                    let mut ff = w.ready_failed_firsts.lock().unwrap();
                    let seen = !ff.insert(f1);
                    if !seen && ch.outcome != "duplicate" && ch.outcome != "cancelled" {
                        rd.failed += 1;
                    }
                    if rd.parallel_from.is_none() {
                        rd.parallel_from = Some(ch.g);
                        rd.limit = rd.workers;
                    }
                    if rd.failed >= rd.max_failed {
                        rd.stopped = true;
                    }
                    (ch.g + 1).max(f1.saturating_sub(1))
                }
            };
            rd.next = Some(rd.next.map_or(nxt, |n| n.max(nxt)));
        }
        if rd.chosen.is_some() || rd.stopped {
            for c in rd.cursor..child_g.len() {
                if !w.board.cancelled(c) {
                    w.board.cancel(c);
                    rd.cancelled += 1;
                }
            }
        }
    }

    fn resolve_final(&self, w: &W, rd: &mut Ready, children: &mut Vec<Child>) -> Option<usize> {
        let mut opt: Vec<Option<Child>> = children.drain(..).map(Some).collect();
        let gs: Vec<u64> = opt.iter().map(|c| c.as_ref().unwrap().g).collect();
        self.resolve(w, rd, &mut opt, &gs, &|_| {});
        children.extend(opt.into_iter().map(|c| c.unwrap()));
        rd.chosen
    }

    /// window.Search._gate (Lemma 5): the engine takes the input on the next frame.
    fn gate(&self, inp: &InputStep, st: &Status, s: &Obs, parent: &mut Box<dyn Engine>) -> EResult<bool> {
        if inp.kind == "key" {
            return Ok(true);
        }
        if st.up <= 0 {
            return Ok(false);
        }
        if inp.kind == "line" {
            let mut at = |x: i32, y: i32| parent.at(x, y);
            return Ok(replay::verb_point(inp.verb.unwrap_or(0), s, &mut at)?.is_some());
        }
        Ok(st.cs == 0 && st.room != 0 && (executor::DOS_CD.verbless_rooms.contains(&st.room) || !s.verbs.is_empty()))
    }

    /// window.Search._goal_writer_fail
    fn goal_writer_fail(
        &self,
        code: &Code,
        plog: &BranchLog,
        g: u64,
        before: Option<&View>,
        now: &View,
        goal: &[(Key, Vec<Val>)],
    ) -> Option<Fail> {
        let facts: Vec<Key> = goal
            .iter()
            .map(|x| x.0)
            .filter(|k| before.is_none_or(|b| b.get(*k) != now.get(*k)))
            .collect();
        let mat = plog.kept.freeze(&[], &[]);
        let eng = strict::eng_of(&mat);
        for run in mat.runs.get(&g).cloned().unwrap_or_default() {
            let key = code.key(run.where_, run.number, run.room);
            let lg = strict::logged(&eng, &run);
            let (Some(key), Some(lg)) = (key, lg) else { continue };
            let idxs: FxHashSet<u32> = lg.ops.iter().filter_map(|op| code.index(key, op.code)).collect();
            for fact in &facts {
                let sites = code.write_sites(key, &fact.text(), None, false);
                if sites.iter().any(|s| idxs.contains(s)) {
                    let mut f = Fail::new(
                        "goal writer",
                        None,
                        g,
                        format!("wrote {}", fact.text()),
                        Some(run),
                        mat.clone(),
                    );
                    f.fact = Some(*fact);
                    return Some(f);
                }
            }
        }
        None
    }

    /// window.Search._outside (9.2, M4)
    fn outside(
        &self,
        rd: &PlayReading,
        ptrack: &Track,
        best: Option<&Child>,
        first: Option<usize>,
        by: &dyn Fn(usize) -> String,
    ) -> J {
        let mut items: Vec<(&str, usize, Key, Option<i64>)> = Vec::new();
        for (i, l) in &ptrack.outside {
            for (k, v) in l {
                items.push(("parent", *i, *k, *v));
            }
        }
        if let Some(b) = best {
            if let Some(t) = &b.track {
                for (i, l) in &t.outside {
                    for (k, v) in l {
                        items.push(("chosen child", *i, *k, *v));
                    }
                }
                if let Some(fi) = first {
                    for (k, v) in &b.outside {
                        items.push(("chosen child", fi, *k, *v));
                    }
                }
            }
        }
        let mut seen: FxHashSet<(usize, Key)> = FxHashSet::default();
        let mut out = Vec::new();
        for (who, i, kk, v) in items {
            if !seen.insert((i, kk)) {
                continue;
            }
            let dom = rd.dom(kk);
            out.push(crate::jobj! {
                "record" => by(i), "branch" => who, "key" => kk.text(), "engine" => v,
                "domain" => dom.map(|d| J::Arr(d.values.iter().take(8).map(|&x| J::Str(vstr(x))).collect())).unwrap_or(J::Null),
                "still_matches" => true,
            });
        }
        J::Arr(out)
    }
}

// --- the worker pool ---------------------------------------------------------------------------------------------------

type Job = (usize, Box<dyn Engine>, Child);

/// Children in fork order, taken by the workers first in, first out (an earlier child always starts first, so a
/// child waiting on an earlier one never waits on a child not started).
struct Pool {
    q: Mutex<(VecDeque<Job>, bool, usize)>,
    cv: Condvar,
    running: AtomicU64,
}

impl Pool {
    fn new(_n: usize) -> Pool {
        Pool {
            q: Mutex::new((VecDeque::new(), false, 0)),
            cv: Condvar::new(),
            running: AtomicU64::new(0),
        }
    }
    fn put(&self, c: usize, e: Box<dyn Engine>, ch: Child) {
        let mut q = self.q.lock().unwrap();
        q.0.push_back((c, e, ch));
        drop(q);
        self.cv.notify_one();
    }
    fn take(&self) -> Option<Job> {
        let mut q = self.q.lock().unwrap();
        loop {
            if let Some(j) = q.0.pop_front() {
                self.running.fetch_add(1, Ordering::AcqRel);
                return Some(j);
            }
            if q.1 {
                return None;
            }
            q = self.cv.wait(q).unwrap();
        }
    }
    fn done(&self) {
        self.running.fetch_sub(1, Ordering::AcqRel);
    }
    fn close(&self) {
        self.q.lock().unwrap().1 = true;
        self.cv.notify_all();
    }
    fn running(&self) -> usize {
        self.running.load(Ordering::Acquire) as usize
    }
    fn queued(&self) -> usize {
        self.q.lock().unwrap().0.len()
    }
}

// --- the search over one plan ------------------------------------------------------------------------------------------

/// window.Search: the window search of one harness run.
pub struct Search {
    pub cfg: Config,
    pub goal_writers: FxHashSet<usize>,
    pub goal_credited: bool,
    pub strack: Track,
    pub pending_cp: Option<(Checkpoint, usize)>,
    pub replan_cause: Option<String>,
    pub replan_cause_detail: Option<J>,
    pub windows: Vec<J>,
    pub causes: Vec<J>,
    pub empty_keys: Vec<(J, u32)>,
    pub unplanned_goals: Vec<J>,
    pub deviations: Vec<J>,
    pub cpu_child: f64,
    pub stone_trials: u32,
    pub oracle: Option<&'static dyn ChildOracle>,
    pub progress: Option<Box<dyn Fn(&str) + Send>>,
    pub steps: Vec<StepLog>,
    pub plan_notes: Vec<J>,
    pub reading: Option<Arc<ReadingData>>,
    pub game: Option<Arc<scumm_ir::Game>>,
    pub movie: Vec<Event>,
    pub root: Option<Box<dyn Engine>>,
    pub windows_path: Option<std::path::PathBuf>,
    /// the hit tests' object answers (executor::HitCache), kept across windows; None: every hit test is asked
    pub hit_cache: Option<Arc<executor::HitCache>>,
}

/// window.Search._goal_writers
pub fn goal_writers_of(recs: &[MRec], goal: &[(Key, Vec<Val>)]) -> FxHashSet<usize> {
    recs.iter()
        .filter(|r| {
            r.exps.iter().any(|e| {
                e.iter().any(|(k, x)| {
                    goal.iter()
                        .find(|g| g.0 == *k)
                        .is_some_and(|g| x.values.iter().any(|v| g.1.contains(v)))
                        && x.changes
                })
            })
        })
        .map(|r| r.i)
        .collect()
}

impl Search {
    pub fn new(cfg: Config) -> Result<Search, String> {
        cfg.check()?;
        Ok(Search {
            cfg,
            goal_writers: FxHashSet::default(),
            goal_credited: false,
            strack: Track::default(),
            pending_cp: None,
            replan_cause: None,
            replan_cause_detail: None,
            windows: vec![],
            causes: vec![],
            empty_keys: vec![],
            unplanned_goals: vec![],
            deviations: vec![],
            cpu_child: 0.0,
            stone_trials: 0,
            oracle: None,
            progress: None,
            steps: vec![],
            plan_notes: vec![],
            reading: None,
            game: None,
            movie: vec![],
            root: None,
            windows_path: None,
            // off by default: the screen layout is not in the cache's key yet (play/HITCACHE.md, "The gap")
            hit_cache: if std::env::var("FMI_HIT_CACHE").is_ok_and(|v| v == "1") {
                Some(Arc::new(executor::HitCache::default()))
            } else {
                None
            },
        })
    }

    fn say(&self, m: &str) {
        if let Some(p) = &self.progress {
            p(m);
        }
    }

    /// the reading's derived data, made again when the reading changes (window._strict_setup)
    pub fn reading_data(&mut self, src: &dyn ReadingSource) -> Arc<ReadingData> {
        let id = src.id();
        if let Some(d) = &self.reading {
            if d.id == id {
                return d.clone();
            }
        }
        let d = Arc::new(ReadingData::new(src, self.cfg.unobserved_actor_rooms));
        self.reading = Some(d.clone());
        d
    }

    /// The window search for step.py's WindowConfig: `workers`, fork-each (7, or 0), `ready`, δ_lo from the reading's
    /// frame costs' dmax. Any other setting is refused by name (`Config::check`).
    pub fn for_step(workers: usize, fork_each: u32, ready: bool, dmax: Option<i64>) -> Result<Search, String> {
        let mut cfg = if ready {
            Config::e010_ready(delta_lo_of(dmax))
        } else {
            Config::e009(delta_lo_of(dmax))
        };
        cfg.workers = workers;
        cfg.fork_each = fork_each;
        Search::new(cfg)
    }

    /// window.Search.summary (its keys; the CPU of this process's search where the reference has Python's)
    pub fn summary(&self) -> J {
        let ws = &self.windows;
        let sum = |k: &str| -> i64 { ws.iter().map(|w| w.i(k).unwrap_or(0)).sum() };
        let mut outcomes: Vec<(String, i64)> = Vec::new();
        for w in ws {
            for (k, v) in w.get("outcomes").map(J::obj).unwrap_or(&[]) {
                match outcomes.iter_mut().find(|x| x.0 == *k) {
                    Some(x) => x.1 += v.int().unwrap_or(0),
                    None => outcomes.push((k.clone(), v.int().unwrap_or(0))),
                }
            }
        }
        let mut outside = Vec::new();
        for w in ws {
            for o in w.a("outside_domain") {
                let mut o = o.clone();
                o.set("plan", w.get("plan").cloned().unwrap_or(J::Null));
                o.set("macro", w.get("macro").cloned().unwrap_or(J::Null));
                outside.push(o);
            }
        }
        crate::jobj! {
            "windows" => ws.len(),
            "children" => sum("forks"),
            "child_frames" => sum("child_frames"),
            "outcomes" => J::Obj(outcomes.into_iter().map(|(k, v)| (k, J::Int(v))).collect()),
            "engine_cpu_s" => crate::jobj! {"child" => J::Float((self.cpu_child * 100.0).round() / 100.0)},
            "python_cpu_s" => J::Float(0.0),
            "forks" => crate::jobj! {"stone trial" => self.stone_trials},
            "early_successes" => sum("early_successes"),
            "bound_violations" => ws.iter().map(|w| w.a("bound_violations").len() as i64).sum::<i64>(),
            "goal_from_the_plan" => self.goal_credited,
            "unplanned_goals" => J::Arr(self.unplanned_goals.clone()),
            "memory_kb" => J::Obj(vec![]),
            "deviations" => J::Arr(self.deviations.clone()),
            "config" => self.cfg.j(),
            "causes" => J::Arr(self.causes.clone()),
            "python_cpu_by_part_s" => J::Obj(vec![]),
            "empty_windows" => J::Arr(self.empty_keys.iter().map(|(k, n)| J::Arr(vec![k.clone(), J::Int(*n as i64)])).collect()),
            "outside_domain" => J::Arr(outside),
        }
    }

    pub fn game(&mut self) -> Result<Arc<scumm_ir::Game>, String> {
        if self.game.is_none() {
            let root = scumm_ir::default_root();
            self.game = Some(Arc::new(
                scumm_ir::Game::load(&root).map_err(|e| format!("Game.load: {e}"))?,
            ));
        }
        Ok(self.game.clone().unwrap())
    }

    /// Harness.observe
    pub fn observe(&self, rd: &PlayReading, unobserved: &Prefixes, ps: &mut PlayState) -> EResult<()> {
        let s = ps.engine.obs(&ObsReq::default())?;
        let obs = facts_of(&s);
        let fv = FastView::new(rd, rd.doms.0.iter().map(|(k, d)| (*k, d)), unobserved);
        let now = fv.view(&obs);
        ps.state = s;
        ps.obs = obs;
        ps.now = view_facts(&now);
        Ok(())
    }

    /// Harness.checkpoint
    pub fn checkpoint(ps: &mut PlayState) {
        let f = ps.engine.status().frame;
        if ps.checkpoints.last().is_none_or(|c| c.frame != f) {
            let cp = replay::checkpoint(f, ps.obs.clone(), &ps.state, ps.engine.status());
            ps.checkpoints.push(cp);
        }
    }

    /// Search.flush: the pending success-frame checkpoint, if no input sent since lands on or before its frame.
    pub fn flush(&mut self, ps: &mut PlayState) {
        let Some((cp, mark)) = self.pending_cp.take() else {
            return;
        };
        if self.movie[mark..]
            .iter()
            .filter(|e| executor::is_input(e))
            .all(|e| e.frame > cp.frame)
        {
            ps.checkpoints.push(cp);
        }
    }

    fn set_branch(&mut self, ps: &mut PlayState, b: Box<dyn Engine>) {
        let old = std::mem::replace(&mut ps.engine, b);
        if self.root.is_none() {
            self.root = Some(old); // the harness's root: never quit here (step.py quits it)
        } else {
            old.quit();
        }
    }

    fn mark(recs: &mut [MRec], track: &Track) {
        for (i, (f, path, _)) in &track.matched {
            let r = &mut recs[*i];
            if !r.done {
                r.done = true;
                r.done_frame = Some(*f);
                r.path = *path;
            }
        }
    }

    fn predict_recs(
        &self,
        recs: &[MRec],
        pred0: &View,
        predicted: &Prefixes,
        ps: &mut PlayState,
        unobserved: &Prefixes,
    ) {
        let mut pred = pred0.clone();
        for r in recs {
            if r.done {
                pred = replay::predict(&pred, &r.whats[r.path], &r.before, predicted);
            }
        }
        set_pred(ps, &pred, unobserved);
    }

    fn logs(&mut self, n_plan: u32, recs: &[MRec], members: &[usize], rec: &J, best: Option<&Child>, then: &str) {
        for &j in members {
            let r = &recs[j];
            let a = &r.whats[r.path];
            let inp = replay::step_input(a).unwrap_or(InputStep {
                kind: "wait",
                verb: None,
                a: None,
                b: None,
                key: None,
            });
            let start = rec.i("start").unwrap_or(0) as u64;
            let mut log = StepLog {
                plan: n_plan,
                index: r.i as u32,
                name: r.name.clone(),
                kind: a.kind.clone(),
                input: replay::input_text(&inp),
                start,
                ..Default::default()
            };
            if r.kind == Kind::Input {
                if let Some(b) = best {
                    log.tries = vec![
                        crate::jobj! {"try" => 1i64, "kind" => "window", "start" => b.g, "issue" => b.issue,
                        "end" => b.end, "how" => b.outcome.clone(), "inputs" => J::Arr(b.events.iter().filter(|e| executor::is_input(e)).map(replay::event_j).collect())},
                    ];
                    log.inputs = b.events.iter().map(replay::event_j).collect();
                    log.issue = b.issue;
                    log.end = b.end;
                    log.unplanned = b.unplanned.clone();
                }
                log.outcome = if best.is_some_and(|b| b.outcome == "goal") {
                    "goal"
                } else if r.done || best.is_some_and(|b| b.matched.iter().any(|m| m.0 == r.i)) {
                    "matched"
                } else if best.is_some_and(|b| b.outcome == "deviation") {
                    "deviation"
                } else {
                    "window empty"
                }
                .into();
            } else {
                let f = if r.done {
                    r.done_frame
                } else {
                    best.and_then(|b| b.matched.iter().find(|m| m.0 == r.i).map(|m| m.1 .0))
                };
                let e = rec.i("end").unwrap_or(0) as u64;
                log.start = f.unwrap_or(e);
                log.end = Some(f.unwrap_or(e));
                log.tries = if f.is_some() {
                    vec![crate::jobj! {"try" => 1i64, "kind" => "window", "start" => log.start, "end" => log.start}]
                } else {
                    vec![]
                };
                log.outcome = if f.is_some() { "matched" } else { "not reached" }.into();
            }
            log.then = then.into();
            log.note = format!(
                "window {}: {}; outcomes {}",
                rec.i("macro").unwrap_or(0),
                rec.s("end_reason").unwrap_or(""),
                Py::from_j(rec.get("outcomes").unwrap_or(&J::Null)).repr()
            );
            log.seconds = if r.kind == Kind::Input {
                rec.get("wall_s")
                    .and_then(|x| if let J::Float(f) = x { Some(*f) } else { None })
                    .unwrap_or(0.0)
            } else {
                0.0
            };
            self.steps.push(log);
        }
    }

    fn note_cause(&mut self, n_plan: u32, k: usize, what: &str, cause: Option<&J>) {
        self.causes.push(
            crate::jobj! {"plan" => n_plan, "macro" => k, "what" => what, "cause" => cause.cloned().unwrap_or(J::Null)},
        );
    }

    /// window.Search.play_plan: the windows of one plan. Returns Goal, Replan (the cause in ps.replan_cause) or Stop
    /// (ps.stop_reason set).
    pub fn play_plan(&mut self, ctx: &PlanCtx<'_>, ps: &mut PlayState, n_plan: u32) -> PlayOutcome {
        match self.play_plan_inner(ctx, ps, n_plan) {
            Ok(o) => o,
            Err(e) => {
                ps.stop_reason = Some(format!("engine: {e}"));
                PlayOutcome::Stop
            }
        }
    }

    fn play_plan_inner(&mut self, ctx: &PlanCtx<'_>, ps: &mut PlayState, n_plan: u32) -> Result<PlayOutcome, String> {
        let t0 = Instant::now();
        self.replan_cause = None;
        self.replan_cause_detail = None;
        let unob_t: Vec<&'static str> = {
            let mut v = vec!["args:", "saved:"];
            if ctx.unobserved_actor_rooms {
                v.push("actorroom:");
            }
            v
        };
        let unobserved = Prefixes::of(&unob_t);
        let mut pt = vec!["pos:", "escpos:"];
        pt.extend(unob_t.iter().copied());
        let predicted = Prefixes::of(&pt);
        let data = self.reading_data(ctx.reading);
        let rd = PlayReading::new(ctx.reading, data.clone());
        let game = self.game()?;
        let code = Code::new(game).with_facts(data.facts.clone());
        let belief = {
            let mut b = facts_view(&ps.now);
            for (k, v) in ps.kept.iter() {
                b.setv(*k, *v);
            }
            b
        };
        let mut recs = build_recs(&RecsCtx {
            rd: &rd,
            result: ctx.result,
            unobserved: &unobserved,
            belief,
        });
        let macros = macro_steps(&mut recs);
        checked_keys(&mut recs, ctx.goal);
        self.goal_writers = goal_writers_of(&recs, ctx.goal);
        let pred0 = facts_view(&ps.pred);
        let mut gw: Vec<usize> = self.goal_writers.iter().copied().collect();
        gw.sort();
        self.plan_notes.push(crate::jobj! {
            "plan" => n_plan,
            "macro_steps" => J::Arr(macros.iter().map(|m| J::Arr(m.iter().map(|&j| J::Str(recs[j].name.clone())).collect())).collect()),
            "goal_writers" => J::Arr(gw.iter().map(|&i| J::Str(recs[i].name.clone())).collect()),
        });
        let sp = Plan::new(&rd, &recs);
        self.strack = Track::default();
        for (k, members) in macros.iter().enumerate() {
            if t0.elapsed().as_secs_f64() > ctx.time_limit_s {
                ps.stop_reason = Some("time limit".into());
                return Ok(PlayOutcome::Stop);
            }
            let tracked: Vec<usize> = recs.iter().filter(|r| !r.done && r.macro_ <= k).map(|r| r.i).collect();
            let needed: FxHashSet<usize> = if k == macros.len() - 1 {
                tracked.iter().copied().collect()
            } else {
                let mut n: FxHashSet<usize> = members.iter().copied().filter(|&j| !recs[j].done).collect();
                for &i in &tracked {
                    if recs[i].macro_ < k && members.iter().any(|&j| recs[j].has_pred(i)) {
                        n.insert(i);
                    }
                }
                n
            };
            self.say(&format!(
                "plan {n_plan} window {}/{}: {} at frame {} ({} records needed, {} tracked)",
                k + 1,
                macros.len(),
                recs[members[0]].name,
                ps.engine.status().frame,
                needed.len(),
                tracked.len()
            ));
            let kept_now = facts_view(&ps.kept);
            let res = self.window_strict(
                &rd,
                &code,
                &sp,
                &recs,
                members,
                &tracked,
                &needed,
                &mut ps.engine,
                ctx.goal,
                &kept_now,
                n_plan,
                k,
                &unobserved,
            )?;
            let wr = res.rec.clone();
            let cause = res.cause.clone();
            if cause.is_some() {
                self.say(&format!("  window {}: cause: {}", k + 1, strict::short(cause.as_ref())));
            }
            let ugs: Vec<&J> = wr
                .a("children")
                .iter()
                .filter(|c| c.arr().get(3).and_then(J::str) == Some("goal, not from the plan"))
                .collect();
            if !ugs.is_empty() {
                self.unplanned_goals.push(
                    crate::jobj! {"plan" => n_plan, "macro" => k, "children" => ugs.len(), "first" => ugs[0].clone()},
                );
            }
            self.say(&format!(
                "  window {}: {}; {} children {}; best {} (Q {}); {} s",
                k + 1,
                wr.s("end_reason").unwrap_or(""),
                wr.i("forks").unwrap_or(0),
                Py::from_j(wr.get("outcomes").unwrap_or(&J::Null)).repr(),
                wr.get("best")
                    .and_then(|b| b.i("end"))
                    .map(|x| x.to_string())
                    .unwrap_or("None".into()),
                wr.get("best")
                    .and_then(|b| b.i("q_end"))
                    .map(|x| x.to_string())
                    .unwrap_or("None".into()),
                crate::pyj::py_float(match wr.get("wall_s") {
                    Some(J::Float(f)) => *f,
                    _ => 0.0,
                })
            ));
            self.windows.push(wr.clone());
            ps.records.push(pyj::dumps(&wr));
            if let Some(p) = &self.windows_path {
                use std::io::Write;
                if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(p) {
                    let _ = writeln!(f, "{}", pyj::dumps(&wr));
                }
            }
            let Some(mut best) = res.best else {
                self.strack = res.track;
                Self::mark(&mut recs, &self.strack);
                self.flush(ps);
                self.observe(&rd, &unobserved, ps).map_err(|e| e.to_string())?;
                Self::checkpoint(ps);
                self.predict_recs(&recs, &pred0, &predicted, ps, &unobserved);
                let er = wr.s("end_reason").unwrap_or("").to_string();
                if er == "goal in the parent" {
                    self.goal_credited = true;
                    self.logs(n_plan, &recs, members, &wr, None, "goal");
                    return Ok(PlayOutcome::Goal);
                }
                if er == "goal in the parent, not from the plan" {
                    self.unplanned_goals.push(crate::jobj! {"plan" => n_plan, "macro" => k, "parent" => ps.engine.status().frame, "qf" => ps.engine.status().qf});
                    let mut sr = format!(
                        "goal, not from the plan: it held in the idling engine at frame {} with none of the plan's goal records matched",
                        ps.engine.status().frame
                    );
                    self.note_cause(n_plan, k, "goal, not from the plan", cause.as_ref());
                    sr += &format!("; cause: {}", strict::short(cause.as_ref()));
                    ps.stop_reason = Some(sr);
                    self.logs(n_plan, &recs, members, &wr, None, "stop");
                    return Ok(PlayOutcome::Stop);
                }
                if wr.get("delta_violation").is_some_and(|d| !d.is_null()) {
                    ps.stop_reason = Some(format!(
                        "var 19 below δ_lo at {}: Lemma 2's premise failed",
                        Py::from_j(wr.get("delta_violation").unwrap()).repr()
                    ));
                    self.logs(n_plan, &recs, members, &wr, None, "stop");
                    return Ok(PlayOutcome::Stop);
                }
                if recs[members[0]].kind != Kind::Input && er == "matched in the parent" {
                    self.logs(n_plan, &recs, members, &wr, None, "next");
                    continue;
                }
                let mut rc = format!(
                    "window empty: {er}; children {}",
                    Py::from_j(wr.get("outcomes").unwrap_or(&J::Null)).repr()
                );
                self.note_cause(n_plan, k, "window empty", cause.as_ref());
                rc += &format!("; cause: {}", strict::short(cause.as_ref()));
                self.replan_cause = Some(rc);
                self.replan_cause_detail =
                    Some(crate::jobj! {"what" => "window empty", "cause" => cause.clone().unwrap_or(J::Null)});
                let ck = |k_: &str| cause.as_ref().and_then(|c| c.get(k_)).cloned().unwrap_or(J::Null);
                let key = J::Arr(vec![
                    J::Str(recs[members[0]].name.clone()),
                    ck("kind"),
                    ck("unit"),
                    ck("offset"),
                    ck("why"),
                ]);
                let n = match self.empty_keys.iter_mut().find(|x| x.0 == key) {
                    Some(x) => {
                        x.1 += 1;
                        x.1
                    }
                    None => {
                        self.empty_keys.push((key, 1));
                        1
                    }
                };
                if n >= self.cfg.repeat_stop {
                    ps.stop_reason = Some(format!(
                        "the same empty window {n} times on {}: {}",
                        recs[members[0]].name,
                        strict::short(cause.as_ref())
                    ));
                    self.logs(n_plan, &recs, members, &wr, None, "stop");
                    return Ok(PlayOutcome::Stop);
                }
                self.logs(n_plan, &recs, members, &wr, None, "replan");
                self.set_cause_text(ps);
                return Ok(PlayOutcome::Replan);
            };
            // a success: the chosen child's snapshot (at the goal, its live branch) is the engine from here
            self.movie.extend(best.events.iter().cloned());
            self.flush(ps);
            self.strack = if best.outcome == "goal" || best.outcome == "deviation" {
                best.track.clone().unwrap_or_default()
            } else {
                best.track_issue.clone().unwrap_or_default()
            };
            Self::mark(&mut recs, &self.strack);
            if best.outcome == "deviation" {
                if let Some(s) = best.snapshot.take() {
                    s.quit();
                }
                ps.checkpoints.push(best.checkpoints[0].clone());
                if best.checkpoints.last().unwrap().frame != best.checkpoints[0].frame {
                    ps.checkpoints.push(best.checkpoints.last().unwrap().clone());
                }
                let mut b_ = best.live.take().unwrap();
                let mut ran = 0u64;
                while !click_gate(b_.status()) && ran < self.cfg.window_frames {
                    let nf = b_.status().frame + 1;
                    b_.run(&RunSpec {
                        frame: Some(nf),
                        ..Default::default()
                    })
                    .map_err(|e| format!("engine ended: {e}"))?;
                    ran += 1;
                }
                if ran > 0 {
                    let s_ = b_.obs(&ObsReq::default()).map_err(|e| e.to_string())?;
                    ps.checkpoints
                        .push(replay::checkpoint(b_.status().frame, facts_of(&s_), &s_, b_.status()));
                }
                self.set_branch(ps, b_);
                self.observe(&rd, &unobserved, ps).map_err(|e| e.to_string())?;
                self.predict_recs(&recs, &pred0, &predicted, ps, &unobserved);
                self.logs(n_plan, &recs, members, &wr, Some(&best), "replan");
                let mut rc = format!(
                    "deviation at frame {}: {}",
                    best.dev_stop.unwrap_or(0),
                    cut(&best.why.as_ref().map(|w| w.str()).unwrap_or("None".into()), 300)
                );
                self.note_cause(n_plan, k, "deviation", cause.as_ref());
                rc += &format!("; cause: {}", strict::short(cause.as_ref()));
                self.replan_cause = Some(rc);
                self.replan_cause_detail =
                    Some(crate::jobj! {"what" => "deviation", "cause" => cause.clone().unwrap_or(J::Null)});
                self.deviations.push(crate::jobj! {
                    "plan" => n_plan, "macro" => k, "frame" => best.dev_stop, "end" => best.end, "qf" => best.q_end,
                    "why" => cut(&best.why.as_ref().map(|w| w.str()).unwrap_or("None".into()), 300),
                    "replan_frame" => ps.engine.status().frame, "frames_to_the_click_gate" => ran,
                });
                self.set_cause_text(ps);
                return Ok(PlayOutcome::Replan);
            }
            let (b_, other) = if best.outcome == "goal" {
                (best.live.take().unwrap(), best.snapshot.take())
            } else {
                self.pending_cp = Some((best.checkpoints.last().unwrap().clone(), self.movie.len()));
                (best.snapshot.take().unwrap(), best.live.take())
            };
            if let Some(o) = other {
                o.quit();
            }
            ps.checkpoints.push(best.checkpoints[0].clone());
            if best.outcome == "goal" && best.checkpoints.last().unwrap().frame != best.checkpoints[0].frame {
                ps.checkpoints.push(best.checkpoints.last().unwrap().clone());
            }
            self.set_branch(ps, b_);
            self.observe(&rd, &unobserved, ps).map_err(|e| e.to_string())?;
            self.predict_recs(&recs, &pred0, &predicted, ps, &unobserved);
            let choices: Vec<(String, Vec<(Key, VV)>)> = best
                .choices
                .iter()
                .map(|(i, c)| (recs[*i].name.clone(), c.clone()))
                .collect();
            let then = if best.outcome == "goal" {
                "goal"
            } else if !choices.is_empty() {
                "replan"
            } else {
                "next"
            };
            self.logs(n_plan, &recs, members, &wr, Some(&best), then);
            if best.outcome == "goal" {
                self.goal_credited = true;
                return Ok(PlayOutcome::Goal);
            }
            if !choices.is_empty() {
                let txt = Py::Dict(
                    choices
                        .iter()
                        .map(|(n, c)| {
                            (
                                Py::s(n.clone()),
                                Py::List(
                                    c.iter()
                                        .map(|(k, v)| Py::Tuple(vec![Py::s(k.text()), v.py()]))
                                        .collect(),
                                ),
                            )
                        })
                        .collect(),
                )
                .repr();
                let mut rc = format!("a guessed write took another value than the plan's: {txt}");
                if let Some(cf) = &best.choice_fail {
                    let c_ = strict::explain(&code, &rd, cf);
                    self.note_cause(n_plan, k, "a guessed write", Some(&c_));
                    rc += &format!("; cause: {}", strict::short(Some(&c_)));
                    self.replan_cause_detail = Some(crate::jobj! {"what" => "a guessed write", "cause" => c_});
                }
                self.replan_cause = Some(rc);
                self.set_cause_text(ps);
                return Ok(PlayOutcome::Replan);
            }
        }
        self.replan_cause = Some("the plan played out without the goal".into());
        self.note_cause(
            n_plan,
            macros.len(),
            "the plan played out without the goal",
            Some(&crate::jobj! {"cause" => J::Null, "kind" => "every window succeeded and the goal does not hold"}),
        );
        self.set_cause_text(ps);
        Ok(PlayOutcome::Replan)
    }

    fn set_cause_text(&self, ps: &mut PlayState) {
        ps.replan_cause = Some(pyj::dumps(&crate::jobj! {
            "replan_cause" => self.replan_cause.clone(),
            "detail" => self.replan_cause_detail.clone().unwrap_or(J::Null),
        }));
    }
}

/// Harness._set_pred: the plan's prediction and its unobserved values (kept).
pub fn set_pred(ps: &mut PlayState, pred: &View, unobserved: &Prefixes) {
    ps.pred = view_facts(pred);
    ps.kept = Facts::from_pairs(
        pred.m
            .iter()
            .filter(|(k, _)| unobserved.has(**k))
            .map(|(k, v)| (*k, v.val().unwrap_or(NONE)))
            .collect(),
    );
}

/// A view's int values as facts (a None value as NONE).
pub fn view_facts(v: &View) -> Facts {
    Facts::from_pairs(
        v.m.iter()
            .map(|(k, x)| {
                (
                    *k,
                    match x {
                        VV::V(n) => *n,
                        VV::Set(_) => NONE,
                    },
                )
            })
            .collect(),
    )
}

/// Facts as a view.
pub fn facts_view(f: &Facts) -> View {
    let mut v = View::with_capacity(f.len());
    for &(k, x) in f.iter() {
        v.setv(k, x);
    }
    v
}

/// fmi_core::Player over the strict window search.
pub struct StrictPlayer {
    pub search: Search,
}

impl Player for StrictPlayer {
    fn observe(&mut self, ctx: &PlanCtx<'_>, ps: &mut PlayState) -> EResult<()> {
        let data = self.search.reading_data(ctx.reading);
        let rd = PlayReading::new(ctx.reading, data);
        let mut t = vec!["args:", "saved:"];
        if ctx.unobserved_actor_rooms {
            t.push("actorroom:");
        }
        self.search.observe(&rd, &Prefixes::of(&t), ps)
    }
    fn play_plan(&mut self, ctx: &PlanCtx<'_>, ps: &mut PlayState, n_plan: u32) -> PlayOutcome {
        self.search.play_plan(ctx, ps, n_plan)
    }
}

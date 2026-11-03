//! A warm task per reading (E011, plan.md "plan", item 2): `serve`'s work in one process for the whole explorer,
//! with no file in between, and kept across goals, bans and cost caps.
//!
//! What `serve` (serve.rs) does per request is done here per `request`, step for step, so that the reduced task is
//! `sas`'s for the same inputs (the test: `taskcore warm` writes `sas`'s files byte for byte from it). The
//! differences are what is kept and how the inputs come:
//!
//! * **The packed file is loaded once** (from its bytes: a mapped reading), not per server; every goal is a request's.
//! * **What is kept per (bans, goal, caps)**, for up to `keep` of them (least recently used dropped first), where
//!   `serve` keeps only the last one: the banned view's pieces, the facts (e) needs on it, the fold's counts and the
//!   first round up to (d) (`reduce::prefix`). None of these reads the start (serve.rs, "What is kept").
//! * **Caps are keyed by the actions they lower.** A caps file changes the task only through the costs it lowers
//!   (bans.rs `apply_caps`: an action's cost becomes the least cap of its label's prefix when that is lower), so two
//!   requests whose caps lower the same actions to the same costs have the same task, whatever the files' texts
//!   (`serve` builds again on any other text). In E008's 225 recorded requests the caps lowered no action.
//! * **The loaded task's costs** are set to a request's caps before anything kept for them is read, as `serve` sets
//!   them before it builds.
//! * **Inputs as values:** the start as (key, value text) pairs (the start file's lines), the bans as `bans::Ban`, the
//!   caps as (prefix, frames) pairs, the goal as taskcore's `--goal` text. Each is checked as `sas` checks the file.
//!
//! Only E009's configuration is kept warm: the strict fold, every reduction, PRODUCT_LIMIT 64, the model encoding,
//! no pick windows (`Config::default`); `request` refuses nothing else because it takes nothing else.

use crate::bans::{self, Ban, BanCounts};
use crate::cli::{counts_json, parse_goal, J};
use crate::fold::{self, FoldMode};
use crate::model::*;
use crate::reduce::{self, Counts, Enabled};
use crate::sas;
use std::collections::{HashMap, HashSet};
use std::time::Instant;

/// The warm task's settings: E009's (module docstring).
#[derive(Clone, Copy, Debug)]
pub struct Config {
    pub fold: FoldMode,
    pub reduce: Enabled,
    pub limit: u128,
    pub model: bool,
    /// how many (bans, goal, caps) are kept
    pub keep: usize,
    /// E011 round 2's faster forms of the same work (reduce/fast.rs); off: round 1's code
    pub fast: bool,
}

impl Default for Config {
    fn default() -> Config {
        Config {
            fold: FoldMode::Strict,
            reduce: Enabled { a: true, b: true, c: true, d: true, e: true, f: true },
            limit: 64,
            model: true,
            keep: 4,
            fast: false,
        }
    }
}

/// One request: `sas`'s per-replan inputs.
#[derive(Clone, Debug, Default)]
pub struct Request {
    /// taskcore's `--goal` text; None: the packed file's goal
    pub goal: Option<String>,
    /// the start file's lines: (key, value text), "" where the domain lacks the value (export_records `write_start`)
    pub start: Vec<(String, String)>,
    pub bans: Vec<Ban>,
    /// the caps file's lines (prefix, frames), each at least 1 (bans.rs `read_caps`); None: no caps file
    pub caps: Option<Vec<(String, u32)>>,
    /// `--start-check needed` (else input)
    pub needed_check: bool,
    /// the start's name in the stats (`sas` writes "file:PATH")
    pub start_name: String,
}

/// What a request gives: `sas`'s prepared task (cli.rs `Prepared`'s fields) and how it was made.
pub struct Reduced {
    pub task: Task,
    pub start: usize,
    pub stats: J,
    pub replaced: Vec<(u32, u32)>,
    /// the facts (e) needs on the banned task before the fold (the operator table's "needed")
    pub needed: Vec<String>,
    /// "built" or "kept"
    pub kept: &'static str,
    pub seconds: f64,
}

/// The kept work for one (bans, goal, caps) (serve.rs `Kept`).
struct Kept {
    key: KeyOf,
    goal: Vec<(u32, Vec<u32>)>,
    pieces: Actions,
    order: Vec<u32>,
    ban_counts: BanCounts,
    ban_seconds: f64,
    needed: Option<Vec<String>>,
    needed_after: Option<Vec<String>>,
    fold_json: String,
    bad: Option<String>,
    task: Option<Task>,
    counts: Counts,
    changed: bool,
    used: u64,
}

/// (the bans as their file's text, the goal's text, the lowered costs (action, cost))
type KeyOf = (String, Option<String>, Vec<(u32, u32)>);

/// The warm task of one packed file (module docstring).
pub struct Warm {
    pub cfg: Config,
    t: Task,
    file_costs: Vec<u32>,
    lowered: Vec<(u32, u32)>,
    reached: Option<String>,
    fact_of: HashMap<String, u32>,
    kept: Vec<Kept>,
    /// (bans text, goal) -> the facts (e) needs on the banned task (they read no cost: no caps in the key)
    needed_memo: HashMap<(String, Option<String>), Vec<String>>,
    clock: u64,
    pub file_bytes: u64,
    pub add_effects: u64,
    pub input_effects: u64,
    pub seconds_load: f64,
    pub name: String,
}

fn needed_keys(nd: &reduce::Needed, t: &Task) -> Vec<String> {
    (0..t.facts.len() as u32).filter(|&f| nd.ncount[f as usize] > 0).map(|f| t.key(f).to_string()).collect()
}

fn fold_json(fc: &fold::FoldCounts, acts: &Actions) -> String {
    let mut j = J::new();
    j.n("groups", fc.groups)
        .n("paths_in_groups", fc.grouped)
        .n("subgroups", fc.subgroups)
        .n("paths_folded", fc.folded_paths)
        .n("folded_actions", fc.folded_actions)
        .n("subgroups_with_a_hole_left_unfolded", fc.holes)
        .n("subgroups_past_128_bits_left_unfolded", fc.overflow)
        .n("subgroups_past_pre_limit_left_unfolded", fc.pre_limit)
        .n("dead_conditional_effects", fc.dead_effects);
    if fc.double_writes > 0 {
        j.n("subgroups_writing_a_fact_twice_left_unfolded", fc.double_writes);
    }
    j.n("actions_after", acts.len()).n("effects_after", acts.n_effects());
    j.done()
}

/// The bans as `export_records.write_bans` writes them (the key of what is kept, as `serve` keys on the file's text).
pub fn bans_text(bans: &[Ban]) -> String {
    let mut s = String::new();
    for b in bans {
        s.push_str(&b.label);
        for (k, v) in &b.point {
            s.push('\t');
            s.push_str(k);
            s.push('\t');
            s.push_str(v);
        }
        s.push('\n');
    }
    s
}

impl Warm {
    /// The warm task of a packed file's bytes (`reached`: the reading's `.reached` text, when it has one).
    pub fn new(bytes: &[u8], name: &str, reached: Option<String>, cfg: Config) -> Result<Warm, String> {
        let t0 = Instant::now();
        let loaded = crate::packed::load_bytes(bytes, name)?;
        let t = loaded.task;
        let file_costs = t.acts.meta.iter().map(|m| m.cost).collect();
        let fact_of = (0..t.facts.len() as u32).map(|f| (t.key(f).to_string(), f)).collect();
        let input_effects = t.acts.n_effects();
        Ok(Warm {
            cfg,
            t,
            file_costs,
            lowered: Vec::new(),
            reached,
            fact_of,
            kept: Vec::new(),
            needed_memo: HashMap::new(),
            clock: 0,
            file_bytes: loaded.file_bytes,
            add_effects: loaded.add_effects,
            input_effects,
            seconds_load: t0.elapsed().as_secs_f64(),
            name: name.to_string(),
        })
    }

    /// The loaded task (the file's costs, or the last request's caps).
    pub fn task(&self) -> &Task {
        &self.t
    }

    /// The actions `caps` lower and their new costs (bans.rs `apply_caps` on the file's costs), in action order.
    pub fn lowered_by(&self, caps: &[(String, u32)]) -> Vec<(u32, u32)> {
        if caps.is_empty() {
            return Vec::new();
        }
        let mut by: HashMap<&str, u32> = HashMap::new();
        for (p, c) in caps {
            let e = by.entry(p.as_str()).or_insert(*c);
            *e = (*e).min(*c);
        }
        // a label's prefix depends only on the label: one lookup per distinct label
        let mut cap_of: HashMap<u32, Option<u32>> = HashMap::new();
        let mut out = Vec::new();
        for (i, m) in self.t.acts.meta.iter().enumerate() {
            let cap = *cap_of.entry(m.label).or_insert_with(|| {
                let label = self.t.strs.get(m.label);
                let prefix = match label.find('#') {
                    Some(i) => Some(&label[..i]),
                    None => label.strip_suffix(')'),
                };
                prefix.and_then(|p| by.get(p).copied())
            });
            if let Some(c) = cap {
                if c < self.file_costs[i] {
                    out.push((i as u32, c));
                }
            }
        }
        out
    }

    /// The loaded task's costs: the file's, then `lowered`.
    fn set_costs(&mut self, lowered: &[(u32, u32)]) {
        if self.lowered.as_slice() == lowered {
            return;
        }
        for &(i, _) in &self.lowered {
            self.t.acts.meta[i as usize].cost = self.file_costs[i as usize];
        }
        for &(i, c) in lowered {
            self.t.acts.meta[i as usize].cost = c;
        }
        self.lowered = lowered.to_vec();
    }

    /// The start's values per fact and the keys it names (bans.rs `read_start` on the file's lines).
    pub fn start_values(&self, lines: &[(String, String)]) -> Result<(Vec<u32>, HashSet<String>), String> {
        let t = &self.t;
        let mut by: HashMap<&str, &str> = HashMap::with_capacity(lines.len());
        for (n, (k, v)) in lines.iter().enumerate() {
            if by.insert(k.as_str(), v.as_str()).is_some() {
                return Err(format!("start:{}: {} given twice", n + 1, k));
            }
        }
        let mut out = Vec::with_capacity(t.facts.len());
        for f in 0..t.facts.len() as u32 {
            let v = by.get(t.key(f)).ok_or(format!("start: no value for fact {}", t.key(f)))?;
            out.push(if v.is_empty() {
                NONE
            } else {
                match t.strs.find(v) {
                    Some(s) => t.facts.dom(f).iter().position(|&x| x == s).map(|p| p as u32).unwrap_or(NONE),
                    None => NONE,
                }
            });
        }
        Ok((out, by.keys().map(|k| k.to_string()).collect()))
    }

    pub fn fact(&self, key: &str) -> Option<u32> {
        self.fact_of.get(key).copied()
    }

    fn find(&self, key: &KeyOf) -> Option<usize> {
        self.kept.iter().position(|k| &k.key == key)
    }

    /// Build what is kept for `key` (serve.rs `build`), dropping the least recently used beyond `keep`.
    fn build(&mut self, key: KeyOf, list: &[Ban], goal: Vec<(u32, Vec<u32>)>) -> usize {
        if self.kept.len() >= self.cfg.keep.max(1) {
            let lru = (0..self.kept.len()).min_by_key(|&i| self.kept[i].used).unwrap();
            self.kept.remove(lru);
        }
        let cfg = self.cfg;
        let t = &self.t;
        let tb = Instant::now();
        let mut bc = BanCounts::default();
        let b = bans::banned(t, list, None, &mut bc).expect("bans::banned stops only on the start file's keys");
        let ban_seconds = tb.elapsed().as_secs_f64();
        let task_of = |acts: Actions| Task {
            strs: t.strs.clone(),
            facts: t.facts.clone(),
            goal: goal.clone(),
            starts: t.starts.clone(),
            acts,
            n_orig: t.n_orig,
        };
        let prof = std::env::var_os("TASKCORE_PROFILE").is_some();
        if prof {
            eprintln!("profile warm bans {:.3} s", ban_seconds);
        }
        let tf = Instant::now();
        let (folded, fold_js) = if cfg.fold != FoldMode::Off {
            let (acts, fc, _) = fold::fold_src(&b, &t.facts, &t.strs, cfg.fold);
            let js = fold_json(&fc, &acts);
            (task_of(acts), js)
        } else {
            let mut acts = Actions::default();
            for i in 0..b.n() {
                acts.copy_src(&b, i);
            }
            (task_of(acts), String::new())
        };
        if prof {
            eprintln!("profile warm fold (first) {:.3} s", tf.elapsed().as_secs_f64());
        }
        let mut counts = Counts::new();
        let (task, bad, changed) = if cfg.reduce.any() {
            match reduce::bad_effect(&folded) {
                Some(e) => (None, Some(e), false),
                None => {
                    let (nt, ch) = reduce::prefix(folded, cfg.reduce, &mut counts).expect("checked");
                    (Some(nt), None, ch)
                }
            }
        } else {
            (Some(folded), None, false)
        };
        let bans::Banned { pieces, order, .. } = b;
        self.clock += 1;
        self.kept.push(Kept {
            key,
            goal,
            pieces,
            order,
            ban_counts: bc,
            ban_seconds,
            needed: None,
            needed_after: None,
            fold_json: fold_js,
            bad,
            task,
            counts,
            changed,
            used: self.clock,
        });
        self.kept.len() - 1
    }

    fn needed(&mut self, k: usize) -> Vec<String> {
        if self.kept[k].needed.is_none() {
            let nd = {
                let kept = &self.kept[k];
                let b = BannedRef { input: &self.t.acts, pieces: &kept.pieces, order: &kept.order };
                reduce::needed_src(&b, &self.t.facts, &kept.goal)
            };
            let keys = needed_keys(&nd, &self.t);
            self.kept[k].needed = Some(keys);
        }
        self.kept[k].needed.clone().unwrap()
    }

    fn needed_after(&mut self, k: usize) -> Vec<String> {
        if self.kept[k].needed_after.is_none() {
            let keys = {
                let kept = &self.kept[k];
                let b = BannedRef { input: &self.t.acts, pieces: &kept.pieces, order: &kept.order };
                let (acts, _, _) = fold::fold_src(&b, &self.t.facts, &self.t.strs, self.cfg.fold);
                let nd = reduce::needed_src(&acts, &self.t.facts, &kept.goal);
                needed_keys(&nd, &self.t)
            };
            self.kept[k].needed_after = Some(keys);
        }
        self.kept[k].needed_after.clone().unwrap()
    }

    /// The reached check of a request's start (`sas`'s, on the reading's .reached text): the error `request` stops
    /// with, if any.
    pub fn reached_error(&self, start: &[(String, String)]) -> Result<(), String> {
        let (vals, _) = self.start_values(start)?;
        if let Some(text) = &self.reached {
            let m = bans::reached_misses_text(&self.t, &vals, text);
            if !m.is_empty() {
                return Err(format!(
                    "the start holds {} values the reading did not reach: {:?}; extend the reading",
                    m.len(),
                    &m[..m.len().min(20)]
                ));
            }
        }
        Ok(())
    }

    /// The facts (e) needs on the banned task for (bans, goal) (the operator table's "needed"): what is kept for them
    /// when a request built it, else the banned view and (e)'s needed values alone (no fold, no reduction).
    pub fn needed_for(&mut self, bans_list: &[Ban], goal: Option<&str>) -> Result<Vec<String>, String> {
        let bt = bans_text(bans_list);
        if let Some(k) = self.kept.iter().position(|k| k.key.0 == bt && k.key.1.as_deref() == goal) {
            return Ok(self.needed(k));
        }
        let mk = (bt, goal.map(String::from));
        if let Some(n) = self.needed_memo.get(&mk) {
            return Ok(n.clone());
        }
        let goal_ids = match goal {
            Some(g) => parse_goal(&self.t, g)?,
            None => self.t.goal.clone(),
        };
        let mut bc = BanCounts::default();
        let b = bans::banned(&self.t, bans_list, None, &mut bc)?;
        let nd = reduce::needed_src(&b, &self.t.facts, &goal_ids);
        drop(b);
        let keys = needed_keys(&nd, &self.t);
        self.needed_memo.insert(mk, keys.clone());
        Ok(keys)
    }

    /// One request (module docstring): serve.rs `request`'s steps on these inputs, to the reduced task.
    pub fn request(&mut self, r: &Request) -> Result<Reduced, String> {
        let t0 = Instant::now();
        let cfg = self.cfg;
        let mut st = J::new();
        st.s("task", &self.name)
            .n("file_bytes", self.file_bytes)
            .n("input_actions", self.t.acts.len())
            .n("input_effects", self.input_effects)
            .n("facts", self.t.facts.len())
            .n("add_effects_expanded", self.add_effects)
            .f("seconds_load", self.seconds_load)
            .n("rss_after_load", 0);
        // the caps: the actions they lower (module docstring)
        let lowered = match &r.caps {
            Some(c) => self.lowered_by(c),
            None => Vec::new(),
        };
        if let Some(c) = &r.caps {
            let mut j = J::new();
            j.n("caps", c.len()).n("actions_lowered", lowered.len());
            st.raw("cost_caps", &j.done());
        }
        // the start and the bans, checked as `sas` checks their files
        let (vals, named) = self.start_values(&r.start)?;
        if let Some(text) = &self.reached {
            let m = bans::reached_misses_text(&self.t, &vals, text);
            if !m.is_empty() {
                return Err(format!(
                    "the start holds {} values the reading did not reach: {:?}; extend the reading",
                    m.len(),
                    &m[..m.len().min(20)]
                ));
            }
        }
        if let Some(e) = bans::named_error(&self.t, &r.bans, Some(&named)) {
            return Err(e);
        }
        let key: KeyOf = (bans_text(&r.bans), r.goal.clone(), lowered.clone());
        self.set_costs(&lowered);
        let (k, built) = match self.find(&key) {
            Some(k) => (k, false),
            None => {
                let goal = match &r.goal {
                    Some(g) => parse_goal(&self.t, g)?,
                    None => self.t.goal.clone(),
                };
                (self.build(key, &r.bans, goal), true)
            }
        };
        self.clock += 1;
        self.kept[k].used = self.clock;
        if !r.bans.is_empty() {
            let kept = &self.kept[k];
            let bc = &kept.ban_counts;
            let mut j = J::new();
            j.n("bans", bc.bans)
                .n("not_applied", bc.not_applied)
                .n("actions_hit", bc.actions_hit)
                .n("pieces", bc.pieces)
                .n("actions_after", bc.actions_after)
                .f("seconds", kept.ban_seconds);
            st.raw("bans", &j.done());
        }
        st.s("start", &r.start_name);
        // the start check on the banned task
        let input_misses = {
            let kept = &self.kept[k];
            let b = BannedRef { input: &self.t.acts, pieces: &kept.pieces, order: &kept.order };
            sas::start_misses_src(&b, &self.t, &kept.goal, &vals, cfg.limit)?
        };
        if !r.needed_check && !input_misses.is_empty() {
            let m = &input_misses;
            return Err(format!("{} facts out of their domains: {:?}", m.len(), &m[..m.len().min(10)]));
        }
        let t_needed = Instant::now();
        let check = r.needed_check && !input_misses.is_empty();
        let needed = self.needed(k);
        let mut seconds_needed = t_needed.elapsed().as_secs_f64();
        if std::env::var_os("TASKCORE_PROFILE").is_some() {
            eprintln!("profile warm start check {:.3} s, needed {:.3} s", t_needed.duration_since(t0).as_secs_f64(), seconds_needed);
        }
        st.s("fold", &format!("{:?}", cfg.fold)).s("reduce", &format!("{:?}", cfg.reduce)).n("product_limit", cfg.limit);
        st.s("encoding", if cfg.model { "model" } else { "task" });
        let passes = if cfg.fold != FoldMode::Off && cfg.reduce.any() { 2 } else { 1 };
        if cfg.fold != FoldMode::Off {
            st.f("seconds_fold", 0.0);
            st.raw("fold_counts", &self.kept[k].fold_json);
        }
        if r.needed_check {
            let t1 = Instant::now();
            let mut j = J::new();
            j.s("on", "the facts the input check stops on that (e) needs, before or after the first fold")
                .n("facts_the_input_check_stops_on", input_misses.len());
            if check {
                let after = if cfg.fold != FoldMode::Off { self.needed_after(k) } else { needed.clone() };
                let need: HashSet<&str> = needed.iter().chain(after.iter()).map(|k| k.as_str()).collect();
                let misses: Vec<&String> = input_misses.iter().filter(|k| need.contains(k.as_str())).collect();
                if !misses.is_empty() {
                    return Err(format!("{} facts out of their domains: {:?}", misses.len(), &misses[..misses.len().min(10)]));
                }
                j.n("needed_facts_before_the_fold", needed.len()).n("needed_facts_after_the_fold", after.len());
            }
            seconds_needed += t1.elapsed().as_secs_f64();
            j.f("seconds", seconds_needed);
            st.raw("start_check", &j.done());
        }
        // the reductions: (d) from this start on the kept task, then the rest of the loop
        let start = self.t.starts.len();
        let out = if std::env::var_os("TASKCORE_CHECK_REDUCE").is_some() && cfg.fast {
            let a = self.reduce_from(k, &vals, &r.start_name, passes, false)?;
            let b = self.reduce_from(k, &vals, &r.start_name, passes, true)?;
            if let Some(d) = reduce::fast::task_diff(&a.task, &b.task) {
                panic!("the fast reductions give another task: {}", d);
            }
            assert!(a.c == b.c, "the fast reductions give other counts: {:?} / {:?}", a.c, b.c);
            assert!(a.replaced == b.replaced, "the fast reductions give other replaced pairs");
            assert!(
                (a.after, &a.fold2, a.after2) == (b.after, &b.fold2, b.after2),
                "the fast reductions give other numbers: {:?} / {:?}",
                (a.after, &a.fold2, a.after2),
                (b.after, &b.fold2, b.after2)
            );
            eprintln!("check: the fast reductions give the reference's task, counts and pairs ({} actions)", b.task.acts.len());
            b
        } else {
            self.reduce_from(k, &vals, &r.start_name, passes, cfg.fast)?
        };
        let ReduceOut { task: t, c, replaced, .. } = out;
        if let Some(n) = out.after {
            st.f("seconds_reduce", out.seconds.0);
            st.n("actions_after_reduce", n);
        }
        if let Some(fj) = &out.fold2 {
            st.f("seconds_fold_second", out.seconds.1);
            st.raw("fold_counts_second", fj);
        }
        if let Some(n) = out.after2 {
            st.f("seconds_reduce_second", out.seconds.2);
            st.n("actions_after_reduce_second", n);
        }
        if cfg.reduce.any() {
            st.raw("reduce_counts", &counts_json(&c));
            let sample: Vec<String> = replaced.iter().take(20).map(|(i, k)| format!("[{}, {}]", i, k)).collect();
            st.raw("replaced_sample", &format!("[{}]", sample.join(", ")));
            st.n("replaced", replaced.len());
        }
        st.n("actions", t.acts.len()).n("effects", t.acts.n_effects()).n("rss_after_reduce", 0);
        Ok(Reduced {
            task: t,
            start,
            stats: st,
            replaced,
            needed,
            kept: if built { "built" } else { "kept" },
            seconds: t0.elapsed().as_secs_f64(),
        })
    }
}

/// The reductions from one start (`request`'s), and the numbers its stats print.
struct ReduceOut {
    task: Task,
    c: Counts,
    replaced: Vec<(u32, u32)>,
    /// actions after the loop, the second fold's counts, actions after the second loop (when they run)
    after: Option<usize>,
    fold2: Option<String>,
    after2: Option<usize>,
    /// seconds of the loop, the second fold, the second loop
    seconds: (f64, f64, f64),
}

impl Warm {
    /// (d) from `vals` on what is kept at `k`, the rest of the loop, the second fold and its loop: `reduce::finish` and
    /// `reduce::run`, or with `fast` their fast forms (reduce/fast.rs, the same result).
    fn reduce_from(&self, k: usize, vals: &[u32], start_name: &str, passes: i32, fast: bool) -> Result<ReduceOut, String> {
        let cfg = self.cfg;
        let kept = &self.kept[k];
        let mut c = kept.counts.clone();
        let mut replaced = Vec::new();
        let start = self.t.starts.len();
        let mut memo = reduce::fast::Memo::default();
        let (mut after, mut fold2, mut after2, mut seconds) = (None, None, None, (0.0, 0.0, 0.0));
        let mut t = if cfg.reduce.any() {
            if let Some(e) = &kept.bad {
                return Err(e.clone());
            }
            let base = kept.task.as_ref().expect("built");
            let t1 = Instant::now();
            let (mut nt, ch) = if cfg.reduce.d {
                let td = Instant::now();
                let r = if fast {
                    reduce::fast::red_d_from_fast(base, Some((0, vals.to_vec())), start, &mut c)
                } else {
                    reduce::red_d_from(base, Some((0, vals.to_vec())), start, &mut c)
                };
                if std::env::var_os("TASKCORE_PROFILE").is_some() {
                    eprintln!("profile d (kept task) {:.3} s, {} actions", td.elapsed().as_secs_f64(), r.0.acts.len());
                }
                r
            } else {
                let mut tt = base.clone();
                tt.starts.push((0, vals.to_vec()));
                (tt, false)
            };
            let id = nt.strs.intern(start_name);
            nt.starts.last_mut().unwrap().0 = id;
            let nt = if fast {
                reduce::fast::finish_fast(nt, cfg.reduce, start, &mut c, &mut replaced, kept.changed | ch, &mut memo)
            } else {
                reduce::finish(nt, cfg.reduce, start, &mut c, &mut replaced, kept.changed | ch)
            };
            seconds.0 = t1.elapsed().as_secs_f64();
            after = Some(nt.acts.len());
            nt
        } else {
            let mut tt = kept.task.as_ref().expect("built").clone();
            let id = tt.strs.intern(start_name);
            tt.starts.push((id, vals.to_vec()));
            tt
        };
        if passes == 2 {
            let t1 = Instant::now();
            let (nt, fc, _) = fold::fold(t, cfg.fold);
            if std::env::var_os("TASKCORE_PROFILE").is_some() {
                eprintln!("profile fold (second) {:.3} s, {} folded", t1.elapsed().as_secs_f64(), fc.folded_actions);
            }
            t = nt;
            seconds.1 = t1.elapsed().as_secs_f64();
            fold2 = Some(fold_json(&fc, &t.acts));
            if fc.folded_actions > 0 {
                let t1 = Instant::now();
                t = if fast {
                    reduce::fast::run_fast(t, cfg.reduce, start, &mut c, &mut replaced, &mut memo)?
                } else {
                    reduce::run(t, cfg.reduce, start, &mut c, &mut replaced)?
                };
                seconds.2 = t1.elapsed().as_secs_f64();
                after2 = Some(t.acts.len());
            }
        }
        Ok(ReduceOut { task: t, c, replaced, after, fold2, after2, seconds })
    }
}

/// The banned view over borrowed pieces (bans.rs `Banned` owns its pieces; what is kept lends them here).
struct BannedRef<'a> {
    input: &'a Actions,
    pieces: &'a Actions,
    order: &'a [u32],
}

use crate::bans::PIECE;

impl<'a> BannedRef<'a> {
    fn at(&self, i: usize) -> (&Actions, usize) {
        let o = self.order[i];
        if o & PIECE != 0 {
            (self.pieces, (o & !PIECE) as usize)
        } else {
            (self.input, o as usize)
        }
    }
}

impl<'a> Src for BannedRef<'a> {
    fn n(&self) -> usize {
        self.order.len()
    }
    fn view(&self, i: usize) -> View<'_> {
        let (a, j) = self.at(i);
        a.view(j)
    }
    fn meta(&self, i: usize) -> Meta {
        let (a, j) = self.at(i);
        a.meta[j]
    }
    fn members(&self, i: usize) -> &[u32] {
        let (a, j) = self.at(i);
        a.members(j)
    }
}

//! taskcore serve: `sas` for many replans in one process, with the work that does not depend on the start done once.
//!
//! usage: taskcore serve --task F.tc [--fold ..] [--reduce ..] [--product-limit N] [--encoding ..] [--goal K=V,..]
//!
//! It loads the packed file once and writes `ready<TAB>{json}` on stdout (the load's numbers). Then it reads one
//! request per line on stdin: `sas`'s per-replan options, tab-separated (`--start-file F` or `--start NAME`, `--bans
//! F`, `--out F`, `--stats F`, `--ops F`, `--kept F`, `--start-check input|needed`, `--goal ..`), and writes the
//! files `taskcore sas` writes with the server's options and these, byte for byte, then one line: `ok<TAB>{json}`
//! (this request's seconds and memory) or `error<TAB>message` (the message `sas` stops with). It exits at the end of
//! stdin. The fold, the reductions, PRODUCT_LIMIT and the encoding are the server's; `--check-fold` is refused.
//!
//! **What is kept.** `sas` (main.rs `prepare`) runs: load; the start file; the bans; the start check; the fold; the
//! reduction loop (`reduce::run`); the second fold and loop; the writer. Of these only the start check and the
//! reductions from (d) on read the start. The bans and the goal come before the fold and change everything after
//! them. So the server keeps:
//!
//! * the loaded task (always);
//! * for the last (bans, goal) it was asked for: the banned task as a view over the loaded one (bans.rs `Banned`, the
//!   pieces only), the facts (e) needs on it (the operator table's head; `reduce::needed` reads no start), the folded
//!   task's counts, and the reduction loop's first round up to (d): the check for a written value outside its
//!   domain, normalize, (a), (b) and (c) (`reduce::prefix`), with their counts. None of these reads the start.
//!
//! Per request it reads the start and redoes what depends on it: the start check (`sas::start_misses` on the banned
//! view, only when the start has a value outside a domain); then (d) from that start on the kept task
//! (`reduce::red_d_from`: the kept task is read, not changed), the rest of the loop (`reduce::finish`), the second
//! fold and loop, the writer. A request with other bans or another goal first builds them again from the loaded
//! task (no file is read again).
//!
//! **Why the files are the same bytes.** The kept task is the one `sas` has when its loop reaches (d) in the first
//! round: the same steps (`reduce::run` is `prefix`, (d), `finish`) on the same task (the fold reads the banned view
//! action by action, as it reads the banned copy). The start only enters there: `sas` pushes the start file's start
//! onto the task's starts after the load, and nothing before (d) reads starts; `red_d_from` reaches from it and
//! gives the task `red_d` gives with it pushed last. From (d) on, the code is `sas`'s.
//!
//! **Not kept, and why.** (e) and (f) read no start either, but the loop runs them after (d): run before it they
//! would hand (d) another task, and (f)'s choice between equal actions (the first in its order) can then differ.
//! The fold of the second pass reads (d)'s result.
//!
//! **Memory.** The loaded task, the kept task (smaller: (b) drops the effects nothing reads), the pieces and one
//! request's reduced task. `peak_rss` and `rss_after_*` in the stats are this request's (the peak is reset before
//! each, /proc/self/clear_refs); the reply gives the process's resident memory now and this request's peak.

use crate::fold::{self, FoldMode};
use crate::model::*;
use crate::reduce::{self, Counts};
use crate::{bans, packed, sas};
use crate::{check_args, counts_json, parse_goal, parse_opts, write_out, Args, Prepared, J};
use std::io::{BufRead, Write};
use std::time::Instant;

/// The options a request may give: `sas`'s per-replan ones.
const PER_REQUEST: [&str; 8] = ["--start-file", "--start", "--bans", "--out", "--stats", "--ops", "--kept", "--start-check"];

/// What the server keeps for one (bans, goal) (module docstring).
struct Kept {
    key: (Option<String>, Option<String>), // the bans file's text, the goal's spec
    goal: Vec<(u32, Vec<u32>)>,
    pieces: Actions,
    order: Vec<u32>,
    ban_counts: bans::BanCounts,
    ban_seconds: f64,
    needed: Option<Vec<String>>,       // on the banned task, before the fold (lazily)
    needed_after: Option<Vec<String>>, // on the folded task (lazily: only a start check that stops needs it)
    fold_seconds: f64,
    fold_json: String,
    bad: Option<String>, // reduce::run's check
    task: Option<Task>,  // reduce::prefix's task (or the folded task, with no reduction)
    counts: Counts,
    changed: bool,
    reduce_seconds: f64,
    build_seconds: f64,
}

struct Server {
    a: Args,
    t: Task,
    file_bytes: u64,
    add_effects: u64,
    input_effects: u64,
    seconds_load: f64,
    rss_after_load: u64,
    kept: Option<Kept>,
}

fn rss_now() -> u64 {
    if let Ok(s) = std::fs::read_to_string("/proc/self/status") {
        for line in s.lines() {
            if let Some(rest) = line.strip_prefix("VmRSS:") {
                let kb: u64 = rest.trim().trim_end_matches("kB").trim().parse().unwrap_or(0);
                return kb * 1024;
            }
        }
    }
    0
}

/// CPU seconds of this process so far (/proc/self/schedstat: nanoseconds on the CPU; one thread).
fn cpu_s() -> f64 {
    std::fs::read_to_string("/proc/self/schedstat")
        .ok()
        .and_then(|s| s.split_whitespace().next().and_then(|n| n.parse::<u64>().ok()))
        .map_or(0.0, |ns| ns as f64 / 1e9)
}

/// The peak resident memory (VmHWM) set back to what is resident now, so that the next peak is this request's.
fn reset_peak() {
    let _ = std::fs::write("/proc/self/clear_refs", "5");
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

/// The banned view over the loaded task, with the kept pieces lent to it; `give_back` returns them.
fn lend<'a>(t: &'a Task, k: &mut Kept) -> bans::Banned<'a> {
    bans::Banned { input: &t.acts, pieces: std::mem::take(&mut k.pieces), order: std::mem::take(&mut k.order) }
}

fn give_back(b: bans::Banned, k: &mut Kept) {
    k.pieces = b.pieces;
    k.order = b.order;
}

impl Server {
    /// Build what is kept for (bans, goal) from the loaded task (module docstring).
    fn build(&mut self, key: (Option<String>, Option<String>), list: &[bans::Ban], goal: Vec<(u32, Vec<u32>)>) {
        let t0 = Instant::now();
        self.kept = None; // the old one's memory first
        let a = &self.a;
        let t = &self.t;
        let tb = Instant::now();
        let mut bc = bans::BanCounts::default();
        // the named check is the request's (bans::named_error: it reads the start file's keys); with no keys given
        // `banned` stops on nothing
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
        let tf = Instant::now();
        let (folded, fold_seconds, fold_js) = if a.fold != FoldMode::Off {
            let (acts, fc, _) = fold::fold_src(&b, &t.facts, &t.strs, a.fold);
            let js = fold_json(&fc, &acts);
            (task_of(acts), tf.elapsed().as_secs_f64(), js)
        } else {
            let mut acts = Actions::default();
            for i in 0..b.n() {
                acts.copy_src(&b, i);
            }
            (task_of(acts), 0.0, String::new())
        };
        let mut counts = Counts::new();
        let tr = Instant::now();
        let (task, bad, changed) = if a.reduce.any() {
            match reduce::bad_effect(&folded) {
                Some(e) => (None, Some(e), false),
                None => {
                    let (nt, ch) = reduce::prefix(folded, a.reduce, &mut counts).expect("checked");
                    (Some(nt), None, ch)
                }
            }
        } else {
            (Some(folded), None, false)
        };
        let reduce_seconds = tr.elapsed().as_secs_f64();
        let bans::Banned { pieces, order, .. } = b;
        self.kept = Some(Kept {
            key,
            goal,
            pieces,
            order,
            ban_counts: bc,
            ban_seconds,
            needed: None,
            needed_after: None,
            fold_seconds,
            fold_json: fold_js,
            bad,
            task,
            counts,
            changed,
            reduce_seconds,
            build_seconds: t0.elapsed().as_secs_f64(),
        });
    }

    /// The facts (e) needs on the banned task, before the fold (kept once asked for).
    fn needed(&mut self) -> Vec<String> {
        let t = &self.t;
        let k = self.kept.as_mut().unwrap();
        if k.needed.is_none() {
            let b = lend(t, k);
            let nd = reduce::needed_src(&b, &t.facts, &k.goal);
            give_back(b, k);
            k.needed = Some(needed_keys(&nd, t));
        }
        k.needed.clone().unwrap()
    }

    /// The facts (e) needs on the folded task (kept once asked for: only a start check that stops asks).
    fn needed_after(&mut self) -> Vec<String> {
        let mode = self.a.fold;
        let t = &self.t;
        let k = self.kept.as_mut().unwrap();
        if k.needed_after.is_none() {
            let b = lend(t, k);
            let (acts, _, _) = fold::fold_src(&b, &t.facts, &t.strs, mode);
            give_back(b, k);
            let nd = reduce::needed_src(&acts, &t.facts, &k.goal);
            k.needed_after = Some(needed_keys(&nd, t));
        }
        k.needed_after.clone().unwrap()
    }

    /// One request (module docstring): `prepare`'s steps, with what is kept, then `write_out`. Returns the reply's
    /// json.
    fn request(&mut self, r: &Args) -> Result<String, String> {
        let t0 = Instant::now();
        let cpu0 = cpu_s();
        let mut st = J::new();
        st.s("task", &self.a.task.display().to_string())
            .n("file_bytes", self.file_bytes)
            .n("input_actions", self.t.acts.len())
            .n("input_effects", self.input_effects)
            .n("facts", self.t.facts.len())
            .n("add_effects_expanded", self.add_effects)
            .f("seconds_load", self.seconds_load)
            .n("rss_after_load", self.rss_after_load);
        // the start file and the bans, read as `sas` reads them
        let mut file_start: Option<(String, Vec<u32>)> = None;
        let mut named = None;
        if let Some(path) = &r.start_file {
            let (vals, keys) = bans::read_start(&self.t, path)?;
            file_start = Some((format!("file:{}", path.display()), vals));
            named = Some(keys);
        }
        let mut list = Vec::new();
        let mut bans_text = None;
        if let Some(path) = &r.bans {
            list = bans::read_bans(path)?;
            bans_text = Some(std::fs::read_to_string(path).map_err(|e| format!("{}: {}", path.display(), e))?);
            if let Some(e) = bans::named_error(&self.t, &list, named.as_ref()) {
                return Err(e);
            }
        }
        let goal_spec = r.goal.clone().or_else(|| self.a.goal.clone());
        let key = (bans_text, goal_spec.clone());
        let built = self.kept.as_ref().map_or(true, |k| k.key != key);
        if built {
            let goal = match &goal_spec {
                Some(g) => parse_goal(&self.t, g)?,
                None => self.t.goal.clone(),
            };
            self.build(key, &list, goal);
        }
        if r.bans.is_some() {
            let k = self.kept.as_ref().unwrap();
            let bc = &k.ban_counts;
            let mut j = J::new();
            j.n("bans", bc.bans)
                .n("not_applied", bc.not_applied)
                .n("actions_hit", bc.actions_hit)
                .n("pieces", bc.pieces)
                .n("actions_after", bc.actions_after)
                .f("seconds", k.ban_seconds);
            st.raw("bans", &j.done());
        }
        // the start: `sas` pushes a start file's start last onto the loaded task's starts
        let start = match &r.start {
            Some(name) => self.t.start_index(name).ok_or(format!("no start named {}", name))?,
            None if file_start.is_some() => self.t.starts.len(),
            None => 0,
        };
        let start_vals: Vec<u32> = match &file_start {
            Some((name, vals)) => {
                st.s("start", name);
                vals.clone()
            }
            None => {
                let (n, v) = self.t.starts.get(start).ok_or("the file has no start")?;
                st.s("start", self.t.strs.get(*n));
                v.clone()
            }
        };
        // the start check on the banned task (module docstring)
        let input_misses = {
            let t = &self.t;
            let k = self.kept.as_mut().unwrap();
            let b = lend(t, k);
            let res = sas::start_misses_src(&b, t, &k.goal, &start_vals, self.a.limit);
            give_back(b, k);
            res?
        };
        if !r.needed_check && !input_misses.is_empty() {
            let m = &input_misses;
            return Err(format!("{} facts out of their domains: {:?}", m.len(), &m[..m.len().min(10)]));
        }
        let t_needed = Instant::now();
        let check = r.needed_check && !input_misses.is_empty();
        let needed = if check || r.ops.is_some() { self.needed() } else { Vec::new() };
        let mut seconds_needed = t_needed.elapsed().as_secs_f64();
        let a = self.a.clone();
        st.s("fold", &format!("{:?}", a.fold)).s("reduce", &format!("{:?}", a.reduce)).n("product_limit", a.limit);
        st.s("encoding", if a.model { "model" } else { "task" });
        let passes = if a.fold != FoldMode::Off && a.reduce.any() { 2 } else { 1 };
        // pass 0: the kept fold
        if a.fold != FoldMode::Off {
            let k = self.kept.as_ref().unwrap();
            st.f("seconds_fold", if built { k.fold_seconds } else { 0.0 });
            st.raw("fold_counts", &k.fold_json);
        }
        if r.needed_check {
            let t1 = Instant::now();
            let mut j = J::new();
            j.s("on", "the facts the input check stops on that (e) needs, before or after the first fold")
                .n("facts_the_input_check_stops_on", input_misses.len());
            if check {
                let after = if a.fold != FoldMode::Off { self.needed_after() } else { needed.clone() };
                let need: std::collections::HashSet<&str> = needed.iter().chain(after.iter()).map(|k| k.as_str()).collect();
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
        let k = self.kept.as_ref().unwrap();
        let mut c = k.counts.clone();
        let mut replaced = Vec::new();
        let base = k.task.as_ref();
        let start = if file_start.is_some() { self.t.starts.len() } else { start };
        let mut t = if a.reduce.any() {
            if let Some(e) = &k.bad {
                return Err(e.clone());
            }
            let base = base.expect("built");
            let t1 = Instant::now();
            let (mut nt, ch) = if a.reduce.d {
                let push = file_start.as_ref().map(|(_, vals)| (0, vals.clone()));
                let t0 = Instant::now();
                let r = reduce::red_d_from(base, push, start, &mut c);
                if std::env::var_os("TASKCORE_PROFILE").is_some() {
                    eprintln!("profile d (kept task) {:.3} s, {} actions, {} words", t0.elapsed().as_secs_f64(), r.0.acts.len(), r.0.acts.words.len());
                }
                r
            } else {
                let mut tt = base.clone();
                if let Some((_, vals)) = &file_start {
                    tt.starts.push((0, vals.clone()));
                }
                (tt, false)
            };
            if let Some((name, _)) = &file_start {
                let id = nt.strs.intern(name);
                nt.starts.last_mut().unwrap().0 = id;
            }
            let nt = reduce::finish(nt, a.reduce, start, &mut c, &mut replaced, k.changed | ch);
            let secs = if built { k.reduce_seconds } else { 0.0 } + t1.elapsed().as_secs_f64();
            st.f("seconds_reduce", secs);
            st.n("actions_after_reduce", nt.acts.len());
            nt
        } else {
            let mut tt = base.expect("built").clone();
            if let Some((name, vals)) = &file_start {
                let id = tt.strs.intern(name);
                tt.starts.push((id, vals.clone()));
            }
            tt
        };
        // pass 1: as `sas`
        if passes == 2 {
            let t1 = Instant::now();
            let (nt, fc, _) = fold::fold(t, a.fold);
            t = nt;
            st.f("seconds_fold_second", t1.elapsed().as_secs_f64());
            st.raw("fold_counts_second", &fold_json(&fc, &t.acts));
            if fc.folded_actions > 0 {
                let t1 = Instant::now();
                t = reduce::run(t, a.reduce, start, &mut c, &mut replaced)?;
                st.f("seconds_reduce_second", t1.elapsed().as_secs_f64());
                st.n("actions_after_reduce_second", t.acts.len());
            }
        }
        if a.reduce.any() {
            st.raw("reduce_counts", &counts_json(&c));
            let sample: Vec<String> = replaced.iter().take(20).map(|(i, k)| format!("[{}, {}]", i, k)).collect();
            st.raw("replaced_sample", &format!("[{}]", sample.join(", ")));
            st.n("replaced", replaced.len());
        }
        st.n("actions", t.acts.len()).n("effects", t.acts.n_effects()).n("rss_after_reduce", peak_rss());
        let p = Prepared { names: Vec::new(), task: t, start, stats: st, replaced, needed };
        let build_seconds = self.kept.as_ref().unwrap().build_seconds;
        write_out(r, p, t0)?;
        let mut j = J::new();
        j.s("kept", if built { "built" } else { "kept" })
            .f("seconds_build", if built { build_seconds } else { 0.0 })
            .f("seconds", t0.elapsed().as_secs_f64())
            .f("cpu_seconds", cpu_s() - cpu0)
            .n("peak_rss", peak_rss())
            .n("rss", rss_now());
        Ok(j.done())
    }
}

pub fn run(a: &Args) -> Result<(), String> {
    if a.check_fold > 0 {
        return Err("serve: --check-fold is not kept across requests".into());
    }
    let t0 = Instant::now();
    let loaded = packed::load(&a.task)?;
    let mut t = loaded.task;
    if let Some(g) = &a.goal {
        t.goal = parse_goal(&t, g)?;
    }
    let input_effects = t.acts.n_effects();
    let seconds_load = t0.elapsed().as_secs_f64();
    let rss_after_load = peak_rss();
    let mut srv = Server {
        a: a.clone(),
        t,
        file_bytes: loaded.file_bytes,
        add_effects: loaded.add_effects,
        input_effects,
        seconds_load,
        rss_after_load,
        kept: None,
    };
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    let mut j = J::new();
    j.s("task", &a.task.display().to_string())
        .n("input_actions", srv.t.acts.len())
        .n("facts", srv.t.facts.len())
        .f("seconds_load", seconds_load)
        .f("cpu_seconds", cpu_s())
        .n("rss", rss_now())
        .n("peak_rss", rss_after_load);
    writeln!(out, "ready\t{}", j.done()).map_err(|e| e.to_string())?;
    out.flush().map_err(|e| e.to_string())?;
    let stdin = std::io::stdin();
    for line in stdin.lock().lines() {
        let line = line.map_err(|e| e.to_string())?;
        if line.trim().is_empty() {
            continue;
        }
        reset_peak();
        let reply = (|| -> Result<String, String> {
            let mut r = srv.a.clone();
            r.start = None;
            r.start_file = None;
            r.bans = None;
            r.out = None;
            r.stats = None;
            r.ops = None;
            r.kept = None;
            r.needed_check = false;
            r.goal = None;
            let given = parse_opts(&mut r, line.split('\t').map(String::from))?;
            if let Some(k) = given.iter().find(|k| !PER_REQUEST.contains(&k.as_str()) && k.as_str() != "--goal") {
                return Err(format!("serve: {} is the server's option, not a request's", k));
            }
            check_args(&r)?;
            srv.request(&r)
        })();
        match reply {
            Ok(js) => writeln!(out, "ok\t{}", js),
            Err(e) => writeln!(out, "error\t{}", e.replace('\n', " ")),
        }
        .map_err(|e| e.to_string())?;
        out.flush().map_err(|e| e.to_string())?;
    }
    Ok(())
}

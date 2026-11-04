//! E015's item gate (experiments/E015-engine-gates/plan.md, "The item gate", and amendment (1)). Every number of the
//! game is an argument; the search, the sweep and the grep name none.
//!
//!   e015 item OUT --movie EVENTS --room-a A --room-b B --object O [--steps 1234]
//!       steps 1-4: boot and play EVENTS, run to room A's first idle (and the frame room A loads); a breadth-first
//!       search over afford's inputs in forks to the first stop in room B (the stop's node is finished, so the record
//!       counts the openings from that state); from room B's first idle the same search to owner:O = the dump's ego;
//!       the chain replayed from boot in a fresh engine (facts with the control keys, room, patch 0007's digest)
//!   e015 item OUT --archive ARCH --entries I,J,.. --room R --local L --class-obj C --class K --room-back RB
//!                 --room-home RH [--steps 56]
//!       step 5: from each entry, afford's step as a try runs it and one more level (depth 2, 3,000 trials); per trial
//!       a Started touch of script L, local:L live, class:C:K cleared, and (not gating) a run of room R's local L;
//!       step 6: from the stops in room RB (distinct by digest, at most 10) a depth-2 sweep to the first stop in RH,
//!       verified from boot
//!   e015 item OUT --grep SCRIPTS --local L --class-obj C --class K
//!       step 5's static half: every start site of script L (literal, or through a Var or Local argument with the
//!       literal values the extraction assigns that Var) and every setClass of C that clears K; places by room, kind
//!       and number and the offset only (no file name: they carry game text)
//!
//! Limits: --depth, --trials (steps 2 and 3: 4 and 5,000; step 5 and 6: 2 and 3,000). Every trial is written to
//! OUT/<step>.trials.jsonl; the summary to OUT/summary-<steps>.json and stdout.

use fmi_core::*;
use fmi_engine::Host;
use fmi_explore::afford::{self, Inputs, Point};
use fmi_explore::events::{event_json, event_of};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashSet};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;

type Res<T> = Result<T, String>;

fn e2s<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

/// How a search runs (amendment (1)).
#[derive(Clone, Debug)]
pub struct Cfg {
    pub depth: usize,
    pub trials: usize,
    /// frames after a stop: a trial's last frame when no idle comes first (afford's W)
    pub horizon: u64,
    /// afford's inputs in full (the verb slots alone in a sentence idle too) and the third click, as a try runs them;
    /// off: the verb slots alone are left out in a sentence idle, and no third click
    pub full: bool,
    /// the search's room: only idle stops in it are expanded
    pub room: i32,
    /// at a goal stop, the rest of that node's inputs are still tried (the openings from one state), then the search
    /// stops
    pub finish_node: bool,
}

/// A stop to expand: the clicks from the search's start engine (absolute frames), its frame and digest.
#[derive(Clone, Debug)]
struct Node {
    path: Vec<Event>,
    names: Vec<String>,
    frame: u64,
    digest: u64,
    depth: usize,
    trial: Option<usize>,
}

/// A trial's stop as a detector reads it.
pub struct Stop<'a> {
    pub st: &'a Status,
    pub facts: &'a Facts,
    pub runs: &'a [Run],
    pub log: &'a InputLog,
}

/// A detector's verdict on a stop: whether it is the search's goal, and what to record.
pub struct Seen {
    pub goal: bool,
    pub extra: Value,
}

/// A search's first goal stop: its path from the start, frame, facts, digest, room.
#[derive(Clone, Debug)]
pub struct Goal {
    pub trial: usize,
    pub path: Vec<Event>,
    pub names: Vec<String>,
    pub frame: u64,
    pub facts: Facts,
    pub digest: u64,
    pub room: i32,
    pub idle: Option<String>,
    pub depth: usize,
}

/// A search's record: every trial (as JSON lines), the nodes expanded, the goal, why it stopped.
pub struct Out {
    pub trials: Vec<Value>,
    pub nodes: Vec<Value>,
    pub goal: Option<Goal>,
    pub goals: usize,
    pub stopped: String,
    pub seconds: f64,
}

struct Search<'a> {
    cfg: Cfg,
    detect: &'a mut dyn FnMut(&Stop) -> Seen,
    out: Out,
    next: Vec<Node>,
    end: u64,
}

fn ev_short(e: &Event) -> Value {
    match &e.what {
        Input::Click(x, y) => json!([e.frame, x, y]),
        _ => event_json(e),
    }
}

enum Ran {
    Stopped(Status),
    Ended(Status),
}

/// The trial's engine after its clicks: run to the frame after the last click, then to the bridge's idle stop or
/// `end` (afford's run_trial, without M4's form and E013's refused inputs).
fn run_trial(b: &mut Box<dyn Engine>, last: u64, end: u64) -> Res<Ran> {
    let go = |b: &mut Box<dyn Engine>, spec: RunSpec| -> Res<Result<Status, Status>> {
        match b.run(&spec) {
            Ok(s) => Ok(Ok(s)),
            Err(EngineError::Ended(s)) => Ok(Err(*s)),
            Err(x) => Err(x.to_string()),
        }
    };
    let mut st = match go(b, RunSpec { frame: Some(last + 1), ..Default::default() })? {
        Ok(s) => s,
        Err(s) => return Ok(Ran::Ended(s)),
    };
    if st.frame < end {
        st = match go(b, RunSpec { idle: true, max_frame: Some(end), ..Default::default() })? {
            Ok(s) => s,
            Err(s) => return Ok(Ran::Ended(s)),
        };
    }
    Ok(Ran::Stopped(st))
}

impl Search<'_> {
    fn full(&self) -> bool {
        self.out.trials.len() >= self.cfg.trials
    }

    /// One trial on `c` (a fork of the node's engine at frame `f0`): `sched` scheduled, `all` its clicks from the
    /// node (absolute frames), `names` its inputs' names. Records it, queues its stop for the next depth, and with
    /// `full` runs the third click when the second left the game composing.
    #[allow(clippy::too_many_arguments)]
    fn trial(&mut self, c: &mut Box<dyn Engine>, node: &Node, f0: u64, sched: &[Event], all: Vec<Event>,
             clicks: Vec<Point>, ctl: (i32, i32)) -> Res<()> {
        let t = Instant::now();
        let idx = self.out.trials.len();
        let name: String = Inputs { clicks: clicks.clone() }.name();
        let mut path = node.path.clone();
        path.extend(all.iter().cloned());
        let mut rec = json!({"i": idx, "depth": node.depth + 1, "node": node.trial, "inputs": name,
                             "events": all.iter().map(ev_short).collect::<Vec<_>>()});
        c.segment(true).map_err(e2s)?;
        c.runs().map_err(e2s)?;
        match c.inputs(sched) {
            Ok(()) => {}
            Err(EngineError::Bridge(why)) => {
                rec["refused"] = json!(why);
                rec["seconds"] = json!(t.elapsed().as_secs_f64());
                self.out.trials.push(rec);
                return Ok(());
            }
            Err(x) => return Err(x.to_string()),
        }
        let last = all.last().map(|x| x.frame).unwrap_or(f0);
        let st = match run_trial(c, last, self.end)? {
            Ran::Ended(s) => {
                rec["ended"] = json!(true);
                rec["reason"] = json!(s.reason);
                rec["frame"] = json!(s.frame);
                rec["room"] = json!(s.room);
                rec["rng"] = json!(s.rng);
                rec["seconds"] = json!(t.elapsed().as_secs_f64());
                self.out.trials.push(rec);
                return Ok(());
            }
            Ran::Stopped(s) => s,
        };
        let facts = c.facts().map_err(e2s)?;
        let digest = c.digest().map_err(e2s)?.digest;
        let runs = c.runs().map_err(e2s)?.runs;
        let log = c.input_log().map_err(e2s)?;
        let seen = (self.detect)(&Stop { st: &st, facts: &facts, runs: &runs, log: &log });
        let verdict = afford::trial_verdict(&log.records);
        rec["frame"] = json!(st.frame);
        rec["reason"] = json!(st.reason);
        rec["idle"] = json!(st.idle);
        rec["room"] = json!(st.room);
        rec["rng"] = json!(st.rng);
        rec["tick"] = json!(st.tick);
        rec["digest"] = json!(format!("{digest:016x}"));
        rec["verdict"] = json!(verdict);
        rec["xstart"] = json!(st.xstart);
        if !seen.extra.is_null() {
            rec["seen"] = seen.extra;
        }
        rec["path"] = json!(path.iter().map(ev_short).collect::<Vec<_>>());
        let mut names = node.names.clone();
        names.push(name);
        if seen.goal {
            rec["goal"] = json!(true);
            self.out.goals += 1;
            if self.out.goal.is_none() {
                self.out.goal = Some(Goal {
                    trial: idx,
                    path: path.clone(),
                    names: names.clone(),
                    frame: st.frame,
                    facts: facts.clone(),
                    digest,
                    room: st.room,
                    idle: st.idle.clone(),
                    depth: node.depth + 1,
                });
            }
        }
        let expand = st.reason == "idle" && st.room == self.cfg.room && node.depth + 1 < self.cfg.depth;
        if expand && !seen.goal {
            self.next.push(Node { path: path.clone(), names, frame: st.frame, digest, depth: node.depth + 1, trial: Some(idx) });
        }
        rec["seconds"] = json!(t.elapsed().as_secs_f64());
        self.out.trials.push(rec);
        // the third click (try_all's rule): over every target of the stop when the second click left the game composing
        if self.cfg.full && clicks.len() == 2 && st.frame < self.end {
            let input = [ctl.0, facts.val(Key::var(afford::VAR_VERB_SCRIPT as u64)) as i32];
            if afford::composing(st.idle.as_deref(), &runs, last, &input, ctl.1) {
                self.out.trials[idx]["composing"] = json!(true);
                let o = c.obs(&ObsReq::default()).map_err(e2s)?;
                let of2 = afford::offer_with(c, &o, false).map_err(e2s)?;
                for p in &of2.targets {
                    if self.full() || self.stop_now() {
                        break;
                    }
                    let mut d = c.fork().map_err(e2s)?;
                    let e3 = Event { frame: st.frame + 1, what: Input::Click(p.x, p.y) };
                    let mut all3 = all.clone();
                    all3.push(e3.clone());
                    let mut cl3 = clicks.clone();
                    cl3.push(*p);
                    self.trial(&mut d, node, f0, &[e3], all3, cl3, ctl)?;
                    d.quit();
                }
            }
        }
        Ok(())
    }

    fn stop_now(&self) -> bool {
        self.out.goal.is_some() && !self.cfg.finish_node
    }
}

/// The node's engine made again from the start engine: a fork, the path's clicks, a run to its frame; None when its
/// digest is not the one recorded at the trial's stop.
fn remake(start: &mut Box<dyn Engine>, n: &Node) -> Res<Option<Box<dyn Engine>>> {
    let mut e = start.fork().map_err(e2s)?;
    if n.trial.is_none() {
        return Ok(Some(e));
    }
    e.inputs(&n.path).map_err(e2s)?;
    e.run(&RunSpec { frame: Some(n.frame), ..Default::default() }).map_err(e2s)?;
    let d = e.digest().map_err(e2s)?.digest;
    if d != n.digest || e.status().frame != n.frame {
        e.quit();
        return Ok(None);
    }
    Ok(Some(e))
}

/// The inputs of a stop (amendment (1)): afford's `inputs_of`, less the verb slots alone in a sentence idle unless
/// `full`.
fn inputs(of: &afford::Offer, full: bool) -> Vec<Inputs> {
    let all = afford::inputs_of(of);
    if full || of.idle.as_deref() != Some("sentence") {
        return all;
    }
    all.into_iter()
        .filter(|i| !(i.clicks.len() == 1 && matches!(i.clicks[0].hit, afford::Hit::Verb { .. })))
        .collect()
}

/// Breadth-first search over afford's inputs in forks from `start` (stopped at an idle), depth by depth, each node's
/// inputs in `inputs_of`'s order. `detect` reads each stop.
pub fn search(start: &mut Box<dyn Engine>, cfg: &Cfg, detect: &mut dyn FnMut(&Stop) -> Seen) -> Res<Out> {
    let t0 = Instant::now();
    let f_start = start.status().frame;
    let mut s = Search {
        cfg: cfg.clone(),
        detect,
        out: Out { trials: Vec::new(), nodes: Vec::new(), goal: None, goals: 0, stopped: String::new(), seconds: 0.0 },
        next: vec![Node { path: vec![], names: vec![], frame: f_start, digest: 0, depth: 0, trial: None }],
        end: 0,
    };
    'levels: for _d in 0..cfg.depth {
        let level = std::mem::take(&mut s.next);
        if level.is_empty() {
            s.out.stopped = "exhausted".into();
            break;
        }
        for node in level {
            if s.full() {
                s.out.stopped = "trials".into();
                break 'levels;
            }
            let mut ne = match remake(start, &node)? {
                Some(e) => e,
                None => {
                    s.out.nodes.push(json!({"trial": node.trial, "depth": node.depth, "remade": false}));
                    continue;
                }
            };
            let f0 = ne.status().frame;
            s.end = f0 + cfg.horizon;
            let rng0 = ne.status().rng;
            if afford::refuse_on() || std::env::var("SPEEDRUN_INPUT").is_ok_and(|v| v == "1") {
                ne.input_log().map_err(e2s)?;
            }
            let o = ne.obs(&ObsReq::default()).map_err(e2s)?;
            let of = afford::offer(&mut ne, &o).map_err(e2s)?;
            let inps = inputs(&of, cfg.full);
            let fv = ne.facts().map_err(e2s)?;
            let ctl = (
                fv.val(Key::var(afford::VAR_VERB_SCRIPT as u64)) as i32,
                fv.val(Key::var(afford::VAR_SENTENCE_SCRIPT as u64)) as i32,
            );
            let (n0, g0) = (s.out.trials.len(), s.out.goals);
            for inp in &inps {
                if s.full() || s.stop_now() {
                    break;
                }
                let mut c = ne.fork().map_err(e2s)?;
                let evs = inp.events(f0);
                s.trial(&mut c, &node, f0, &evs, evs.clone(), inp.clicks.clone(), ctl)?;
                c.quit();
            }
            s.out.nodes.push(json!({"trial": node.trial, "depth": node.depth, "frame": f0, "rng": rng0,
                                    "names": node.names, "offer": of.json(), "inputs": inps.len(),
                                    "trials": s.out.trials.len() - n0, "goals": s.out.goals - g0}));
            ne.quit();
            if s.out.goal.is_some() {
                s.out.stopped = "goal".into();
                break 'levels;
            }
        }
    }
    if s.out.stopped.is_empty() {
        s.out.stopped = if s.full() { "trials".into() } else { "depth".into() };
    }
    s.out.seconds = t0.elapsed().as_secs_f64();
    Ok(s.out)
}

/// A fresh engine from boot with `movie`, run to `frame`; compared with the search's stop on every fact (control
/// keys included), the room and the digest.
pub fn verify(h: &Host, out: &Path, movie: &[Event], frame: u64, facts: &Facts, room: i32, digest: u64) -> Res<Value> {
    let t = Instant::now();
    let cfg = BootCfg { seed: 1, events: movie.to_vec(), frame: Some(frame), dump_locals: false, lean: true, out_dir: out.to_path_buf() };
    let mut e: Box<dyn Engine> = Box::new(h.boot_bin(&cfg).map_err(e2s)?);
    let st = e.status().clone();
    let f = e.facts().map_err(e2s)?;
    let d = e.digest().map_err(e2s)?.digest;
    e.quit();
    let diff = facts.diff(&f);
    let first: Vec<Value> = diff.iter().take(8).map(|(k, a, b)| json!([k.text(), a, b])).collect();
    let n = diff.len() + usize::from(st.room != room) + usize::from(d != digest) + usize::from(st.frame != frame);
    Ok(json!({"frame": frame, "got_frame": st.frame, "facts": f.0.len(), "facts_differ": diff.len(), "first": first,
              "room": st.room, "room_same": st.room == room, "digest_same": d == digest,
              "digest": format!("{d:016x}"), "differences": n, "seconds": t.elapsed().as_secs_f64()}))
}

// --- the command ---------------------------------------------------------------------------------------------------

struct Args {
    pos: Vec<String>,
    opts: BTreeMap<String, String>,
}

fn args(argv: &[String]) -> Args {
    let (mut pos, mut opts) = (Vec::new(), BTreeMap::new());
    let mut i = 0;
    while i < argv.len() {
        if let Some(n) = argv[i].strip_prefix("--") {
            opts.insert(n.to_string(), argv.get(i + 1).cloned().unwrap_or_default());
            i += 2;
        } else {
            pos.push(argv[i].clone());
            i += 1;
        }
    }
    Args { pos, opts }
}

impl Args {
    fn num(&self, k: &str) -> Res<i64> {
        self.opts.get(k).ok_or(format!("--{k}"))?.parse().map_err(|e| format!("--{k}: {e}"))
    }
    fn num_or(&self, k: &str, d: i64) -> Res<i64> {
        if self.opts.contains_key(k) {
            self.num(k)
        } else {
            Ok(d)
        }
    }
}

fn read_movie(p: &str) -> Res<Vec<Event>> {
    let text = std::fs::read_to_string(p).map_err(|e| format!("{p}: {e}"))?;
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str::<Value>(l).map_err(e2s).and_then(|v| event_of(&v).ok_or(format!("an event: {l}"))))
        .collect()
}

fn write_trials(out: &Path, step: &str, o: &Out) -> Res<()> {
    let mut f = std::fs::File::create(out.join(format!("{step}.trials.jsonl"))).map_err(e2s)?;
    for t in &o.trials {
        writeln!(f, "{t}").map_err(e2s)?;
    }
    let mut g = std::fs::File::create(out.join(format!("{step}.nodes.jsonl"))).map_err(e2s)?;
    for n in &o.nodes {
        writeln!(g, "{n}").map_err(e2s)?;
    }
    Ok(())
}

fn summary(o: &Out) -> Value {
    let mut rooms: BTreeMap<i64, usize> = BTreeMap::new();
    let mut depths: BTreeMap<i64, usize> = BTreeMap::new();
    for t in &o.trials {
        *rooms.entry(t["room"].as_i64().unwrap_or(-1)).or_default() += 1;
        *depths.entry(t["depth"].as_i64().unwrap_or(-1)).or_default() += 1;
    }
    let ms = if o.trials.is_empty() { 0.0 } else { 1e3 * o.seconds / o.trials.len() as f64 };
    json!({"trials": o.trials.len(), "nodes": o.nodes.len(), "stopped": o.stopped, "goals": o.goals,
           "goal": o.goal.as_ref().map(|g| json!({"trial": g.trial, "depth": g.depth, "frame": g.frame, "room": g.room,
                                                  "idle": g.idle, "inputs": g.names,
                                                  "path": g.path.iter().map(ev_short).collect::<Vec<_>>()})),
           "rooms": rooms.iter().map(|(r, c)| json!([r, c])).collect::<Vec<_>>(),
           "by_depth": depths.iter().map(|(r, c)| json!([r, c])).collect::<Vec<_>>(),
           "refused": o.trials.iter().filter(|t| t.get("refused").is_some()).count(),
           "ended": o.trials.iter().filter(|t| t["ended"] == json!(true)).count(),
           "seconds": o.seconds, "ms_per_trial": ms})
}

/// The engine at an idle stop: `e` itself, or a fork run with no input to the first idle within W.
fn idle(e: Box<dyn Engine>) -> Res<Box<dyn Engine>> {
    let mut e = e;
    match afford::ready(&mut e, afford::W).map_err(e2s)? {
        None => Ok(e),
        Some(Some(b)) => {
            e.quit();
            Ok(b)
        }
        Some(None) => Err(format!("no idle stop within {} frames of frame {}", afford::W, e.status().frame)),
    }
}

fn cfg_of(a: &Args, depth: i64, trials: i64, full: bool, room: i32, finish: bool) -> Res<Cfg> {
    Ok(Cfg {
        depth: a.num_or("depth", depth)? as usize,
        trials: a.num_or("trials", trials)? as usize,
        horizon: afford::W,
        full,
        room,
        finish_node: finish,
    })
}

pub fn main(argv: &[String]) -> Res<()> {
    let a = args(argv);
    let out = PathBuf::from(a.pos.first().ok_or("OUT")?);
    std::fs::create_dir_all(&out).map_err(e2s)?;
    let res = if let Some(dir) = a.opts.get("grep") {
        grep(Path::new(dir), &a)?
    } else if a.opts.contains_key("archive") {
        sweep(&out, &a)?
    } else {
        chain(&out, &a)?
    };
    let steps = a.opts.get("steps").cloned().unwrap_or_else(|| if a.opts.contains_key("grep") { "5s".into() } else { "all".into() });
    std::fs::write(out.join(format!("summary-{steps}.json")), serde_json::to_string_pretty(&res).map_err(e2s)?).map_err(e2s)?;
    println!("{res}");
    Ok(())
}

/// Steps 1-4.
fn chain(out: &Path, a: &Args) -> Res<Value> {
    let h = Host::from_env().map_err(e2s)?;
    let movie = read_movie(a.opts.get("movie").ok_or("--movie EVENTS")?)?;
    let (ra, rb, obj) = (a.num("room-a")? as i32, a.num("room-b")? as i32, a.num("object")? as u64);
    let last = movie.iter().map(|e| e.frame).max().ok_or("an empty movie")?;
    let mut res = json!({});
    // (1) boot to the movie's last click, then frame by frame to room A, then to the first idle
    let t = Instant::now();
    let cfg = BootCfg { seed: 1, events: movie.clone(), frame: Some(last), dump_locals: false, lean: true, out_dir: out.join("boot") };
    let mut e: Box<dyn Engine> = Box::new(h.boot_bin(&cfg).map_err(e2s)?);
    let mut load = None;
    while e.status().frame < last + afford::W {
        let f = e.status().frame + 1;
        let st = e.run(&RunSpec { frame: Some(f), ..Default::default() }).map_err(e2s)?;
        if st.room == ra {
            load = Some(st.frame);
            break;
        }
    }
    let load = load.ok_or(format!("room {ra} not loaded within {} frames of the last click", afford::W))?;
    let st = e.run(&RunSpec { idle: true, max_frame: Some(load + afford::W), ..Default::default() }).map_err(e2s)?;
    res["step1"] = json!({"room": ra, "load_frame": load, "idle_frame": st.frame, "idle": st.idle, "reason": st.reason,
                          "rng": st.rng, "xstart": st.xstart, "seconds": t.elapsed().as_secs_f64()});
    eprintln!("step 1: {}", res["step1"]);
    if st.reason != "idle" {
        return Ok(res);
    }
    // (2) room A to room B
    let ego_key = Key::pack(K_EGO, 0, 0, 0);
    let c2 = cfg_of(a, 4, 5000, false, ra, true)?;
    let mut d2 = |s: &Stop| Seen { goal: s.st.room == rb, extra: Value::Null };
    let o2 = search(&mut e, &c2, &mut d2)?;
    write_trials(out, "step2", &o2)?;
    res["step2"] = summary(&o2);
    eprintln!("step 2: {}", res["step2"]);
    let g2 = match o2.goal.clone() {
        Some(g) => g,
        None => {
            res["verdict"] = json!("not shown: no stop in room B within the limits");
            e.quit();
            return Ok(res);
        }
    };
    let mut m2 = movie.clone();
    m2.extend(g2.path.iter().cloned());
    res["step2"]["verify"] = verify(&h, &out.join("verify2"), &m2, g2.frame, &g2.facts, g2.room, g2.digest)?;
    // (3) room B's first idle, then to owner:O = ego
    let n = Node { path: g2.path.clone(), names: vec![], frame: g2.frame, digest: g2.digest, depth: 0, trial: Some(g2.trial) };
    let b = remake(&mut e, &n)?.ok_or("room B's stop not made again (digest)")?;
    let mut b = idle(b)?;
    res["step3_start"] = json!({"frame": b.status().frame, "idle": b.status().idle, "rng": b.status().rng, "room": b.status().room});
    let c3 = cfg_of(a, 4, 5000, false, rb, true)?;
    let ok = Key::owner(obj);
    let mut d3 = |s: &Stop| {
        let (ow, eg) = (s.facts.val(ok), s.facts.val(ego_key));
        Seen { goal: eg != 0 && ow == eg, extra: json!({"owner": ow, "ego": eg}) }
    };
    let o3 = search(&mut b, &c3, &mut d3)?;
    write_trials(out, "step3", &o3)?;
    res["step3"] = summary(&o3);
    eprintln!("step 3: {}", res["step3"]);
    let mut owners: BTreeMap<i64, usize> = BTreeMap::new();
    for t in &o3.trials {
        *owners.entry(t["seen"]["owner"].as_i64().unwrap_or(-1)).or_default() += 1;
    }
    res["step3"]["owner_values"] = json!(owners.iter().map(|(k, v)| json!([k, v])).collect::<Vec<_>>());
    b.quit();
    e.quit();
    let g3 = match o3.goal.clone() {
        Some(g) => g,
        None => {
            res["verdict"] = json!("not shown: owner = ego not reached within the limits");
            return Ok(res);
        }
    };
    // (4) the whole chain from boot
    let mut m3 = m2.clone();
    m3.extend(g3.path.iter().cloned());
    res["step4"] = verify(&h, &out.join("verify4"), &m3, g3.frame, &g3.facts, g3.room, g3.digest)?;
    res["step4"]["movie"] = json!(m3.iter().map(ev_short).collect::<Vec<_>>());
    eprintln!("step 4: {}", res["step4"]);
    Ok(res)
}

/// An archive's entry movie (its chain's events up to its frame) and frame (afford-check's `movie`).
fn entry_movie(arch: &Path, idx: u64) -> Res<(Vec<Event>, u64)> {
    let read = |i: u64| fmi_explore::files::read_gz(&arch.join("entries").join(format!("{i}.json.gz")));
    let e = read(idx)?;
    let frame = e["frame"].as_u64().ok_or("entry without a frame")?;
    let mut chain = vec![e];
    while let Some(p) = chain.last().unwrap().get("parent").and_then(|p| p.as_u64()) {
        chain.push(read(p)?);
    }
    chain.reverse();
    let evs = chain
        .iter()
        .flat_map(|c| c["events"].as_array().cloned().unwrap_or_default())
        .filter(|v| v["frame"].as_u64().is_some_and(|f| f <= frame))
        .filter_map(|v| event_of(&v))
        .collect();
    Ok((evs, frame))
}

/// Steps 5 and 6.
fn sweep(out: &Path, a: &Args) -> Res<Value> {
    let h = Host::from_env().map_err(e2s)?;
    let arch = PathBuf::from(&a.opts["archive"]);
    let entries: Vec<u64> = a.opts.get("entries").ok_or("--entries")?.split(',').map(|x| x.parse().map_err(e2s)).collect::<Res<_>>()?;
    let room = a.num("room")? as i32;
    let local = a.num("local")? as i32;
    let (cobj, cls) = (a.num("class-obj")? as u64, a.num("class")? as u64);
    let (back, home) = (a.num("room-back")? as i32, a.num("room-home")? as i32);
    let ck = Key::class(cobj, cls);
    let lk = Key::pack(K_LOCAL, 0, local as u64, 0);
    let mut rows = Vec::new();
    let mut tot: BTreeMap<&str, usize> = BTreeMap::new();
    let mut backs: Vec<(u64, Vec<Event>, u64, u64)> = Vec::new(); // (entry, path, frame, digest)
    let mut seen_back: HashSet<u64> = HashSet::new();
    let mut step6: Vec<Value> = Vec::new();
    let mut found6: Option<Value> = None;
    for &idx in &entries {
        let (movie, frame) = entry_movie(&arch, idx)?;
        let cfg = BootCfg { seed: 1, events: movie.clone(), frame: Some(frame), dump_locals: false, lean: true, out_dir: out.join("boot") };
        let e: Box<dyn Engine> = Box::new(h.boot_bin(&cfg).map_err(e2s)?);
        let mut e = idle(e)?;
        let f0 = e.facts().map_err(e2s)?;
        let c5 = cfg_of(a, 2, 3000, true, room, false)?;
        let mut d5 = |s: &Stop| {
            let started = s.log.records.iter().any(|r| {
                r.writes.iter().any(|w| w.kind == TouchKind::Started && w.a == local && w.b == WIO_LOCAL)
            });
            let live = s.facts.val(lk) != 0;
            let cleared = s.facts.val(ck) == 0;
            let ran = s.runs.iter().any(|r| r.where_ == WIO_LOCAL && r.number == local && r.room == room);
            Seen { goal: false, extra: json!({"started": started, "live": live, "cleared": cleared, "ran": ran}) }
        };
        let o5 = search(&mut e, &c5, &mut d5)?;
        write_trials(out, &format!("step5-{idx}"), &o5)?;
        let cnt = |k: &str| o5.trials.iter().filter(|t| t["seen"][k] == json!(true)).count();
        let row = json!({"entry": idx, "frame": e.status().frame, "entry_frame": frame,
                         "class_at_entry": f0.val(ck), "local_live_at_entry": f0.val(lk) != 0,
                         "started": cnt("started"), "live": cnt("live"), "cleared": cnt("cleared"), "ran": cnt("ran"),
                         "search": summary(&o5)});
        for k in ["started", "live", "cleared", "ran"] {
            *tot.entry(k).or_default() += cnt(k);
        }
        *tot.entry("trials").or_default() += o5.trials.len();
        eprintln!("step 5: {row}");
        rows.push(row);
        // the stops in room RB, distinct by digest
        for t in &o5.trials {
            if t["room"].as_i64() == Some(back as i64) && t["ended"] != json!(true) && t.get("refused").is_none() {
                let d = u64::from_str_radix(t["digest"].as_str().unwrap_or("0"), 16).unwrap_or(0);
                if seen_back.insert(d) {
                    let path: Vec<Event> = t["path"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|p| Event { frame: p[0].as_u64().unwrap(), what: Input::Click(p[1].as_i64().unwrap() as i32, p[2].as_i64().unwrap() as i32) })
                        .collect();
                    backs.push((idx, path, t["frame"].as_u64().unwrap(), d));
                }
            }
        }
        // (6) from this entry's stops in room RB, while its engine is up
        if a.opts.get("steps").map_or(true, |s| s.contains('6')) {
            let mine: Vec<(u64, Vec<Event>, u64, u64)> = backs.iter().filter(|b| b.0 == idx).cloned().collect();
            for (k, (_, path, fr, dg)) in mine.into_iter().enumerate() {
                if found6.is_some() || step6.len() >= 10 {
                    break;
                }
                let n = Node { path: path.clone(), names: vec![], frame: fr, digest: dg, depth: 0, trial: Some(0) };
                let Some(b) = remake(&mut e, &n)? else {
                    step6.push(json!({"entry": idx, "frame": fr, "remade": false}));
                    continue;
                };
                let mut b = match idle(b) {
                    Ok(b) => b,
                    Err(why) => {
                        step6.push(json!({"entry": idx, "frame": fr, "idle": why}));
                        continue;
                    }
                };
                let c6 = cfg_of(a, 2, 3000, true, back, false)?;
                let mut d6 = |s: &Stop| Seen { goal: s.st.room == home, extra: Value::Null };
                let o6 = search(&mut b, &c6, &mut d6)?;
                let tag = format!("step6-{idx}-{k}");
                write_trials(out, &tag, &o6)?;
                let mut r6 = json!({"entry": idx, "stop_frame": fr, "start_frame": b.status().frame, "search": summary(&o6)});
                if let Some(g) = &o6.goal {
                    // the whole chain from boot: the entry's movie, the clicks to room RB, the sweep's clicks
                    let mut m = movie.clone();
                    m.extend(path.iter().cloned());
                    m.extend(g.path.iter().cloned());
                    r6["verify"] = verify(&h, &out.join("verify6"), &m, g.frame, &g.facts, g.room, g.digest)?;
                    found6 = Some(r6.clone());
                }
                eprintln!("step 6: {r6}");
                b.quit();
                step6.push(r6);
            }
        }
        e.quit();
    }
    Ok(json!({"step5": {"entries": rows, "totals": tot}, "step6": {"stops_in_back_room": backs.len(), "sweeps": step6,
                                                               "found": found6}}))
}

// --- step 5's static half ------------------------------------------------------------------------------------------

/// A place in the extraction by room and kind and number only: "room-042/local-212", "global/script-004" (the file
/// names after the number carry game text and are cut).
fn place(dir: &Path, file: &Path) -> String {
    let d = dir.file_name().and_then(|s| s.to_str()).unwrap_or("");
    let room = d.split('-').take(2).collect::<Vec<_>>().join("-");
    let stem = file.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    let kind = stem.split('-').take(2).collect::<Vec<_>>().join("-");
    format!("{room}/{kind}")
}

/// The offset of a descumm line ("[00EC] ...").
fn offset(l: &str) -> String {
    l.trim_start().strip_prefix('[').and_then(|r| r.split(']').next()).unwrap_or("").to_string()
}

fn grep(root: &Path, a: &Args) -> Res<Value> {
    let local = a.num("local")?;
    let (cobj, cls) = (a.num("class-obj")?, a.num("class")?);
    let mut lit = Vec::new();
    let mut ind = Vec::new();
    let mut clears = Vec::new();
    let mut assigns: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    let mut files: Vec<(PathBuf, PathBuf)> = Vec::new();
    for d in std::fs::read_dir(root).map_err(e2s)?.flatten() {
        if d.path().is_dir() {
            for f in std::fs::read_dir(d.path()).map_err(e2s)?.flatten() {
                files.push((d.path(), f.path()));
            }
        }
    }
    files.sort();
    let mut texts = Vec::new();
    for (d, f) in &files {
        if let Ok(t) = std::fs::read_to_string(f) {
            texts.push((place(d, f), t));
        }
    }
    for (p, t) in &texts {
        for l in t.lines() {
            for op in ["startScript(", "chainScript(", "startScriptQuick("] {
                if let Some(i) = l.find(op) {
                    let arg = l[i + op.len()..].split([',', ')']).next().unwrap_or("").trim().to_string();
                    if arg == local.to_string() {
                        lit.push(json!({"at": p, "offset": offset(l), "op": op.trim_end_matches('(')}));
                    } else if arg.starts_with("Var[") || arg.starts_with("Local[") {
                        ind.push(json!({"at": p, "offset": offset(l), "op": op.trim_end_matches('('), "arg": arg}));
                    }
                }
            }
            if let Some(i) = l.find("setClass(") {
                let rest = &l[i + 9..];
                let o = rest.split(',').next().unwrap_or("").trim();
                if o == cobj.to_string() {
                    let list: Vec<i64> = rest
                        .split('[')
                        .nth(1)
                        .and_then(|r| r.split(']').next())
                        .map(|r| r.split(',').filter_map(|x| x.trim().parse().ok()).collect())
                        .unwrap_or_default();
                    // SCUMM v5 setClass: a value with bit 0x80 sets class (value & 0x7F), without it clears it
                    if list.iter().any(|&c| c == cls) {
                        clears.push(json!({"at": p, "offset": offset(l), "classes": list}));
                    }
                }
            }
        }
    }
    // the literal values the extraction assigns to each Var named by an indirect start
    let vars: HashSet<String> = ind.iter().filter_map(|x| x["arg"].as_str()).filter(|s| s.starts_with("Var[")).map(String::from).collect();
    for (p, t) in &texts {
        for l in t.lines() {
            for v in &vars {
                let pat = format!("{v} = ");
                if let Some(i) = l.find(&pat) {
                    let val = l[i + pat.len()..].trim_end_matches(';').trim();
                    assigns.entry(v.clone()).or_default().push(json!({"at": p, "offset": offset(l), "value": val}));
                }
            }
        }
    }
    Ok(json!({"local": local, "literal_starts": lit, "indirect_starts": ind, "indirect_var_assigns": assigns,
              "class_obj": cobj, "class": cls, "class_clears": clears}))
}

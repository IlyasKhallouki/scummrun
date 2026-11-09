//! E024 Part 2, item 5 (plan.md, "Scan"): a step found again from the state the steps before it left.
//!
//! * **The forks**: from the base engine E_{k-1} (stopped at the previous step's last input, or at the boot), a child
//!   at every frame t from its stop on: the stop itself by a fork, the later frames by a producer fork's fork_each in
//!   chunks of `Cfg::chunk`, while t can still complete before the best so far (`Bound`). `Cfg::workers` threads take
//!   the children. Whether a step can start at t is not monotone in t: every t is tried, no bisection, no sampling.
//! * **A candidate** (`attempt`): at its stop t the step's first anchor re-placed (label::place_hit: the offer,
//!   afford::offer_cfg; a walk's room point less xstart, checked with `at`), the click at t + 1 and its verdict
//!   (verdict::click). Then each inner input by its timing class (label.rs): "next" at the first frame up to its gap
//!   after the last taken input where it is taken with its identity (each try a fork), "idle" at the first idle stop
//!   after the last taken input. Then the step's verdict (verdict::step) while it can still come first (`Bound`).
//! * **The bound** (`Bound`): by the objective, (tick, frame, t), so that ties go to the earliest t whatever order the
//!   workers finish in. A candidate runs to the best's frame (item 4's max_frame, at the best's own frame since the
//!   key breaks a tie), and past it only while its tick is below the best's: frames are not all as long (a skipped
//!   cutscene's are shorter), and a completion at a later frame with a lower tick was lost to a prune by frame (the
//!   review of stage A).
//! * **The result**: the earliest completion by (tick, frame, t), its inputs at their frames; with counts of what
//!   each t met and the candidates' costs (G6).
//! * **A candidate's engine error** (its process gone, a bridge error) is that t's outcome, counted with its message
//!   (the review of stage A: one candidate's crash ended the pass); only the base's or the producer's ends the scan.

use crate::label::{self, Anchor, HitId, Identity, Timing};
use crate::verdict::{self, Click, Goal, Run};
use crate::Res;
use fmi_core::*;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::sync::Mutex;
use std::time::Instant;

/// One input of a step as the scan places it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct In {
    pub anchor: Anchor,
    pub timing: Timing,
    /// frames from the last taken input in the movie
    pub gap: u64,
    /// the movie's identity of it
    pub want: Identity,
    /// the movie's input (its kind)
    pub orig: Input,
}

/// A step as the scan finds it again.
#[derive(Clone, Debug)]
pub struct StepSpec {
    pub k: usize,
    pub inputs: Vec<In>,
    pub goal: Goal,
    /// the goal with its draw keys judged as changed from the root (pass.rs), when it holds one
    pub relaxed: Option<Goal>,
}

#[derive(Clone, Copy, Debug)]
pub struct Cfg {
    pub workers: usize,
    /// children per fork_each
    pub chunk: u32,
}

impl Default for Cfg {
    fn default() -> Cfg {
        Cfg { workers: 8, chunk: 16 }
    }
}

/// A completion: the stop t the step's first input was placed at, the inputs at their frames, the completion.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Found {
    pub t: u64,
    pub events: Vec<Event>,
    pub frame: u64,
    pub tick: u64,
}

impl Found {
    fn key(&self) -> (u64, u64, u64) {
        (self.tick, self.frame, self.t)
    }
    pub fn json(&self) -> Value {
        json!({"t": self.t, "frame": self.frame, "tick": self.tick,
               "events": self.events.iter().map(fmi_explore::events::event_json).collect::<Vec<_>>()})
    }
}

/// What a candidate met.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// t could no longer complete before the best: not tried
    Pruned,
    /// the first anchor: no point gives it
    NoPoint,
    /// the first click: no record, or dropped or ignored
    Refused,
    /// the first click: taken with another identity
    Other,
    /// an inner input (its index in the step): no point, or not taken with its identity
    InnerNoPoint(usize),
    InnerRefused(usize),
    /// the goal not within max_frame (the goal's conditions that did not hold there, for the first few)
    Late(Vec<String>),
    /// the game ended with another reason
    Ended(String),
    /// the candidate's engine failed (the error)
    Error(String),
    Done(Found),
}

/// The counts of what the t's met.
#[derive(Clone, Debug, Default)]
pub struct Counts {
    pub tried: usize,
    pub pruned: usize,
    pub no_point: usize,
    pub refused: usize,
    pub other: usize,
    pub inner_no_point: usize,
    pub inner_refused: usize,
    pub late: usize,
    pub ended: usize,
    pub error: usize,
    pub done: usize,
}

impl Counts {
    fn add(&mut self, o: &Outcome) {
        self.tried += !matches!(o, Outcome::Pruned) as usize;
        match o {
            Outcome::Pruned => self.pruned += 1,
            Outcome::NoPoint => self.no_point += 1,
            Outcome::Refused => self.refused += 1,
            Outcome::Other => self.other += 1,
            Outcome::InnerNoPoint(_) => self.inner_no_point += 1,
            Outcome::InnerRefused(_) => self.inner_refused += 1,
            Outcome::Late(_) => self.late += 1,
            Outcome::Ended(_) => self.ended += 1,
            Outcome::Error(_) => self.error += 1,
            Outcome::Done(_) => self.done += 1,
        }
    }
    pub fn json(&self) -> Value {
        json!({"tried": self.tried, "pruned": self.pruned, "no_point": self.no_point, "refused": self.refused,
               "other": self.other, "inner_no_point": self.inner_no_point, "inner_refused": self.inner_refused,
               "late": self.late, "ended": self.ended, "error": self.error, "done": self.done})
    }
}

/// G6 (plan.md): a refused candidate's budget, and an accepted one's plus a frame's.
pub const BUDGET_MS: f64 = 3.0;
pub const BUDGET_MS_PER_FRAME: f64 = 0.15;

/// The candidates' costs (G6): those whose first click the game refused (or took with another identity), those
/// whose first anchor no point gave (no click: not G6's refused candidate, the review of stage A), as (n, ms, max ms);
/// and those whose first click was taken (wall ms and frames run).
#[derive(Clone, Debug, Default)]
pub struct Cost {
    pub refused: (usize, f64, f64),
    /// refused candidates over BUDGET_MS
    pub refused_over: usize,
    pub no_point: (usize, f64, f64),
    pub accepted: Vec<(f64, u64)>,
}

/// Where a candidate stopped, for its cost.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Stopped {
    NoPoint,
    Click,
    Taken,
}

fn add3(b: &mut (usize, f64, f64), ms: f64) {
    b.0 += 1;
    b.1 += ms;
    b.2 = b.2.max(ms);
}

fn merge3(b: &mut (usize, f64, f64), o: &(usize, f64, f64)) {
    b.0 += o.0;
    b.1 += o.1;
    b.2 = b.2.max(o.2);
}

impl Cost {
    fn add(&mut self, ms: f64, frames: u64, at: Stopped) {
        match at {
            Stopped::NoPoint => add3(&mut self.no_point, ms),
            Stopped::Click => {
                add3(&mut self.refused, ms);
                self.refused_over += (ms > BUDGET_MS) as usize;
            }
            Stopped::Taken => self.accepted.push((ms, frames)),
        }
    }
    pub fn merge(&mut self, o: &Cost) {
        merge3(&mut self.refused, &o.refused);
        self.refused_over += o.refused_over;
        merge3(&mut self.no_point, &o.no_point);
        self.accepted.extend(o.accepted.iter().copied());
    }
    /// The accepted candidates' least-squares line ms = a + b * frames.
    pub fn fit(&self) -> Option<(f64, f64)> {
        let n = self.accepted.len() as f64;
        if n < 2.0 {
            return None;
        }
        let (sx, sy) = self.accepted.iter().fold((0.0, 0.0), |a, &(ms, f)| (a.0 + f as f64, a.1 + ms));
        let (mx, my) = (sx / n, sy / n);
        let (sxx, sxy) = self.accepted.iter().fold((0.0, 0.0), |a, &(ms, f)| (a.0 + (f as f64 - mx).powi(2), a.1 + (f as f64 - mx) * (ms - my)));
        if sxx == 0.0 {
            return None;
        }
        let b = sxy / sxx;
        Some((my - b * mx, b))
    }
    pub fn json(&self) -> Value {
        let (n, sum, max) = self.refused;
        let (pn, psum, pmax) = self.no_point;
        let frames: u64 = self.accepted.iter().map(|a| a.1).sum();
        let ms: f64 = self.accepted.iter().map(|a| a.0).sum();
        // G6's budget for an accepted candidate: BUDGET_MS plus BUDGET_MS_PER_FRAME a frame run
        let over = self.accepted.iter().filter(|&&(ms, f)| ms > BUDGET_MS + BUDGET_MS_PER_FRAME * f as f64).count();
        json!({"refused": {"n": n, "mean_ms": if n > 0 { sum / n as f64 } else { 0.0 }, "max_ms": max, "over_budget": self.refused_over},
               "no_point": {"n": pn, "mean_ms": if pn > 0 { psum / pn as f64 } else { 0.0 }, "max_ms": pmax},
               "accepted": {"n": self.accepted.len(), "ms": ms, "frames": frames, "over_budget": over,
                            "fit": self.fit().map(|(a, b)| json!({"ms": a, "ms_per_frame": b}))}})
    }
}

/// Late candidates whose goal's misses are asked (one condition at a time: a round trip each).
pub const MISS_ASKED: usize = 4;
/// Candidates' engine errors kept with their message.
pub const ERRORS_KEPT: usize = 4;

/// A step's scan.
#[derive(Clone, Debug, Default)]
pub struct Scan {
    pub found: Option<Found>,
    pub counts: Counts,
    pub cost: Cost,
    /// the conditions that did not hold at the stop of the first late candidates: (condition, candidates)
    pub misses: BTreeMap<String, usize>,
    /// the first candidates' engine errors: (t, the error)
    pub errors: Vec<(u64, String)>,
    /// the base's stop (the first t) and the latest completion frame allowed
    pub from: u64,
    pub limit: u64,
    /// the last t a child was made at
    pub to: u64,
    pub wall_s: f64,
}

impl Scan {
    pub fn json(&self) -> Value {
        json!({"from": self.from, "limit": self.limit, "to": self.to, "wall_s": self.wall_s,
               "found": self.found.as_ref().map(Found::json), "counts": self.counts.json(), "cost": self.cost.json(),
               "misses": self.misses, "errors": self.errors})
    }
}

/// A candidate's bound (module docstring): its completion at most at `limit`, and before the best so far.
struct Bound<'a> {
    t: u64,
    /// the tick at t's stop
    tick: u64,
    limit: u64,
    best: &'a Mutex<Option<Found>>,
}

impl Bound<'_> {
    fn best(&self) -> Option<(u64, u64, u64)> {
        self.best.lock().unwrap().as_ref().map(Found::key)
    }

    /// Whether a completion at `frame`, its tick at least `tick`, can still come before the best.
    fn open(&self, tick: u64, frame: u64) -> bool {
        frame <= self.limit && self.best().is_none_or(|b| (tick, frame, self.t) < b)
    }

    /// The frame a run from a stop at (`frame`, `tick`) may go to: the limit while no best is found; the best's frame
    /// before it; past it, the frames the ticks left before the best's take at this candidate's pace (at least one).
    fn cap(&self, frame: u64, tick: u64) -> u64 {
        let c = match self.best() {
            None => self.limit,
            Some((_, bf, _)) if frame < bf => bf,
            Some((bt, _, _)) => {
                let pace = (tick.saturating_sub(self.tick) / frame.saturating_sub(self.t).max(1)).max(1);
                frame + (bt.saturating_sub(tick) / pace).max(1)
            }
        };
        c.min(self.limit)
    }
}

/// Item 5: the step `spec` from `base`'s stop on, every t whose candidate can complete by `limit` (a frame) or before
/// the best found. `base` is left where it was.
pub fn scan(base: &mut Box<dyn Engine>, spec: &StepSpec, limit: u64, cfg: &Cfg) -> Res<Scan> {
    let t0 = Instant::now();
    let from = base.status().frame;
    let mut out = Scan { from, limit, to: from, ..Default::default() };
    if spec.inputs.is_empty() {
        return Err(format!("step {}: no input to place", spec.k));
    }
    let best: Mutex<Option<Found>> = Mutex::new(None);
    let tally: Mutex<(Counts, Cost, BTreeMap<String, usize>, Vec<(u64, String)>)> = Mutex::new(Default::default());
    let asked = AtomicUsize::new(0);
    let (tx, rx) = mpsc::sync_channel::<(u64, Box<dyn Engine>)>(cfg.workers.max(1) * 2);
    let rx = Mutex::new(rx);
    // the producer's error (the base's fork, a fork_each): it ends the scan
    let err: Mutex<Option<String>> = Mutex::new(None);
    let produced = std::thread::scope(|sc| -> Res<u64> {
        for _ in 0..cfg.workers.max(1) {
            sc.spawn(|| loop {
                let next = rx.lock().unwrap().recv();
                let Ok((t, child)) = next else { break };
                let s = Instant::now();
                let (o, frames) = attempt(child, t, spec, &best, limit, &asked).unwrap_or_else(|e| (Outcome::Error(e.to_string()), 0));
                let ms = s.elapsed().as_secs_f64() * 1e3;
                let at = match o {
                    Outcome::NoPoint => Stopped::NoPoint,
                    Outcome::Refused | Outcome::Other => Stopped::Click,
                    _ => Stopped::Taken,
                };
                let mut g = tally.lock().unwrap();
                g.0.add(&o);
                if !matches!(o, Outcome::Pruned | Outcome::Error(_)) {
                    g.1.add(ms, frames, at);
                }
                match &o {
                    Outcome::Late(m) => {
                        for c in m {
                            *g.2.entry(c.clone()).or_default() += 1;
                        }
                    }
                    Outcome::Error(e) if g.3.len() < ERRORS_KEPT => g.3.push((t, e.clone())),
                    _ => {}
                }
                drop(g);
                if let Outcome::Done(f) = o {
                    let mut b = best.lock().unwrap();
                    if b.as_ref().is_none_or(|x| f.key() < x.key()) {
                        *b = Some(f);
                    }
                }
            });
        }
        let r = produce(base, &tx, &best, limit, cfg, &err);
        drop(tx);
        r
    });
    if let Some(e) = err.into_inner().unwrap() {
        return Err(format!("step {}: {e}", spec.k));
    }
    out.to = produced?;
    let (c, k, m, x) = tally.into_inner().unwrap();
    out.counts = c;
    out.cost = k;
    out.misses = m;
    out.errors = x;
    out.found = best.into_inner().unwrap();
    out.wall_s = t0.elapsed().as_secs_f64();
    Ok(out)
}

/// The producer: a child at the base's stop, then fork_each chunks from a fork of the base while the next t can still
/// complete before the best. Returns the last t made.
fn produce(
    base: &mut Box<dyn Engine>,
    tx: &mpsc::SyncSender<(u64, Box<dyn Engine>)>,
    best: &Mutex<Option<Found>>,
    limit: u64,
    cfg: &Cfg,
    err: &Mutex<Option<String>>,
) -> Res<u64> {
    let e2s = |e: EngineError| e.to_string();
    let from = base.status().frame;
    tx.send((from, base.fork().map_err(e2s)?)).map_err(|e| e.to_string())?;
    let mut p = base.fork().map_err(e2s)?;
    let mut last = from;
    loop {
        if err.lock().unwrap().is_some() {
            break;
        }
        let (pf, ptk) = (p.status().frame, p.status().tick);
        let next = pf + 1;
        // a click at t + 1 completes at t + 1 at the earliest, its tick at least the producer's
        let b = Bound { t: next, tick: ptk, limit, best };
        let last_t = b.cap(pf, ptk).saturating_sub(1);
        if !b.open(ptk, next + 1) || next > last_t {
            break;
        }
        let n = (last_t - next + 1).min(cfg.chunk.max(1) as u64) as u32;
        match p.fork_each(n) {
            Ok((_, kids)) => {
                for k in kids {
                    last = k.status().frame;
                    tx.send((last, k)).map_err(|e| e.to_string())?;
                }
            }
            // the game ended with no input on the way (the children made before it are gone)
            Err(EngineError::Ended(_)) => return Ok(last),
            Err(x) => return Err(x.to_string()),
        }
    }
    p.quit();
    Ok(last)
}

/// `e` run to the stop of `frame` (no input given on the way).
fn run_to(e: &mut Box<dyn Engine>, frame: u64) -> EResult<()> {
    if e.status().frame < frame {
        e.run(&RunSpec { frame: Some(frame), ..Default::default() })?;
    }
    Ok(())
}

/// The click `inp` at `frame` (one frame after `e`'s stop) and its verdict against `want`, its point's hit at the
/// stop being `hit`. Err(Ended) when the game ended in the click's frame.
fn click_at(e: &mut Box<dyn Engine>, inp: &Input, frame: u64, hit: HitId, want: &Identity) -> EResult<Click> {
    e.inputs(&[Event { frame, what: inp.clone() }])?;
    run_to(e, frame)?;
    let recs = e.input_log()?.records;
    Ok(verdict::click(&recs, frame, hit, want))
}

/// One candidate: the child `x` at stop t (module docstring). Returns its outcome and the frames it ran; `x` is quit
/// (an engine that ended is quit as well: its process is gone and quitting it does nothing).
pub fn attempt(
    mut x: Box<dyn Engine>,
    t: u64,
    spec: &StepSpec,
    best: &Mutex<Option<Found>>,
    limit: u64,
    asked: &AtomicUsize,
) -> EResult<(Outcome, u64)> {
    let b = Bound { t, tick: x.status().tick, limit, best };
    if !b.open(b.tick, t + 1) {
        x.quit();
        return Ok((Outcome::Pruned, 0));
    }
    let mut r = play(&mut x, t, spec, &b);
    if let Ok(Outcome::Late(m)) = &mut r {
        if asked.fetch_add(1, Ordering::Relaxed) < MISS_ASKED {
            *m = verdict::misses(&mut x, &spec.goal).unwrap_or_default();
        }
    }
    let n = x.status().frame.saturating_sub(t);
    x.quit();
    r.map(|o| {
        let n = match &o {
            Outcome::Done(f) => f.frame.saturating_sub(t).max(n),
            _ => n,
        };
        (o, n)
    })
}

/// One candidate at `x`'s stop with no best but `limit` (item 7's cheap replay); `x` is left where it stopped.
pub fn once(x: &mut Box<dyn Engine>, spec: &StepSpec, limit: u64) -> EResult<Outcome> {
    let best = Mutex::new(None);
    let t = x.status().frame;
    let b = Bound { t, tick: x.status().tick, limit, best: &best };
    if !b.open(b.tick, t + 1) {
        return Ok(Outcome::Late(Vec::new()));
    }
    play(x, t, spec, &b)
}

/// The outcome of the game's end in a candidate that placed `events`, in an input's frame or while it waited for the
/// next input: the step's completion when the step is the goal's and the end has its reason (with the site), the
/// inputs not placed yet not needed; else an end (the review of stage A: an end in a wait ended the pass).
fn ended_in(spec: &StepSpec, st: &Status, t: u64, events: Vec<Event>) -> Outcome {
    match &spec.goal {
        Goal::End(want) if st.reason == *want => Outcome::Done(Found { t, events, frame: st.frame, tick: st.tick }),
        _ => Outcome::Ended(st.reason.clone()),
    }
}

/// The candidate's inputs and its verdict on `x`.
fn play(x: &mut Box<dyn Engine>, t: u64, spec: &StepSpec, b: &Bound) -> EResult<Outcome> {
    let first = &spec.inputs[0];
    let Some((inp, hit)) = label::place_hit(x, &first.anchor, &first.orig)? else { return Ok(Outcome::NoPoint) };
    let mut events = vec![Event { frame: t + 1, what: inp.clone() }];
    match click_at(x, &inp, t + 1, hit, &first.want) {
        Ok(Click::Taken) => {}
        Ok(Click::Other(_)) => return Ok(Outcome::Other),
        Ok(Click::NoRecord | Click::Refused(_)) => return Ok(Outcome::Refused),
        Err(EngineError::Ended(st)) => return Ok(ended_in(spec, &st, t, events)),
        Err(e) => return Err(e),
    }
    let mut last = t + 1;
    for (i, inn) in spec.inputs.iter().enumerate().skip(1) {
        match inner(x, inn, last, b)? {
            Inner::Taken(ev) => {
                last = ev.frame;
                events.push(ev);
            }
            Inner::NoPoint => return Ok(Outcome::InnerNoPoint(i)),
            Inner::Refused => return Ok(Outcome::InnerRefused(i)),
            Inner::Late => return Ok(Outcome::Late(Vec::new())),
            Inner::Ended(st, ev) => {
                events.push(ev);
                return Ok(ended_in(spec, &st, t, events));
            }
            Inner::EndedBefore(st) => return Ok(ended_in(spec, &st, t, events)),
        }
    }
    // the verdict to the bound's cap, again from there while the bound is open
    loop {
        let (f, tk) = (x.status().frame, x.status().tick);
        if !b.open(tk, f + 1) {
            return Ok(Outcome::Late(Vec::new()));
        }
        match verdict::step(x, &spec.goal, b.cap(f, tk))? {
            Run::Done(st) => return Ok(Outcome::Done(Found { t, events, frame: st.frame, tick: st.tick })),
            Run::Late(_) => {}
            Run::Ended(r) => return Ok(Outcome::Ended(r)),
        }
    }
}

enum Inner {
    Taken(Event),
    NoPoint,
    Refused,
    Late,
    /// the game ended in the input's frame
    Ended(Box<Status>, Event),
    /// the game ended before the input was placed
    EndedBefore(Box<Status>),
}

/// An inner input placed by its timing class after the last taken input at `last` (`x` stopped there); `x` goes on
/// from the stop of its frame.
fn inner(x: &mut Box<dyn Engine>, inn: &In, last: u64, b: &Bound) -> EResult<Inner> {
    match inn.timing {
        Timing::Next => {
            let mut why = Inner::NoPoint;
            for g in 1..=inn.gap.max(1) {
                let s = last + g - 1;
                if s + 1 > b.limit {
                    return Ok(Inner::Late);
                }
                match run_to(x, s) {
                    Ok(()) => {}
                    Err(EngineError::Ended(st)) => return Ok(Inner::EndedBefore(st)),
                    Err(e) => return Err(e),
                }
                if !b.open(x.status().tick, s + 1) {
                    return Ok(Inner::Late);
                }
                let mut y = x.fork()?;
                let Some((inp, hit)) = label::place_hit(&mut y, &inn.anchor, &inn.orig)? else {
                    y.quit();
                    continue;
                };
                let ev = Event { frame: s + 1, what: inp.clone() };
                match click_at(&mut y, &inp, s + 1, hit, &inn.want) {
                    Ok(Click::Taken) => {
                        std::mem::swap(x, &mut y);
                        y.quit();
                        return Ok(Inner::Taken(ev));
                    }
                    Ok(_) => why = Inner::Refused,
                    Err(EngineError::Ended(st)) => return Ok(Inner::Ended(st, ev)),
                    Err(e) => {
                        y.quit();
                        return Err(e);
                    }
                }
                y.quit();
            }
            Ok(why)
        }
        Timing::Idle => {
            // the idle stop s, with the click at s + 1 still within the bound
            let st = loop {
                let (f, tk) = (x.status().frame, x.status().tick);
                if !b.open(tk, f + 2) {
                    return Ok(Inner::Late);
                }
                let cap = b.cap(f, tk).saturating_sub(1).max(f + 1);
                match x.run(&RunSpec { idle: true, max_frame: Some(cap), ..Default::default() }) {
                    Ok(st) if st.reason == "idle" => break st,
                    Ok(_) => {}
                    Err(EngineError::Ended(st)) => return Ok(Inner::EndedBefore(st)),
                    Err(e) => return Err(e),
                }
            };
            if !b.open(st.tick, st.frame + 1) {
                return Ok(Inner::Late);
            }
            let s = st.frame;
            let Some((inp, hit)) = label::place_hit(x, &inn.anchor, &inn.orig)? else { return Ok(Inner::NoPoint) };
            let ev = Event { frame: s + 1, what: inp.clone() };
            Ok(match click_at(x, &inp, s + 1, hit, &inn.want) {
                Ok(Click::Taken) => Inner::Taken(ev),
                Ok(_) => Inner::Refused,
                Err(EngineError::Ended(st)) => Inner::Ended(st, ev),
                Err(e) => return Err(e),
            })
        }
    }
}

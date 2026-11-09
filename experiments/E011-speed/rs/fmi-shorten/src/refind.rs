//! E024 Part 2, item 6 (plan.md, "Fallbacks"): a step the scan does not complete within the original step's length
//! plus afford::W, found again another way. In order, the first that completes:
//!
//! * **realize** (fmi-play realize.rs): the step's sentences (each input's Sentence touch in the movie, (v, a, b)) and
//!   dialog lines (its Dialog anchors) realized from the first idle stop by the engine's own search (the approach, the
//!   clicks judged by the input log), each from the stop the one before left, run to the next idle stop between them.
//! * **offer trials** (afford.rs `offer_cfg`, `inputs_of`): at the first idle stop, every input the interface offers
//!   there (each verb slot, each verb then each target, each target, each walk box's point; M4's two-frame clicks),
//!   each in a fork, judged by the step's goal up to the original step's length.
//! * **the dialog lines** (`dialogs`): a depth-first search over the verb slots offered at each idle stop whose input
//!   handler is one the movie chose dialog lines under (VAR_VERB_SCRIPT), each line taken by the input log, run on to
//!   the goal or the next idle stop; at most `LEAVES` leaves.
//! * **skips off for the step's span**: the scan again (strict, then relaxed) from a fork with Skips{0,0} the frame
//!   after the base's stop, and Skips{1,1} the frame after the completion (only in a pass under skips).
//!
//! realize, the offer trials and the dialog lines judge by the step's relaxed goal when it has one (the scan showed the
//! strict one completes at no t); every judge is the engine's. No click is shifted or named: a found step's inputs are
//! labelled again (`relabel`: anchor, timing class, identity, as label.rs reads the movie's), so a later pass (item 7)
//! places them by what they act on, as it places the movie's.
//!
//! A trial's engine error (a fork's process gone, a timeout: realize's run, an offer trial, a dialog line or node) is
//! that trial's outcome, counted with its message in the fallback's report, as the scan counts a candidate's (the
//! review of stage B: one trial's error stopped the pass); only an error of the base's fork or of the offer's
//! producer ends the fallbacks.

use crate::label::{self, Anchor, DIALOG};
use crate::scan::{self, Cfg, Found, In, StepSpec};
use crate::verdict::{self, Goal, Run};
use crate::{e2s, Res};
use fmi_core::*;
use fmi_explore::afford;
use fmi_play::realize;
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::sync::{mpsc, Mutex};

/// The dialog search's leaves (plan.md item 6).
pub const LEAVES: usize = 2000;

/// How a step was found.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum How {
    Scan,
    Relaxed,
    Realize,
    Offer,
    Dialogs,
    SkipsOff,
}

impl How {
    pub fn name(self) -> &'static str {
        match self {
            How::Scan => "scan",
            How::Relaxed => "relaxed",
            How::Realize => "realize",
            How::Offer => "offer",
            How::Dialogs => "dialogs",
            How::SkipsOff => "skips_off",
        }
    }
}

/// The fallbacks' settings.
#[derive(Clone, Debug)]
pub struct RCfg {
    pub scan: Cfg,
    /// the pass runs under skips (the last fallback needs them)
    pub skips: bool,
    pub leaves: usize,
    /// the input handlers the movie queued sentences under, and those it chose dialog lines under (label.rs)
    pub sentence: BTreeSet<i32>,
    pub dialog: BTreeSet<i32>,
}

/// A step found by a fallback: how, its completion (its events at their frames: with skips off, the two Skips events
/// with them), its inputs labelled as placed, whether skips were off for its span.
#[derive(Clone, Debug)]
pub struct Refound {
    pub how: How,
    pub found: Found,
    pub inputs: Vec<In>,
    pub skips_off: bool,
}

fn skips(on: bool) -> Input {
    Input::Skips { text: on, cutscenes: on, spare: None }
}

/// A fork of `base` run to frame `after` (the step before's completion) when that is later than its stop: a
/// fallback starts where the step before is done, not at its last input (the scan's t's cover the frames between).
fn start(base: &mut Box<dyn Engine>, after: u64) -> EResult<Box<dyn Engine>> {
    let mut e = base.fork()?;
    if e.status().frame < after {
        if let Err(x) = e.run(&RunSpec { frame: Some(after), ..Default::default() }) {
            e.quit();
            return Err(x);
        }
    }
    Ok(e)
}

/// A fork of `base` at its first idle stop from frame `after` on, at most at `limit` (`base` is left where it was);
/// None: no idle stop.
fn idle_fork(base: &mut Box<dyn Engine>, after: u64, limit: u64) -> EResult<Option<Box<dyn Engine>>> {
    let mut e = match start(base, after) {
        Ok(e) => e,
        Err(EngineError::Ended(_)) => return Ok(None),
        Err(x) => return Err(x),
    };
    if e.status().idle.is_some() {
        return Ok(Some(e));
    }
    match e.run(&RunSpec { idle: true, max_frame: Some(limit), ..Default::default() }) {
        Ok(st) if st.reason == "idle" => Ok(Some(e)),
        Ok(_) | Err(EngineError::Ended(_)) => {
            e.quit();
            Ok(None)
        }
        Err(x) => {
            e.quit();
            Err(x)
        }
    }
}

/// `e` judged by `goal` up to `limit`: the completion, its first stop `t` and the inputs placed.
fn judge(e: &mut Box<dyn Engine>, goal: &Goal, limit: u64, t: u64, events: Vec<Event>) -> EResult<Option<Found>> {
    Ok(match verdict::step(e, goal, limit)? {
        Run::Done(st) => Some(Found { t, events, frame: st.frame, tick: st.tick }),
        _ => None,
    })
}

fn better(best: &Mutex<Option<Found>>, f: Found) {
    let mut b = best.lock().unwrap();
    if b.as_ref().is_none_or(|x| (f.tick, f.frame) < (x.tick, x.frame)) {
        *b = Some(f);
    }
}

/// The step's sentences and dialog lines as realize's steps, in order (module docstring).
pub fn realize_steps(spec: &StepSpec) -> Vec<realize::Step> {
    spec.inputs
        .iter()
        .filter_map(|i| match i.anchor {
            Anchor::Dialog { id } => Some(realize::Step::Dialog { v: id }),
            _ => i.want.sentences.first().map(|&(v, a, b)| realize::Step::Sentence { v, a, b: (b != 0).then_some(b) }),
        })
        .collect()
}

/// realize (module docstring).
fn by_realize(base: &mut Box<dyn Engine>, spec: &StepSpec, goal: &Goal, after: u64, limit: u64) -> Res<(Option<Found>, Value)> {
    let steps = realize_steps(spec);
    if !realize::on() || steps.is_empty() {
        return Ok((None, json!({"why": if steps.is_empty() { "no sentence or dialog line" } else { "FMI_REALIZE is off" }})));
    }
    let Some(mut e) = idle_fork(base, after, limit).map_err(e2s)? else { return Ok((None, json!({"why": "no idle stop"}))) };
    let t = e.status().frame;
    let mut events = Vec::new();
    let names: Vec<String> = steps.iter().map(|s| s.text()).collect();
    let rcfg = realize::Cfg::default();
    for (i, s) in steps.iter().enumerate() {
        if i > 0 && e.status().idle.is_none() {
            match e.run(&RunSpec { idle: true, max_frame: Some(limit), ..Default::default() }) {
                Ok(st) if st.reason == "idle" => {}
                Ok(_) | Err(EngineError::Ended(_)) => {
                    e.quit();
                    return Ok((None, json!({"steps": names, "why": format!("no idle stop before {}", s.text())})));
                }
                Err(x) => return Ok(engine_error(e, &names, x)),
            }
        }
        match realize::realize(&mut e, s, &rcfg) {
            Ok(Ok(r)) => {
                events.extend(r.events);
                std::mem::replace(&mut e, r.engine).quit();
            }
            Ok(Err(m)) => {
                e.quit();
                return Ok((None, json!({"steps": names, "why": format!("{}: {}", s.text(), m.reason)})));
            }
            Err(EngineError::Ended(st)) => {
                e.quit();
                return Ok((None, json!({"steps": names, "why": format!("ended: {}", st.reason)})));
            }
            Err(x) => return Ok(engine_error(e, &names, x)),
        }
    }
    let f = match judge(&mut e, goal, limit, t, events) {
        Ok(f) => f,
        Err(x) => return Ok(engine_error(e, &names, x)),
    };
    e.quit();
    let why = if f.is_some() { "done" } else { "the goal not by the limit" };
    Ok((f, json!({"steps": names, "why": why})))
}

/// realize's engine error (its fork's process gone, a timeout): the fallback's outcome, with its message, not the
/// pass's (the review of stage B); `e` is quit.
fn engine_error(e: Box<dyn Engine>, names: &[String], x: EngineError) -> (Option<Found>, Value) {
    e.quit();
    (None, json!({"steps": names, "why": format!("engine error: {x}"), "error": 1}))
}

/// `run` on a fork of `e` per item, in `workers` threads (a producer forks, the workers take the forks).
fn par<T: Send>(e: &mut Box<dyn Engine>, items: Vec<T>, workers: usize, run: &(dyn Fn(Box<dyn Engine>, T) -> EResult<()> + Sync)) -> Res<()> {
    let (tx, rx) = mpsc::sync_channel::<(T, Box<dyn Engine>)>(workers.max(1) * 2);
    let rx = Mutex::new(rx);
    let err: Mutex<Option<String>> = Mutex::new(None);
    std::thread::scope(|sc| -> Res<()> {
        for _ in 0..workers.max(1) {
            sc.spawn(|| loop {
                let next = rx.lock().unwrap().recv();
                let Ok((it, b)) = next else { break };
                if let Err(x) = run(b, it) {
                    err.lock().unwrap().get_or_insert(x.to_string());
                }
            });
        }
        for it in items {
            if err.lock().unwrap().is_some() {
                break;
            }
            let b = e.fork().map_err(e2s)?;
            tx.send((it, b)).map_err(e2s)?;
        }
        drop(tx);
        Ok(())
    })?;
    err.into_inner().unwrap().map_or(Ok(()), Err)
}

/// The offer trials (module docstring), each up to the original step's length `orig` from the offer's frame.
fn by_offer(base: &mut Box<dyn Engine>, goal: &Goal, after: u64, orig: u64, limit: u64, workers: usize) -> Res<(Option<Found>, Value)> {
    let Some(mut e) = idle_fork(base, after, limit).map_err(e2s)? else { return Ok((None, json!({"why": "no idle stop"}))) };
    let f0 = e.status().frame;
    let limit = (f0 + orig).min(limit);
    let o = e.obs(&ObsReq::default()).map_err(e2s)?;
    let of = afford::offer_cfg(&mut e, &o, afford::boxes_on(), afford::offer_screen_on()).map_err(e2s)?;
    let inputs = afford::inputs_of(&of);
    let n = inputs.len();
    let best: Mutex<Option<Found>> = Mutex::new(None);
    // (refused by the bridge, run, completed, its engine failed)
    let counts = Mutex::new((0usize, 0usize, 0usize, 0usize));
    let errors: Mutex<Vec<(String, String)>> = Mutex::new(Vec::new());
    // a trial's engine error (its fork's process gone, a timeout) is that trial's outcome, counted with its message as
    // the scan counts a candidate's (scan.rs; the review of stage B: one trial's error stopped the pass)
    let failed = |inp: &afford::Inputs, x: EngineError| {
        counts.lock().unwrap().3 += 1;
        let mut v = errors.lock().unwrap();
        if v.len() < scan::ERRORS_KEPT {
            v.push((inp.name(), x.to_string()));
        }
    };
    let trial = |mut b: Box<dyn Engine>, inp: afford::Inputs| -> EResult<()> {
        let evs = inp.events(f0);
        let cap = best.lock().unwrap().as_ref().map_or(limit, |f| f.frame).min(limit);
        match b.inputs(&evs) {
            Ok(()) => {}
            Err(EngineError::Bridge(_)) => {
                counts.lock().unwrap().0 += 1;
                b.quit();
                return Ok(());
            }
            Err(x) => {
                b.quit();
                failed(&inp, x);
                return Ok(());
            }
        }
        let r = judge(&mut b, goal, cap, f0, evs);
        b.quit();
        match r {
            Ok(f) => {
                let mut c = counts.lock().unwrap();
                c.1 += 1;
                if let Some(f) = f {
                    c.2 += 1;
                    drop(c);
                    better(&best, f);
                }
            }
            Err(x) => failed(&inp, x),
        }
        Ok(())
    };
    // only the producer's error (a fork of the offer's engine) ends the trials
    let r = par(&mut e, inputs, workers, &trial);
    e.quit();
    r?;
    let (refused, run, done, error) = counts.into_inner().unwrap();
    let f = best.into_inner().unwrap();
    Ok((f, json!({"at": f0, "offer": of.json(), "inputs": n, "refused": refused, "run": run, "done": done, "error": error,
                  "errors": errors.into_inner().unwrap()})))
}

/// The dialog lines' search (module docstring).
fn by_dialogs(base: &mut Box<dyn Engine>, goal: &Goal, after: u64, limit: u64, cfg: &RCfg) -> Res<(Option<Found>, Value)> {
    if cfg.dialog.is_empty() {
        return Ok((None, json!({"why": "the movie chose no dialog line"})));
    }
    let Some(e) = idle_fork(base, after, limit).map_err(e2s)? else { return Ok((None, json!({"why": "no idle stop"}))) };
    let t = e.status().frame;
    let until = match goal {
        Goal::Keep(c) => c.clone(),
        Goal::End(_) => Vec::new(),
    };
    let mut best: Option<Found> = None;
    let (mut leaves, mut nodes, mut lines) = (0usize, 0usize, 0usize);
    // a node's or a line's engine error (a fork's process gone, a timeout) is that leaf's outcome, counted with its
    // message (the frame the node stopped at) as the scan counts a candidate's (scan.rs; the review of stage B: one
    // line's error stopped the pass)
    let mut error = 0usize;
    let mut errors: Vec<(u64, String)> = Vec::new();
    let mut failed = |f: u64, x: EngineError| {
        error += 1;
        if errors.len() < scan::ERRORS_KEPT {
            errors.push((f, x.to_string()));
        }
    };
    let mut stack: Vec<(Box<dyn Engine>, Vec<Event>)> = vec![(e, Vec::new())];
    while let Some((mut n, evs)) = stack.pop() {
        let cap = best.as_ref().map_or(limit, |f| f.frame).min(limit);
        let f = n.status().frame;
        if leaves >= cfg.leaves || f + 1 > cap {
            leaves += 1;
            n.quit();
            continue;
        }
        nodes += 1;
        let offer = (|| -> EResult<Option<afford::Offer>> {
            let o = n.obs(&ObsReq::default())?;
            let h = o.vars.get(afford::VAR_VERB_SCRIPT).copied().unwrap_or(0);
            if !cfg.dialog.contains(&h) {
                return Ok(None);
            }
            afford::offer_cfg(&mut n, &o, false, afford::offer_screen_on()).map(Some)
        })();
        let of = match offer {
            Ok(Some(of)) => of,
            Ok(None) => {
                leaves += 1;
                n.quit();
                continue;
            }
            Err(x) => {
                leaves += 1;
                failed(f, x);
                n.quit();
                continue;
            }
        };
        // pushed last first: the offer's first line is tried first
        for p in of.verbs.iter().rev() {
            let ev = Event { frame: f + 1, what: Input::Click(p.x, p.y) };
            let mut c = match n.fork() {
                Ok(c) => c,
                Err(x) => {
                    leaves += 1;
                    failed(f, x);
                    break;
                }
            };
            let mut path = evs.clone();
            path.push(ev.clone());
            let step = |c: &mut Box<dyn Engine>| -> EResult<Option<Status>> {
                c.inputs(std::slice::from_ref(&ev))?;
                c.run(&RunSpec { frame: Some(f + 1), ..Default::default() })?;
                let taken = c.input_log()?.records.iter().any(|r| r.frame == f + 1 && r.verdict == Verdict::Taken);
                if !taken {
                    return Ok(None);
                }
                Ok(Some(c.run(&RunSpec { until: until.clone(), idle: true, max_frame: Some(cap), ..Default::default() })?))
            };
            lines += 1;
            match step(&mut c) {
                Ok(Some(st)) if st.reason == "until" => {
                    leaves += 1;
                    best = Some(Found { t, events: path, frame: st.frame, tick: st.tick });
                    c.quit();
                }
                Ok(Some(st)) if st.reason == "idle" => stack.push((c, path)),
                Ok(_) | Err(EngineError::Bridge(_)) => {
                    leaves += 1;
                    c.quit();
                }
                Err(EngineError::Ended(st)) => {
                    leaves += 1;
                    if matches!(goal, Goal::End(w) if *w == st.reason) && best.as_ref().is_none_or(|b| (st.tick, st.frame) < (b.tick, b.frame)) {
                        best = Some(Found { t, events: path, frame: st.frame, tick: st.tick });
                    }
                }
                Err(x) => {
                    leaves += 1;
                    failed(f, x);
                    c.quit();
                }
            }
        }
        n.quit();
    }
    Ok((best, json!({"at": t, "leaves": leaves, "nodes": nodes, "lines": lines, "error": error, "errors": errors})))
}

/// The scan with skips off for the step's span (module docstring), from the step before's completion `after` (the
/// step before's own span keeps its skips).
fn by_skips_off(base: &mut Box<dyn Engine>, spec: &StepSpec, after: u64, limit: u64, cfg: &RCfg) -> Res<(Option<Found>, Value)> {
    if !cfg.skips {
        return Ok((None, json!({"why": "the pass has no skips"})));
    }
    let mut b = match start(base, after) {
        Ok(b) => b,
        Err(EngineError::Ended(st)) => return Ok((None, json!({"why": format!("ended: {}", st.reason)}))),
        Err(x) => return Err(e2s(x)),
    };
    let off = Event { frame: b.status().frame + 1, what: skips(false) };
    let r = (|| -> Res<(Option<Found>, Value)> {
        b.inputs(std::slice::from_ref(&off)).map_err(e2s)?;
        let sc = scan::scan(&mut b, spec, limit, &cfg.scan)?;
        let mut rep = json!({"scan": sc.json()});
        let mut f = sc.found;
        if let (None, Some(g)) = (&f, &spec.relaxed) {
            let r = scan::scan(&mut b, &StepSpec { goal: g.clone(), ..spec.clone() }, limit, &cfg.scan)?;
            rep["relaxed"] = r.json();
            f = r.found;
        }
        Ok((f, rep))
    })();
    b.quit();
    let (f, rep) = r?;
    Ok((
        f.map(|mut f| {
            f.events.insert(0, off);
            f.events.push(Event { frame: f.frame + 1, what: skips(true) });
            f
        }),
        rep,
    ))
}

/// The inputs of `events` (from `base`'s stop) labelled as the movie's are (label.rs): each at the stop before it,
/// its record after it; the inputs the game did not take are left out, as pass.rs leaves the movie's out.
pub fn relabel(base: &mut Box<dyn Engine>, events: &[Event], cfg: &RCfg) -> EResult<Vec<In>> {
    let mut e = base.fork()?;
    let r = (|| -> EResult<Vec<In>> {
        let mut out = Vec::new();
        let mut taken: Option<u64> = None;
        for ev in events {
            if matches!(ev.what, Input::Skips { .. }) {
                e.inputs(std::slice::from_ref(ev))?;
                continue;
            }
            if e.status().frame + 1 < ev.frame {
                e.run(&RunSpec { frame: Some(ev.frame - 1), ..Default::default() })?;
            }
            let seen = label::observe(&mut e, ev)?;
            e.inputs(std::slice::from_ref(ev))?;
            e.run(&RunSpec { frame: Some(ev.frame), ..Default::default() })?;
            let Some(r) = e.input_log()?.records.into_iter().find(|r| r.frame == ev.frame && r.verdict == Verdict::Taken) else { continue };
            let dialog = seen.idle.as_deref() == Some(DIALOG) || (!cfg.sentence.is_empty() && !cfg.sentence.contains(&r.handler));
            let gap = taken.map(|f| ev.frame - f);
            out.push(In {
                anchor: label::anchor(&seen, ev, dialog),
                timing: label::timing(gap, seen.idle.is_some()),
                gap: gap.unwrap_or(0),
                want: label::identity(&r, seen.hit),
                orig: ev.what.clone(),
            });
            taken = Some(ev.frame);
        }
        Ok(out)
    })();
    e.quit();
    r
}

/// Item 6 for the step `spec` from `base`'s stop (`base` is left where it was), each fallback from the step
/// before's completion `after` on: the fallbacks in order (module docstring), the scan's `limit` for all but the
/// offer trials, which run up to the original step's length `orig` past the offer. The first that completes,
/// labelled; and each fallback's report.
pub fn refind(base: &mut Box<dyn Engine>, spec: &StepSpec, orig: u64, after: u64, limit: u64, cfg: &RCfg) -> Res<(Option<Refound>, Vec<Value>)> {
    let goal = spec.relaxed.as_ref().unwrap_or(&spec.goal);
    let mut reps = Vec::new();
    let done = |how: How, r: (Option<Found>, Value), reps: &mut Vec<Value>| -> Option<(How, Found)> {
        let (f, mut v) = r;
        v["how"] = json!(how.name());
        v["found"] = f.as_ref().map_or(Value::Null, Found::json);
        reps.push(v);
        f.map(|f| (how, f))
    };
    let mut hit = done(How::Realize, by_realize(base, spec, goal, after, limit)?, &mut reps);
    if hit.is_none() {
        hit = done(How::Offer, by_offer(base, goal, after, orig, limit, cfg.scan.workers)?, &mut reps);
    }
    if hit.is_none() {
        hit = done(How::Dialogs, by_dialogs(base, goal, after, limit, cfg)?, &mut reps);
    }
    if hit.is_none() {
        hit = done(How::SkipsOff, by_skips_off(base, spec, after, limit, cfg)?, &mut reps);
    }
    let Some((how, found)) = hit else { return Ok((None, reps)) };
    let inputs = if how == How::SkipsOff { spec.inputs.clone() } else { relabel(base, &found.events, cfg).map_err(e2s)? };
    Ok((Some(Refound { how, found, inputs, skips_off: how == How::SkipsOff }), reps))
}

/// Skips off for a step's span from the step before's completion `after` (a later pass's step found so before, item
/// 7): the scan of `spec` with them, strict then relaxed.
pub fn skips_off(base: &mut Box<dyn Engine>, spec: &StepSpec, after: u64, limit: u64, cfg: &RCfg) -> Res<(Option<Found>, Value)> {
    by_skips_off(base, spec, after, limit, cfg)
}

/// One fallback alone from `base`'s stop (item 7's cheap replay: a step an earlier pass found by it), with its limits
/// as `refind` gives them; the scan's own methods are none.
pub fn by(how: How, base: &mut Box<dyn Engine>, spec: &StepSpec, orig: u64, after: u64, limit: u64, cfg: &RCfg) -> Res<Option<Found>> {
    let goal = spec.relaxed.as_ref().unwrap_or(&spec.goal);
    Ok(match how {
        How::Realize => by_realize(base, spec, goal, after, limit)?.0,
        How::Offer => by_offer(base, goal, after, orig, limit, cfg.scan.workers)?.0,
        How::Dialogs => by_dialogs(base, goal, after, limit, cfg)?.0,
        How::SkipsOff => by_skips_off(base, spec, after, limit, cfg)?.0,
        How::Scan | How::Relaxed => None,
    })
}

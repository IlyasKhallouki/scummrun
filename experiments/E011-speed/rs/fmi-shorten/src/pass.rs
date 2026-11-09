//! E024 Part 2, the forward pass (plan.md, "fmi shorten run"): each step of the movie found again in order from boot
//! with item 5's scan, from the state the steps before it left.
//!
//! * **The steps** (`specs`): chain.rs's steps with label.rs's anchors, timing classes and identities; an input the
//!   game refused in the movie (its record's verdict) did nothing there and is not placed. Every step but the last
//!   must reach keep(k) and (`ego`) leave ego where the movie's step left it, not moving: a walk changes no key, and
//!   one cut short by the next input otherwise passes; the last, the game's end with the movie's end reason
//!   (verdict.rs). A step whose twins (item 3) mark a key of its keep(k) or ego's position is scanned again, when no t
//!   reaches the movie's values, with those judged as changed from the root (keys) or not at all (ego's position).
//! * **The pass** (`forward`): the engine E from boot with the boot's events (skips from boot: item 8); for each step
//!   the scan from E's stop, its latest completion allowed the original step's length (from the movie's last input
//!   before it to its completion) plus afford::W after E's stop; then E_k: a fork of E with the found inputs, run to
//!   the last one's frame. A step the scan does not complete goes to the fallbacks (item 6, refind.rs) when the pass
//!   has them; one none of them completes stops the pass, named with what its t's met and each fallback's report; so
//!   does an engine error of the base's (its scan's producer, E_k), named with the error, and the steps found before
//!   it are kept (the review of stage A: a crash lost them all).
//! * **A later pass** (`steps_from`, item 7): from an engine at a step's base stop (`resume`: the route's events
//!   before it from boot), each step placed by the inputs an earlier pass placed it with (`Plan`: a fallback's
//!   inputs labelled, skips off for its span first when they were), the fallbacks again when those do not complete.

use crate::chain::Chain;
use crate::label::{Labels, Timing};
use crate::refind::{self, How, RCfg};
use crate::scan::{self, Cfg, Found, In, Scan, StepSpec};
use crate::verdict::{self, Goal};
use crate::{chain, e2s, Res};
use fmi_core::*;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::time::Instant;

/// A step of the movie as it was: its first stop t (the movie's last input before it, or the boot's stop), its
/// completion, its rooms.
#[derive(Clone, Debug)]
pub struct Orig {
    pub from: u64,
    pub frame: u64,
    pub tick: u64,
    pub rooms: Vec<i64>,
    /// the movie's inputs it holds, and those not placed (refused in the movie)
    pub inputs: usize,
    pub skipped: Vec<usize>,
}

impl Orig {
    pub fn len(&self) -> u64 {
        self.frame.saturating_sub(self.from)
    }
}

/// The steps to find again, and the movie's (item 1-2's labels). With `ego`, a step's goal holds where the movie's
/// step left ego as well (verdict::ego_at): ego's position is no key, and a walk shows in nothing else. A key whose
/// value at step k's end a draw gave (its last change, up to k, at a step whose twins mark it: item 3) is judged by
/// value first, and in the step's relaxed goal as changed from its value in `root` (verdict::conds).
pub fn specs(ch: &Chain, l: &Labels, ego: bool, root: &dyn Fn(Key) -> Val) -> Res<(Vec<StepSpec>, Vec<Orig>)> {
    specs_without(ch, l, ego, root, &BTreeSet::new())
}

/// `specs` without the steps `dropped` (item 7): a later step's keep(k) holds only the keys the steps kept up to it
/// changed, at the movie's values at its end.
pub fn specs_without(ch: &Chain, l: &Labels, ego: bool, root: &dyn Fn(Key) -> Val, dropped: &BTreeSet<usize>) -> Res<(Vec<StepSpec>, Vec<Orig>)> {
    let mut specs = Vec::new();
    let mut origs = Vec::new();
    let mut from = ch.first.frame;
    // per key: whether its last change was a draw's
    let mut drawn: BTreeMap<Key, bool> = BTreeMap::new();
    // the keys the steps kept so far changed
    let mut cum: BTreeSet<Key> = BTreeSet::new();
    for s0 in &ch.steps {
        if dropped.contains(&s0.k) && s0.goal.is_none() {
            from = s0.last;
            continue;
        }
        cum.extend(s0.changed.iter().copied());
        let s = &chain::Step { keep: s0.keep.iter().filter(|(k, _)| cum.contains(k)).copied().collect(), ..s0.clone() };
        let marks: BTreeSet<Key> = s.twin.iter().flat_map(|t| t.draw.iter().copied()).collect();
        for &k in s.changed.iter().chain(marks.iter()) {
            drawn.insert(k, marks.contains(&k));
        }
        let mut inputs = Vec::new();
        let mut skipped = Vec::new();
        for i in s.inputs.clone() {
            let m = &ch.met[i];
            match (&l.ids[i], &m.record) {
                (Some(want), Some(r)) if r.verdict == Verdict::Taken => inputs.push(In {
                    anchor: l.anchors[i].clone(),
                    timing: l.timing[i],
                    gap: l.gap[i].unwrap_or(0),
                    want: want.clone(),
                    orig: m.ev.what.clone(),
                }),
                _ => skipped.push(i),
            }
        }
        if inputs.first().is_some_and(|x| x.timing == Timing::Next && x.gap > 0) && s.k > 0 {
            // a step's first input follows its step's change, never the input before (chain.rs merges those)
            return Err(format!("step {}: its first input is of timing class next", s.k));
        }
        // `drawn_ego`: a twin's ego stood elsewhere at the step's end, so its position is a draw's: not judged
        let keep = |d: &dyn Fn(Key) -> Option<Val>, drawn_ego: bool| -> Res<Goal> {
            let mut c = verdict::conds(&s.keep, d).map_err(|k| format!("step {}: keep(k)'s key {} has no condition", s.k, k.text()))?;
            if let (true, Some((x, y)), true) = (ego, s.end.ego, s.ego > 0) {
                let at = verdict::ego_at(s.ego, x, y);
                c.extend(at.into_iter().filter(|c| !drawn_ego || c.kind == CondKind::ActorMoving));
            }
            Ok(Goal::Keep(c))
        };
        let (goal, relaxed) = match &s.goal {
            Some(reason) => (Goal::End(reason.clone()), None),
            None => {
                let draw = |k: Key| (drawn.get(&k) == Some(&true)).then(|| root(k));
                let ego_drawn = s.twin.as_ref().is_some_and(|t| t.ego);
                let any = ego_drawn || s.keep.iter().any(|&(k, _)| draw(k).is_some());
                (keep(&|_| None, false)?, if any { Some(keep(&draw, ego_drawn)?) } else { None })
            }
        };
        let (frame, tick) = s.completion.ok_or(format!("step {}: no completion in the movie", s.k))?;
        origs.push(Orig { from, frame, tick, rooms: s.rooms.clone(), inputs: s.inputs.len(), skipped });
        specs.push(StepSpec { k: s.k, inputs, goal, relaxed });
        from = s.last;
    }
    Ok((specs, origs))
}

#[derive(Clone, Debug)]
pub struct PassCfg {
    pub seed: u32,
    pub scan: Cfg,
    /// the engines' folder
    pub out: PathBuf,
    /// the frames a step may take beyond its original length (afford::W)
    pub w: u64,
    /// the fallbacks (item 6); None: a step the scan does not complete stops the pass
    pub refind: Option<RCfg>,
}

/// How an earlier pass placed a step (item 7's later passes): its inputs as placed, whether skips were off for its
/// span.
#[derive(Clone, Debug)]
pub struct Plan {
    pub inputs: Vec<In>,
    pub skips_off: bool,
    pub how: How,
}

/// One step of the pass.
#[derive(Clone, Debug)]
pub struct StepRun {
    pub k: usize,
    /// E_{k-1}'s stop, the step's first t
    pub from: u64,
    pub scan: Scan,
    /// the scan with the relaxed goal, when the first found nothing
    pub relaxed: Option<Scan>,
    /// E_k's stop: the found inputs' last frame
    pub next: Option<u64>,
    /// how it was found, and the completion (None: not found)
    pub how: Option<How>,
    pub found: Option<Found>,
    /// the fallbacks' reports, in order
    pub fallbacks: Vec<Value>,
    /// its inputs as placed and whether skips were off for its span (a later pass's `Plan`)
    pub plan: Option<Plan>,
}

impl StepRun {
    fn new(k: usize, from: u64) -> StepRun {
        StepRun { k, from, scan: Scan::default(), relaxed: None, next: None, how: None, found: None, fallbacks: Vec::new(), plan: None }
    }
}

/// The pass: the route (the boot's events and every found input), each step's scan, the end, or the step it stopped
/// at and why.
#[derive(Clone, Debug, Default)]
pub struct Pass {
    pub boot: Vec<Event>,
    pub steps: Vec<StepRun>,
    pub route: Vec<Event>,
    pub end: Option<Found>,
    pub stopped: Option<Value>,
    pub wall_s: f64,
}

/// The forward pass (module docstring) on `host`, from boot with `boot` (events at frames up to BOOT_FRAME).
pub fn forward(host: &dyn EngineHost, boot: &[Event], specs: &[StepSpec], origs: &[Orig], cfg: &PassCfg) -> Res<Pass> {
    let t0 = Instant::now();
    let mut pass = Pass { boot: boot.to_vec(), route: boot.to_vec(), ..Default::default() };
    let e = resume(host, boot, 0, cfg.seed, &cfg.out)?;
    steps_from(e, specs, origs, &BTreeMap::new(), cfg, &mut pass)?;
    pass.wall_s = t0.elapsed().as_secs_f64();
    Ok(pass)
}

/// The engine from boot with `events` (the boot's and a route's), stopped at frame `at` (at least where the boot and
/// the events' last frame leave it): E_{k-1} again for a later pass (item 7).
pub fn resume(host: &dyn EngineHost, events: &[Event], at: u64, seed: u32, out: &std::path::Path) -> Res<Box<dyn Engine>> {
    let (mut e, mut sched) = chain::boot(host, events, seed, out)?;
    // the boot's events past BOOT_FRAME are given before any scan: the bridge takes no event before one it holds
    sched.upto(&mut e, u64::MAX)?;
    if let Some(f) = events.iter().map(|x| x.frame).max().into_iter().chain([at]).max().filter(|&f| f > e.status().frame) {
        e.run(&RunSpec { frame: Some(f), ..Default::default() }).map_err(e2s)?;
    }
    Ok(e)
}

/// The steps `specs` found in order from `e` (module docstring), into `pass` (its route, its steps, its end or where
/// it stopped); a step in `plans` is placed by its plan first. `e` is quit.
pub fn steps_from(mut e: Box<dyn Engine>, specs: &[StepSpec], origs: &[Orig], plans: &BTreeMap<usize, Plan>, cfg: &PassCfg, pass: &mut Pass) -> Res<()> {
    for (spec0, orig) in specs.iter().zip(origs) {
        let plan = plans.get(&spec0.k);
        let spec = &StepSpec { inputs: plan.map_or_else(|| spec0.inputs.clone(), |p| p.inputs.clone()), ..spec0.clone() };
        let limit = e.status().frame + orig.len() + cfg.w;
        let show = |what: &str, sc: &Scan| {
            eprintln!(
                "step {} from {}{what}: {} ({:.1} s, {} t's)",
                spec.k,
                sc.from,
                sc.found.as_ref().map(|f| format!("t {} completion {} tick {}", f.t, f.frame, f.tick)).unwrap_or("none".into()),
                sc.wall_s,
                sc.counts.tried
            )
        };
        let error = |pass: &mut Pass, x: String| {
            eprintln!("step {}: {x}", spec.k);
            pass.stopped = Some(json!({"k": spec.k, "why": format!("engine error: {x}")}));
        };
        let mut run = StepRun::new(spec.k, e.status().frame);
        // the step before's completion: where a fallback starts (refind.rs)
        let after = pass.steps.last().and_then(|s| s.found.as_ref()).map_or(0, |f| f.frame);
        // a step an earlier pass found with skips off for its span: so first
        if let (Some(p), Some(rc)) = (plan, &cfg.refind) {
            if p.skips_off {
                match refind::skips_off(&mut e, spec, after, limit, rc) {
                    Ok((f, rep)) => {
                        eprintln!("step {} from {}, skips off: {}", spec.k, e.status().frame, f.as_ref().map(|f| format!("t {} completion {} tick {}", f.t, f.frame, f.tick)).unwrap_or("none".into()));
                        run.fallbacks.push(rep);
                        if f.is_some() {
                            run.how = Some(How::SkipsOff);
                            run.plan = Some(p.clone());
                        }
                        run.found = f;
                    }
                    Err(x) => {
                        error(pass, x);
                        pass.steps.push(run);
                        e.quit();
                        return Ok(());
                    }
                }
            }
        }
        if run.found.is_none() {
            run.scan = match scan::scan(&mut e, spec, limit, &cfg.scan) {
                Ok(x) => x,
                Err(x) => {
                    error(pass, x);
                    pass.steps.push(run);
                    e.quit();
                    return Ok(());
                }
            };
            show("", &run.scan);
            run.found = run.scan.found.clone();
            run.how = run.found.as_ref().map(|_| How::Scan);
        }
        // the draw keys judged as changed from the root when no t gives the movie's values
        if let (None, Some(g)) = (&run.found, &spec.relaxed) {
            match scan::scan(&mut e, &StepSpec { goal: g.clone(), ..spec.clone() }, limit, &cfg.scan) {
                Ok(r) => {
                    show(", relaxed", &r);
                    run.found = r.found.clone();
                    run.how = run.found.as_ref().map(|_| How::Relaxed);
                    run.relaxed = Some(r);
                }
                Err(x) => {
                    error(pass, x);
                    pass.steps.push(run);
                    e.quit();
                    return Ok(());
                }
            }
        }
        if run.found.is_some() && run.plan.is_none() {
            run.plan = Some(Plan { inputs: spec.inputs.clone(), skips_off: false, how: run.how.unwrap_or(How::Scan) });
        }
        // item 6: the fallbacks
        if let (None, Some(rc)) = (&run.found, &cfg.refind) {
            match refind::refind(&mut e, spec, orig.len(), after, limit, rc) {
                Ok((r, reps)) => {
                    for v in &reps {
                        eprintln!("step {} fallback {}: {}", spec.k, v["how"].as_str().unwrap_or(""),
                                  v["found"].as_object().map(|f| format!("t {} completion {} tick {}", f["t"], f["frame"], f["tick"])).unwrap_or("none".into()));
                    }
                    run.fallbacks.extend(reps);
                    if let Some(r) = r {
                        run.how = Some(r.how);
                        run.plan = Some(Plan { inputs: r.inputs, skips_off: r.skips_off, how: r.how });
                        run.found = Some(r.found);
                    }
                }
                Err(x) => {
                    error(pass, x);
                    pass.steps.push(run);
                    e.quit();
                    return Ok(());
                }
            }
        }
        let Some(f) = run.found.clone() else {
            pass.stopped = Some(json!({"k": spec.k, "why": "no t completes within the original step's length plus W, nor a fallback",
                                       "anchors": spec.inputs.iter().map(|i| i.anchor.json()).collect::<Vec<_>>(),
                                       "timing": spec.inputs.iter().map(|i| i.timing.name()).collect::<Vec<_>>(),
                                       "scan": run.scan.json(), "relaxed": run.relaxed.as_ref().map(Scan::json),
                                       "fallbacks": run.fallbacks}));
            pass.steps.push(run);
            e.quit();
            return Ok(());
        };
        pass.route.extend(f.events.iter().cloned());
        pass.route.sort_by_key(|x| x.frame);
        let last = f.events.last().map(|x| x.frame).unwrap_or(f.t);
        if matches!(spec.goal, Goal::End(_)) {
            pass.end = Some(f);
            pass.steps.push(run);
            e.quit();
            return Ok(());
        }
        // E_k: the found inputs from E's stop, run to the last one's frame
        let ek = |e: &mut Box<dyn Engine>| -> EResult<Box<dyn Engine>> {
            let mut n = e.fork()?;
            n.inputs(&f.events)?;
            if n.status().frame < last {
                n.run(&RunSpec { frame: Some(last), ..Default::default() })?;
            }
            Ok(n)
        };
        match ek(&mut e) {
            Ok(n) => {
                e.quit();
                e = n;
                run.next = Some(last);
                pass.steps.push(run);
            }
            Err(x) => {
                // the step was found: its inputs stay in the route, the pass stops after it
                error(pass, format!("E_k: {x}"));
                pass.steps.push(run);
                e.quit();
                return Ok(());
            }
        }
    }
    e.quit();
    Ok(())
}

/// The boot's events of a pass: none, or with skips the Skips event at the segment's start frame (item 8).
pub fn boot_events(skips_at: Option<u64>) -> Vec<Event> {
    skips_at.map(|f| vec![Event { frame: f, what: Input::Skips { text: true, cutscenes: true, spare: None } }]).unwrap_or_default()
}

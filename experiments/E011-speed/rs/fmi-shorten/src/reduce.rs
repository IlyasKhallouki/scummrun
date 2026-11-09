//! E024 Part 2, item 7 (plan.md, "Reductions"): drops from the best verified route, each kept only when the route
//! found again without it ends earlier, verified from boot.
//!
//! * **The candidates** (`candidates`): inside a step, each inner input of its plan (pass.rs `Plan`: its inputs as
//!   placed); across steps, each step but the goal's, each room loop (the steps from a room back to it through
//!   another, by the movie's rooms) and ddmin's chunks of the steps (halves, quarters, eighths). In order of waste,
//!   the ticks a step adds to the best route (from the completion before it to its own), summed over a candidate's
//!   steps (the plan's "idle time from control to input plus the skip records": the route's own ticks measure both).
//! * **A test** (`attempt`): E_{j-1} again from boot (pass::resume; j the first step the drop touches), the steps
//!   after it from the specs without the dropped steps (pass::specs_without: a later keep(k) holds only the keys the
//!   kept steps changed). First the cheap anchored replay (`cheap`): each step once, at the first t from its base's
//!   first idle stop whose first click the game takes (at most `CHEAP_TS` t's), by its plan's inputs and its relaxed
//!   goal (the weaker), skips off for its span when its plan had them, and a step its plan found by a fallback by
//!   that fallback alone when that does not complete. Only when it reaches the goal, the full pass
//!   (pass::steps_from, the fallbacks with it).
//! * **Kept** (verdict::earlier): when that pass ends before the best by (tick, frame) and its version verifies from
//!   boot (the caller's `version`, movie.rs).
//! * **Rounds**: at most `rounds`, each over the candidates of the best at its start (a candidate whose steps a drop
//!   kept in the round took is passed over); the rounds stop when one gains less than `MIN_GAIN` ticks, or at the
//!   deadline. A test's error (an engine's: a boot, a fork, a timeout; or its version's) fails that test, recorded with
//!   its message, and the next goes on: the best so far is verified and stays (the review of stage B: one test's error
//!   ended every round, and `run` exited without its summary).

use crate::pass::{self, Orig, Pass, PassCfg, Plan, StepRun};
use crate::refind::{self, How};
use crate::scan::{self, Found, Outcome, StepSpec};
use crate::verdict::{self, End, Goal};
use crate::{e2s, Res};
use fmi_core::*;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::Instant;

/// A round's least gain (plan.md's stop rule: 1 s of game time).
pub const MIN_GAIN: u64 = 60;
/// The cheap replay's t's per step past its first idle stop.
pub const CHEAP_TS: u64 = 64;

/// What a candidate drops: steps (by the movie's index), or one input of a step's plan.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Drop {
    Steps(Vec<usize>),
    Input { k: usize, i: usize },
}

#[derive(Clone, Debug)]
pub struct Cand {
    pub what: Drop,
    pub kind: &'static str,
    pub waste: u64,
}

impl Cand {
    pub fn json(&self) -> Value {
        let what = match &self.what {
            Drop::Steps(v) => json!({"steps": v}),
            Drop::Input { k, i } => json!({"step": k, "input": i}),
        };
        json!({"kind": self.kind, "drop": what, "waste": self.waste})
    }
    fn steps(&self) -> Vec<usize> {
        match &self.what {
            Drop::Steps(v) => v.clone(),
            Drop::Input { k, .. } => vec![*k],
        }
    }
}

/// The best route so far: its pass (every step found), its verified end and version, the steps it dropped.
#[derive(Clone, Debug)]
pub struct Best {
    pub pass: Pass,
    pub end: End,
    pub path: PathBuf,
    pub dropped: BTreeSet<usize>,
}

impl Best {
    pub fn plans(&self) -> BTreeMap<usize, Plan> {
        self.pass.steps.iter().filter_map(|s| s.plan.clone().map(|p| (s.k, p))).collect()
    }
}

/// What the reductions need of the stage: the specs without some steps, the engines, the pass's settings, the boot's
/// events, and a version of a pass (its files, its check from boot: the verified end, or None).
pub struct Ctx<'a> {
    pub host: &'a dyn EngineHost,
    pub boot: &'a [Event],
    pub specs: &'a dyn Fn(&BTreeSet<usize>) -> Res<(Vec<StepSpec>, Vec<Orig>)>,
    pub cfg: &'a PassCfg,
    pub out: &'a Path,
    pub version: &'a dyn Fn(&Pass, &BTreeSet<usize>) -> Res<(Option<End>, PathBuf, Value)>,
}

/// The ticks each step adds to the route: from the completion before it (the boot's stop's for the first) to its own.
pub fn waste(p: &Pass) -> BTreeMap<usize, u64> {
    let mut prev = 0;
    let mut out = BTreeMap::new();
    for s in &p.steps {
        let t = s.found.as_ref().map_or(prev, |f| f.tick);
        out.insert(s.k, t.saturating_sub(prev));
        prev = prev.max(t);
    }
    out
}

/// The candidates of the best route (module docstring), by waste, most first.
pub fn candidates(best: &Best, origs: &BTreeMap<usize, Orig>) -> Vec<Cand> {
    let w = waste(&best.pass);
    let ks: Vec<usize> = best.pass.steps.iter().filter(|s| s.how.is_some() && Some(s.k) != best.pass.steps.last().map(|x| x.k)).map(|s| s.k).collect();
    let sum = |v: &[usize]| v.iter().map(|k| w.get(k).copied().unwrap_or(0)).sum::<u64>();
    let mut out: Vec<Cand> = Vec::new();
    let mut seen: BTreeSet<Vec<usize>> = BTreeSet::new();
    let mut add = |out: &mut Vec<Cand>, v: Vec<usize>, kind: &'static str| {
        if !v.is_empty() && seen.insert(v.clone()) {
            out.push(Cand { waste: sum(&v), what: Drop::Steps(v), kind });
        }
    };
    for &k in &ks {
        add(&mut out, vec![k], "step");
    }
    // room loops: from the room before step a back to it at the end of step b, through another room
    let room = |k: usize, first: bool| origs.get(&k).and_then(|o| if first { o.rooms.first() } else { o.rooms.last() }).copied();
    for (ia, &a) in ks.iter().enumerate() {
        let Some(x) = room(a, true) else { continue };
        let mut away = false;
        for &b in &ks[ia..] {
            away |= origs.get(&b).is_some_and(|o| o.rooms.iter().any(|&r| r != x));
            if away && room(b, false) == Some(x) {
                add(&mut out, ks.iter().copied().filter(|&k| k >= a && k <= b).collect(), "loop");
                break;
            }
        }
    }
    // ddmin's chunks
    for n in [2usize, 4, 8] {
        let size = ks.len().div_ceil(n);
        for c in ks.chunks(size.max(1)) {
            add(&mut out, c.to_vec(), "chunk");
        }
    }
    // inner inputs
    for s in &best.pass.steps {
        if let Some(p) = &s.plan {
            for i in 1..p.inputs.len() {
                out.push(Cand { what: Drop::Input { k: s.k, i }, kind: "input", waste: w.get(&s.k).copied().unwrap_or(0) / p.inputs.len() as u64 });
            }
        }
    }
    out.sort_by_key(|c| std::cmp::Reverse(c.waste));
    out
}

/// The steps found again from `e` with no best but the step's limit (the cheap anchored replay, module docstring):
/// None when they reach the goal, else why not.
fn cheap(mut e: Box<dyn Engine>, specs: &[StepSpec], origs: &[Orig], plans: &BTreeMap<usize, Plan>, mut after: u64, cfg: &PassCfg) -> Res<Option<String>> {
    let Some(rc) = &cfg.refind else { return Err("the cheap replay needs the fallbacks' settings".into()) };
    for (spec0, orig) in specs.iter().zip(origs) {
        let plan = plans.get(&spec0.k);
        let spec = StepSpec {
            inputs: plan.map_or_else(|| spec0.inputs.clone(), |p| p.inputs.clone()),
            goal: spec0.relaxed.clone().unwrap_or_else(|| spec0.goal.clone()),
            relaxed: None,
            ..spec0.clone()
        };
        let limit = e.status().frame + orig.len() + cfg.w;
        let off = plan.is_some_and(|p| p.skips_off);
        let r = (|| -> Res<Option<Found>> {
            let mut b = e.fork().map_err(e2s)?;
            let mut pre = Vec::new();
            if off && b.status().frame < after {
                // skips off from the step before's completion (refind.rs)
                b.run(&RunSpec { frame: Some(after), ..Default::default() }).map_err(e2s)?;
            }
            if off {
                let ev = Event { frame: b.status().frame + 1, what: Input::Skips { text: false, cutscenes: false, spare: None } };
                b.inputs(std::slice::from_ref(&ev)).map_err(e2s)?;
                pre.push(ev);
            }
            let r = (|| -> Res<Option<Found>> {
                if b.status().idle.is_none() {
                    match b.run(&RunSpec { idle: true, max_frame: Some(limit), ..Default::default() }) {
                        Ok(st) if st.reason == "idle" => {}
                        Ok(_) | Err(EngineError::Ended(_)) => return Ok(None),
                        Err(x) => return Err(e2s(x)),
                    }
                }
                let t0 = b.status().frame;
                while b.status().frame <= t0 + CHEAP_TS {
                    let mut y = b.fork().map_err(e2s)?;
                    let o = scan::once(&mut y, &spec, limit);
                    y.quit();
                    match o.map_err(e2s)? {
                        Outcome::Done(f) => return Ok(Some(f)),
                        Outcome::NoPoint | Outcome::Refused | Outcome::Other => {}
                        _ => return Ok(None),
                    }
                    match b.run(&RunSpec { frame: Some(b.status().frame + 1), ..Default::default() }) {
                        Ok(_) => {}
                        Err(EngineError::Ended(_)) => return Ok(None),
                        Err(x) => return Err(e2s(x)),
                    }
                }
                Ok(None)
            })();
            b.quit();
            Ok(r?.map(|mut f| {
                f.events.splice(0..0, pre);
                if off {
                    f.events.push(Event { frame: f.frame + 1, what: Input::Skips { text: true, cutscenes: true, spare: None } });
                }
                f
            }))
        })();
        let mut f = r?;
        if f.is_none() {
            if let Some(h) = plan.map(|p| p.how).filter(|h| ![How::Scan, How::Relaxed, How::SkipsOff].contains(h)) {
                f = refind::by(h, &mut e, &spec, orig.len(), after, limit, rc)?;
            }
        }
        let Some(f) = f else {
            e.quit();
            return Ok(Some(format!("step {}", spec.k)));
        };
        if matches!(spec.goal, Goal::End(_)) {
            e.quit();
            return Ok(None);
        }
        after = f.frame;
        let last = f.events.last().map_or(f.t, |x| x.frame);
        let n = (|| -> EResult<Box<dyn Engine>> {
            let mut n = e.fork()?;
            n.inputs(&f.events)?;
            if n.status().frame < last {
                n.run(&RunSpec { frame: Some(last), ..Default::default() })?;
            }
            Ok(n)
        })();
        e.quit();
        match n {
            Ok(n) => e = n,
            Err(EngineError::Ended(_)) => return Ok(Some(format!("step {}: ended", spec.k))),
            Err(x) => return Err(e2s(x)),
        }
    }
    e.quit();
    Ok(Some("no goal step".into()))
}

/// One candidate against `best` (module docstring): the new best when kept, and the test's record.
pub fn attempt(c: &Cand, best: &Best, cx: &Ctx) -> Res<(Option<Best>, Value)> {
    let t0 = Instant::now();
    let mut dropped = best.dropped.clone();
    let mut plans = best.plans();
    match &c.what {
        Drop::Steps(v) => dropped.extend(v.iter().copied()),
        Drop::Input { k, i } => {
            if let Some(p) = plans.get_mut(k) {
                if *i < p.inputs.len() {
                    p.inputs.remove(*i);
                }
            }
        }
    }
    let j = c.steps().into_iter().min().unwrap_or(0);
    let mut rec = c.json();
    let (specs, origs) = (cx.specs)(&dropped)?;
    let (rest, rest_o): (Vec<StepSpec>, Vec<Orig>) = specs.into_iter().zip(origs).filter(|(s, _)| s.k >= j).unzip();
    // E_{j-1}: the best's steps before j from boot
    let prefix: Vec<StepRun> = best.pass.steps.iter().take_while(|s| s.k < j).cloned().collect();
    let mut events: Vec<Event> = best.pass.boot.clone();
    events.extend(prefix.iter().flat_map(|s| s.found.iter().flat_map(|f| f.events.iter().cloned())));
    events.sort_by_key(|x| x.frame);
    let at = prefix.last().and_then(|s| s.next).unwrap_or(0);
    let out = cx.out.join("reduce");
    let e = pass::resume(cx.host, &events, at, cx.cfg.seed, &out)?;
    let after = prefix.last().and_then(|s| s.found.as_ref()).map_or(0, |f| f.frame);
    if let Some(why) = cheap(e, &rest, &rest_o, &plans, after, cx.cfg)? {
        rec["result"] = json!("cheap replay failed");
        rec["why"] = json!(why);
        rec["seconds"] = json!(t0.elapsed().as_secs_f64());
        return Ok((None, rec));
    }
    let mut p = Pass { boot: best.pass.boot.clone(), steps: prefix, route: events.clone(), ..Default::default() };
    let e = pass::resume(cx.host, &events, at, cx.cfg.seed, &out)?;
    pass::steps_from(e, &rest, &rest_o, &plans, cx.cfg, &mut p)?;
    p.wall_s = t0.elapsed().as_secs_f64();
    let Some(f) = p.end.clone() else {
        rec["result"] = json!("the pass stopped");
        rec["stopped"] = p.stopped.clone().map_or(Value::Null, |s| s["k"].clone());
        rec["seconds"] = json!(t0.elapsed().as_secs_f64());
        return Ok((None, rec));
    };
    let end = End { tick: f.tick, frame: f.frame, reason: best.end.reason.clone() };
    rec["end"] = json!({"frame": end.frame, "tick": end.tick});
    if !verdict::earlier(Some(&end), &best.end) {
        rec["result"] = json!("not earlier");
        rec["seconds"] = json!(t0.elapsed().as_secs_f64());
        return Ok((None, rec));
    }
    let (v, path, vrep) = (cx.version)(&p, &dropped)?;
    rec["verify"] = vrep;
    rec["seconds"] = json!(t0.elapsed().as_secs_f64());
    match v {
        Some(v) if verdict::earlier(Some(&v), &best.end) => {
            rec["result"] = json!("kept");
            rec["route"] = json!(path.display().to_string());
            Ok((Some(Best { pass: p, end: v, path, dropped }), rec))
        }
        _ => {
            rec["result"] = json!("not verified");
            Ok((None, rec))
        }
    }
}

/// The rounds (module docstring) from `best`: the best at the end, and each round's record.
pub fn rounds(mut best: Best, origs: &BTreeMap<usize, Orig>, n: usize, deadline: Instant, cx: &Ctx) -> (Best, Vec<Value>) {
    let mut out = Vec::new();
    for r in 0..n {
        let t0 = Instant::now();
        let start = best.end.clone();
        let cands = candidates(&best, origs);
        let mut taken: BTreeSet<usize> = BTreeSet::new();
        let mut tried = Vec::new();
        let mut kept = 0;
        let mut errors = 0;
        let mut timed_out = false;
        for c in &cands {
            if Instant::now() >= deadline {
                timed_out = true;
                break;
            }
            if c.steps().iter().any(|k| taken.contains(k) || best.dropped.contains(k)) {
                continue;
            }
            let t1 = Instant::now();
            let (nb, rec) = attempt(c, &best, cx).unwrap_or_else(|x| {
                let mut rec = c.json();
                rec["result"] = json!("error");
                rec["error"] = json!(x);
                rec["seconds"] = json!(t1.elapsed().as_secs_f64());
                errors += 1;
                (None, rec)
            });
            eprintln!("round {}: {} -> {}{}{}", r + 1, c.json(), rec["result"].as_str().unwrap_or(""),
                      rec["end"].as_object().map(|e| format!(" (end tick {})", e["tick"])).unwrap_or_default(),
                      rec["error"].as_str().map(|e| format!(": {e}")).unwrap_or_default());
            tried.push(rec);
            if let Some(nb) = nb {
                taken.extend(c.steps());
                kept += 1;
                best = nb;
            }
        }
        let gain = start.tick.saturating_sub(best.end.tick);
        out.push(json!({"round": r + 1, "candidates": cands.len(), "tried": tried.len(), "kept": kept, "errors": errors, "gain_ticks": gain,
                        "start": {"frame": start.frame, "tick": start.tick}, "end": {"frame": best.end.frame, "tick": best.end.tick},
                        "route": best.path.display().to_string(), "seconds": t0.elapsed().as_secs_f64(), "deadline": timed_out,
                        "tests": tried}));
        if timed_out || gain < MIN_GAIN {
            break;
        }
    }
    (best, out)
}

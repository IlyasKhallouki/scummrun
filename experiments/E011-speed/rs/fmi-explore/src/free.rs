//! E013's model-free share of tries (experiments/E013-robust/plan.md, section D, and the amendment "component
//! `free`"; switch FMI_FREE=ρ0, off by default).
//!
//! * **The kind** (`pick`): job n is free when floor((n + 1)·ρ) > floor(n·ρ), ρ being FMI_FREE's ρ0 or, with
//!   FMI_FREE_SHIFT=1, stall's share; else the model's candidate, which stall's cell rule may make free. Stall's two
//!   rules come through the `Shift` port. A free job's entry is chosen in the free order (`Archive::select_free`: E012's
//!   order with the cell's free counts).
//! * **A free try** (`explore`, called by the try after its return, reading, T and wait): at the entry and at each of
//!   the wait's stops, the interface's inputs (E012's affordance step, with A's enumeration and B's verdicts and dedupe
//!   when those are on), each trial followed by the wait's stops; the inputs that consumed more random draws than the
//!   no-input twin tried again 1, 2, 3, 5, 8, 13 and 21 frames later; one more level of offers from each outcome kept.
//!   Kept by the try's novelty (IW(1) within the try), committed as entries with "how": "free".
//!
//! Every kept entry is the entry's movie and the trial's clicks, a real run: it verifies from boot like any entry (A0).
//! Nothing is pruned: what is not kept can be made again from the same entry.

use crate::afford::{self, AffordCfg, AffordOut, Inputs, Offer, Trial};
use crate::archive::Archive;
use crate::events;
use crate::files::Res;
use crate::mech::{dead_json, end_extra, Extra, Stop, TestedT, TryNovel, WaitPort};
use fmi_core::{is_game_end, Checkpoint, EResult, Engine, EngineError, Event, ObsReq, ReadingSource, RunSpec};
use serde_json::{json, Map, Value};
use std::cell::Cell;
use std::collections::{BTreeMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Instant;

/// The switch: FMI_FREE=ρ0, the standing share of free tries.
pub const SWITCH: &str = "FMI_FREE";
/// Stall's switch for its two rules (the share's shift and the cell rule), read here where the kind is decided.
pub const SHIFT_SWITCH: &str = "FMI_FREE_SHIFT";
/// The delays a draw-consuming input is tried again after (plan.md D.6: TAS's frame-delay lever).
pub const DELAYS: [u64; 7] = [1, 2, 3, 5, 8, 13, 21];
/// The most random calls looked for between two states (E012 delay's RNG_CAP).
pub const RNG_CAP: u32 = 100_000;

fn rho0_text() -> Option<String> {
    std::env::var(SWITCH).ok().filter(|v| !v.trim().is_empty())
}

/// ρ0 when FMI_FREE holds a share in (0, 1]; None when it is unset, empty or not such a share.
pub fn rho0() -> Option<f64> {
    let x: f64 = rho0_text()?.trim().parse().ok()?;
    (x > 0.0 && x <= 1.0).then_some(x)
}

/// FMI_FREE_SHIFT=1 (stall's rules decide with the share).
pub fn shift_on() -> bool {
    std::env::var(SHIFT_SWITCH).is_ok_and(|v| v == "1")
}

/// The driver's check at its start: with FMI_FREE set, a share in (0, 1], the progress order (FMI_SELECT=progress),
/// and stall's port when FMI_FREE_SHIFT=1. With FMI_FREE unset there is nothing to check.
pub fn check_env(a: &Archive) -> Res<()> {
    let Some(t) = rho0_text() else { return Ok(()) };
    if rho0().is_none() {
        return Err(format!("{SWITCH}={t}: not a share in (0, 1]"));
    }
    if a.progress.is_none() {
        return Err(format!("{SWITCH}={t}: the free order needs the progress order (FMI_SELECT=progress)"));
    }
    if shift_on() && shift().is_none() {
        return Err(format!("{SHIFT_SWITCH}=1: stall's rules are not wired (free::set_shift)"));
    }
    Ok(())
}

/// A job's kind as the driver decides it, and why: "share" (the share's sequence) or "cell" (stall's cell rule).
#[derive(Clone, Debug, PartialEq)]
pub struct Kind {
    pub free: bool,
    pub why: &'static str,
    pub rho: f64,
}

impl Kind {
    pub fn text(&self) -> &'static str {
        if self.free {
            "free"
        } else {
            "model"
        }
    }
}

/// Job n is free under share ρ when floor((n + 1)·ρ) > floor(n·ρ) (a tolerance of 10⁻⁹ keeps shares like 0.3 exact).
pub fn is_free_job(n: u64, rho: f64) -> bool {
    let fl = |x: u64| ((x as f64) * rho + 1e-9).floor() as i64;
    fl(n + 1) > fl(n)
}

/// A job's number: j00003 is 3.
pub fn job_number(name: &str) -> Option<u64> {
    name.strip_prefix('j')?.parse().ok()
}

/// job.json says the job is free.
pub fn job_is_free(job: &Value) -> bool {
    job.get("kind").and_then(|k| k.as_str()) == Some("free")
}

/// A committed log line is a free try's.
pub fn line_is_free(x: &Value) -> bool {
    x.get("kind").and_then(|k| k.as_str()) == Some("free")
}

/// Stall's two rules (component `stall`: `share(&Stalls, ρ0)` and `free_cell(&Stalls, cell)` over the archive's files),
/// as the kind reads them. Wired by the integration; with FMI_FREE_SHIFT unset they are never asked.
pub trait Shift: Send + Sync {
    /// The current share of free tries.
    fn share(&self, a: &Archive, rho0: f64) -> Result<f64, String>;
    /// The cell's last two model tries both ended non-productive in the same class: its next try is free.
    fn free_cell(&self, a: &Archive, cell: &str) -> Result<bool, String>;
}

static SHIFT: OnceLock<Box<dyn Shift>> = OnceLock::new();

/// Wires stall's rules (once per process). False when they were wired already.
pub fn set_shift(s: Box<dyn Shift>) -> bool {
    SHIFT.set(s).is_ok()
}

pub fn shift() -> Option<&'static dyn Shift> {
    SHIFT.get().map(|b| &**b)
}

/// The driver's next job: its entry and kind (None: no cell is selectable). FMI_FREE unset: E012's selection and no
/// kind.
pub fn pick(a: &Archive) -> Res<Option<(usize, Option<Kind>)>> {
    pick_with(a, rho0(), if shift_on() { shift() } else { None })
}

/// `pick` with ρ0 and stall's rules given.
pub fn pick_with(a: &Archive, rho0: Option<f64>, sh: Option<&dyn Shift>) -> Res<Option<(usize, Option<Kind>)>> {
    let Some(r0) = rho0 else { return Ok(a.select(1).first().map(|&i| (i, None))) };
    let n = job_number(&a.next_job()).unwrap_or(0);
    let rho = match sh {
        Some(s) => s.share(a, r0)?,
        None => r0,
    };
    if is_free_job(n, rho) {
        return Ok(a.select_free(1).first().map(|&i| (i, Some(Kind { free: true, why: "share", rho }))));
    }
    let Some(&i) = a.select(1).first() else { return Ok(None) };
    if let Some(s) = sh {
        if s.free_cell(a, &a.rows[i].cell)? {
            return Ok(Some((i, Some(Kind { free: true, why: "cell", rho }))));
        }
    }
    Ok(Some((i, Some(Kind { free: false, why: "share", rho }))))
}

// --- the random calls ----------------------------------------------------------------------------------------------

/// RandomSource::scrambleSeed (ScummVM `common/random.cpp`): one call's step (fmi-play delay's `rng_step`).
pub fn rng_step(s: u32) -> u32 {
    let mut s = s;
    s ^= s >> 13;
    s ^= s << 21;
    s ^= s >> 11;
    s
}

/// The calls between two states: the n ≤ cap with n steps from a giving b (fmi-play delay's `rng_calls`).
pub fn rng_calls(a: u32, b: u32, cap: u32) -> Option<u32> {
    let mut s = a;
    for n in 0..=cap {
        if s == b {
            return Some(n);
        }
        s = rng_step(s);
    }
    None
}

/// The delay rule's verdict on one trial: its calls and the twin's from the offer's state over the same frames.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Draws {
    /// the trial made more calls: its input is tried again after the delays
    More,
    /// as many or fewer
    NotMore,
    /// a count not found within the cap (or the trial's stop not reached again)
    Unknown,
}

pub fn draws(rng0: u32, trial: Option<u32>, twin: Option<u32>) -> Draws {
    match (trial.and_then(|t| rng_calls(rng0, t, RNG_CAP)), twin.and_then(|t| rng_calls(rng0, t, RNG_CAP))) {
        (Some(a), Some(b)) if a > b => Draws::More,
        (Some(_), Some(_)) => Draws::NotMore,
        _ => Draws::Unknown,
    }
}

/// A trial the delay rule looks at: one or two clicks, not ended (a third click's frame follows the second's stop, so a
/// three-click trial has no delayed form), and taken.
fn delay_candidate(t: &Trial) -> bool {
    t.inputs.clicks.len() < 3 && !t.ended && taken(t)
}

/// Whether a trial's input was taken. E012's trials carry no verdict: every trial counts (refuse's verdict replaces
/// this when it is merged).
fn taken(_t: &Trial) -> bool {
    true
}

// --- the free try --------------------------------------------------------------------------------------------------

/// What the try gives the free step.
pub struct Ctx<'a> {
    /// the reading's T (the wait's atoms)
    pub t: &'a TestedT,
    /// T merged with the archive's: the trials' outcome keys
    pub keys: &'a TestedT,
    pub nov: &'a TryNovel,
    pub wait: Arc<dyn WaitPort>,
    pub rd: &'a dyn ReadingSource,
    /// job.json's dead moves of the entry: their inputs are not tried again
    pub dead_moves: &'a [Value],
    pub cancel: &'a AtomicBool,
}

/// The free step's outcome: the entries it keeps (in order), its dead moves, its record (result.json "free").
#[derive(Debug, Default)]
pub struct FreeOut {
    pub kept: Vec<Extra>,
    pub dead: Vec<Value>,
    pub record: Value,
    pub cpu_s: f64,
}

/// The counts over the step's offer points.
#[derive(Default)]
struct Tally {
    offers: usize,
    trials: usize,
    stopped: usize,
    kept: BTreeMap<&'static str, usize>,
    verdicts: BTreeMap<String, usize>,
    delays: Map<String, Value>,
    delay_s: f64,
}

impl Tally {
    fn add_trials(&mut self, r: &AffordOut) {
        self.trials += r.trials.len();
        self.stopped += r.trials.iter().filter(|t| !t.ended).count();
        let rec = r.json();
        if let Some(v) = rec.get("verdicts").and_then(|v| v.as_object()) {
            for (k, n) in v {
                *self.verdicts.entry(k.clone()).or_default() += n.as_u64().unwrap_or(0) as usize;
            }
        }
    }
    fn add_delay(&mut self, k: &str, n: u64) {
        let x = self.delays.get(k).and_then(|v| v.as_u64()).unwrap_or(0) + n;
        self.delays.insert(k.into(), json!(x));
    }
}

/// An engine at a state the entry's movie reaches: a fork of the entry's engine with `events` (absolute frames after
/// the entry) scheduled, run to the checkpoint's frame, held to the checkpoint (facts, tick, random state, actors).
pub fn engine_at(root: &mut Box<dyn Engine>, events: &[Value], cp: &Checkpoint) -> Result<Box<dyn Engine>, String> {
    let mut b = root.fork().map_err(|e| format!("EngineError: {e}"))?;
    let evs: Vec<Event> = events.iter().filter_map(events::event_of).collect();
    let differ = (|| -> Result<Option<String>, String> {
        if evs.len() != events.len() {
            return Ok(Some("an event the bridge cannot read".into()));
        }
        if !evs.is_empty() {
            b.inputs(&evs).map_err(|e| format!("EngineError: {e}"))?;
        }
        let st = b.run(&RunSpec { frame: Some(cp.frame), ..Default::default() }).map_err(|e| format!("EngineError: {e}"))?;
        if st.frame != cp.frame {
            return Ok(Some(format!("the run stopped at frame {} ({})", st.frame, st.reason)));
        }
        let o = b.obs(&ObsReq::default()).map_err(|e| format!("EngineError: {e}"))?;
        let d = crate::tryjob::compare_checkpoint(&crate::harness::checkpoint_json(cp), &o, b.status());
        Ok((!d.is_empty()).then(|| crate::files::py_repr(&Value::Object(d))))
    })();
    match differ {
        Ok(None) => Ok(b),
        Ok(Some(d)) => {
            b.quit();
            Err(format!("E013 free: the state at frame {} is not reached again from the entry: {d}", cp.frame))
        }
        Err(e) => {
            b.quit();
            Err(e)
        }
    }
}

/// One offer point's step: E012's affordance step (mech::afford_step's order: ready, the offer, the inputs less the
/// dead moves, the trials with the wait's stops after each, the outcomes kept by the try's novelty), then, at the
/// first level, the delays on the offer's engine. The kept outcomes continue `prefix_events` and `prefix_cps`.
/// Returns the step's record (its "seconds" the offers' alone) and the delays' record (its own "seconds").
#[allow(clippy::too_many_arguments)]
fn offer_point(e: &mut Box<dyn Engine>, cx: &Ctx, then: afford::Then<'_>, prefix_events: &[Value], prefix_cps: &[Checkpoint],
               delays: bool, out: &mut FreeOut, tally: &mut Tally, level: &'static str) -> EResult<(Value, Option<Value>)> {
    let t0 = Instant::now();
    let frame = e.status().frame;
    let mut fork = match afford::ready(e, afford::W)? {
        None => None,
        Some(Some(b)) => Some(b),
        Some(None) => return Ok((json!({"frame": frame, "ready": false, "seconds": t0.elapsed().as_secs_f64()}), None)),
    };
    let res = (|| -> EResult<(Value, Option<Value>)> {
        let eng: &mut Box<dyn Engine> = match fork.as_mut() {
            Some(b) => b,
            None => e,
        };
        let o = eng.obs(&ObsReq::default())?;
        let f0 = eng.status().frame;
        let horizon = afford::W + cx.wait.last_due(&o).map(|d| d.saturating_sub(f0)).unwrap_or(0);
        let offer = afford::offer(eng, &o)?;
        let all = afford::inputs_of(&offer);
        let dead: HashSet<String> = cx.dead_moves.iter().filter_map(|d| d.get("events")).map(|ev| ev.to_string()).collect();
        let mut skipped = 0;
        let inputs: Vec<Inputs> = all
            .into_iter()
            .filter(|inp| {
                if dead.is_empty() {
                    return true;
                }
                let mut evs: Vec<Value> = prefix_events.to_vec();
                evs.extend(inp.events(f0).iter().map(events::event_json));
                let d = dead.contains(&Value::Array(evs).to_string());
                skipped += d as usize;
                !d
            })
            .collect();
        let cfg = AffordCfg { horizon, ..Default::default() };
        let r = afford::try_all(eng, &offer, &inputs, cx.keys.keys(), cx.nov, &cfg, Some(then))?;
        tally.offers += 1;
        tally.add_trials(&r);
        let n_kept = keep(&r, cx, prefix_events, prefix_cps, out, tally, level);
        out.cpu_s += r.engine_cpu_s;
        let mut rec = r.json();
        rec["frame"] = json!(frame);
        rec["offer_frame"] = json!(f0);
        rec["ready"] = json!(true);
        rec["horizon"] = json!(horizon);
        rec["skipped_dead"] = json!(skipped);
        rec["kept"] = json!(n_kept);
        rec["seconds"] = json!(t0.elapsed().as_secs_f64());
        let d = if delays { Some(delay_step(eng, &offer, &r, horizon, cx, then, prefix_events, prefix_cps, out, tally)?) } else { None };
        Ok((rec, d))
    })();
    if let Some(b) = fork.take() {
        out.cpu_s += b.cpu_s();
        b.quit();
    }
    res
}

/// The outcomes a step kept: entries of the free try (the prefix, then the trial's clicks; the prefix's checkpoints
/// before the kept one), novel no more in this try; the trials that ended in the game's end are dead moves, and with
/// E014's FMI_GOAL_END=1 one that a script ended is also an end entry (mech::end_extra, as the affordance step's).
fn keep(r: &AffordOut, cx: &Ctx, prefix_events: &[Value], prefix_cps: &[Checkpoint], out: &mut FreeOut, tally: &mut Tally,
        level: &'static str) -> usize {
    for k in &r.kept {
        cx.nov.keep(&k.novel);
        let mut evs = prefix_events.to_vec();
        evs.extend(k.events_json());
        let mut cps: Vec<Checkpoint> = prefix_cps.iter().filter(|c| c.frame < k.checkpoint.frame).cloned().collect();
        cps.push(k.checkpoint.clone());
        out.kept.push(Extra { how: "free", events: evs, cps, novel: k.novel.clone() });
        *tally.kept.entry(level).or_default() += 1;
    }
    for tr in r.trials.iter().filter(|t| t.ended && is_game_end(&t.reason)) {
        let mut evs = prefix_events.to_vec();
        evs.extend(tr.events.iter().map(events::event_json));
        out.dead.push(dead_json(&tr.reason, tr.frame, &evs, "free"));
        if let Some(x) = end_extra(&tr.reason, tr.frame, tr.tick, evs, prefix_cps) {
            out.kept.push(x);
            *tally.kept.entry("end").or_default() += 1;
        }
    }
    r.kept.len()
}

/// The delays at an offer point (plan.md D.6): each candidate trial's random calls against the no-input twin's over the
/// same frames, counted exactly from the offer's state; the inputs with more calls tried again d frames later for each
/// d of DELAYS, on the offer's engine run d frames with no input.
#[allow(clippy::too_many_arguments)]
fn delay_step(eng: &mut Box<dyn Engine>, offer: &Offer, r: &AffordOut, horizon: u64, cx: &Ctx, then: afford::Then<'_>,
              prefix_events: &[Value], prefix_cps: &[Checkpoint], out: &mut FreeOut, tally: &mut Tally) -> EResult<Value> {
    let t0 = Instant::now();
    let f0 = eng.status().frame;
    let rng0 = eng.status().rng as u32;
    let cands: Vec<&Trial> = r.trials.iter().filter(|t| delay_candidate(t)).collect();
    // the twin: one fork with no input through the candidates' stop frames
    let mut frames: Vec<u64> = cands.iter().map(|t| t.frame).collect();
    frames.sort_unstable();
    frames.dedup();
    let mut twin: BTreeMap<u64, u32> = BTreeMap::new();
    if !frames.is_empty() {
        let mut c = eng.fork()?;
        for &fr in &frames {
            if fr > c.status().frame {
                match c.run(&RunSpec { frame: Some(fr), ..Default::default() }) {
                    Ok(_) => {}
                    Err(EngineError::Ended(_)) => break,
                    Err(x) => return Err(x),
                }
            }
            if c.status().frame == fr {
                twin.insert(fr, c.status().rng as u32);
            }
        }
        out.cpu_s += c.cpu_s();
        c.quit();
    }
    // each candidate's own run to its stop
    let (mut more, mut unknown, mut missed) = (Vec::<Inputs>::new(), 0u64, 0u64);
    for t in &cands {
        let mut b = eng.fork()?;
        b.inputs(&t.events)?;
        let mine = match b.run(&RunSpec { frame: Some(t.frame), ..Default::default() }) {
            Ok(st) if st.frame == t.frame => Some(st.rng as u32),
            Ok(_) | Err(EngineError::Ended(_)) => {
                missed += 1;
                None
            }
            Err(x) => return Err(x),
        };
        out.cpu_s += b.cpu_s();
        b.quit();
        match draws(rng0, mine, twin.get(&t.frame).copied()) {
            Draws::More => {
                if !more.contains(&t.inputs) {
                    more.push(t.inputs.clone());
                }
            }
            Draws::NotMore => {}
            Draws::Unknown => unknown += 1,
        }
    }
    let t_count = t0.elapsed().as_secs_f64();
    let (mut trials, mut kept) = (0usize, 0usize);
    let mut by_d: Vec<Value> = Vec::new();
    if !more.is_empty() {
        for d in DELAYS {
            let mut b = eng.fork()?;
            let at = match b.run(&RunSpec { frame: Some(f0 + d), ..Default::default() }) {
                Ok(st) if st.frame == f0 + d => true,
                Ok(_) | Err(EngineError::Ended(_)) => false,
                Err(x) => {
                    b.quit();
                    return Err(x);
                }
            };
            if !at {
                out.cpu_s += b.cpu_s();
                b.quit();
                by_d.push(json!({"d": d, "reached": false}));
                break;
            }
            let cfg = AffordCfg { horizon, ..Default::default() };
            let rd = afford::try_all(&mut b, offer, &more, cx.keys.keys(), cx.nov, &cfg, Some(then));
            out.cpu_s += b.cpu_s();
            b.quit();
            let rd = rd?;
            tally.add_trials(&rd);
            let k = keep(&rd, cx, prefix_events, prefix_cps, out, tally, "delay");
            out.cpu_s += rd.engine_cpu_s;
            trials += rd.trials.len();
            kept += k;
            by_d.push(json!({"d": d, "trials": rd.trials.len(), "kept": k, "seconds": rd.seconds}));
        }
    }
    tally.add_delay("counted", cands.len() as u64);
    tally.add_delay("delayed_inputs", more.len() as u64);
    tally.add_delay("unknown", unknown);
    tally.add_delay("missed", missed);
    tally.add_delay("trials", trials as u64);
    tally.add_delay("kept", kept as u64);
    tally.delay_s += t0.elapsed().as_secs_f64();
    Ok(json!({"counted": cands.len(), "more": more.len(), "inputs": more.iter().map(|i| i.name()).collect::<Vec<_>>(),
              "unknown": unknown, "missed": missed, "trials": trials, "kept": kept, "by_delay": by_d,
              "count_seconds": t_count, "seconds": t0.elapsed().as_secs_f64()}))
}

/// The free try's offers (plan.md D.5-D.8), on the entry's engine `root` after the wait (`stops`, the wait's): the
/// entry and each stop with delays, then one more level from each outcome kept. Returns the entries kept in order.
pub fn explore(root: &mut Box<dyn Engine>, stops: &[Stop], cx: &Ctx) -> Result<FreeOut, String> {
    let t0 = Instant::now();
    let waits = Cell::new(0usize);
    let wait = cx.wait.clone();
    let then_f = |b: &mut Box<dyn Engine>| -> Result<Vec<Checkpoint>, String> {
        waits.set(waits.get() + 1);
        Ok(wait.wait(b, &cx.t.flat, cx.rd, 0)?.stops.into_iter().map(|s| s.checkpoint).collect())
    };
    let then: afford::Then<'_> = &then_f;
    let mut out = FreeOut::default();
    let mut tally = Tally::default();
    let mut points: Vec<Value> = Vec::new();
    let killed = || cx.cancel.load(Ordering::SeqCst);
    // 1-2: the entry, then each of the wait's stops (time passing, then acting), with delays
    for p in 0..=stops.len() {
        if killed() {
            return Err("the guard's kill".into());
        }
        let tp = Instant::now();
        let (at, frame) = if p == 0 { ("entry", root.status().frame) } else { ("stop", stops[p - 1].frame) };
        let (rec, d) = if p == 0 {
            offer_point(root, cx, then, &[], &[], true, &mut out, &mut tally, "1")
        } else {
            let cp = &stops[p - 1].checkpoint;
            let mut b = engine_at(root, &[], cp)?;
            let r = offer_point(&mut b, cx, then, &[], std::slice::from_ref(cp), true, &mut out, &mut tally, "1");
            out.cpu_s += b.cpu_s();
            b.quit();
            r
        }
        .map_err(|e| format!("EngineError: free: {e}"))?;
        // the point's seconds are its offers' (the engine at a stop, the step), its delays' are their own
        let ds = d.as_ref().and_then(|d| d["seconds"].as_f64()).unwrap_or(0.0);
        let mut pt = json!({"where": at, "frame": frame, "level": 1, "seconds": tp.elapsed().as_secs_f64() - ds, "record": rec});
        if let Some(d) = d {
            pt["delays"] = d;
        }
        points.push(pt);
    }
    // 3: one more level from each outcome kept above (by 1 or 2: the offers or the delays), without delays; an end
    // entry has no state to offer from
    let level1: Vec<Extra> = out.kept.iter().filter(|x| x.how == "free").cloned().collect();
    for x in &level1 {
        if killed() {
            return Err("the guard's kill".into());
        }
        let tp = Instant::now();
        let cp = x.cps.last().unwrap().clone();
        let mut b = engine_at(root, &x.events, &cp)?;
        let r = offer_point(&mut b, cx, then, &x.events, &x.cps, false, &mut out, &mut tally, "2");
        out.cpu_s += b.cpu_s();
        b.quit();
        let (rec, _) = r.map_err(|e| format!("EngineError: free: {e}"))?;
        points.push(json!({"where": "kept", "frame": cp.frame, "level": 2, "seconds": tp.elapsed().as_secs_f64(), "record": rec}));
    }
    let mut kept = Map::new();
    for (k, n) in &tally.kept {
        kept.insert((*k).into(), json!(n));
    }
    let mut delays = tally.delays.clone();
    delays.insert("seconds".into(), json!(tally.delay_s));
    let mut rec = json!({
        "offers": tally.offers, "n_points": points.len(), "trials": tally.trials, "kept": Value::Object(kept),
        "kept_total": out.kept.len(), "dead": out.dead.len(), "waits_run": waits.get(),
        "waits_reused": tally.stopped.saturating_sub(waits.get()), "delays": Value::Object(delays),
        "seconds": t0.elapsed().as_secs_f64(), "points": points,
    });
    if !tally.verdicts.is_empty() {
        rec["verdicts"] = json!(tally.verdicts);
    }
    out.record = rec;
    Ok(out)
}

/// The merged keys of the trials' outcomes: the reading's T and the archive's T's keys (job.json "novelty").
pub fn merged_keys(t: &TestedT, archive_t: &[crate::progress::TAtom]) -> TestedT {
    let mut flat = t.flat.clone();
    let have: HashSet<fmi_core::Key> = t.keys().iter().copied().collect();
    for a in archive_t {
        let k = fmi_core::Key::parse(&a.key);
        if !have.contains(&k) {
            flat.push((k, a.vals.clone()));
        }
    }
    TestedT::new(flat, Value::Null)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_kind_sequence() {
        let free: Vec<u64> = (0..16).filter(|&n| is_free_job(n, 0.25)).collect();
        assert_eq!(free, vec![3, 7, 11, 15]);
        assert!((0..80).all(|n| is_free_job(n, 1.0)));
        let half: Vec<u64> = (0..8).filter(|&n| is_free_job(n, 0.5)).collect();
        assert_eq!(half, vec![1, 3, 5, 7]);
        // a share whose products are not exact in binary: 3 in every 10
        let n3 = (0..100).filter(|&n| is_free_job(n, 0.3)).count();
        assert_eq!(n3, 30);
        assert_eq!((0..10).filter(|&n| is_free_job(n, 0.3)).collect::<Vec<_>>(), vec![3, 6, 9]);
        // over any run the count of free jobs is floor(N·ρ)
        for rho in [0.1, 0.2, 0.25, 1.0 / 3.0, 0.4, 0.5, 0.75] {
            for big in [7u64, 20, 80, 600] {
                let n = (0..big).filter(|&k| is_free_job(k, rho)).count() as i64;
                assert_eq!(n, ((big as f64) * rho + 1e-9).floor() as i64, "rho {rho} N {big}");
            }
        }
    }

    #[test]
    fn job_numbers_and_kinds() {
        assert_eq!(job_number("j00003"), Some(3));
        assert_eq!(job_number("j01234"), Some(1234));
        assert_eq!(job_number("x"), None);
        assert!(job_is_free(&json!({"kind": "free"})));
        assert!(!job_is_free(&json!({"kind": "model"})));
        assert!(!job_is_free(&json!({})));
        assert!(line_is_free(&json!({"kind": "free", "outcome": "ok"})));
        assert_eq!(Kind { free: true, why: "share", rho: 0.25 }.text(), "free");
        assert_eq!(Kind { free: false, why: "share", rho: 0.25 }.text(), "model");
    }

    #[test]
    fn random_calls_counted_exactly() {
        let s0 = 3_182_679_328u32;
        let mut s = s0;
        for n in 0..500 {
            assert_eq!(rng_calls(s0, s, RNG_CAP), Some(n));
            s = rng_step(s);
        }
        // the step is ScummVM's scrambleSeed: 13, 21, 11
        let x = 1u32;
        let y = x ^ (x >> 13);
        let y = y ^ (y << 21);
        let y = y ^ (y >> 11);
        assert_eq!(rng_step(1), y);
        assert_eq!(rng_calls(s0, 12345, 1000), None);
        let s5 = (0..5).fold(s0, |a, _| rng_step(a));
        let s7 = (0..7).fold(s0, |a, _| rng_step(a));
        assert_eq!(draws(s0, Some(s7), Some(s5)), Draws::More);
        assert_eq!(draws(s0, Some(s5), Some(s5)), Draws::NotMore);
        assert_eq!(draws(s0, Some(s5), Some(s7)), Draws::NotMore);
        assert_eq!(draws(s0, None, Some(s5)), Draws::Unknown);
        assert_eq!(draws(s0, Some(12345), Some(s5)), Draws::Unknown);
    }
}

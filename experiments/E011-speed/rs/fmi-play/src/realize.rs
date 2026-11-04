//! E015's G1 (experiments/E015-engine-gates/plan.md, "G1: realize a planned sentence through the engine, not a
//! profile"): a planned step turned into clicks by searching in forks of the engine, each candidate judged by the
//! engine's own input log (patch 0007). Switch `FMI_REALIZE=1`, off by default; nothing on the run path calls it in
//! E015 (the measurement binary `e015 realize` does).
//!
//! * **The offer** is afford's (`offer_cfg`): the visible verb slots, the touchable room objects and the actors with a
//!   point the engine's hit test confirms, and the walk boxes' floor points.
//! * **The approach** (a target with no hit point on the screen): the floor points, nearest first to the target's
//!   walk point in room coordinates, each clicked in a fork with no verb click and run to the bridge's next idle. A
//!   click is accepted when its input log shows a walk and the camera's xstart moved toward the target, or the target
//!   gained a hit point. From the accepted fork it repeats, at most `max_approach` clicks.
//! * **The sentence** (v, a, b): candidates in order, (i) a bare click on a, (ii) each offered verb slot then a, and
//!   for a two-object sentence each of those followed by b, or by each visible slot when b has no hit point. Each is
//!   tried at up to `max_points` of a's hit points, in a fork run until the input log has the last click's record. A
//!   candidate is accepted when the log's `Sentence` touch is (v, a, b). At most `max_candidates`.
//! * **A dialog verb** v: the offered verb slot whose dump id is v (in SCUMM v5 a slot's id is its verb number),
//!   clicked in a fork; accepted when the handler took the click (verdict "taken").
//!
//! The engine is the only judge: no interface profile, no verb or variable number of a game, no wait for a verb.

use fmi_core::*;
use fmi_explore::afford::{self, Hit, Inputs, Offer, Point};
use std::time::Instant;

/// The switch: `realize` runs only with FMI_REALIZE=1.
pub const SWITCH: &str = "FMI_REALIZE";

pub fn on() -> bool {
    std::env::var(SWITCH).is_ok_and(|v| v == "1")
}

/// A planned step: a sentence (verb v on object a, with object b for a two-object sentence) or a dialog verb.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Step {
    Sentence { v: i32, a: i32, b: Option<i32> },
    Dialog { v: i32 },
}

impl Step {
    /// "s<v>:<a>[:<b>]" or "d<v>".
    pub fn parse(s: &str) -> Option<Step> {
        if let Some(r) = s.strip_prefix('d') {
            return Some(Step::Dialog { v: r.parse().ok()? });
        }
        let r = s.strip_prefix('s')?;
        let p: Vec<&str> = r.split(':').collect();
        match p.as_slice() {
            [v, a] => Some(Step::Sentence { v: v.parse().ok()?, a: a.parse().ok()?, b: None }),
            [v, a, b] => Some(Step::Sentence { v: v.parse().ok()?, a: a.parse().ok()?, b: Some(b.parse().ok()?) }),
            _ => None,
        }
    }
    pub fn text(&self) -> String {
        match self {
            Step::Sentence { v, a, b: None } => format!("s{v}:{a}"),
            Step::Sentence { v, a, b: Some(b) } => format!("s{v}:{a}:{b}"),
            Step::Dialog { v } => format!("d{v}"),
        }
    }
}

/// The search's limits (plan.md, G1).
#[derive(Clone, Debug)]
pub struct Cfg {
    /// approach clicks accepted before a miss with "approach"
    pub max_approach: usize,
    /// sentence candidates before a miss with "no sentence"
    pub max_candidates: usize,
    /// hit points of the target each candidate is tried at
    pub max_points: usize,
    /// visible slots tried as the second object when b has no hit point
    pub max_slots: usize,
    /// frames an approach click's run may take to the bridge's next idle
    pub horizon: u64,
    /// frames after the last click within which its input log record must appear
    pub log_frames: u64,
}

impl Default for Cfg {
    fn default() -> Self {
        Cfg { max_approach: 6, max_candidates: 60, max_points: 3, max_slots: 15, horizon: afford::W, log_frames: 8 }
    }
}

/// One fork tried: its phase ("approach", "sentence", "dialog"), its clicks' names, its events, whether it was
/// accepted, and what the engine showed.
#[derive(Clone, Debug)]
pub struct Attempt {
    pub phase: &'static str,
    pub name: String,
    pub events: Vec<Event>,
    pub accepted: bool,
    /// approach: "walk", "toward", "hit"; sentence: the Sentence touches seen; dialog: the verdict
    pub seen: String,
    /// the fork's frame when judged
    pub frame: u64,
    pub seconds: f64,
}

/// The step realized: every input (the approach clicks and the accepted candidate's) at absolute frames, the accepted
/// fork (stopped where it was judged), and the counts.
pub struct Realized {
    pub events: Vec<Event>,
    pub engine: Box<dyn Engine>,
    /// approach clicks accepted
    pub approach: usize,
    /// sentence or dialog candidates tried (the accepted one included)
    pub candidates: usize,
    /// forks run
    pub tries: usize,
    pub attempts: Vec<Attempt>,
    pub seconds: f64,
}

/// A miss: why ("approach", "no sentence", "no slot", "not taken"), and what was tried.
#[derive(Clone, Debug)]
pub struct Miss {
    pub reason: String,
    pub approach: usize,
    pub candidates: usize,
    pub tries: usize,
    pub attempts: Vec<Attempt>,
    pub seconds: f64,
}

/// What the search has done so far.
#[derive(Default)]
struct Rec {
    events: Vec<Event>,
    approach: usize,
    candidates: usize,
    tries: usize,
    attempts: Vec<Attempt>,
}

impl Rec {
    fn miss(self, reason: &str, t0: Instant) -> Miss {
        Miss {
            reason: reason.into(),
            approach: self.approach,
            candidates: self.candidates,
            tries: self.tries,
            attempts: self.attempts,
            seconds: t0.elapsed().as_secs_f64(),
        }
    }
    fn done(self, engine: Box<dyn Engine>, t0: Instant) -> Realized {
        Realized {
            events: self.events,
            engine,
            approach: self.approach,
            candidates: self.candidates,
            tries: self.tries,
            attempts: self.attempts,
            seconds: t0.elapsed().as_secs_f64(),
        }
    }
}

/// Realizes `step` from `e`, which is stopped where the bridge is idle. `e` is not moved: every input is tried in a
/// fork (its input log is taken first, so each fork's log starts empty). Ok(Err(miss)) when the search misses within
/// its limits; Err when the engine fails or the switch is off.
pub fn realize(e: &mut Box<dyn Engine>, step: &Step, cfg: &Cfg) -> EResult<Result<Realized, Miss>> {
    if !on() {
        return Err(EngineError::Bridge(format!("realize: {SWITCH} is off")));
    }
    let t0 = Instant::now();
    let mut rec = Rec::default();
    match *step {
        Step::Dialog { v } => {
            e.input_log()?;
            let of = offer_at(e)?;
            let Some(slot) = of.verbs.iter().find(|p| matches!(p.hit, Hit::Verb { id, .. } if id == v)).copied() else {
                return Ok(Err(rec.miss("no slot", t0)));
            };
            rec.candidates += 1;
            let (b, records, att) = try_clicks(e, "dialog", &[slot], cfg)?;
            let verdict = afford::trial_verdict(&records);
            let ok = verdict == "taken";
            rec.attempts.push(Attempt { accepted: ok, seen: verdict, ..att });
            rec.tries += 1;
            if !ok {
                b.quit();
                return Ok(Err(rec.miss("not taken", t0)));
            }
            rec.events.extend(rec.attempts.last().unwrap().events.clone());
            Ok(Ok(rec.done(b, t0)))
        }
        Step::Sentence { v, a, b } => {
            // the approach: from `e`, then from each accepted fork
            let mut cur: Option<Box<dyn Engine>> = None;
            loop {
                let base: &mut Box<dyn Engine> = match cur.as_mut() {
                    Some(c) => c,
                    None => &mut *e,
                };
                base.input_log()?;
                let of = offer_at(base)?;
                if has_hit(&of, a) {
                    break;
                }
                if rec.approach >= cfg.max_approach {
                    if let Some(c) = cur {
                        c.quit();
                    }
                    return Ok(Err(rec.miss("approach", t0)));
                }
                match approach_once(base, &of, a, cfg, &mut rec)? {
                    Some(next) => {
                        rec.approach += 1;
                        rec.events.extend(rec.attempts.last().unwrap().events.clone());
                        if let Some(c) = cur.replace(next) {
                            c.quit();
                        }
                    }
                    None => {
                        if let Some(c) = cur {
                            c.quit();
                        }
                        return Ok(Err(rec.miss("approach", t0)));
                    }
                }
            }
            let base: &mut Box<dyn Engine> = match cur.as_mut() {
                Some(c) => c,
                None => &mut *e,
            };
            let found = sentence(base, v, a, b, cfg, &mut rec)?;
            if let Some(c) = cur {
                c.quit();
            }
            match found {
                Some(fork) => {
                    rec.events.extend(rec.attempts.last().unwrap().events.clone());
                    Ok(Ok(rec.done(fork, t0)))
                }
                None => Ok(Err(rec.miss("no sentence", t0))),
            }
        }
    }
}

/// afford's offer at the engine's stop (its full dump; the walk boxes with FMI_BOXES, actors on the screen with
/// FMI_OFFER_SCREEN).
fn offer_at(e: &mut Box<dyn Engine>) -> EResult<Offer> {
    let o = e.obs(&ObsReq::default())?;
    afford::offer_cfg(e, &o, afford::boxes_on(), afford::offer_screen_on())
}

/// Whether the offer has a hit point for object or actor `a`.
fn has_hit(of: &Offer, a: i32) -> bool {
    of.targets.iter().any(|p| p.hit == Hit::Object(a) || p.hit == Hit::Actor(a))
}

/// The target's place in room coordinates: a room object's walk point, else an actor's feet. None when the room
/// holds neither.
fn place_of(o: &Obs, a: i32) -> Option<(i32, i32)> {
    if let Some(r) = o.room_objects.as_deref().unwrap_or(&[]).iter().find(|r| r.id == a) {
        return Some(r.walk);
    }
    o.actors.iter().find(|x| x.id == a).map(|x| (x.x, x.y))
}

/// How far the camera's view is from x (room coordinates): the distance from x to the screen's middle column.
fn view_dist(xstart: i32, x: i32) -> i32 {
    (x - (xstart + afford::SCREEN_W / 2)).abs()
}

/// `clicks` at the engine's frame + 1 + 2k (afford's timing) in a fork of `e`, run until the input log holds the
/// last click's record (at most `log_frames` frames after it). Returns the fork, the records, and the attempt (not
/// yet judged). A click the bridge refuses gives an empty record list.
fn try_clicks(e: &mut Box<dyn Engine>, phase: &'static str, clicks: &[Point], cfg: &Cfg)
              -> EResult<(Box<dyn Engine>, Vec<InputRecord>, Attempt)> {
    let t = Instant::now();
    let inp = Inputs { clicks: clicks.to_vec() };
    let evs = inp.events(e.status().frame);
    let last = evs.last().map(|x| x.frame).unwrap_or(e.status().frame);
    let mut b = e.fork()?;
    let mut records = Vec::new();
    match b.inputs(&evs) {
        Ok(()) => {
            let mut to = last + 1;
            loop {
                match b.run(&RunSpec { frame: Some(to), ..Default::default() }) {
                    Ok(_) => {}
                    Err(EngineError::Ended(_)) => break,
                    Err(x) => return Err(x),
                }
                records.extend(b.input_log()?.records);
                if records.iter().any(|r| r.frame >= last) || to >= last + cfg.log_frames {
                    break;
                }
                to += 1;
            }
        }
        Err(EngineError::Bridge(_)) => {}
        Err(x) => return Err(x),
    }
    let att = Attempt {
        phase,
        name: inp.name(),
        events: evs,
        accepted: false,
        seen: String::new(),
        frame: b.status().frame,
        seconds: t.elapsed().as_secs_f64(),
    };
    Ok((b, records, att))
}

/// One approach click: the floor points nearest first to the target's place; the first accepted fork, run to the
/// bridge's next idle (None: no point accepted, or the target has no place in the room).
fn approach_once(e: &mut Box<dyn Engine>, of: &Offer, a: i32, cfg: &Cfg, rec: &mut Rec) -> EResult<Option<Box<dyn Engine>>> {
    let o = e.obs(&ObsReq::default())?;
    let Some(place) = place_of(&o, a) else {
        return Ok(None);
    };
    let st = e.status().clone();
    let xs = st.xstart;
    let top = st.screen.map(|s| s.0).unwrap_or(0);
    let d2 = |p: &Point| {
        let (rx, ry) = (p.x + xs, p.y - top);
        let (dx, dy) = ((rx - place.0) as i64, (ry - place.1) as i64);
        dx * dx + dy * dy
    };
    let mut floor: Vec<Point> = of.walks.clone();
    floor.sort_by_key(d2);
    for p in floor {
        let (mut b, mut records, mut att) = try_clicks(e, "approach", &[p], cfg)?;
        rec.tries += 1;
        let end = b.status().frame + cfg.horizon;
        let ran = match b.run(&RunSpec { idle: true, max_frame: Some(end), ..Default::default() }) {
            Ok(_) => true,
            Err(EngineError::Ended(_)) => false,
            Err(x) => return Err(x),
        };
        let mut ok = false;
        if ran {
            records.extend(b.input_log()?.records);
            let walk = records.iter().any(|r| r.writes.iter().any(|t| t.kind == TouchKind::Walk));
            let xs2 = b.status().xstart;
            let toward = view_dist(xs2, place.0) < view_dist(xs, place.0);
            let hit = has_hit(&offer_at(&mut b)?, a);
            ok = (walk && toward) || hit;
            att.seen = format!("walk={} toward={} hit={} xstart={}->{}", walk as u8, toward as u8, hit as u8, xs, xs2);
        } else {
            att.seen = "ended".into();
        }
        att.frame = b.status().frame;
        att.accepted = ok;
        rec.attempts.push(att);
        if ok {
            return Ok(Some(b));
        }
        b.quit();
    }
    Ok(None)
}

/// Up to `max_points` points at which the engine's hit test answers object or actor `a`: the object's grid points
/// (afford's), else the offer's point for it.
fn hit_points(e: &mut Box<dyn Engine>, of: &Offer, a: i32, max: usize) -> EResult<Vec<Point>> {
    let o = e.obs(&ObsReq::default())?;
    let st = e.status().clone();
    let top = st.screen.map(|s| s.0).unwrap_or(0);
    let mut out = Vec::new();
    if let Some(r) = o.room_objects.as_deref().unwrap_or(&[]).iter().find(|r| r.id == a) {
        let pts = afford::object_grid(r, st.xstart, top);
        if !pts.is_empty() {
            let ats = e.at_many(&pts)?;
            for (p, at) in pts.iter().zip(ats.iter()) {
                if at.main.map(|m| m.2) == Some(a) && !out.iter().any(|q: &Point| (q.x, q.y) == *p) {
                    out.push(Point { x: p.0, y: p.1, hit: Hit::Object(a) });
                    if out.len() >= max {
                        break;
                    }
                }
            }
        }
    }
    if out.is_empty() {
        out.extend(of.targets.iter().filter(|p| p.hit == Hit::Object(a) || p.hit == Hit::Actor(a)).take(max).copied());
    }
    Ok(out)
}

/// The sentence's candidates in order, each tried at the target's hit points; the accepted fork (None: no candidate
/// within the limit gave the Sentence touch (v, a, b)).
fn sentence(e: &mut Box<dyn Engine>, v: i32, a: i32, b: Option<i32>, cfg: &Cfg, rec: &mut Rec) -> EResult<Option<Box<dyn Engine>>> {
    e.input_log()?;
    let of = offer_at(e)?;
    let pts = hit_points(e, &of, a, cfg.max_points)?;
    if pts.is_empty() {
        return Ok(None);
    }
    // the candidates as click lists with the target's place marked None (filled with each hit point)
    let mut firsts: Vec<Vec<Option<Point>>> = vec![vec![None]];
    for s in &of.verbs {
        firsts.push(vec![Some(*s), None]);
    }
    let cands: Vec<Vec<Option<Point>>> = match b {
        None => firsts,
        Some(bo) => {
            let bp: Vec<Point> = of.targets.iter().filter(|p| p.hit == Hit::Object(bo) || p.hit == Hit::Actor(bo)).take(1).copied().collect();
            let seconds: Vec<Point> = if bp.is_empty() { of.verbs.iter().take(cfg.max_slots).copied().collect() } else { bp };
            let mut v = Vec::new();
            for f in &firsts {
                for s in &seconds {
                    let mut c = f.clone();
                    c.push(Some(*s));
                    v.push(c);
                }
            }
            v
        }
    };
    let want = (v, a, b.unwrap_or(0));
    for c in cands {
        if rec.candidates >= cfg.max_candidates {
            break;
        }
        rec.candidates += 1;
        for p in &pts {
            let clicks: Vec<Point> = c.iter().map(|x| x.unwrap_or(*p)).collect();
            let (fork, records, mut att) = try_clicks(e, "sentence", &clicks, cfg)?;
            rec.tries += 1;
            let seen: Vec<(i32, i32, i32)> = records
                .iter()
                .flat_map(|r| r.writes.iter())
                .filter(|t| t.kind == TouchKind::Sentence)
                .map(|t| (t.a, t.b, t.old))
                .collect();
            let ok = seen.contains(&want);
            att.seen = seen.iter().map(|s| format!("({},{},{})", s.0, s.1, s.2)).collect::<Vec<_>>().join(" ");
            att.accepted = ok;
            rec.attempts.push(att);
            if ok {
                return Ok(Some(fork));
            }
            fork.quit();
        }
    }
    Ok(None)
}

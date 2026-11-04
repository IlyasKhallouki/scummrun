//! E015's G1 (experiments/E015-engine-gates/plan.md, "G1: realize a planned sentence through the engine, not a
//! profile"): a planned step turned into clicks by searching in forks of the engine, each candidate judged by the
//! engine's own input log (patch 0007). Switch `FMI_REALIZE=1`, off by default. E016 puts it on the run path
//! (experiments/E016-realize-run/plan.md, "A. Realization on the run path"; amendment (1)): with the switch on, a window
//! child's "click" and "line" steps are realized here (`play`), not by the executor and its profile.
//!
//! * **The offer** is afford's (`offer_cfg`): the visible verb slots, the touchable room objects and the actors with a
//!   point the engine's hit test confirms, and the walk boxes' floor points.
//! * **The approach** (a target with no hit point on the screen): the floor points, nearest first to the target's
//!   walk point in room coordinates, each clicked in a fork with no verb click and run to the bridge's next idle. A
//!   click is accepted when its input log shows a walk and the camera's xstart moved toward the target, or the target
//!   gained a hit point. From the accepted fork it repeats, at most `max_approach` clicks. E016 amendments (3) and (4):
//!   each box's point nearest the target joins the boxes' usual points in that order; then, as stepping stones, bare
//!   clicks on room objects nearer the target, judged by the same test.
//! * **The sentence** (v, a, b): candidates in order, (i) a bare click on a, (ii) each offered verb slot then a, and
//!   for a two-object sentence each of those followed by b, or by each visible slot when b has no hit point. Each is
//!   tried at up to `max_points` of a's hit points, in a fork run until the input log has the last click's record. A
//!   candidate is accepted when the log's `Sentence` touch is (v, a, b). At most `max_candidates`. E016 amendment (5):
//!   also when the log shows no Sentence touch at all and object a's script starts before the next idle stop (the
//!   engine's slot runs; the verb is not checked). A walk_to step is a
//!   sentence with the record's walk verb (amendment (1): its run shows as the Sentence touch (w, a, 0)).
//! * **An inventory object** (E016 A, amendment (1)): when a has no hit point on the screen and no place in the room,
//!   each visible verb slot is tried as a's click: a verb choice, the slot, then b's hit point (or each visible slot
//!   when b has none); accepted on the same Sentence touch. The approach then goes to b when b has a place.
//! * **A dialog verb** v: the offered verb slot whose dump id is v (in SCUMM v5 a slot's id is its verb number),
//!   clicked in a fork; accepted when the handler took the click (verdict "taken").
//!
//! The engine is the only judge: no interface profile, no verb or variable number of a game, no wait for a verb.

use crate::executor::{Br, PResult, PlayErr};
use crate::replay::InputStep;
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
            // the approach: from `e`, then from each accepted fork. Its target is a; when a has no hit point and no
            // place in the room (an inventory object), b when b has a place and no hit point (E016 A); else none.
            let mut cur: Option<Box<dyn Engine>> = None;
            let mut target: Option<Option<i32>> = None;
            loop {
                let base: &mut Box<dyn Engine> = match cur.as_mut() {
                    Some(c) => c,
                    None => &mut *e,
                };
                base.input_log()?;
                let (o, of) = obs_offer(base)?;
                let t = *target.get_or_insert_with(|| {
                    if has_hit(&of, a) || place_of(&o, a).is_some() {
                        Some(a)
                    } else {
                        b.filter(|&bo| !has_hit(&of, bo) && place_of(&o, bo).is_some())
                    }
                });
                let Some(t) = t else {
                    break;
                };
                if has_hit(&of, t) {
                    break;
                }
                if rec.approach >= cfg.max_approach {
                    if let Some(c) = cur {
                        c.quit();
                    }
                    return Ok(Err(rec.miss("approach", t0)));
                }
                match approach_once(base, &of, t, cfg, &mut rec)? {
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
    Ok(obs_offer(e)?.1)
}

/// The dump at the engine's stop and afford's offer from it.
fn obs_offer(e: &mut Box<dyn Engine>) -> EResult<(Obs, Offer)> {
    let o = e.obs(&ObsReq::default())?;
    let of = afford::offer_cfg(e, &o, afford::boxes_on(), afford::offer_screen_on())?;
    Ok((o, of))
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
    // E016 amendment (5): the fork's slot runs are read from here on (`judge`)
    b.segment(true)?;
    b.runs()?;
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
/// bridge's next idle (None: no point accepted, or the target has no place in the room). E016 amendments (3) and
/// (4): the walk boxes' usual points and each box's point nearest the target (`near_points`), in one list by
/// distance to the target; when no floor point is accepted, a bare click on a room object nearer the target as a
/// stepping stone (`stones`).
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
    // the boxes' usual points and each box's nearest point in one list, nearest the target first (amendment (4))
    let mut floor: Vec<(&'static str, Point)> = of.walks.iter().map(|p| ("approach", *p)).collect();
    floor.extend(near_points(e, of, place)?.into_iter().map(|p| ("near", p)));
    floor.sort_by_key(|f| d2(&f.1));
    let stones = stones(&o, of, a, place, xs);
    let tries = floor.into_iter().map(|(ph, p)| (ph, p, None));
    for (phase, p, stone) in tries.chain(stones.into_iter().map(|(p, s)| ("stone", p, Some(s)))) {
        if let Some(b) = approach_try(e, phase, p, stone, a, place, cfg, rec)? {
            return Ok(Some(b));
        }
    }
    Ok(None)
}

/// One approach candidate in a fork: the click, the run to the bridge's next idle, and the test (E015 G1): a walk and
/// the camera's xstart moved toward the target, or the target gained a hit point. For a stepping stone the walk is a
/// Walk touch or the click's Sentence touch on the stone (patch 0007 logs the handler's writes; a walk the sentence
/// script starts is not among them: amendment (3)).
#[allow(clippy::too_many_arguments)]
fn approach_try(e: &mut Box<dyn Engine>, phase: &'static str, p: Point, stone: Option<i32>, a: i32, place: (i32, i32), cfg: &Cfg,
                rec: &mut Rec) -> EResult<Option<Box<dyn Engine>>> {
    let xs = e.status().xstart;
    let (mut b, mut records, mut att) = try_clicks(e, phase, &[p], cfg)?;
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
        let walk = records.iter().any(|r| {
            r.writes.iter().any(|t| t.kind == TouchKind::Walk || (t.kind == TouchKind::Sentence && stone.is_some_and(|s| t.b == s)))
        });
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
    Ok(None)
}

/// E016 amendment (3): for each walkable box (afford's rule), its point nearest the target's place `t` (room
/// coordinates) on the screen: y the target's, clamped to the box's rows and the main screen's; x the target's,
/// clamped to the box's columns at that row (between its left edge ul-ll and its right edge ur-lr) and to the
/// screen's. Kept where the engine's hit test answers no object and no actor (afford's floor test; one AtMany), and
/// not the box's usual point. Empty without the walk boxes (FMI_BOXES off).
fn near_points(e: &mut Box<dyn Engine>, of: &Offer, t: (i32, i32)) -> EResult<Vec<Point>> {
    if of.boxes.is_none() {
        return Ok(vec![]);
    }
    let st = e.status().clone();
    let xs = st.xstart;
    let (top, h) = st.screen.map(|s| (s.0, s.1)).unwrap_or((0, afford::SCREEN_H));
    // the main screen's rows and columns in room coordinates
    let (ry0, ry1) = (0, h - top - 1);
    let (rx0, rx1) = (xs, xs + afford::SCREEN_W - 1);
    let edge = |p: (i32, i32), q: (i32, i32), y: i32| {
        if q.1 == p.1 {
            p.0.min(q.0)
        } else {
            p.0 + (q.0 - p.0) * (y - p.1).clamp(0.min(q.1 - p.1), 0.max(q.1 - p.1)) / (q.1 - p.1)
        }
    };
    let mut cands: Vec<(usize, (i32, i32))> = Vec::new();
    for (i, w) in e.boxes()?.boxes.iter().enumerate().skip(afford::FIRST_VALID_BOX) {
        if !afford::walkable(w.flags) {
            continue;
        }
        let (by0, by1) = (w.ul.1.max(w.ur.1), w.ll.1.min(w.lr.1));
        let (by0, by1) = if by0 <= by1 { (by0, by1) } else { (w.ul.1.min(w.ur.1), w.ll.1.max(w.lr.1)) };
        let (y0, y1) = (by0.max(ry0), by1.min(ry1));
        if y0 > y1 {
            continue;
        }
        let y = t.1.clamp(y0, y1);
        let (l, r) = (edge(w.ul, w.ll, y), edge(w.ur, w.lr, y));
        let (l, r) = (l.min(r).max(rx0), l.max(r).min(rx1));
        if l > r {
            continue;
        }
        let p = (t.0.clamp(l, r) - xs, y + top);
        if !of.walks.iter().any(|q| q.hit == Hit::Walk(i as i32) && (q.x, q.y) == p) {
            cands.push((i, p));
        }
    }
    if cands.is_empty() {
        return Ok(vec![]);
    }
    let ats = e.at_many(&cands.iter().map(|c| c.1).collect::<Vec<_>>())?;
    Ok(cands
        .into_iter()
        .zip(ats)
        .filter(|(_, at)| at.main.is_some_and(|m| m.2 == 0 && m.3 == 0))
        .map(|((i, p), _)| Point { x: p.0, y: p.1, hit: Hit::Walk(i as i32) })
        .collect())
}

/// E016 amendment (3): the stepping stones: each room object of the offer (a hit point on the screen), not a, whose
/// walk point is nearer the target's place than the view's middle column is, the nearest to the target first; with
/// its point for a bare click.
fn stones(o: &Obs, of: &Offer, a: i32, t: (i32, i32), xs: i32) -> Vec<(Point, i32)> {
    let view = view_dist(xs, t.0);
    let ros = o.room_objects.as_deref().unwrap_or(&[]);
    let mut v: Vec<(i64, Point, i32)> = of
        .targets
        .iter()
        .filter_map(|p| match p.hit {
            Hit::Object(id) if id != a => ros.iter().find(|r| r.id == id).map(|r| (r.walk, *p, id)),
            _ => None,
        })
        .filter(|(w, _, _)| (w.0 - t.0).abs() < view)
        .map(|(w, p, id)| {
            let (dx, dy) = ((w.0 - t.0) as i64, (w.1 - t.1) as i64);
            (dx * dx + dy * dy, p, id)
        })
        .collect();
    v.sort_by_key(|x| x.0);
    v.into_iter().map(|(_, p, id)| (p, id)).collect()
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
    let (o, of) = obs_offer(e)?;
    let pts = hit_points(e, &of, a, cfg.max_points)?;
    if pts.is_empty() {
        return inventory(e, &o, &of, v, a, b, cfg, rec);
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
    let live = live_slots(&o, a);
    for c in cands {
        if rec.candidates >= cfg.max_candidates {
            break;
        }
        rec.candidates += 1;
        for p in &pts {
            let clicks: Vec<Point> = c.iter().map(|x| x.unwrap_or(*p)).collect();
            let (mut fork, records, mut att) = try_clicks(e, "sentence", &clicks, cfg)?;
            rec.tries += 1;
            let (ok, seen) = judge(&mut fork, &records, want, &att, &live, cfg)?;
            att.seen = seen;
            att.frame = fork.status().frame;
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

/// Where an object's script runs (ScummVM's WIO_INVENTORY, WIO_ROOM, WIO_FLOBJECT).
const OBJECT_WHERE: [i32; 3] = [WIO_INVENTORY, WIO_ROOM, WIO_FLOBJECT];

/// The slots that hold a script of object `a` in the dump `o` (live at the step's start).
fn live_slots(o: &Obs, a: i32) -> Vec<i32> {
    o.slots.iter().filter(|s| OBJECT_WHERE.contains(&s.where_) && s.number == a).map(|s| s.slot).collect()
}

/// A sentence candidate judged on its fork (stopped where its last click's record appeared), E016 amendment (5):
/// accepted when its input log shows the Sentence touch `want` (as E015 G1); or, when the log shows no Sentence touch
/// at all, when object a's script starts after the candidate's first click: the fork is run on to the bridge's next
/// idle stop (at most `horizon` frames) and its slot runs (the segment, armed in `try_clicks`) are read for a run with
/// where inventory, room or floating object, number a, in a slot not in `live`. The engine reports no verb for such a
/// run, so the verb is not checked (the amendment's weakness). The verdict and what was seen.
fn judge(fork: &mut Box<dyn Engine>, records: &[InputRecord], want: (i32, i32, i32), att: &Attempt, live: &[i32], cfg: &Cfg)
         -> EResult<(bool, String)> {
    let seen = sentences(records);
    if seen.contains(&want) || !seen.is_empty() {
        return Ok((seen.contains(&want), seen_text(&seen)));
    }
    let first = att.events.first().map(|x| x.frame).unwrap_or(0);
    let mut runs = fork.runs()?.runs;
    let end = fork.status().frame + cfg.horizon;
    match fork.run(&RunSpec { idle: true, max_frame: Some(end), ..Default::default() }) {
        Ok(_) => runs.extend(fork.runs()?.runs),
        Err(EngineError::Ended(_)) => return Ok((false, "ended".into())),
        Err(x) => return Err(x),
    }
    let started = runs.iter().find(|r| OBJECT_WHERE.contains(&r.where_) && r.number == want.1 && r.frame >= first && !live.contains(&r.slot));
    Ok(match started {
        Some(r) => (true, format!("run of {}'s script at frame {} (slot {}, code {})", want.1, r.frame, r.slot, r.code0)),
        None => (false, String::new()),
    })
}

/// The Sentence touches (verb, object A, object B) of the records, in order.
fn sentences(records: &[InputRecord]) -> Vec<(i32, i32, i32)> {
    records
        .iter()
        .flat_map(|r| r.writes.iter())
        .filter(|t| t.kind == TouchKind::Sentence)
        .map(|t| (t.a, t.b, t.old))
        .collect()
}

fn seen_text(seen: &[(i32, i32, i32)]) -> String {
    seen.iter().map(|s| format!("({},{},{})", s.0, s.1, s.2)).collect::<Vec<_>>().join(" ")
}

/// ScummVM's `kImageVerbType`: a verb slot drawn from an image. In SCUMM v5 that image is an object's (o5_verbOps'
/// image subop calls setVerbObject with an object number), so a slot that shows an object is of this type whenever the
/// game draws its inventory as images (amendment (2)).
pub const IMAGE_VERB: i32 = 1;

/// E016 A, the inventory extension (plan.md, A "Inventory objects"; amendment (1)): a has no hit point on the screen.
/// Each visible verb slot is tried as a's click: a verb choice, the slot, then b's hit point (or each visible slot when
/// b has none). The verb choices in order: the offered slot whose dump id is v (in SCUMM v5 a slot's id is its verb
/// number, as the dialog step uses), no verb click, then the other offered slots in the dump's order. For each verb
/// choice a's slot runs over the offered slots without the choice's own, at most `max_slots`: the image slots first
/// (`IMAGE_VERB` in the dump `o`; amendment (2), fix 1), then the others, each in the dump's order; b's slots in the
/// same order. The engine judges each candidate as a room object's: the Sentence touch (v, a, b); the slot that
/// shows a is never read. The accepted fork, or None within `max_candidates`.
#[allow(clippy::too_many_arguments)]
fn inventory(e: &mut Box<dyn Engine>, o: &Obs, of: &Offer, v: i32, a: i32, b: Option<i32>, cfg: &Cfg, rec: &mut Rec)
             -> EResult<Option<Box<dyn Engine>>> {
    let vslot = of.verbs.iter().find(|p| matches!(p.hit, Hit::Verb { id, .. } if id == v)).copied();
    let mut choices: Vec<Option<Point>> = Vec::with_capacity(of.verbs.len() + 1);
    choices.extend(vslot.map(Some));
    choices.push(None);
    choices.extend(of.verbs.iter().filter(|p| Some(**p) != vslot).map(|p| Some(*p)));
    // the slots an object's click may be: the image slots first, then the others
    let image = |p: &Point| matches!(p.hit, Hit::Verb { slot, .. } if o.verbs.iter().any(|x| x.slot == slot && x.kind == IMAGE_VERB));
    let ordered: Vec<Point> = of.verbs.iter().filter(|p| image(p)).chain(of.verbs.iter().filter(|p| !image(p))).copied().collect();
    // b's clicks: its hit point, or each visible slot (b in the inventory too); none for a one-object sentence
    let thirds: Vec<Option<Point>> = match b {
        None => vec![None],
        Some(bo) => {
            let bp: Vec<Point> = of.targets.iter().filter(|p| p.hit == Hit::Object(bo) || p.hit == Hit::Actor(bo)).take(1).copied().collect();
            if bp.is_empty() {
                ordered.iter().take(cfg.max_slots).map(|p| Some(*p)).collect()
            } else {
                bp.into_iter().map(Some).collect()
            }
        }
    };
    let want = (v, a, b.unwrap_or(0));
    let live = live_slots(o, a);
    for c in &choices {
        let slots: Vec<Point> = ordered.iter().filter(|p| Some(**p) != *c).take(cfg.max_slots).copied().collect();
        for s in &slots {
            for t in &thirds {
                if rec.candidates >= cfg.max_candidates {
                    return Ok(None);
                }
                rec.candidates += 1;
                let clicks: Vec<Point> = c.iter().chain(std::iter::once(s)).chain(t.iter()).copied().collect();
                let (mut fork, records, mut att) = try_clicks(e, "inventory", &clicks, cfg)?;
                rec.tries += 1;
                let (ok, seen) = judge(&mut fork, &records, want, &att, &live, cfg)?;
                att.seen = seen;
                att.frame = fork.status().frame;
                att.accepted = ok;
                rec.attempts.push(att);
                if ok {
                    return Ok(Some(fork));
                }
                fork.quit();
            }
        }
    }
    Ok(None)
}

// --- E016 A: the run path's hook ------------------------------------------------------------------------------------

/// The step realize takes for a window child's input (amendment (1)): a "click" step (a sentence or walk_to record,
/// whose verb is the record's) is `Step::Sentence`, a "line" step (a dialog_line record) `Step::Dialog`; None for
/// another kind or a step without its verb or object.
pub fn step_of(inp: &InputStep) -> Option<Step> {
    match inp.kind {
        "click" => Some(Step::Sentence { v: inp.verb? as i32, a: inp.a? as i32, b: inp.b.map(|x| x as i32) }),
        "line" => Some(Step::Dialog { v: inp.verb? as i32 }),
        _ => None,
    }
}

/// E016 A (plan.md, "A. Realization on the run path"; amendment (1)): a window child's "click" or "line" step realized
/// on the child's engine with FMI_REALIZE=1 (`executor::play_input` calls it). The branch is first run to the bridge's
/// idle stop when it is not at one (at most `Cfg::horizon` frames). The accepted events are then played in place on
/// the branch: scheduled with `Br::inputs` and run to the last one's frame through `Br::run`, so the frame observer
/// and the strict matcher see every frame as with the executor's input; the accepted fork is quit. `movie` gets the
/// events. A miss is `PlayErr::Route` with realize's reason ("approach", "no sentence", "no slot", "not taken").
pub fn play(b: &mut Br, inp: &InputStep, name: &str, movie: &mut Vec<Event>) -> PResult<()> {
    let route = |m: String| Err(PlayErr::Route(format!("{name}: realize: {m}")));
    let Some(step) = step_of(inp) else {
        return route(format!("a {} step without its verb or object", inp.kind));
    };
    let cfg = Cfg::default();
    if b.status().idle.is_none() {
        let end = b.frame() + cfg.horizon;
        match b.run(&RunSpec { idle: true, max_frame: Some(end), ..Default::default() }) {
            Ok(st) if st.reason == "idle" => {}
            Ok(_) => return route(format!("no idle stop within {} frames", cfg.horizon)),
            Err(EngineError::Ended(st)) => return route(format!("the run ended: {}", st.reason)),
            Err(x) => return Err(x.into()),
        }
    }
    match realize(&mut *b.e, &step, &cfg)? {
        Ok(r) => {
            r.engine.quit();
            let Some(last) = r.events.iter().map(|x| x.frame).max() else {
                return route("no input".into());
            };
            b.inputs(&r.events)?;
            movie.extend(r.events.iter().cloned());
            match b.run(&RunSpec { frame: Some(last), ..Default::default() }) {
                Ok(_) => Ok(()),
                Err(EngineError::Ended(st)) => route(format!("the run ended: {}", st.reason)),
                Err(x) => Err(x.into()),
            }
        }
        Err(m) => route(format!("{} ({} approach clicks, {} candidates, {} forks)", m.reason, m.approach, m.candidates, m.tries)),
    }
}

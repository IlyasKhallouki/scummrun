//! direct: a search in the engine itself (a spike, not yet an experiment's component).
//!
//! From the boot's first idle stop, every input the interface offers at a stop is tried in a fork of the engine, and a
//! stop that holds something new is kept and expanded in turn. The engine is the model: the scripts are read only for
//! the guide's keys (which variables the game's code tests), never for what an input does.
//!
//!   direct OUT [--minutes M] [--workers W] [--horizon H] [--no-skips] [--guide FILE] [--rungs FILE] [--watch KEY=V,..]
//!   direct verify NODES.jsonl ID OUT [KEY=V,..]
//!
//! * **Inputs** are click sequences, found breadth first at a stop (`clicks`). The clickable points are afford's offer
//!   (the visible verb slots, the objects and actors the hit test answers) and a grid of floor points. A click that
//!   leaves the game composing a sentence (afford's `composing`: only the input script ran, and ego stands where it
//!   stood) gives a stop to click on from, unless an earlier sequence showed the same interface over the same world;
//!   any other click ends its sequence. At most MAX_CLICKS clicks.
//! * **A trial** is a fork, the click, the pointer parked on a point that is no verb, object or actor (the sentence
//!   line names what the pointer is over), then a run to the next stop (`calm`), within the horizon, or four times
//!   it. A stop is where ego stands still and the bridge is idle, or, BUSY frames after the inputs, where the game
//!   takes a click all the same (user input on, no cutscene): a scene whose actors talk on has no idle frame.
//! * **The game's own moves.** Each stop is also run with no input for WAITS frames. Where that changes a key of the
//!   world that fewer than MOTION_TRIES stops of that room and level saw change, the inputs are tried again from
//!   there (a door a timer opens is open only then; a room's own motion is tried a few times, not at every stop).
//! * **The world** of a stop is its room, owners, states, classes and bits, and the guide's variables but the
//!   interface's own (those the input script writes, and those a fifth of a stop's inputs change), those that change
//!   with no input (clocks), and those seen with more than VAR_CAP values (counters without end).
//! * **Levels.** A stop's level is the number of objects ego has owned on the way to it (and did not own at the
//!   boot). Something is new to a stop when no kept stop of its level or a higher one holds it: with one more object
//!   in hand, every room, input and position counts again. The highest level is expanded first.
//! * **Kept.** Within its level a stop goes to the first queue it fits: (0) a world atom other than a variable's
//!   that is new; (1) an interface or a thing on the screen new in that room; (2) a walk box or camera bucket new in
//!   that room; (3) a variable's value that is new; (4) an atom of the guide's keys new in that room; (5) any other
//!   world atom new in that room, or a new world (its atoms together, without those that change by themselves). A
//!   stop that fits none is dropped. Queue 0 is expanded first, then 1, and so on; queues 4 and 5 wait until no
//!   level holds a stop in queues 0 to 3.
//! * **Order.** Stops are taken from the queues a batch at a time and expanded side by side, each from the search's
//!   state at the batch's start; their results are committed in the batch's order. A run is the same every time.
//!
//! OUT/nodes.jsonl holds one line per kept stop (its parent, clicks and movie, so it replays from boot); offers.jsonl
//! one line per expanded stop. The rungs file is the evaluation's: the search never reads it for a choice.

use fmi_core::*;
use fmi_engine::{Host, HostCfg};
use fmi_explore::afford::{self, Hit, Point};
use serde_json::{json, Value};
use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, HashSet, VecDeque};
use std::hash::{Hash, Hasher};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// The longest click sequence of one input (the inventory scrolled, a verb and two objects).
const MAX_CLICKS: usize = 4;
/// Frames a stop is held before it is read (`calm`).
const CALM: u64 = 6;
/// Frames a trial's inputs take: the click, then the pointer's move.
const LEAD: u64 = 3;
/// Frames after a trial's inputs from which a stop need not be idle (the module's text, "A trial").
const BUSY: u64 = 300;
/// The game's own moves: frames run with no input, each one trial of a stop.
const WAITS: [u64; 3] = [50, 200, 800];
/// Stops of a room and level whose waits a key may change before its change stops calling for the inputs again.
const MOTION_TRIES: u32 = 3;
/// Pixels of the camera's bucket: a fifth of the screen, so a walk across part of it shows.
const CAM_BUCKET: i32 = 64;
/// The floor clicks' grid over the main screen: the columns, and the rows in eighths of its height.
const FLOOR_X: [i32; 7] = [6, 58, 110, 160, 210, 262, 313];
const FLOOR_Y: [i32; 4] = [3, 5, 7, 8];
/// Engines kept alive for stops waiting in a queue; the others are booted again from their movies.
const LIVE_MAX: usize = 32;
/// The queues (the module's text, "Kept"), and the last one whose stops get a kept engine.
const QUEUES: usize = 6;
const LIVE_QUEUES: usize = 3;
/// The queues expanded at every level, the highest first, before any level's later ones.
const FIRST_QUEUES: usize = 4;
/// Stops of one batch for each worker.
const BATCH: usize = 3;
/// A variable seen with more values than this in kept stops counts no more.
const VAR_CAP: usize = 8;

struct Node {
    events: Vec<Event>,
    frame: u64,
    depth: u32,
    /// the objects ego has owned on the way here and did not own at the boot, ascending; the level is their count
    ever: Vec<u32>,
}

/// Whether `k` is new at `level`: no kept stop of that level or a higher one holds it. Marks it held at `level`.
fn fresh<K: Hash + Eq>(m: &mut HashMap<K, u32>, k: K, level: u32) -> bool {
    match m.get(&k) {
        Some(&l) if l >= level => false,
        _ => {
            m.insert(k, level);
            true
        }
    }
}

/// Which variables are world (the module's text, "The world").
#[derive(Default)]
struct Policy {
    /// the guide's keys, every kind
    guide: HashSet<Key>,
    iface_vars: HashSet<Key>,
    clocks: HashSet<Key>,
    capped: HashSet<Key>,
}

impl Policy {
    fn world(&self, k: Key) -> bool {
        world_kind(k) || (k.kind() == K_VAR && self.guide.contains(&k) && !self.iface_vars.contains(&k) && !self.clocks.contains(&k) && !self.capped.contains(&k))
    }
    fn world_of(&self, f: &Facts) -> Vec<(Key, Val)> {
        f.iter().filter(|p| self.world(p.0)).copied().collect()
    }
}

/// One rung of the evaluation's ladder: `least` of its conditions (key, at least?, value; None: ego) hold.
struct Rung {
    name: String,
    conds: Vec<(Key, bool, Option<Val>)>,
    least: usize,
    at: Option<(usize, f64)>,
}

#[derive(Default)]
struct Search {
    nodes: Vec<Node>,
    /// per level, the queues
    queues: Vec<[VecDeque<usize>; QUEUES]>,
    live: HashMap<usize, Box<dyn Engine>>,
    /// what kept stops hold, each with the highest level of a stop that holds it (`fresh`)
    seen: HashMap<(Key, Val), u32>,
    seen_at: HashMap<(i32, Key, Val), u32>,
    sees: HashMap<(i32, i32), u32>,
    ifaces: HashMap<(i32, u64), u32>,
    boxes: HashMap<(i32, i32), u32>,
    cams: HashMap<(i32, i32), u32>,
    cells: HashMap<u64, u32>,
    /// the objects ego owns at the boot
    root_owned: HashSet<u32>,
    var_vals: HashMap<Key, Vec<Val>>,
    policy: Policy,
    /// the world keys a run with no input changed somewhere: left out of the cells
    ambient: HashSet<Key>,
    /// per level, room and key: the stops expanded there whose waits changed it
    motion: HashMap<(u32, i32, Key), u32>,
    rooms: Vec<i32>,
    /// the highest level of a kept stop
    top: u32,
    /// engines being made for kept stops (counted against LIVE_MAX)
    making: usize,
    live_max: usize,
    phase: [f64; 5],
    frames: u64,
    skipped: u64,
    trials: u64,
    refused: u64,
    not_idle: u64,
    expanded: u64,
    boots: u64,
    kept: [u64; QUEUES],
    ends: Vec<(usize, String)>,
    watch: Vec<(Key, Val, Option<(usize, f64)>)>,
    rungs: Vec<Rung>,
    log: Option<std::fs::File>,
    offers: Option<std::fs::File>,
    t0: Option<Instant>,
}

struct Cfg {
    out: PathBuf,
    horizon: u64,
    workers: usize,
}

static DIRS: AtomicU64 = AtomicU64::new(0);

fn eng_dir(out: &Path) -> PathBuf {
    let d = out.join("eng").join(format!("b{}", DIRS.fetch_add(1, Ordering::Relaxed)));
    std::fs::create_dir_all(&d).expect("an engine folder");
    d
}

fn world_kind(k: Key) -> bool {
    matches!(k.kind(), K_ROOM | K_OWNER | K_STATE | K_CLASS | K_BIT)
}

const NAMES: ObsReq = ObsReq { objects: None, names: true, exact: false };

/// What the interface shows at a stop: the visible verb slots with their ids and texts (the verbs, the inventory's
/// names, the sentence being composed, a dialog's choices). Only the hash is kept: no game text leaves the process.
fn iface(o: &Obs) -> u64 {
    let mut v: Vec<(i32, i32, &str)> = o.verbs.iter().map(|x| (x.slot, x.id, x.name.as_deref().unwrap_or(""))).collect();
    v.sort_unstable();
    let mut h = DefaultHasher::new();
    v.hash(&mut h);
    h.finish()
}

/// One stop as read from its engine: the facts, and what is not in them.
struct Res {
    name: String,
    events: Vec<Event>,
    st: Status,
    facts: Facts,
    room: i32,
    /// the objects whose box is partly on the screen, and the actors on it (as negative ids)
    vis: Vec<i32>,
    walkbox: i32,
    cam: i32,
    /// what the interface shows (`iface`)
    iface: u64,
}

fn read(name: String, events: Vec<Event>, o: &Obs, st: Status) -> Res {
    let facts = facts_of(o);
    let (x0, x1) = (st.xstart, st.xstart + afford::SCREEN_W);
    let mut vis: Vec<i32> = o
        .room_objects
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .filter(|r| r.w > 0 && r.h > 0 && r.x < x1 && r.x + r.w > x0)
        .map(|r| r.id)
        .chain(o.actors.iter().filter(|a| a.x >= x0 && a.x < x1).map(|a| -a.id))
        .collect();
    vis.sort_unstable();
    Res {
        name,
        events,
        room: facts.val(Key::pack(K_ROOM, 0, 0, 0)) as i32,
        vis,
        walkbox: st.walkbox.unwrap_or(-1),
        cam: st.xstart.div_euclid(CAM_BUCKET),
        iface: iface(o),
        facts,
        st,
    }
}

impl Res {
    /// A stop inside a sentence: what the interface shows. A click that leaves a sentence half made ran the input
    /// script only, so the world it stands in is the round's, but for what changes by itself.
    fn mid(&self) -> u64 {
        self.iface
    }
}

/// The world's cell: the room, the world's atoms but the ambient keys', and the interface, wherever ego stands.
fn cell(room: i32, iface: u64, world: &[(Key, Val)], ambient: &HashSet<Key>) -> u64 {
    let mut h = DefaultHasher::new();
    (room, iface).hash(&mut h);
    for p in world.iter().filter(|p| !ambient.contains(&p.0)) {
        p.hash(&mut h);
    }
    h.finish()
}

enum Played {
    Stop(Status),
    Ended(Status),
    NotIdle,
}

fn go(b: &mut Box<dyn Engine>, spec: RunSpec) -> EResult<Result<Status, Status>> {
    match b.run(&spec) {
        Ok(s) => Ok(Ok(s)),
        Err(EngineError::Ended(s)) => Ok(Err(*s)),
        Err(x) => Err(x),
    }
}

/// Whether the game takes a click at this stop (the engine's own gate, checkExecVerbs: user input on, no cutscene,
/// a room).
fn takes_click(st: &Status) -> bool {
    st.up > 0 && st.cs == 0 && st.room != 0
}

/// On to a stop where ego stands still and the bridge is idle, or, from frame `busy` on, the game takes a click; or
/// `end`.
fn settle(b: &mut Box<dyn Engine>, end: u64, busy: u64, ego: i32) -> EResult<Played> {
    loop {
        let st = b.status().clone();
        if st.frame >= end {
            return Ok(Played::NotIdle);
        }
        let ready = st.idle.is_some() || (st.frame >= busy && takes_click(&st));
        if ready && st.mv.unwrap_or(0) == 0 {
            return Ok(Played::Stop(st));
        }
        let spec = if ready {
            // ego still walking: on to where it stands
            let stands = Cond { kind: CondKind::ActorMoving, subject: ego, op: CondOp::Eq, value: 0, values: Vec::new(), negate: false };
            RunSpec { until: vec![stands], max_frame: Some(end), ..Default::default() }
        } else if st.frame < busy {
            RunSpec { idle: true, max_frame: Some(busy.min(end)), ..Default::default() }
        } else {
            // past `busy` and no click taken yet: on to the first frame that is idle or takes one
            RunSpec { idle: true, control: true, max_frame: Some(end), ..Default::default() }
        };
        if let Err(s) = go(b, spec)? {
            return Ok(Played::Ended(s));
        }
    }
}

/// `settle`, then CALM frames on and `settle` again: the texts an input leaves on the interface for a moment are gone.
/// A stop not reached within `horizon` frames of `from` gets four times as many.
fn calm(b: &mut Box<dyn Engine>, from: u64, horizon: u64, ego: i32) -> EResult<Played> {
    for end in [from + horizon, from + 4 * horizon] {
        let at = b.status().frame;
        match settle(b, end, from + LEAD + BUSY, ego)? {
            // it stood already, CALM frames past the inputs' frames (the trial ran them before this): read at once
            Played::Stop(st) if st.frame == at && at >= from + LEAD + CALM => return Ok(Played::Stop(st)),
            Played::Stop(st) => {
                if let Err(s) = go(b, RunSpec { frame: Some(st.frame + CALM), ..Default::default() })? {
                    return Ok(Played::Ended(s));
                }
                match settle(b, end.max(st.frame + CALM + 1), from + LEAD + BUSY, ego)? {
                    Played::NotIdle => {}
                    x => return Ok(x),
                }
            }
            Played::NotIdle => {}
            x => return Ok(x),
        }
    }
    Ok(Played::NotIdle)
}

fn name_of(pre: &str, clicks: &[Point]) -> String {
    format!("{pre}{}", clicks.iter().map(|p| p.hit.name()).collect::<String>())
}

/// A stop inside a sentence being composed: its engine (None: the round's own), and the clicks that led to it.
struct Mid {
    engine: Option<Box<dyn Engine>>,
    clicks: Vec<Point>,
    events: Vec<Event>,
    /// what the interface shows there
    iface: u64,
}

/// What a stop's trials gave.
#[derive(Default)]
struct Out {
    results: Vec<Res>,
    ends: Vec<String>,
    trials: u64,
    refused: u64,
    not_idle: u64,
    mids: u64,
    /// clicks not tried: slots known to pick another verb or to do nothing
    skipped: u64,
    offer: Value,
    /// the variables the input script wrote in a trial, or that a fifth of the stop's inputs changed
    iface_vars: HashSet<Key>,
    /// the variables, and the world's other keys, a run with no input changed
    clocks: HashSet<Key>,
    ambient: HashSet<Key>,
    room: i32,
    /// the trials' wall seconds by phase (the fork, the inputs, the run to the stop, the stop's reading, the quit), and
    /// the frames they ran
    phase: [f64; 5],
    frames: u64,
}

/// A point where the pointer is over no verb, object or actor, by the engine's hit test at this stop.
fn park_point(e: &mut Box<dyn Engine>) -> EResult<Option<(i32, i32)>> {
    let (w, h) = (afford::SCREEN_W, afford::SCREEN_H);
    let cands = [(w - 1, h - 1), (0, h - 1), (w / 2, h - 1), (w - 1, 0), (0, 0), (w / 2, 0), (w - 1, h / 2), (0, h / 2)];
    let ats = e.at_many(&cands)?;
    Ok(cands.iter().zip(ats.iter()).find(|(_, a)| a.verb_slot == 0 && a.main.map_or(true, |m| m.2 == 0 && m.3 == 0)).map(|(p, _)| *p))
}

/// The click sequences from the engine's stop (the module's text, "Inputs"), each a trial; `pre` names the round.
fn clicks(e: &mut Box<dyn Engine>, pre: &str, cfg: &Cfg, out: &mut Out) -> EResult<()> {
    let o0 = e.obs(&NAMES)?;
    let st0 = e.status().clone();
    let r0 = read(String::new(), Vec::new(), &o0, st0);
    let sentence = r0.facts.val(Key::var(afford::VAR_SENTENCE_SCRIPT as u64)) as i32;
    let input0 = r0.facts.val(Key::var(afford::VAR_VERB_SCRIPT as u64)) as i32;
    let ego = r0.facts.val(Key::pack(K_EGO, 0, 0, 0)) as i32;
    let first = out.results.len();
    let mut seen_mid: HashSet<u64> = HashSet::from([r0.mid()]);
    let mut frontier: Vec<Mid> = vec![Mid { engine: None, clicks: Vec::new(), events: Vec::new(), iface: r0.iface }];
    // the verb slots as the first clicks left them (a half-made stop's interface), and those a later click need not
    // try: a slot that from the first half-made stop gave what it gave from the round's stop (it only picks another
    // verb), or gave that stop again (it does nothing)
    let mut first_gave: HashMap<Hit, u64> = HashMap::new();
    let mut skip: Option<HashSet<Hit>> = None;
    for depth in 1..=MAX_CLICKS {
        let mut next: Vec<Mid> = Vec::new();
        for mut m in frontier {
            let me: &mut Box<dyn Engine> = match m.engine.as_mut() {
                Some(x) => x,
                None => &mut *e,
            };
            let o = me.obs(&ObsReq::default())?;
            let of = afford::offer_cfg(me, &o, false, true)?;
            let park = park_point(me)?;
            // the trials' forks record their slot runs and start with empty logs
            me.segment(true)?;
            me.runs()?;
            let _ = me.input_log();
            let mf = me.status().frame;
            let learn = depth == 2 && skip.is_none();
            let mut learned: HashSet<Hit> = HashSet::new();
            let mego = o.ego_pos;
            let mut points: Vec<Point> = of.verbs.clone();
            points.extend(of.targets.iter().copied());
            if depth == 1 {
                if out.offer.is_null() {
                    out.offer = of.json();
                }
                // floor clicks: a grid over the main screen, where the hit test finds no object and no actor
                let (top, h) = me.status().screen.map(|s| (s.0, s.1)).unwrap_or((0, afford::SCREEN_H));
                let grid: Vec<(i32, i32)> =
                    FLOOR_Y.iter().flat_map(|&k| FLOOR_X.iter().map(move |&x| (x, (top + (h - top) * k / 8).min(h - 2)))).collect();
                let ats = me.at_many(&grid)?;
                for (i, (&(x, y), a)) in grid.iter().zip(ats.iter()).enumerate() {
                    if a.main.is_some_and(|m| m.2 == 0 && m.3 == 0) {
                        points.push(Point { x, y, hit: Hit::Walk(i as i32) });
                    }
                }
            }
            for p in &points {
                if depth >= 2 && skip.as_ref().is_some_and(|s| s.contains(&p.hit)) {
                    out.skipped += 1;
                    continue;
                }
                let t = Instant::now();
                let mut b = me.fork()?;
                out.phase[0] += t.elapsed().as_secs_f64();
                let t = Instant::now();
                let mut cl = m.clicks.clone();
                cl.push(*p);
                let mut events = m.events.clone();
                events.push(Event { frame: mf + 1, what: Input::Click(p.x, p.y) });
                if let Some((x, y)) = park {
                    events.push(Event { frame: mf + 2, what: Input::Move(x, y) });
                }
                out.trials += 1;
                match b.inputs(&events[m.events.len()..]) {
                    Ok(()) => {}
                    Err(EngineError::Bridge(_)) => {
                        out.refused += 1;
                        b.quit();
                        continue;
                    }
                    Err(x) => return Err(x),
                }
                out.phase[1] += t.elapsed().as_secs_f64();
                let t = Instant::now();
                if let Err(st) = go(&mut b, RunSpec { frame: Some(mf + LEAD + CALM), ..Default::default() })? {
                    out.ends.push(format!("{} {}", name_of(pre, &cl), st.reason));
                    b.quit();
                    continue;
                }
                if depth == 1 {
                    // the variables the input script wrote for this click: the interface's own
                    if let Ok(l) = b.input_log() {
                        out.iface_vars.extend(l.records.iter().flat_map(|r| r.writes.iter()).filter(|t| t.kind == TouchKind::Var).map(|t| Key::var(t.a as u64)));
                    }
                }
                let played = calm(&mut b, mf, cfg.horizon, ego)?;
                out.phase[2] += t.elapsed().as_secs_f64();
                out.frames += b.status().frame.saturating_sub(mf);
                let t = Instant::now();
                match played {
                    Played::Stop(st) => {
                        let ob = b.obs(&NAMES)?;
                        let r = read(name_of(pre, &cl), events, &ob, st);
                        // only the input script ran since the click and ego stands where it stood: a sentence half made
                        let half = ob.ego_pos == mego && {
                            let runs = b.runs()?;
                            let input = [input0, r.facts.val(Key::var(afford::VAR_VERB_SCRIPT as u64)) as i32];
                            afford::composing(r.st.idle.as_deref().or(Some("sentence")), &runs.runs, mf + 1, &input, sentence)
                        };
                        out.phase[3] += t.elapsed().as_secs_f64();
                        if half {
                            if matches!(p.hit, Hit::Verb { .. }) {
                                if depth == 1 {
                                    first_gave.insert(p.hit, r.iface);
                                } else if learn && (first_gave.get(&p.hit) == Some(&r.iface) || r.iface == m.iface) {
                                    learned.insert(p.hit);
                                }
                            }
                            // a stop to click on from; at the last click's depth it is no stop at all
                            if depth < MAX_CLICKS && seen_mid.insert(r.mid()) {
                                out.mids += 1;
                                next.push(Mid { engine: Some(b), clicks: cl, iface: r.iface, events: r.events });
                                continue;
                            }
                        } else {
                            out.results.push(r);
                        }
                    }
                    Played::Ended(st) => out.ends.push(format!("{} {}", name_of(pre, &cl), st.reason)),
                    Played::NotIdle => out.not_idle += 1,
                }
                let t = Instant::now();
                b.quit();
                out.phase[4] += t.elapsed().as_secs_f64();
            }
            if learn {
                skip = Some(learned);
            }
            if let Some(x) = m.engine.take() {
                x.quit();
            }
        }
        frontier = next;
    }
    for mut m in frontier {
        if let Some(x) = m.engine.take() {
            x.quit();
        }
    }
    // a variable a fifth of the round's stops changed echoes the input
    let made = &out.results[first..];
    if made.len() >= 10 {
        let mut changed: HashMap<Key, usize> = HashMap::new();
        for r in made {
            for (k, _, _) in r0.facts.diff(&r.facts) {
                if k.kind() == K_VAR {
                    *changed.entry(k).or_default() += 1;
                }
            }
        }
        let n = made.len();
        out.iface_vars.extend(changed.into_iter().filter(|&(_, c)| c * 5 >= n).map(|(k, _)| k));
    }
    Ok(())
}

/// What `explore` reads of the search: the rooms' motion at the batch's start.
struct Snap {
    motion: HashMap<(u32, i32, Key), u32>,
}

/// Every input of the stop tried, at the stop and after the game's own moves. Reads the engine and `snap` only.
fn explore(e: &mut Box<dyn Engine>, level: u32, snap: &Snap, cfg: &Cfg) -> EResult<Out> {
    let o0 = e.obs(&NAMES)?;
    let st0 = e.status().clone();
    let r0 = read(String::new(), Vec::new(), &o0, st0.clone());
    let mut out = Out { room: r0.room, ..Default::default() };
    let ego = r0.facts.val(Key::pack(K_EGO, 0, 0, 0)) as i32;
    clicks(e, "", cfg, &mut out)?;
    // the game's own moves: no input for a while; where a key moved that few stops of this room and level saw move,
    // the inputs again from there
    let known = |k: Key| snap.motion.get(&(level, r0.room, k)).is_some_and(|&c| c >= MOTION_TRIES);
    for n in WAITS {
        let mut b = e.fork()?;
        out.trials += 1;
        match go(&mut b, RunSpec { frame: Some(st0.frame + n), ..Default::default() })? {
            Err(st) => out.ends.push(format!("wait{n} {}", st.reason)),
            Ok(_) => match calm(&mut b, st0.frame + n, cfg.horizon, ego)? {
                Played::Stop(st) => {
                    let ob = b.obs(&NAMES)?;
                    let r = read(format!("wait{n}"), Vec::new(), &ob, st);
                    let mut moved = false;
                    for (k, _, _) in r0.facts.diff(&r.facts) {
                        if k.kind() == K_VAR {
                            out.clocks.insert(k);
                        } else if world_kind(k) {
                            out.ambient.insert(k);
                            moved |= !known(k);
                        }
                    }
                    out.results.push(r);
                    if moved {
                        clicks(&mut b, &format!("wait{n}+"), cfg, &mut out)?;
                    }
                }
                Played::Ended(st) => out.ends.push(format!("wait{n} {}", st.reason)),
                Played::NotIdle => out.not_idle += 1,
            },
        }
        b.quit();
    }
    Ok(out)
}

/// A kept stop's engine: a fork of its parent's, the trial's events again (the engine is deterministic).
fn remake(e: &mut Box<dyn Engine>, events: &[Event], frame: u64) -> EResult<Box<dyn Engine>> {
    let mut b = e.fork()?;
    let run = (|| -> EResult<()> {
        let mut i = 0;
        while i < events.len() {
            // the events of one frame together, given at the frame before the first's own or later
            let f = events[i].frame;
            if f - 1 > b.status().frame {
                b.run(&RunSpec { frame: Some(f - 1), ..Default::default() })?;
            }
            let j = events[i..].iter().position(|x| x.frame > f + 1).map_or(events.len(), |k| i + k);
            b.inputs(&events[i..j])?;
            i = j;
        }
        if frame > b.status().frame {
            b.run(&RunSpec { frame: Some(frame), ..Default::default() })?;
        }
        Ok(())
    })();
    match run {
        Ok(()) => Ok(b),
        Err(x) => {
            b.quit();
            Err(x)
        }
    }
}

impl Search {
    /// One expanded stop's results committed: the kept stops made nodes and queued. Returns each kept stop to make an
    /// engine for: (the node, its events past the parent's, its frame).
    fn commit(&mut self, id: usize, out: Out) -> Vec<(usize, Vec<Event>, u64)> {
        let room0 = out.room;
        let level0 = self.nodes[id].ever.len() as u32;
        for &k in &out.ambient {
            *self.motion.entry((level0, room0, k)).or_default() += 1;
        }
        self.policy.iface_vars.extend(out.iface_vars.iter().copied());
        self.policy.clocks.extend(out.clocks.iter().copied());
        self.ambient.extend(out.ambient.iter().copied());
        self.trials += out.trials;
        self.skipped += out.skipped;
        self.frames += out.frames;
        for (a, b) in self.phase.iter_mut().zip(out.phase.iter()) {
            *a += b;
        }
        self.refused += out.refused;
        self.not_idle += out.not_idle;
        for x in out.ends {
            self.ends.push((id, x));
        }
        let results = out.results;
        let (movie, depth, ever0) = (self.nodes[id].events.clone(), self.nodes[id].depth, self.nodes[id].ever.clone());
        let mut keep: Vec<(usize, usize, usize, usize)> = Vec::new(); // (result, node, level, queue)
        for (i, r) in results.iter().enumerate() {
            let world = self.policy.world_of(&r.facts);
            let mut novel: Vec<String> = Vec::new();
            let mut class = QUEUES;
            let ego = r.facts.val(Key::pack(K_EGO, 0, 0, 0));
            let mut ever = ever0.clone();
            for p in r.facts.iter().filter(|p| p.0.kind() == K_OWNER && p.1 == ego) {
                let o = p.0.a() as u32;
                if !self.root_owned.contains(&o) {
                    if let Err(at) = ever.binary_search(&o) {
                        ever.insert(at, o);
                    }
                }
            }
            let level = ever.len() as u32;
            for &(k, val) in &world {
                if k.kind() == K_VAR {
                    if fresh(&mut self.seen, (k, val), level) {
                        novel.push(format!("{k}={val}"));
                        class = class.min(3);
                        let vals = self.var_vals.entry(k).or_default();
                        if !vals.contains(&val) {
                            vals.push(val);
                        }
                        if vals.len() > VAR_CAP {
                            self.policy.capped.insert(k);
                        }
                    }
                    continue;
                }
                if fresh(&mut self.seen, (k, val), level) {
                    novel.push(format!("{k}={val}"));
                    class = 0;
                }
                if fresh(&mut self.seen_at, (r.room, k, val), level) {
                    class = class.min(if self.policy.guide.contains(&k) { 4 } else { 5 });
                }
            }
            for &t in &r.vis {
                if fresh(&mut self.sees, (r.room, t), level) {
                    novel.push(format!("sees={t}"));
                    class = class.min(1);
                }
            }
            if fresh(&mut self.ifaces, (r.room, r.iface), level) {
                novel.push("iface".into());
                class = class.min(1);
            }
            if fresh(&mut self.boxes, (r.room, r.walkbox), level) {
                novel.push(format!("box={}", r.walkbox));
                class = class.min(2);
            }
            if fresh(&mut self.cams, (r.room, r.cam), level) {
                novel.push(format!("cam={}", r.cam));
                class = class.min(2);
            }
            if fresh(&mut self.cells, cell(r.room, r.iface, &world, &self.ambient), level) {
                class = class.min(5);
            }
            // nothing new, alone or together: dropped
            if class == QUEUES {
                continue;
            }
            if !self.rooms.contains(&r.room) {
                self.rooms.push(r.room);
            }
            let nid = self.nodes.len();
            let mut events = movie.clone();
            events.extend(r.events.iter().cloned());
            let secs = self.t0.map(|t| t.elapsed().as_secs_f64()).unwrap_or(0.0);
            for w in self.watch.iter_mut() {
                if w.2.is_none() && r.facts.val(w.0) == w.1 {
                    w.2 = Some((nid, secs));
                    eprintln!("[{secs:7.1}s] WATCH {}={} first held by node {nid}", w.0, w.1);
                }
            }
            for g in self.rungs.iter_mut().filter(|g| g.at.is_none()) {
                let held = g.conds.iter().filter(|&&(k, ge, v)| {
                    let (have, want) = (r.facts.val(k), v.unwrap_or(ego));
                    if ge { have >= want } else { have == want }
                });
                if held.count() >= g.least {
                    g.at = Some((nid, secs));
                    eprintln!("[{secs:7.1}s] RUNG {} first held by node {nid} (level {level}, {} trials so far)", g.name, self.trials);
                }
            }
            if novel.len() > 12 {
                // a stop new to its level holds everything anew: the count is enough
                let n = novel.len();
                novel.truncate(12);
                novel.push(format!("+{}", n - 12));
            }
            let line = json!({"id": nid, "parent": id, "depth": depth + 1, "frame": r.st.frame, "room": r.room, "level": level,
                "class": class, "how": r.name, "novel": novel, "t": (secs * 10.0).round() / 10.0,
                "events": events.iter().map(fmi_explore::events::event_json).collect::<Vec<Value>>()});
            if let Some(f) = self.log.as_mut() {
                let _ = writeln!(f, "{line}");
            }
            self.nodes.push(Node { events, frame: r.st.frame, depth: depth + 1, ever });
            self.kept[class] += 1;
            self.top = self.top.max(level);
            keep.push((i, nid, level as usize, class));
        }
        if let Some(f) = self.offers.as_mut() {
            let _ = writeln!(f, "{}", json!({"node": id, "room": room0, "offer": out.offer, "trials": out.trials, "skipped": out.skipped, "mids": out.mids,
                "stops": results.len(), "kept": keep.len(), "refused": out.refused, "not_idle": out.not_idle}));
        }
        let mut make = Vec::new();
        for (i, nid, level, class) in keep {
            if self.queues.len() <= level {
                self.queues.resize_with(level + 1, Default::default);
            }
            self.queues[level][class].push_back(nid);
            if class < LIVE_QUEUES && self.live.len() + self.making < self.live_max {
                self.making += 1;
                make.push((nid, results[i].events.clone(), results[i].st.frame));
            }
        }
        make
    }

    fn progress(&self, last: &mut u64, dt: f64) {
        let secs = self.t0.map(|t| t.elapsed().as_secs_f64()).unwrap_or(0.0);
        let rungs: Vec<&str> = self.rungs.iter().filter(|g| g.at.is_some()).map(|g| g.name.as_str()).collect();
        let queued: Vec<usize> = (0..QUEUES).map(|c| self.queues.iter().map(|q| q[c].len()).sum()).collect();
        let at_top: usize = self.queues.last().map_or(0, |q| q.iter().map(|x| x.len()).sum());
        eprintln!(
            "[{secs:7.1}s] kept {} {:?}, expanded {}, level {} ({} queued there), queued {:?}, trials {} ({:.0}/s), rooms {}, rungs {:?}, boots {}",
            self.nodes.len(), self.kept, self.expanded, self.top, at_top, queued,
            self.trials, (self.trials - *last) as f64 / dt.max(0.1), self.rooms.len(), rungs, self.boots
        );
        *last = self.trials;
    }
}

/// The evaluation's ladder (fmi-checks/rungs): each rung's `all` conditions, or `least` of its `of`.
fn rungs_of(path: &str) -> Vec<Rung> {
    let v: Value = serde_json::from_str(&std::fs::read_to_string(path).expect("--rungs")).expect("the rungs' JSON");
    let cond = |c: &Value| -> (Key, bool, Option<Val>) {
        (Key::parse(c[0].as_str().expect("a key")), c[1].as_str() == Some(">="), c[2].as_i64())
    };
    v["rungs"]
        .as_array()
        .expect("rungs")
        .iter()
        .map(|g| {
            let all: Vec<_> = g["all"].as_array().map(|a| a.iter().map(cond).collect()).unwrap_or_default();
            let of: Vec<_> = g["of"].as_array().map(|a| a.iter().map(cond).collect()).unwrap_or_default();
            let (conds, least) = if of.is_empty() { let n = all.len(); (all, n) } else { (of, g["least"].as_u64().unwrap_or(1) as usize) };
            Rung { name: g["rung"].as_str().unwrap_or("?").to_string(), conds, least, at: None }
        })
        .collect()
}

/// `direct show NODES.jsonl ID OUT [FRAMES]`: a measurement tool. The kept stop's movie played from boot, then each
/// single click of its offer (targets and floor points) in a fork, run FRAMES frames at most: what the stop was, or
/// where the run stood every 400 frames when it reached none (room, idle, cutscene depth, user input, ego moving).
fn show(nodes: &str, id: u64, out: &Path, frames: u64) {
    let text = std::fs::read_to_string(nodes).expect("NODES.jsonl");
    let line = text.lines().filter_map(|l| serde_json::from_str::<Value>(l).ok()).find(|v| v["id"].as_u64() == Some(id)).expect("the node");
    let events: Vec<Event> = line["events"].as_array().unwrap().iter().filter_map(fmi_explore::events::event_of).collect();
    let frame = line["frame"].as_u64().unwrap();
    std::fs::create_dir_all(out).expect("OUT");
    let host = Host::new(HostCfg::from_env()).expect("the host");
    let mut e = host
        .boot(&BootCfg { seed: 1, events, frame: Some(frame), dump_locals: false, lean: true, out_dir: eng_dir(out) })
        .expect("the boot");
    let o = e.obs(&ObsReq::default()).expect("obs");
    let base = facts_of(&o);
    let ego = base.val(Key::pack(K_EGO, 0, 0, 0)) as i32;
    let of = afford::offer_cfg(&mut e, &o, false, true).expect("offer");
    println!("stop: frame {frame} room {} offer {}", e.status().room, of.json());
    let mut points: Vec<Point> = of.targets.clone();
    let (top, h) = e.status().screen.map(|s| (s.0, s.1)).unwrap_or((0, afford::SCREEN_H));
    let grid: Vec<(i32, i32)> = FLOOR_Y.iter().flat_map(|&k| FLOOR_X.iter().map(move |&x| (x, (top + (h - top) * k / 8).min(h - 2)))).collect();
    let ats = e.at_many(&grid).expect("at");
    for (i, (&(x, y), a)) in grid.iter().zip(ats.iter()).enumerate() {
        if a.main.is_some_and(|m| m.2 == 0 && m.3 == 0) {
            points.push(Point { x, y, hit: Hit::Walk(i as i32) });
        }
    }
    for p in &points {
        let mut b = e.fork().expect("fork");
        b.inputs(&[Event { frame: frame + 1, what: Input::Click(p.x, p.y) }]).expect("inputs");
        let _ = go(&mut b, RunSpec { frame: Some(frame + LEAD + CALM), ..Default::default() });
        let mut trace: Vec<String> = Vec::new();
        let mut end = frame + 400;
        let res = loop {
            match settle(&mut b, end.min(frame + frames), frame + LEAD + BUSY, ego) {
                Ok(Played::Stop(st)) => break format!("stop at +{} room {}", st.frame - frame, st.room),
                Ok(Played::Ended(st)) => break format!("ended: {} at +{}", st.reason, st.frame - frame),
                Ok(Played::NotIdle) => {
                    let st = b.status().clone();
                    trace.push(format!("+{} room {} idle {:?} cs {} up {} mv {:?} blk {:?}", st.frame - frame, st.room, st.idle, st.cs, st.up, st.mv, st.blk));
                    if end >= frame + frames {
                        break "no stop".to_string();
                    }
                    end += 400;
                }
                Err(x) => break format!("error: {x}"),
            }
        };
        let diff: Vec<String> = b.facts().map(|f| base.diff(&f).into_iter().filter(|d| world_kind(d.0)).map(|d| format!("{}:{}->{}", d.0, d.1, d.2)).take(8).collect()).unwrap_or_default();
        println!("{} ({},{}) -> {res} world {:?}", p.hit.name(), p.x, p.y, diff);
        for t in trace.iter().take(20) {
            println!("    {t}");
        }
        b.quit();
    }
    e.quit();
}

/// `direct verify NODES.jsonl ID OUT [KEY=V,..]`: the kept stop's movie played from boot in a fresh engine, to its
/// frame; prints the room and each key's value there, and whether each equals V.
fn verify(nodes: &str, id: u64, out: &Path, keys: &str) {
    let text = std::fs::read_to_string(nodes).expect("NODES.jsonl");
    let line = text
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .find(|v| v["id"].as_u64() == Some(id))
        .expect("the node");
    let events: Vec<Event> = line["events"].as_array().unwrap().iter().filter_map(fmi_explore::events::event_of).collect();
    assert_eq!(events.len(), line["events"].as_array().unwrap().len(), "every event read");
    let frame = line["frame"].as_u64().unwrap();
    std::fs::create_dir_all(out).expect("OUT");
    let host = Host::new(HostCfg::from_env()).expect("the host");
    let t0 = Instant::now();
    let mut e = host
        .boot(&BootCfg { seed: 1, events: events.clone(), frame: Some(frame), dump_locals: false, lean: true, out_dir: eng_dir(out) })
        .expect("the boot");
    let f = e.facts().expect("facts");
    let room = f.val(Key::pack(K_ROOM, 0, 0, 0));
    let mut ok = room == line["room"].as_i64().unwrap();
    let mut held = Vec::new();
    for w in keys.split(',').filter(|x| !x.is_empty()) {
        let (k, v) = w.split_once('=').expect("KEY=V");
        let got = f.val(Key::parse(k));
        ok &= got == v.parse::<i64>().expect("a value");
        held.push(json!([k, got]));
    }
    let clicks = events.iter().filter(|x| matches!(x.what, Input::Click(..))).count();
    println!("{}", json!({"node": id, "from_boot": true, "frame": frame, "room": room, "keys": held, "clicks": clicks,
        "seconds": t0.elapsed().as_secs_f64(), "ok": ok}));
    e.quit();
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let opt = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
    if a.first().map(String::as_str) == Some("forkbench") {
        // `direct forkbench OUT N`: the cost of a fork, a one-frame run, a dump and a quit, one engine, one thread
        let out = PathBuf::from(&a[1]);
        std::fs::create_dir_all(&out).expect("OUT");
        let n: usize = a[2].parse().expect("N");
        let host = Host::new(HostCfg::from_env()).expect("the host");
        let mut e = host
            .boot(&BootCfg { seed: 1, events: Vec::new(), frame: None, dump_locals: false, lean: true, out_dir: eng_dir(&out) })
            .expect("the boot");
        let f0 = e.status().frame;
        let mut t = [0f64; 6];
        for _ in 0..n {
            let t0 = Instant::now();
            let mut b = e.fork().expect("fork");
            t[0] += t0.elapsed().as_secs_f64();
            let t0 = Instant::now();
            b.run(&RunSpec { frame: Some(f0 + 1), ..Default::default() }).expect("run 1");
            t[1] += t0.elapsed().as_secs_f64();
            let t0 = Instant::now();
            b.run(&RunSpec { frame: Some(f0 + 101), ..Default::default() }).expect("run 100");
            t[2] += t0.elapsed().as_secs_f64();
            let t0 = Instant::now();
            let _ = b.facts().expect("facts");
            t[3] += t0.elapsed().as_secs_f64();
            let t0 = Instant::now();
            let _ = b.obs(&NAMES).expect("obs");
            t[4] += t0.elapsed().as_secs_f64();
            let t0 = Instant::now();
            b.quit();
            t[5] += t0.elapsed().as_secs_f64();
        }
        let ms = |x: f64| (x * 1e6 / n as f64).round() / 1e3;
        let ns = |i: usize| fmi_engine::host::FORK_NS[i].load(Ordering::Relaxed) as f64 / 1e6 / n as f64;
        eprintln!("fork: {:.3} ms to the parent's reply, {:.3} ms to the child's connection", ns(0), ns(1));
        println!("{}", json!({"n": n, "ms": {"fork": ms(t[0]), "run_1_frame": ms(t[1]), "run_100_frames": ms(t[2]), "facts": ms(t[3]), "obs_names": ms(t[4]), "quit": ms(t[5])}}));
        e.quit();
        return;
    }
    if a.first().map(String::as_str) == Some("show") {
        return show(&a[1], a[2].parse().expect("ID"), Path::new(&a[3]), a.get(4).and_then(|x| x.parse().ok()).unwrap_or(6400));
    }
    if a.first().map(String::as_str) == Some("verify") {
        return verify(&a[1], a[2].parse().expect("ID"), Path::new(&a[3]), a.get(4).map(String::as_str).unwrap_or(""));
    }
    let out = PathBuf::from(a.first().expect("direct OUT [--minutes M] [--workers W] [--horizon H] [--no-skips] [--guide F] [--rungs F] [--watch K=V,..]"));
    std::fs::create_dir_all(&out).expect("OUT");
    let minutes: f64 = opt("--minutes").map(|v| v.parse().unwrap()).unwrap_or(10.0);
    let cfg = Cfg {
        out: out.clone(),
        horizon: opt("--horizon").map(|v| v.parse().unwrap()).unwrap_or(1600),
        workers: opt("--workers").map(|v| v.parse().unwrap()).unwrap_or(6),
    };
    let host = Host::new(HostCfg::from_env()).expect("the host");
    let root_events = if a.iter().any(|x| x == "--no-skips") {
        Vec::new()
    } else {
        vec![Event { frame: 1, what: Input::Skips { text: true, cutscenes: true, spare: None } }]
    };
    let t0 = Instant::now();
    let mut e = host
        .boot(&BootCfg { seed: 1, events: root_events.clone(), frame: None, dump_locals: false, lean: true, out_dir: eng_dir(&out) })
        .expect("the boot");
    let st = e.status().clone();
    eprintln!("boot: frame {} room {} idle {:?} in {:.1} s", st.frame, st.room, st.idle, t0.elapsed().as_secs_f64());
    let mut s = Search {
        log: Some(std::fs::File::create(out.join("nodes.jsonl")).expect("nodes.jsonl")),
        offers: Some(std::fs::File::create(out.join("offers.jsonl")).expect("offers.jsonl")),
        t0: Some(t0),
        live_max: LIVE_MAX.max(4 * cfg.workers),
        ..Default::default()
    };
    if let Some(p) = opt("--guide") {
        // the guide's keys (fmi guide map): the variables among them are the ones the game's code tests
        let g: Value = serde_json::from_str(&std::fs::read_to_string(&p).expect("--guide")).expect("the guide's JSON");
        s.policy.guide = g["keys"].as_object().expect("the guide's keys").keys().map(|k| Key::parse(k)).collect();
        eprintln!("guide: {} keys, {} of them variables", s.policy.guide.len(), s.policy.guide.iter().filter(|k| k.kind() == K_VAR).count());
    }
    if let Some(p) = opt("--rungs") {
        s.rungs = rungs_of(&p);
    }
    for w in opt("--watch").unwrap_or_default().split(',').filter(|x| !x.is_empty()) {
        let (k, v) = w.split_once('=').expect("--watch KEY=V");
        s.watch.push((Key::parse(k), v.parse().expect("a value"), None));
    }
    let o = e.obs(&NAMES).expect("the root's dump");
    let r = read(String::new(), Vec::new(), &o, st.clone());
    let ego = r.facts.val(Key::pack(K_EGO, 0, 0, 0));
    s.root_owned = r.facts.iter().filter(|p| p.0.kind() == K_OWNER && p.1 == ego).map(|p| p.0.a() as u32).collect();
    for p in r.facts.iter().filter(|p| world_kind(p.0)) {
        s.seen.insert(*p, 0);
        s.seen_at.insert((r.room, p.0, p.1), 0);
    }
    for p in r.facts.iter().filter(|p| p.0.kind() == K_VAR) {
        s.seen.insert(*p, 0);
    }
    for &t in &r.vis {
        s.sees.insert((r.room, t), 0);
    }
    s.ifaces.insert((r.room, r.iface), 0);
    s.boxes.insert((r.room, r.walkbox), 0);
    s.cams.insert((r.room, r.cam), 0);
    s.rooms.push(r.room);
    for g in s.rungs.iter_mut().filter(|g| g.conds.is_empty()) {
        g.at = Some((0, 0.0));
    }
    s.nodes.push(Node { events: root_events, frame: st.frame, depth: 0, ever: Vec::new() });
    s.queues.push(Default::default());
    s.queues[0][0].push_back(0);
    s.live.insert(0, e);
    let limit = Duration::from_secs_f64(minutes * 60.0);
    let (mut last, mut printed) = (0u64, Instant::now());
    type Job = (usize, Option<Box<dyn Engine>>, Vec<Event>, u64, u32);
    type Done = (usize, Result<(Box<dyn Engine>, Out), String>, bool);
    type Parent = (Box<dyn Engine>, Vec<(usize, Vec<Event>, u64)>);
    while t0.elapsed() < limit {
        // a batch: the next stops in the queues' order, each with its kept engine if it has one
        let mut batch: Vec<Mutex<Option<Job>>> = Vec::new();
        while batch.len() < cfg.workers * BATCH {
            // the highest level first and in it the first queue, over the first queues of every level; then the same
            // over the later ones
            let first = s.queues.iter_mut().rev().find_map(|q| q[..FIRST_QUEUES].iter_mut().find_map(|c| c.pop_front()));
            let Some(id) = first.or_else(|| s.queues.iter_mut().rev().find_map(|q| q[FIRST_QUEUES..].iter_mut().find_map(|c| c.pop_front()))) else { break };
            let n = &s.nodes[id];
            batch.push(Mutex::new(Some((id, s.live.remove(&id), n.events.clone(), n.frame, n.ever.len() as u32))));
        }
        if batch.is_empty() {
            break;
        }
        let snap = Snap { motion: s.motion.clone() };
        // side by side: each stop explored, from the search's state at the batch's start
        let n = batch.len();
        let done: Vec<Mutex<Option<Done>>> = (0..n).map(|_| Mutex::new(None)).collect();
        let next = AtomicUsize::new(0);
        std::thread::scope(|sc| {
            for _ in 0..cfg.workers.min(n) {
                sc.spawn(|| loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    if i >= n {
                        break;
                    }
                    let (id, engine, events, frame, level) = batch[i].lock().unwrap().take().unwrap();
                    let booted = engine.is_none();
                    let e = match engine {
                        Some(e) => Ok(e),
                        None => host.boot(&BootCfg { seed: 1, events, frame: Some(frame), dump_locals: false, lean: true, out_dir: eng_dir(&cfg.out) }),
                    };
                    let r = match e {
                        Ok(mut e) => match explore(&mut e, level, &snap, &cfg) {
                            Ok(out) => Ok((e, out)),
                            Err(x) => {
                                e.quit();
                                Err(x.to_string())
                            }
                        },
                        Err(x) => Err(format!("boot: {x}")),
                    };
                    *done[i].lock().unwrap() = Some((id, r, booted));
                });
            }
        });
        // in the batch's order: committed
        let mut parents: Vec<Mutex<Option<Parent>>> = Vec::new();
        for d in done {
            let (id, r, booted) = d.into_inner().unwrap().expect("every stop of the batch explored");
            s.boots += booted as u64;
            s.expanded += 1;
            match r {
                Ok((e, out)) => {
                    let make = s.commit(id, out);
                    parents.push(Mutex::new(Some((e, make))));
                }
                Err(x) => eprintln!("node {id}: {x}"),
            }
        }
        // side by side: the kept stops' engines, each a fork of its parent's
        let made: Mutex<Vec<(usize, Box<dyn Engine>)>> = Mutex::new(Vec::new());
        let next = AtomicUsize::new(0);
        std::thread::scope(|sc| {
            for _ in 0..cfg.workers.min(parents.len()) {
                sc.spawn(|| loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    if i >= parents.len() {
                        break;
                    }
                    let (mut e, make) = parents[i].lock().unwrap().take().unwrap();
                    for (nid, events, frame) in make {
                        if let Ok(b) = remake(&mut e, &events, frame) {
                            made.lock().unwrap().push((nid, b));
                        }
                    }
                    e.quit();
                });
            }
        });
        s.making = 0;
        for (nid, b) in made.into_inner().unwrap() {
            s.live.insert(nid, b);
        }
        if printed.elapsed() >= Duration::from_secs(10) {
            s.progress(&mut last, printed.elapsed().as_secs_f64());
            printed = Instant::now();
        }
    }
    s.progress(&mut last, printed.elapsed().as_secs_f64());
    let live: Vec<Box<dyn Engine>> = s.live.drain().map(|x| x.1).collect();
    for e in live {
        e.quit();
    }
    let depth = s.nodes.iter().map(|n| n.depth).max().unwrap_or(0);
    let sum = json!({"seconds": t0.elapsed().as_secs_f64(), "workers": cfg.workers, "kept": s.nodes.len(), "by_queue": s.kept,
        "expanded": s.expanded, "trials": s.trials, "skipped": s.skipped, "refused": s.refused, "not_idle": s.not_idle, "rooms": s.rooms,
        "max_depth": depth, "boots": s.boots, "top_level": s.top,
        "trial_seconds": {"fork": s.phase[0], "inputs": s.phase[1], "run": s.phase[2], "read": s.phase[3], "quit": s.phase[4]}, "trial_frames": s.frames,
        "queued": s.queues.iter().map(|q| q.iter().map(|c| c.len()).collect::<Vec<_>>()).collect::<Vec<_>>(),
        "iface_vars": s.policy.iface_vars.len(), "clocks": s.policy.clocks.len(), "capped": s.policy.capped.len(),
        "ends": s.ends.iter().map(|(n, x)| json!([n, x])).collect::<Vec<Value>>(),
        "rungs": s.rungs.iter().map(|g| json!({"rung": g.name, "node": g.at.map(|x| x.0), "seconds": g.at.map(|x| x.1)})).collect::<Vec<Value>>(),
        "watch": s.watch.iter().map(|(k, v, at)| json!({"key": format!("{k}"), "value": v, "node": at.map(|x| x.0), "seconds": at.map(|x| x.1)})).collect::<Vec<Value>>()});
    std::fs::write(out.join("summary.json"), format!("{sum}\n")).expect("summary.json");
    println!("{sum}");
}

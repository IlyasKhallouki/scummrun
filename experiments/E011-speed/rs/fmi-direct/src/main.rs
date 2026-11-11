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
//!   world that fewer than MOTION_TRIES stops of that level, room and walk box saw change, the inputs are tried again
//!   from there (a door a timer opens is open only then, and ego must stand near it; a room's own motion is tried a
//!   few times from each part of the room, not at every stop).
//! * **A click that changes only what the interface shows** (the bridge idle again LEAD frames on and still so CALM
//!   frames later, ego where it stood, the world the same, and every running script where it is in a fork given no
//!   input: the same place, delay and locals) is no stop: it is clicked on from. A dialog's line is none: it clears
//!   the choices at once, and its script goes on. Where it changed one
//!   slot it is the start or the middle of a sentence, a stop to click on from like a half-made one. Where it
//!   changed several (another page of the interface) and came from the round's stop, it is clicked on from at the
//!   same depth: at the slots it changed, at the slots that start a sentence (and from that sentence at the slots
//!   it changed), and at the slots that change the page again. Inside a sentence the page is not changed.
//! * **After an input.** A trial's stop is set against a twin: a fork of the expanded stop run to the same frame
//!   with no input. Where they differ in a key of the world, the input changed it, and the trial is run PROBE frames
//!   on with no input. The stop reached there is one more stop to keep (an effect that lands after the first stop: a
//!   count the game raises when its actor has landed). And where a key the input changed is back at its old value
//!   there, or PROBE_LONG frames on, the first stop is a window that closes by itself: the inputs are tried from it,
//!   whether it holds anything new or not (the first MAX_WINDOWS windows of a stop). A key the stop's own waits
//!   moved is not counted as changed by an input: it moves by itself from that stop. Nor is a key of the game's own
//!   motion: one the waits moved at some stop where no input's change of it was pending (a stop carries the keys the
//!   inputs on its path changed, each until it is back at its old value: PENDING of them at most; the stops inside
//!   a window and after a wait teach nothing). Such motion runs another way after any input, and is no effect of
//!   it. And a window is followed only
//!   while fewer than WINDOW_TRIES windows on one of its keys were followed from the same cell (its room, its
//!   interface and its world without the keys that move by themselves): where a room's own motion differs after
//!   every input, the same window is not entered at every stop.
//! * **The world** of a stop is its room, owners, states, classes and bits, and the guide's variables but the
//!   interface's own (those the input script writes, and those a fifth of a stop's own inputs change where they change
//!   nothing else in the world), the clocks (those
//!   that take two values or more over one stop's waits), and those seen with more than VAR_CAP values.
//! * **Lines and levels.** A stop's line is what ego has gained and lost on the way to it: the objects it has owned
//!   (and did not own at the boot) and those of them it owns no more. Its level is their count. Something is new to
//!   a stop when no kept stop of its line holds it: with an object gained or used up, every room, input and position
//!   counts again. The highest level is expanded first. With `--lines gains` a line is the gains alone and the
//!   level their count: a loss starts no line (what it changed is new in its line, the rooms are not).
//! * **Kept.** Within its level a stop goes to the first queue it fits: (0) a world atom other than a variable's
//!   that is new; (1) an interface new in its line, or a thing on the screen new in that room; (2) a variable's value that is new, or
//!   a walk box or camera bucket new in that room, or a new value of a local variable of a script running at the
//!   stop (a script that waits for the player keeps there what the facts do not show: how often a line was said;
//!   those a wait moves, and those seen with more than LOCAL_CAP values, do not count); (3) an atom of the guide's keys new in that room; (4) any other
//!   world atom new in that room, or a new world (its atoms together, without those that change by themselves). A
//!   stop that fits none is dropped. Queue 0 is expanded first, then 1, and so on; queue 4 waits until no
//!   level holds a stop in queues 0 to 3 (queue 3 carries a flag from room to room, which nothing else does), and
//!   gives its stop with the most flags set first.
//! * **Flags.** A flag is a bit, or a class of an object. A stop whose set of flags (those not seen unset again
//!   somewhere) is held whole by no kept stop of
//!   its line is new, whatever else it holds, and goes before every queue of its level, the last such stop first.
//!   Flags are won one at a time and on different paths; the stop that has won the most is the one to go on from,
//!   and once one stop holds them all the others' sets are within its own. With `--flag-lines` such a stop also
//!   starts a line of its own, as a gain does (every room and input counts again from it), and every stop of
//!   queue 0 goes on the stack. The newest line's stops go before any older line's: its stack's newest stop
//!   first, then its queues 1 to 3 in their own order. The search works on its newest line, walks through it,
//!   then goes through what it saw there, and only then turns to the line before.
//! * **Where to go (`--go`).** A stop that begins a line has won something: an object, a flag. The scripts name
//!   it somewhere (an owner asked for, the other object of a sentence, a flag tested): from that stop one walk is
//!   made to each of the nearest GO_ROOMS rooms whose scripts do, over the clicks that were seen to lead from room
//!   to room, one trial a walk. Those stops are expanded first. The scripts advise only: a walk that fails, or a
//!   room that holds nothing, costs a trial or a stop, and the walk through every room stays.
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
use std::sync::{Arc, Mutex};
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
/// Frames a trial that changed the world is run on with no input (the module's text, "After an input"), and the
/// windows one round follows.
const PROBE: u64 = 60;
const PROBE_LONG: u64 = 240;
const MAX_WINDOWS: usize = 3;
const WINDOW_TRIES: u32 = 3;
/// The keys a stop carries as changed by the inputs on its path and not yet back (the module's text, "After an input").
const PENDING: usize = 16;
/// Stops of a level, room and walk box whose waits a key may change before its change stops calling for the inputs
/// again.
const MOTION_TRIES: u32 = 3;
/// Pixels of the camera's bucket: a fifth of the screen, so a walk across part of it shows.
const CAM_BUCKET: i32 = 64;
/// The floor clicks' grid over the main screen: the columns, and the rows in eighths of its height.
const FLOOR_X: [i32; 7] = [6, 58, 110, 160, 210, 262, 313];
const FLOOR_Y: [i32; 4] = [3, 5, 7, 8];
/// Engines kept alive for stops waiting in a queue; the others are booted again from their movies.
const LIVE_MAX: usize = 32;
/// The queues (the module's text, "Kept"), and the last one whose stops get a kept engine.
const QUEUES: usize = 5;
const LIVE_QUEUES: usize = 3;
/// The queues expanded at every level, the highest first, before any level's later ones.
const FIRST_QUEUES: usize = 4;
/// `--go`: the rooms walked to from a stop that has won something, and the rooms a walk may cross.
const GO_ROOMS: usize = 6;
const GO_HOPS: usize = 14;
/// Floor clicks a walk may make in one room to bring a thing onto the screen.
const GO_SCROLLS: usize = 8;
/// The kept stops of one expanded stop that get a kept engine.
const LIVE_EACH: usize = 8;
/// Stops of one batch for each worker.
const BATCH: usize = 3;
/// A variable seen with more values than this in kept stops counts no more.
const VAR_CAP: usize = 8;
/// A local variable of a running script seen with more values than this counts no more.
const LOCAL_CAP: usize = 5;

struct Node {
    events: Vec<Event>,
    frame: u64,
    depth: u32,
    /// the objects ego has owned on the way here and did not own at the boot, ascending
    ever: Vec<u32>,
    /// the stop's level and line (the module's text)
    level: u32,
    line: u64,
    room: i32,
    /// the keys the inputs on the way here changed and that are not back yet, each with its old value; and whether
    /// the stop was made inside a window or after a wait
    pending: Vec<(Key, Val)>,
    inside: bool,
    /// the flags it holds set, but those seen unset again somewhere
    flags: u32,
    /// the last stop on the way here (or this one) whose set of flags was new in its line of gains
    fline: usize,
    /// its line of gains
    gline: u64,
    /// what it won over the stop it came from, where it began a line: the objects gained and the flags set
    news: Vec<Key>,
}

/// The line of a stop: its gains (`ever`) and the objects it owns now (so its losses), hashed.
fn line_of(ever: &[u32], owned: &[u32]) -> u64 {
    let mut h = DefaultHasher::new();
    (ever, owned).hash(&mut h);
    h.finish()
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
    /// per level, the queues; and the stops whose flags no kept stop of their line held all of, the last first
    queues: Vec<[VecDeque<usize>; QUEUES]>,
    stacks: Vec<Vec<usize>>,
    /// per line: the largest sets of flags kept stops hold (none within another), as bits by the flag's number
    tops: HashMap<u64, Vec<Vec<u64>>>,
    live: HashMap<usize, Box<dyn Engine>>,
    /// what the kept stops of each line hold (the first field is the line)
    seen: HashSet<(u64, Key, Val)>,
    seen_at: HashSet<(u64, i32, Key, Val)>,
    sees: HashSet<(u64, i32, i32)>,
    ifaces: HashSet<(u64, u64)>,
    boxes: HashSet<(u64, i32, i32)>,
    cams: HashSet<(u64, i32, i32)>,
    cells: HashSet<(u64, u64)>,
    /// per line: the counts of flags set that a kept stop has held; and the flags seen unset again (they are no
    /// record of anything: a dialog's choices, a door's state)
    counts: HashSet<(u64, u32)>,
    toggles: HashSet<Key>,
    /// lines by gains alone (--lines gains); a new set of flags starts a line (--flag-lines)
    gains_only: bool,
    flag_lines: bool,
    /// --go: where the scripts name what a stop has won, and the clicks seen to lead from a room to the next
    go: bool,
    mentions: Mentions,
    edges: HashMap<(i32, i32), Vec<Hit>>,
    edges_log: Option<std::fs::File>,
    /// the ways a walk found false (from, to, the clicks), and how often
    dead: HashMap<(i32, i32, String), u32>,
    /// the scripts' local variables: per line the values kept stops held, the values seen, the clocks, the capped
    lseen: HashSet<(u64, u64, i32)>,
    lvals: HashMap<u64, Vec<i32>>,
    lclocks: HashSet<u64>,
    lcapped: HashSet<u64>,
    /// the objects ego owns at the boot
    root_owned: HashSet<u32>,
    var_vals: HashMap<Key, Vec<Val>>,
    policy: Policy,
    /// the world keys a run with no input changed somewhere: left out of the cells
    ambient: HashSet<Key>,
    /// per level, room, walk box and key: the stops expanded there whose waits changed it
    motion: HashMap<(u32, i32, i32, Key), u32>,
    /// per cell and key: the windows on that key followed from that cell
    wins: HashMap<(u64, Key), u32>,
    /// the game's own motion: the keys a wait moved at a stop where no input's change of them was pending
    noise: HashSet<Key>,
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
    /// the forks a round's first clicks are dealt to, side by side; and the stops of one batch for each stop
    /// expanded at once (workers / slices of them)
    slices: usize,
    batch: usize,
    /// the stops expanded at once (0: workers / slices)
    at_once: usize,
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

/// The local variables of the scripts running at a stop: a script that waits for the player (a dialog's) keeps its
/// count of what was said in them, and nothing else of the game's state shows it.
fn locals_of(o: &Obs) -> Vec<(u64, i32)> {
    let mut v = Vec::new();
    for sl in &o.slots {
        if let Some(ls) = &sl.locals {
            for (i, &x) in ls.iter().enumerate() {
                let mut h = DefaultHasher::new();
                (sl.where_, sl.number, i).hash(&mut h);
                v.push((h.finish(), x));
            }
        }
    }
    v
}

/// What the scripts running at a stop are at: each one's place, delay and local variables, hashed together.
fn scripts_of(o: &Obs) -> u64 {
    let mut v: Vec<(i32, i32, u64, i32, i32, &[i32])> =
        o.slots.iter().map(|sl| (sl.where_, sl.number, sl.offs, sl.frozen, sl.delay, sl.locals.as_deref().unwrap_or(&[]))).collect();
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
    /// the world's and the variables' atoms that differ from the expanded stop's (set by `explore`)
    changed: Vec<(Key, Val)>,
    /// the keys its input changed (against the twin), each with its old value; and whether it was made inside a
    /// window or after a wait
    own: Vec<(Key, Val)>,
    inside: bool,
    /// the local variables of the scripts that run at the stop: (script and index hashed, value); after `explore`
    /// only those that differ from the expanded stop's
    locals: Vec<(u64, i32)>,
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
        changed: Vec::new(),
        own: Vec::new(),
        inside: false,
        locals: locals_of(o),
        facts,
        st,
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
    // the frames at which the engine's own tests said ego stands, and a click would count (the status's fields may
    // say otherwise: the moving flags hold a pose's bits, the gate has more to it than the status shows)
    let (mut stood, mut gate): (Option<u64>, Option<u64>) = (None, None);
    let mut same = 0;
    loop {
        let st = b.status().clone();
        if st.frame >= end || same > 8 {
            return Ok(Played::NotIdle);
        }
        let ready = st.idle.is_some() || (st.frame >= busy && (takes_click(&st) || gate == Some(st.frame)));
        if ready && (st.mv.unwrap_or(0) == 0 || stood == Some(st.frame)) {
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
        match go(b, spec)? {
            Err(s) => return Ok(Played::Ended(s)),
            Ok(s) => {
                if ready && s.reason == "until" {
                    stood = Some(s.frame);
                } else if s.reason == "control" {
                    gate = Some(s.frame);
                }
                // a run that stopped where it started must have told something new, or the loop ends
                same = if s.frame == st.frame { same + 1 } else { 0 };
            }
        }
    }
}

/// `settle`, then CALM frames on and `settle` again: the texts an input leaves on the interface for a moment are gone.
/// A stop not reached within `horizon` frames of `from` gets four times as many when a cutscene runs then.
fn calm(b: &mut Box<dyn Engine>, from: u64, horizon: u64, ego: i32) -> EResult<Played> {
    for end in [from + horizon, from + 4 * horizon] {
        // the longer horizon is a cutscene's: outside one, a run that reached no stop in the first is left there
        if end > from + horizon && b.status().cs == 0 {
            break;
        }
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
    /// after a click that changed only what the interface shows (the module's text, "Inputs"): the slots shown
    /// before it. From such a stop only the slots that changed are clicked, and the slots that start a sentence;
    /// from the sentence only those that changed
    only: Option<Arc<HashSet<(i32, i32, String)>>>,
}

/// A stop that is a window (the module's text, "After an input"): its engine, its round's name, its events, the keys
/// its input changed (with their old values), its cell and the keys that came back.
struct Cand {
    engine: Box<dyn Engine>,
    name: String,
    events: Vec<Event>,
    keys: Vec<(Key, Val)>,
    cell: u64,
    back: Vec<Key>,
}

/// What a stop's trials gave.
#[derive(Default)]
struct Out {
    /// the windows found in a round, for the stop's expansion to follow or not
    cands: Vec<Cand>,
    results: Vec<Res>,
    ends: Vec<String>,
    trials: u64,
    refused: u64,
    not_idle: u64,
    mids: u64,
    /// stops that were windows
    windows: u64,
    /// clicks not tried: slots known to pick another verb or to do nothing
    skipped: u64,
    /// the windows followed: each one's cell with each of its keys
    followed: Vec<(u64, Key)>,
    /// the ways (from, to) whose clicks a walk made and that did not lead on
    bad: Vec<(i32, i32)>,
    /// the keys the stop's waits moved while no input's change of them was pending
    noise: HashSet<Key>,
    /// the scripts' local variables the stop's waits moved: clocks
    lclocks: HashSet<u64>,
    offer: Value,
    /// the variables the input script wrote in a trial, or that a fifth of the stop's inputs changed
    iface_vars: HashSet<Key>,
    /// the variables, and the world's other keys, a run with no input changed
    clocks: HashSet<Key>,
    ambient: HashSet<Key>,
    room: i32,
    walkbox: i32,
    /// the trials' wall seconds by phase (the fork, the inputs, the run to the stop, the stop's reading, the quit), and
    /// the frames they ran
    phase: [f64; 5],
    frames: u64,
}

impl Out {
    /// Another part of the same stop's trials, after this one.
    fn absorb(&mut self, o: Out) {
        self.cands.extend(o.cands);
        self.results.extend(o.results);
        self.ends.extend(o.ends);
        self.trials += o.trials;
        self.refused += o.refused;
        self.not_idle += o.not_idle;
        self.mids += o.mids;
        self.windows += o.windows;
        self.skipped += o.skipped;
        self.followed.extend(o.followed);
        self.bad.extend(o.bad);
        if self.offer.is_null() {
            self.offer = o.offer;
        }
        self.iface_vars.extend(o.iface_vars);
        self.clocks.extend(o.clocks);
        self.ambient.extend(o.ambient);
        self.noise.extend(o.noise);
        self.lclocks.extend(o.lclocks);
        for (a, b) in self.phase.iter_mut().zip(o.phase.iter()) {
            *a += b;
        }
        self.frames += o.frames;
    }
}

/// A half-made stop's identity in a round: its interface and scripts' locals (`base`), and the values of the
/// variables a page of the interface was seen to change.
fn mid_key(base: u64, vars: &Facts, pvars: &[Key]) -> u64 {
    if pvars.is_empty() {
        return base;
    }
    let mut h = DefaultHasher::new();
    (base, pvars.iter().map(|k| vars.val(*k)).collect::<Vec<Val>>()).hash(&mut h);
    h.finish()
}

/// Whether the round has not seen this half-made stop yet; it is noted with its variables.
fn see(seen: &mut HashSet<u64>, list: &mut Vec<(u64, Facts)>, base: u64, facts: &Facts, pvars: &[Key]) -> bool {
    let fresh = seen.insert(mid_key(base, facts, pvars));
    if fresh {
        list.push((base, Facts(facts.iter().filter(|p| p.0.kind() == K_VAR).copied().collect())));
    }
    fresh
}

/// A line on stderr when DIRECT_TRACE is set (a measurement aid).
fn trace(text: &str) {
    if std::env::var_os("DIRECT_TRACE").is_some() {
        eprintln!("TRACE {text}");
    }
}

/// The world of the round's stop run to frame `f` with no input (None: its run ended before): from the cache, or
/// from the twin run on, or from a new fork of `src` when the twin stands past `f`.
fn twin_world(
    src: &mut Box<dyn Engine>,
    twin: &mut Option<Box<dyn Engine>>,
    cache: &mut HashMap<u64, Option<Facts>>,
    f: u64,
) -> EResult<Option<Facts>> {
    if let Some(w) = cache.get(&f) {
        return Ok(w.clone());
    }
    if twin.as_ref().map_or(true, |t| t.status().frame > f) {
        if let Some(t) = twin.take() {
            t.quit();
        }
        *twin = Some(src.fork()?);
    }
    let t = twin.as_mut().unwrap();
    let alive = t.status().frame == f || go(t, RunSpec { frame: Some(f), ..Default::default() })?.is_ok();
    let w = if alive {
        Some(Facts(t.facts()?.iter().filter(|p| world_kind(p.0)).copied().collect()))
    } else {
        if let Some(t) = twin.take() {
            t.quit();
        }
        None
    };
    cache.insert(f, w.clone());
    Ok(w)
}

/// A point where the pointer is over no verb, object or actor, by the engine's hit test at this stop.
fn park_point(e: &mut Box<dyn Engine>) -> EResult<Option<(i32, i32)>> {
    let (w, h) = (afford::SCREEN_W, afford::SCREEN_H);
    let cands = [(w - 1, h - 1), (0, h - 1), (w / 2, h - 1), (w - 1, 0), (0, 0), (w / 2, 0), (w - 1, h / 2), (0, h / 2)];
    let ats = e.at_many(&cands)?;
    Ok(cands.iter().zip(ats.iter()).find(|(_, a)| a.verb_slot == 0 && a.main.map_or(true, |m| m.2 == 0 && m.3 == 0)).map(|(p, _)| *p))
}

/// The click sequences from the engine's stop (the module's text, "Inputs"), each a trial; `pre` names the round.
fn clicks(
    e: &mut Box<dyn Engine>,
    pre: &str,
    lead: &[Event],
    motion: Option<&HashSet<Key>>,
    carry: &[(Key, Val)],
    lown: &HashSet<u64>,
    part: (usize, usize),
    snap: &Snap<'_>,
    cfg: &Cfg,
    out: &mut Out,
) -> EResult<()> {
    let o0 = e.obs(&NAMES)?;
    let st0 = e.status().clone();
    let r0 = read(String::new(), Vec::new(), &o0, st0);
    let sentence = r0.facts.val(Key::var(afford::VAR_SENTENCE_SCRIPT as u64)) as i32;
    let input0 = r0.facts.val(Key::var(afford::VAR_VERB_SCRIPT as u64)) as i32;
    let ego = r0.facts.val(Key::pack(K_EGO, 0, 0, 0)) as i32;
    // a stop inside a sentence or a dialog, for the round: what the interface shows there, and the running scripts'
    // locals but the clocks (a dialog that shows the same choices again has counted the line said)
    let mkey = |r: &Res| -> u64 {
        let mut v: Vec<(u64, i32)> = r.locals.iter().filter(|l| !snap.lclocks.contains(&l.0) && !lown.contains(&l.0)).copied().collect();
        v.sort_unstable();
        let mut h = DefaultHasher::new();
        (r.iface, v).hash(&mut h);
        h.finish()
    };
    // and the values of the variables that a slot's click changed while it changed nothing else (a page of slots that
    // show no text: what they hold is not in what the interface shows). The list grows in the round; the stops seen
    // are kept with their variables, for their keys under the longer list
    let mut pvars: Vec<Key> = Vec::new();
    let mut seen_mid: HashSet<u64> = HashSet::new();
    let mut seen_list: Vec<(u64, Facts)> = Vec::new();
    see(&mut seen_mid, &mut seen_list, mkey(&r0), &r0.facts, &pvars);
    let mut frontier: Vec<Mid> = vec![Mid { engine: None, clicks: Vec::new(), events: lead.to_vec(), iface: r0.iface, only: None }];
    // the stops that are windows (the module's text, "After an input"), MAX_WINDOWS of them at most
    let mut found = 0;
    // the twin: a copy of the round's stop to fork it from, the fork run on, and its world at the frames asked
    let mut twin_src: Option<Box<dyn Engine>> = if motion.is_some() { Some(e.fork()?) } else { None };
    let mut twin: Option<Box<dyn Engine>> = None;
    let mut twin_at: HashMap<u64, Option<Facts>> = HashMap::new();
    // the verb slots as the first clicks left them (a half-made stop's interface), and those a later click need not
    // try: a slot that from the first half-made stop gave what it gave from the round's stop (it only picks another
    // verb), or gave that stop again (it does nothing)
    let mut first_gave: HashMap<Hit, u64> = HashMap::new();
    let mut skip: Option<HashSet<Hit>> = None;
    // the slots whose click from the round's stop changed only what the interface shows
    let mut shifters: HashSet<Hit> = HashSet::new();
    for depth in 1..=MAX_CLICKS {
        let mut next: Vec<Mid> = Vec::new();
        let mut work: VecDeque<Mid> = frontier.into_iter().collect();
        while let Some(mut m) = work.pop_front() {
            let own = m.engine.is_none();
            let me: &mut Box<dyn Engine> = match m.engine.as_mut() {
                Some(x) => x,
                None => &mut *e,
            };
            let o = me.obs(&NAMES)?;
            let shown: Arc<HashSet<(i32, i32, String)>> = Arc::new(o.verbs.iter().map(|x| (x.slot, x.id, x.name.clone().unwrap_or_default())).collect());
            let of = afford::offer_cfg(me, &o, false, true)?;
            let park = park_point(me)?;
            // the trials' forks record their slot runs and start with empty logs
            me.segment(true)?;
            me.runs()?;
            let _ = me.input_log();
            let mf = me.status().frame;
            // what the scripts are at LEAD and LEAD + CALM frames on with no input (made at the first click that may
            // have changed the interface alone)
            let mut quiet: Option<(u64, u64, Option<Facts>, Option<Facts>)> = None;
            let learn = depth == 2 && skip.is_none() && m.only.is_none();
            let mut learned: HashSet<Hit> = HashSet::new();
            let mego = o.ego_pos;
            let mut points: Vec<Point> = match &m.only {
                None => {
                    let mut v = of.verbs.clone();
                    v.extend(of.targets.iter().copied());
                    v
                }
                // after an interface-only click: the slots that changed (a slot that shows no text is taken as
                // changed: what it holds cannot be read); at the round's depth also the slots that start a sentence
                // and those that change the interface again
                Some(before) => of
                    .verbs
                    .iter()
                    .filter(|p| match p.hit {
                        Hit::Verb { slot, id } => {
                            shown.iter().any(|x| x.0 == slot && x.1 == id && (x.2.is_empty() || !before.contains(x)))
                                || (depth == 1 && (first_gave.contains_key(&p.hit) || shifters.contains(&p.hit)))
                        }
                        _ => false,
                    })
                    .copied()
                    .collect(),
            };
            if depth == 1 && m.only.is_none() {
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
            // this fork's share of the round's first clicks; the other forks' slots are clicked too, only for what
            // the click teaches (the interface it leaves): no stop and no sentence comes of them here
            let mut shadows: Vec<bool> = vec![false; points.len()];
            if own && part.1 > 1 {
                let all = std::mem::take(&mut points);
                shadows.clear();
                for (j, p) in all.into_iter().enumerate() {
                    let share = j % part.1 == part.0;
                    if share || matches!(p.hit, Hit::Verb { .. }) {
                        points.push(p);
                        shadows.push(!share);
                    }
                }
            }
            for (p, &shadow) in points.iter().zip(shadows.iter()) {
                if depth >= 2 && (skip.as_ref().is_some_and(|s| s.contains(&p.hit)) || shifters.contains(&p.hit)) {
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
                let early = match go(&mut b, RunSpec { frame: Some(mf + LEAD), ..Default::default() })? {
                    Ok(st) => st,
                    Err(st) => {
                        out.ends.push(format!("{} {}", name_of(pre, &cl), st.reason));
                        b.quit();
                        continue;
                    }
                };
                if depth == 1 {
                    // the variables the input script wrote for this click: the interface's own
                    if let Ok(l) = b.input_log() {
                        out.iface_vars.extend(l.records.iter().flat_map(|r| r.writes.iter()).filter(|t| t.kind == TouchKind::Var).map(|t| Key::var(t.a as u64)));
                    }
                }
                // a sentence half made shows at once (the game is idle again, ego stands where it stood, only the
                // input script ran): the stop to click on from is this one, LEAD frames after the click
                let mut settled_half: Option<bool> = None;
                let mut ran_on = false;
                if (early.idle.is_some() || takes_click(&early)) && early.mv.unwrap_or(0) == 0 {
                    let mut ob = b.obs(&NAMES)?;
                    let mut at = early.clone();
                    let half3 = ob.ego_pos == mego && {
                        let runs = b.runs()?;
                        let input = [input0, facts_of(&ob).val(Key::var(afford::VAR_VERB_SCRIPT as u64)) as i32];
                        afford::composing(early.idle.as_deref().or(Some("sentence")), &runs.runs, mf + 1, &input, sentence)
                    };
                    // or only what the interface shows changed, and CALM frames on it still is so (a sentence that is
                    // about to run shows too): clicked on from as well. The world is held against what it is with
                    // no input at the same frame: a room moves by itself (a juggler, a fire), and that is not the
                    // click's doing
                    let world_same = |x: &Obs, w: &Option<Facts>| w.as_ref().is_some_and(|w| !w.diff(&facts_of(x)).iter().any(|d| world_kind(d.0)));
                    // a page of slots that show no text: at the round's depth a slot's click that left no text of
                    // the interface another (no verb, no sentence) and two or more variables other than they are
                    // with no input
                    let slot = depth == 1 && matches!(p.hit, Hit::Verb { .. }) && {
                        let texts = |v: &[Verb]| -> Vec<(i32, i32, String)> {
                            let mut t: Vec<(i32, i32, String)> = v.iter().filter_map(|x| x.name.clone().filter(|n| !n.is_empty()).map(|n| (x.slot, x.id, n))).collect();
                            t.sort_unstable();
                            t
                        };
                        texts(&ob.verbs) == texts(&o.verbs)
                    };
                    let mut shows = !half3 && early.idle.is_some() && ob.ego_pos == mego && (slot || iface(&ob) != m.iface);
                    let mut vd: Vec<Key> = Vec::new();
                    let mut half = half3;
                    if half3 || shows {
                        // and no script is anywhere else than with no input at all: a dialog's line clears the
                        // choices at once and its script goes on a few frames later; such a click is read where
                        // the game has settled, as any other
                        if quiet.is_none() {
                            let mut tw = me.fork()?;
                            out.trials += 1;
                            let mut at = [0u64; 2];
                            let mut w: [Option<Facts>; 2] = [None, None];
                            for (k, f) in [mf + LEAD, mf + LEAD + CALM].into_iter().enumerate() {
                                if go(&mut tw, RunSpec { frame: Some(f), ..Default::default() })?.is_ok() {
                                    let to = tw.obs(&NAMES)?;
                                    at[k] = scripts_of(&to);
                                    w[k] = Some(facts_of(&to));
                                }
                            }
                            tw.quit();
                            let [w0, w1] = w;
                            quiet = Some((at[0], at[1], w0, w1));
                        }
                        let still = quiet.as_ref().is_some_and(|q| q.0 == scripts_of(&ob));
                        if shows && slot {
                            if let Some(w) = quiet.as_ref().and_then(|q| q.2.as_ref()) {
                                vd = w.diff(&facts_of(&ob)).into_iter().filter(|d| d.0.kind() == K_VAR).map(|d| d.0).collect();
                            }
                        }
                        if vd.len() < 2 {
                            vd.clear();
                        }
                        shows = shows && still && (iface(&ob) != m.iface || !vd.is_empty()) && quiet.as_ref().is_some_and(|q| world_same(&ob, &q.2));
                        half = half3 && still;
                    }
                    if !half3 || half {
                        settled_half = Some(half);
                    }
                    if shows {
                        let shown3 = iface(&ob);
                        match go(&mut b, RunSpec { frame: Some(mf + LEAD + CALM), ..Default::default() })? {
                            Err(st) => {
                                out.ends.push(format!("{} {}", name_of(pre, &cl), st.reason));
                                b.quit();
                                continue;
                            }
                            Ok(st2) => {
                                ran_on = true;
                                let ob2 = b.obs(&NAMES)?;
                                shows = st2.idle.is_some() && st2.mv.unwrap_or(0) == 0 && ob2.ego_pos == mego && iface(&ob2) == shown3 && quiet.as_ref().is_some_and(|q| q.1 == scripts_of(&ob2) && world_same(&ob2, &q.3));
                                if shows {
                                    ob = ob2;
                                    at = st2;
                                }
                            }
                        }
                    }
                    if half || shows {
                        let r = read(name_of(pre, &cl), events, &ob, at);
                        out.phase[2] += t.elapsed().as_secs_f64();
                        if shows {
                            // a page of the interface (several slots changed) from the round's stop: clicked on from
                            // at the same depth, at the slots that changed. Else the start or the middle of a
                            // sentence: a stop to click on from like any other
                            let now: HashSet<(i32, i32, String)> = ob.verbs.iter().map(|x| (x.slot, x.id, x.name.clone().unwrap_or_default())).collect();
                            let mut slots: Vec<i32> = now.symmetric_difference(&shown).map(|x| x.0).collect();
                            slots.sort_unstable();
                            slots.dedup();
                            let page = slots.len() >= 2 || !vd.is_empty();
                            trace(&format!("    shows {} depth {depth} ({} slots changed, {} variables)", r.name, slots.len(), vd.len()));
                            if !vd.is_empty() {
                                let n = pvars.len();
                                pvars.extend(vd.iter().copied());
                                pvars.sort_unstable();
                                pvars.dedup();
                                if pvars.len() != n {
                                    seen_mid = seen_list.iter().map(|(b, f)| mid_key(*b, f, &pvars)).collect();
                                }
                            }
                            if shadow {
                                if page && m.only.is_none() {
                                    shifters.insert(p.hit);
                                }
                                see(&mut seen_mid, &mut seen_list, mkey(&r), &r.facts, &pvars);
                                b.quit();
                                continue;
                            }
                            if page && depth == 1 {
                                shifters.insert(p.hit);
                                if see(&mut seen_mid, &mut seen_list, mkey(&r), &r.facts, &pvars) {
                                    out.mids += 1;
                                    let before = m.only.clone().unwrap_or_else(|| shown.clone());
                                    work.push_back(Mid { engine: Some(b), clicks: cl, iface: r.iface, events: r.events, only: Some(before) });
                                } else {
                                    b.quit();
                                }
                            } else if !page && depth < MAX_CLICKS && see(&mut seen_mid, &mut seen_list, mkey(&r), &r.facts, &pvars) {
                                out.mids += 1;
                                next.push(Mid { engine: Some(b), clicks: cl, iface: r.iface, events: r.events, only: None });
                            } else {
                                b.quit();
                            }
                            continue;
                        }
                        if matches!(p.hit, Hit::Verb { .. }) && m.only.is_none() {
                            if depth == 1 {
                                first_gave.insert(p.hit, r.iface);
                            } else if learn && (first_gave.get(&p.hit) == Some(&r.iface) || r.iface == m.iface) {
                                learned.insert(p.hit);
                            }
                        }
                        if shadow {
                            see(&mut seen_mid, &mut seen_list, mkey(&r), &r.facts, &pvars);
                            b.quit();
                            continue;
                        }
                        if depth < MAX_CLICKS && see(&mut seen_mid, &mut seen_list, mkey(&r), &r.facts, &pvars) {
                            out.mids += 1;
                            // a sentence begun after a page of the interface goes on at the slots that changed
                            let only = if depth == 1 { m.only.clone() } else { None };
                            next.push(Mid { engine: Some(b), clicks: cl, iface: r.iface, events: r.events, only });
                        } else {
                            b.quit();
                        }
                        continue;
                    }
                }
                if shadow {
                    b.quit();
                    continue;
                }
                if !ran_on {
                    if let Err(st) = go(&mut b, RunSpec { frame: Some(mf + LEAD + CALM), ..Default::default() })? {
                        out.ends.push(format!("{} {}", name_of(pre, &cl), st.reason));
                        b.quit();
                        continue;
                    }
                }
                let played = calm(&mut b, mf, cfg.horizon, ego)?;
                out.phase[2] += t.elapsed().as_secs_f64();
                out.frames += b.status().frame.saturating_sub(mf);
                let t = Instant::now();
                match played {
                    Played::Stop(st) => {
                        let ob = b.obs(&NAMES)?;
                        let mut r = read(name_of(pre, &cl), events, &ob, st);
                        r.inside = !pre.is_empty();
                        // inside a window: its keys are still the window's input's doing, where not back yet
                        r.own = carry.iter().filter(|c| r.facts.val(c.0) != c.1).copied().collect();
                        // only the input script ran since the click and ego stands where it stood: a sentence half made
                        let half = settled_half.is_none() && ob.ego_pos == mego && {
                            let runs = b.runs()?;
                            let input = [input0, r.facts.val(Key::var(afford::VAR_VERB_SCRIPT as u64)) as i32];
                            afford::composing(r.st.idle.as_deref().or(Some("sentence")), &runs.runs, mf + 1, &input, sentence)
                        };
                        // or a slot's click left the interface another and the world what it is with no input at
                        // the same frame (an object's own script ran for the slot, and no sentence): half made too
                        let half = half
                            || (matches!(p.hit, Hit::Verb { .. }) && r.room == r0.room && ob.ego_pos == mego && r.st.idle.as_deref() == Some("sentence") && r.iface != m.iface && {
                                let mine = Facts(r.facts.iter().filter(|p| world_kind(p.0)).copied().collect());
                                match twin_src.as_mut() {
                                    Some(src) => twin_world(src, &mut twin, &mut twin_at, r.st.frame)?.is_some_and(|tw| tw.diff(&mine).is_empty()),
                                    None => false,
                                }
                            });
                        out.phase[3] += t.elapsed().as_secs_f64();
                        if half {
                            if matches!(p.hit, Hit::Verb { .. }) && m.only.is_none() {
                                if depth == 1 {
                                    first_gave.insert(p.hit, r.iface);
                                } else if learn && (first_gave.get(&p.hit) == Some(&r.iface) || r.iface == m.iface) {
                                    learned.insert(p.hit);
                                }
                            }
                            // a stop to click on from. The game ran on before it took a click again (a dialog's
                            // line, not a verb's slot): a stop like any other, too
                            let fresh = see(&mut seen_mid, &mut seen_list, mkey(&r), &r.facts, &pvars);
                            if depth < MAX_CLICKS && fresh {
                                out.mids += 1;
                                next.push(Mid { engine: Some(b), clicks: cl, iface: r.iface, events: r.events.clone(), only: None });
                                out.results.push(r);
                                continue;
                            }
                            out.results.push(r);
                        } else {
                            // what the input changed: the stop against the twin at its frame (each with its old value)
                            let mut touched: Vec<(Key, Val)> = Vec::new();
                            if let (Some(src), true) = (twin_src.as_mut(), r.room == r0.room) {
                                if let Some(tw) = twin_world(src, &mut twin, &mut twin_at, r.st.frame)? {
                                    let mine = Facts(r.facts.iter().filter(|p| world_kind(p.0)).copied().collect());
                                    touched = tw
                                        .diff(&mine)
                                        .into_iter()
                                        .filter(|d| !motion.is_some_and(|m| m.contains(&d.0)) && !snap.noise.contains(&d.0))
                                        .map(|d| (d.0, r0.facts.val(d.0)))
                                        .collect();
                                }
                            }
                            if !touched.is_empty() {
                                r.own = touched.clone();
                            }
                            // the keys back at their old value PROBE frames on, or PROBE_LONG frames on
                            let mut back: Vec<Key> = Vec::new();
                            if !touched.is_empty() {
                                let mut pb = b.fork()?;
                                out.trials += 1;
                                let at = pb.status().frame;
                                if go(&mut pb, RunSpec { frame: Some(at + PROBE), ..Default::default() })?.is_ok() {
                                    if let Played::Stop(st2) = calm(&mut pb, at + PROBE, cfg.horizon, ego)? {
                                        let ob2 = pb.obs(&NAMES)?;
                                        let mut r2 = read(format!("{}~", r.name), r.events.clone(), &ob2, st2);
                                        r2.own = touched.clone();
                                        back = touched.iter().filter(|&&(k, old)| r2.facts.val(k) == old).map(|t| t.0).collect();
                                        let room2 = r2.room;
                                        out.results.push(r2);
                                        if back.is_empty() && room2 == r0.room && go(&mut pb, RunSpec { frame: Some(at + PROBE_LONG), ..Default::default() })?.is_ok() {
                                            let w3 = pb.facts()?;
                                            back = touched.iter().filter(|&&(k, old)| w3.val(k) == old).map(|t| t.0).collect();
                                        }
                                    }
                                }
                                pb.quit();
                            }
                            if !back.is_empty() && found < MAX_WINDOWS {
                                // a window, while one of its keys was followed from this cell fewer than
                                // WINDOW_TRIES times: for the stop's expansion to follow
                                let c = cell(r.room, r.iface, &snap.policy.world_of(&r.facts), snap.ambient);
                                if back.iter().any(|&k| snap.wins.get(&(c, k)).copied().unwrap_or(0) < WINDOW_TRIES) {
                                    found += 1;
                                    out.cands.push(Cand { engine: b, name: format!("{}>", r.name), events: r.events.clone(), keys: touched.clone(), cell: c, back });
                                    out.results.push(r);
                                    continue;
                                }
                            }
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
    if let Some(t) = twin.take() {
        t.quit();
    }
    if let Some(t) = twin_src.take() {
        t.quit();
    }
    Ok(())
}

/// One round of inputs from the engine's stop (`clicks`), its first clicks dealt to `cfg.slices` forks of the stop
/// side by side; their results one after the other, so a round is the same every time.
fn round(e: &mut Box<dyn Engine>, pre: &str, lead: &[Event], motion: Option<&HashSet<Key>>, carry: &[(Key, Val)], lown: &HashSet<u64>, snap: &Snap<'_>, cfg: &Cfg) -> EResult<Out> {
    let n = cfg.slices.max(1);
    if n == 1 {
        let mut out = Out::default();
        clicks(e, pre, lead, motion, carry, lown, (0, 1), snap, cfg, &mut out)?;
        return Ok(out);
    }
    let mut forks: Vec<Box<dyn Engine>> = Vec::new();
    for _ in 0..n {
        match e.fork() {
            Ok(f) => forks.push(f),
            Err(x) => {
                for f in forks {
                    f.quit();
                }
                return Err(x);
            }
        }
    }
    let parts: Vec<EResult<Out>> = std::thread::scope(|sc| {
        let hs: Vec<_> = forks
            .into_iter()
            .enumerate()
            .map(|(i, mut f)| {
                sc.spawn(move || {
                    let mut out = Out::default();
                    let r = clicks(&mut f, pre, lead, motion, carry, lown, (i, n), snap, cfg, &mut out);
                    f.quit();
                    match r {
                        Ok(()) => Ok(out),
                        Err(x) => {
                            for c in out.cands {
                                c.engine.quit();
                            }
                            Err(x)
                        }
                    }
                })
            })
            .collect();
        hs.into_iter().map(|h| h.join().expect("a round's part")).collect()
    });
    let mut out = Out::default();
    let mut bad = None;
    for p in parts {
        match p {
            Ok(o) => out.absorb(o),
            Err(x) => bad = Some(x),
        }
    }
    match bad {
        None => Ok(out),
        Some(x) => {
            for c in out.cands {
                c.engine.quit();
            }
            Err(x)
        }
    }
}

/// Where the game's scripts name an object or test a flag: per object and per flag, the rooms whose scripts do (the
/// scripts' dump, FMI_GAME_SCRIPTS: one folder a room, one file a script). Advice on where to go with what was
/// won, never taken as true: a room that is named and holds nothing costs one stop.
#[derive(Default)]
struct Mentions {
    objs: HashMap<u32, Vec<i32>>,
    bits: HashMap<u32, Vec<i32>>,
    /// (from room, to room, object): the object's own code leads ego from its room to the other
    exits: Vec<(i32, i32, i32)>,
}

/// The integers that follow each `pat` in `line`.
fn nums_after(line: &str, pat: &str) -> Vec<u32> {
    let mut v = Vec::new();
    let mut at = 0;
    while let Some(i) = line[at..].find(pat) {
        let from = at + i + pat.len();
        let digits: String = line[from..].chars().take_while(|c| c.is_ascii_digit()).collect();
        if let Ok(n) = digits.parse() {
            v.push(n);
        }
        at = from;
    }
    v
}

/// The objects a script's text names (an owner asked for, a second object compared, an object's code started, a
/// class asked for or set, a sentence's objects) and the flags it tests (a flag written is no test).
fn names_of(text: &str) -> (Vec<u32>, Vec<u32>) {
    let (mut objs, mut bits) = (Vec::new(), Vec::new());
    for line in text.lines().filter(|l| !l.starts_with('#')) {
        for pat in ["getObjectOwner(", "startObject(", "classOfIs(", "setClass(", "pickupObject("] {
            objs.extend(nums_after(line, pat));
        }
        // `Local[n] == N`: the sentence's other object, as an object's own code asks for it
        let mut at = 0;
        while let Some(i) = line[at..].find("Local[") {
            let from = at + i + 6;
            let rest = line[from..].trim_start_matches(|c: char| c.is_ascii_digit());
            if let Some(r) = rest.strip_prefix("] == ") {
                let digits: String = r.chars().take_while(|c| c.is_ascii_digit()).collect();
                if let Ok(n) = digits.parse() {
                    objs.push(n);
                }
            }
            at = from;
        }
        if let Some(i) = line.find("doSentence(") {
            let args = line[i + 11..].split(')').next().unwrap_or("");
            objs.extend(args.split(',').skip(1).filter_map(|a| a.trim().parse::<u32>().ok()));
        }
        let mut at = 0;
        while let Some(i) = line[at..].find("Bit[") {
            let from = at + i + 4;
            let digits: String = line[from..].chars().take_while(|c| c.is_ascii_digit()).collect();
            let after = &line[from + digits.len()..];
            if let (Ok(n), true) = (digits.parse(), after.starts_with(']') && !after.starts_with("] = ")) {
                bits.push(n);
            }
            at = from;
        }
    }
    (objs, bits)
}

/// The mentions of a scripts' dump: a room's scripts' own, those of the global scripts few rooms start (GLOBAL_ROOMS
/// at most: a script every room starts says nothing of a room), and for an object the rooms of the objects its
/// own code names.
fn mentions_of(dir: &Path) -> Mentions {
    const GLOBAL_ROOMS: usize = 3;
    let mut m = Mentions::default();
    let num = |name: &str, pre: &str| -> Option<u32> { name.strip_prefix(pre)?.chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse().ok() };
    let mut home: HashMap<u32, i32> = HashMap::new();
    let mut own: HashMap<u32, Vec<u32>> = HashMap::new();
    let mut globals: HashMap<u32, (Vec<u32>, Vec<u32>)> = HashMap::new();
    let mut starts: HashMap<u32, Vec<i32>> = HashMap::new();
    let Ok(rd) = std::fs::read_dir(dir) else { return m };
    let mut folders: Vec<PathBuf> = rd.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.is_dir()).collect();
    folders.sort();
    for folder in folders {
        let fname = folder.file_name().and_then(|x| x.to_str()).unwrap_or("").to_string();
        let room = num(&fname, "room-").map(|r| r as i32);
        let Ok(rd) = std::fs::read_dir(&folder) else { continue };
        let mut files: Vec<PathBuf> = rd.filter_map(|e| e.ok()).map(|e| e.path()).collect();
        files.sort();
        for f in files {
            let name = f.file_name().and_then(|x| x.to_str()).unwrap_or("").to_string();
            let Ok(text) = std::fs::read_to_string(&f) else { continue };
            let (objs, bits) = names_of(&text);
            match room {
                Some(r) => {
                    let me = num(&name, "obj-");
                    if let Some(o) = me {
                        // the rooms the object's own code leads ego to: a way the scripts advise
                        for l in text.lines().filter(|l| !l.starts_with('#')) {
                            if let Some(at) = l.find("loadRoomWithEgo(") {
                                let args: Vec<&str> = l[at + "loadRoomWithEgo(".len()..].split(',').collect();
                                if let Some(to) = args.get(1).and_then(|x| x.trim().parse::<i32>().ok()) {
                                    if to != r && !m.exits.contains(&(r, to, o as i32)) {
                                        m.exits.push((r, to, o as i32));
                                    }
                                }
                            }
                        }
                        home.insert(o, r);
                        own.entry(o).or_default().extend(objs.iter().copied().filter(|x| *x != o));
                    }
                    for o in objs.into_iter().filter(|x| Some(*x) != me) {
                        let v = m.objs.entry(o).or_default();
                        if !v.contains(&r) {
                            v.push(r);
                        }
                    }
                    for b in bits {
                        let v = m.bits.entry(b).or_default();
                        if !v.contains(&r) {
                            v.push(r);
                        }
                    }
                    for g in text.lines().filter(|l| !l.starts_with('#')).flat_map(|l| nums_after(l, "startScript(")) {
                        let v = starts.entry(g).or_default();
                        if !v.contains(&r) {
                            v.push(r);
                        }
                    }
                }
                None => {
                    if let Some(g) = num(&name, "script-") {
                        globals.insert(g, (objs, bits));
                    }
                }
            }
        }
    }
    for (g, (objs, bits)) in &globals {
        let Some(rooms) = starts.get(g).filter(|r| r.len() <= GLOBAL_ROOMS) else { continue };
        for &r in rooms {
            for o in objs {
                let v = m.objs.entry(*o).or_default();
                if !v.contains(&r) {
                    v.push(r);
                }
            }
            for b in bits {
                let v = m.bits.entry(*b).or_default();
                if !v.contains(&r) {
                    v.push(r);
                }
            }
        }
    }
    for (o, others) in &own {
        for r in others.iter().filter_map(|x| home.get(x)) {
            let v = m.objs.entry(*o).or_default();
            if !v.contains(r) {
                v.push(*r);
            }
        }
    }
    for v in m.objs.values_mut().chain(m.bits.values_mut()) {
        v.sort_unstable();
    }
    m
}

/// The clicks of a stop's name (`v<verb>`, `t<object>`, `t-<actor>`, `w<floor point>`); None where the name holds
/// more than clicks (a wait, a window, a walk of the search's own).
fn hits_of(name: &str) -> Option<Vec<Hit>> {
    let name = name.strip_suffix('~').unwrap_or(name);
    let mut v = Vec::new();
    let b = name.as_bytes();
    let mut i = 0;
    while i < b.len() {
        let kind = b[i];
        i += 1;
        let neg = i < b.len() && b[i] == b'-';
        if neg {
            i += 1;
        }
        let from = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        let n: i32 = name[from..i].parse().ok()?;
        v.push(match (kind, neg) {
            (b'v', false) => Hit::Verb { slot: 0, id: n },
            (b't', false) => Hit::Object(n),
            (b't', true) => Hit::Actor(n),
            (b'w', false) => Hit::Walk(n),
            _ => return None,
        });
    }
    (!v.is_empty()).then_some(v)
}

/// The shortest way from room to room over the clicks that were seen to lead from one to the next.
fn way<'a>(edges: &'a HashMap<(i32, i32), Vec<Hit>>, from: i32, to: i32) -> Option<Vec<(i32, &'a Vec<Hit>)>> {
    let mut prev: HashMap<i32, i32> = HashMap::from([(from, from)]);
    let mut queue = VecDeque::from([from]);
    while let Some(a) = queue.pop_front() {
        if a == to {
            break;
        }
        let mut next: Vec<i32> = edges.keys().filter(|k| k.0 == a && !prev.contains_key(&k.1)).map(|k| k.1).collect();
        next.sort_unstable();
        for b in next {
            prev.insert(b, a);
            queue.push_back(b);
        }
    }
    let mut path = Vec::new();
    let mut at = to;
    while at != from {
        let a = *prev.get(&at)?;
        path.push((at, edges.get(&(a, at))?));
        at = a;
    }
    path.reverse();
    (!path.is_empty() && path.len() <= GO_HOPS).then_some(path)
}

/// A walk from the engine's stop along `path`, each room's clicks as they were seen to lead on: the stop at its
/// end, or None where a click finds nothing to click or a room is not the one expected (the world is another now).
fn walk(e: &mut Box<dyn Engine>, path: &[(i32, &Vec<Hit>)], ego: i32, cfg: &Cfg, bad: &mut Vec<(i32, i32)>) -> EResult<Option<Res>> {
    let mut cur = e.fork()?;
    let mut events: Vec<Event> = Vec::new();
    let mut last = cur.status().clone();
    'hops: for (room, hits) in path {
        let mut from = cur.status().frame;
        let here = cur.status().room;
        for (k, hit) in hits.iter().enumerate() {
            // a click at the screen's edge that left the room: the stop it came to
            let mut left: Option<Status> = None;
            // the click's point; a thing of the room that is off the screen (a room wider than it) is walked
            // towards first, floor click by floor click at the screen's edge on its side, until it shows
            let mut point = None;
            for _ in 0..GO_SCROLLS {
                let o = cur.obs(&NAMES)?;
                let of = afford::offer_cfg(&mut cur, &o, false, true)?;
                let (top, h) = cur.status().screen.map(|s| (s.0, s.1)).unwrap_or((0, afford::SCREEN_H));
                let grid: Vec<(i32, i32)> =
                    FLOOR_Y.iter().flat_map(|&k| FLOOR_X.iter().map(move |&x| (x, (top + (h - top) * k / 8).min(h - 2)))).collect();
                point = match *hit {
                    Hit::Walk(i) => grid.get(i as usize).copied(),
                    Hit::Verb { id, .. } => of.verbs.iter().find(|p| matches!(p.hit, Hit::Verb { id: i, .. } if i == id)).map(|p| (p.x, p.y)),
                    other => of.targets.iter().find(|p| p.hit == other).map(|p| (p.x, p.y)),
                };
                if point.is_some() {
                    break;
                }
                // where the thing is in the room, against what the screen shows
                let (x0, x1) = (cur.status().xstart, cur.status().xstart + afford::SCREEN_W);
                let at = match *hit {
                    Hit::Object(id) => o.room_objects.as_deref().unwrap_or(&[]).iter().find(|r| r.id == id).map(|r| r.x + r.w / 2),
                    Hit::Actor(id) => o.actors.iter().find(|a| a.id == id).map(|a| a.x),
                    _ => None,
                };
                let Some(at) = at.filter(|x| *x < x0 || *x >= x1) else {
                    trace(&format!("    walk: {} is not in the room's list, or on the screen and not offered (screen {x0}..{x1}, at {at:?})", hit.name()));
                    break;
                };
                // the points at the screen's edge on that side, the outermost column first and in it the lowest
                // first: a click there walks ego that way (to a thing, if one lies there), and the room scrolls
                // once ego is near the screen's edge
                let cols: Vec<usize> = if at < x0 { (0..FLOOR_X.len()).collect() } else { (0..FLOOR_X.len()).rev().collect() };
                let ats = cur.at_many(&grid)?;
                let is_floor = |i: usize| ats.get(i).is_some_and(|a| a.main.is_some_and(|m| m.2 == 0 && m.3 == 0));
                // (only points of the room's picture: below it lie the verbs)
                let mut cands: Vec<usize> = cols
                    .iter()
                    .take(3)
                    .flat_map(|&c| (0..FLOOR_Y.len()).rev().map(move |r| r * FLOOR_X.len() + c))
                    .filter(|&i| ats.get(i).is_some_and(|a| a.main.is_some()))
                    .collect();
                cands.truncate(FLOOR_Y.len());
                let before = cur.status().xstart;
                let mut moved = false;
                for &i in cands.iter().take(4) {
                    let f = cur.status().frame;
                    let ev = Event { frame: f + 1, what: Input::Click(grid[i].0, grid[i].1) };
                    if cur.inputs(std::slice::from_ref(&ev)).is_err() || go(&mut cur, RunSpec { frame: Some(f + LEAD + CALM), ..Default::default() })?.is_err() {
                        break;
                    }
                    events.push(ev);
                    match calm(&mut cur, f, cfg.horizon, ego)? {
                        Played::Stop(st) if st.room != here => {
                            left = Some(st);
                            break;
                        }
                        Played::Stop(_) => {}
                        _ => break,
                    }
                    // the room goes on scrolling after ego stands: on until it rests
                    for _ in 0..40 {
                        let x = cur.status().xstart;
                        let f = cur.status().frame;
                        if go(&mut cur, RunSpec { frame: Some(f + 8), ..Default::default() })?.is_err() || cur.status().xstart == x {
                            break;
                        }
                    }
                    trace(&format!("    walk: room {} click at {:?} (floor {}) -> xstart {} ego {:?} (of {} points in the picture)", cur.status().room, grid[i], is_floor(i), cur.status().xstart, cur.status().ego, cands.len()));
                    if cur.status().xstart != before {
                        moved = true;
                        break;
                    }
                }
                if left.is_some() {
                    break;
                }
                if !moved {
                    trace(&format!("    walk: no click at the screen's edge moved the room towards {} (screen {x0}..{x1}, at {at})", hit.name()));
                    break;
                }
            }
            if let Some(st) = left {
                // the way on was the screen's edge itself: the hop is made if the room is the one expected
                if st.room == *room {
                    trace(&format!("    walk: the screen's edge led from room {here} to {room}"));
                    last = st;
                    continue 'hops;
                }
                trace(&format!("    walk: the screen's edge led from room {here} to {} not {room}", st.room));
                cur.quit();
                return Ok(None);
            }
            let Some((x, y)) = point else {
                trace(&format!("    walk: in room {} nothing to click for {} (on the way to {room})", cur.status().room, hit.name()));
                cur.quit();
                return Ok(None);
            };
            let park = park_point(&mut cur)?;
            from = cur.status().frame;
            let mut evs = vec![Event { frame: from + 1, what: Input::Click(x, y) }];
            if let Some((px, py)) = park {
                evs.push(Event { frame: from + 2, what: Input::Move(px, py) });
            }
            if cur.inputs(&evs).is_err() {
                cur.quit();
                return Ok(None);
            }
            events.extend(evs);
            if k + 1 < hits.len() && go(&mut cur, RunSpec { frame: Some(from + LEAD + CALM), ..Default::default() })?.is_err() {
                cur.quit();
                return Ok(None);
            }
        }
        if go(&mut cur, RunSpec { frame: Some(from + LEAD + CALM), ..Default::default() })?.is_err() {
            cur.quit();
            return Ok(None);
        }
        match calm(&mut cur, from, cfg.horizon, ego)? {
            Played::Stop(st) if st.room == *room => last = st,
            other => {
                // the clicks were made and did not lead on: the way is false (or the world another)
                bad.push((here, *room));
                trace(&format!("    walk: {} led to room {} not {room} ({})", hits.iter().map(|h| h.name()).collect::<String>(), cur.status().room, match other { Played::Stop(_) => "a stop", Played::Ended(_) => "the end", Played::NotIdle => "no stop" }));
                cur.quit();
                return Ok(None);
            }
        }
    }
    let ob = cur.obs(&NAMES)?;
    let r = read(format!("go{}", last.room), events, &ob, last);
    cur.quit();
    Ok(Some(r))
}

/// What `explore` reads of the search: the rooms' motion at the batch's start.
struct Snap<'a> {
    motion: &'a HashMap<(u32, i32, i32, Key), u32>,
    wins: &'a HashMap<(u64, Key), u32>,
    noise: &'a HashSet<Key>,
    lclocks: &'a HashSet<u64>,
    policy: &'a Policy,
    ambient: &'a HashSet<Key>,
    mentions: &'a Mentions,
    edges: &'a HashMap<(i32, i32), Vec<Hit>>,
}

/// Every input of the stop tried, at the stop and after the game's own moves. Reads the engine and `snap` only.
fn explore(e: &mut Box<dyn Engine>, level: u32, pending: &[(Key, Val)], inside: bool, news: &[Key], snap: &Snap<'_>, cfg: &Cfg) -> EResult<Out> {
    let o0 = e.obs(&NAMES)?;
    let st0 = e.status().clone();
    let r0 = read(String::new(), Vec::new(), &o0, st0.clone());
    let mut out = Out { room: r0.room, walkbox: r0.walkbox, ..Default::default() };
    let ego = r0.facts.val(Key::pack(K_EGO, 0, 0, 0)) as i32;
    // the game's own moves: no input for a while. What they move is the stop's own motion; a variable that takes two
    // values or more over them is a clock
    let known = |k: Key| snap.motion.get(&(level, r0.room, r0.walkbox, k)).is_some_and(|&c| c >= MOTION_TRIES);
    let mut waited: Vec<(u64, Box<dyn Engine>)> = Vec::new();
    let mut values: HashMap<Key, Vec<Val>> = HashMap::new();
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
                    for (k, _, now) in r0.facts.diff(&r.facts) {
                        if k.kind() == K_VAR {
                            let v = values.entry(k).or_default();
                            if !v.contains(&now) {
                                v.push(now);
                            }
                        } else if world_kind(k) {
                            out.ambient.insert(k);
                            moved |= !known(k);
                        }
                    }
                    out.results.push(r);
                    if moved {
                        waited.push((n, b));
                        continue;
                    }
                }
                Played::Ended(st) => out.ends.push(format!("wait{n} {}", st.reason)),
                Played::NotIdle => out.not_idle += 1,
            },
        }
        b.quit();
    }
    let moving: HashSet<Key> = values.keys().copied().collect();
    out.clocks.extend(values.into_iter().filter(|x| x.1.len() >= 2).map(|x| x.0));
    // what the stop's own waits moved: no input's doing
    let motion = out.ambient.clone();
    // and the game's own motion, where no input's change of the key was pending
    if !inside {
        out.noise = out.ambient.iter().filter(|k| !pending.iter().any(|p| p.0 == **k)).copied().collect();
    }
    trace(&format!("  room {} waits done, {} moved", r0.room, waited.len()));
    // the scripts' locals the stop's own waits moved: its clocks, whatever the search knows of them yet
    let lown: HashSet<u64> = {
        let at0: HashMap<u64, i32> = r0.locals.iter().copied().collect();
        out.results.iter().flat_map(|r| r.locals.iter()).filter(|l| at0.get(&l.0) != Some(&l.1)).map(|l| l.0).collect()
    };
    let mut base = round(e, "", &[], Some(&motion), &[], &lown, snap, cfg)?;
    let cands = std::mem::take(&mut base.cands);
    let first = out.results.len();
    out.absorb(base);
    trace(&format!("  room {} round done: {} trials, {} windows found", r0.room, out.trials, cands.len()));
    // a variable a fifth of the stop's own inputs changed echoes the input: counted on the expanded stop's round
    // only (in a window's round, or after a wait, an earlier input's effect lands in every trial), over its first
    // stops (not those read PROBE frames on) that changed nothing else in the world (where a dialog has three
    // lines and one of them wins, a fifth of the inputs raise the count of what was won: that is no echo), and
    // never a variable the stop's waits moved
    {
        let made: Vec<&Res> = out.results[first..]
            .iter()
            .filter(|r| !r.name.ends_with('~') && !r0.facts.diff(&r.facts).iter().any(|d| world_kind(d.0)))
            .collect();
        if made.len() >= 10 {
            let mut changed: HashMap<Key, usize> = HashMap::new();
            for r in &made {
                for (k, _, _) in r0.facts.diff(&r.facts) {
                    if k.kind() == K_VAR && !moving.contains(&k) {
                        *changed.entry(k).or_default() += 1;
                    }
                }
            }
            let n = made.len();
            out.iface_vars.extend(changed.into_iter().filter(|&(_, c)| c * 5 >= n).map(|(k, _)| k));
        }
    }
    // the windows: the first MAX_WINDOWS whose cell has tries left; the inputs from each, with no window found
    // inside one
    let mut mine: HashMap<(u64, Key), u32> = HashMap::new();
    let mut bad = None;
    for c in cands {
        let mut b = c.engine;
        let fresh = c.back.iter().any(|&k| snap.wins.get(&(c.cell, k)).copied().unwrap_or(0) + mine.get(&(c.cell, k)).copied().unwrap_or(0) < WINDOW_TRIES);
        if bad.is_none() && fresh && (out.windows as usize) < MAX_WINDOWS {
            for &k in &c.back {
                *mine.entry((c.cell, k)).or_default() += 1;
                out.followed.push((c.cell, k));
            }
            out.windows += 1;
            trace(&format!("    window {} ({} trials so far)", c.name, out.trials));
            match round(&mut b, &c.name, &c.events, None, &c.keys, &lown, snap, cfg) {
                Ok(o) => out.absorb(o),
                Err(x) => bad = Some(x),
            }
        }
        b.quit();
    }
    // where the world moved by itself (and few stops of this room and level saw that): the inputs again from there
    for (n, mut b) in waited {
        if bad.is_none() {
            match round(&mut b, &format!("wait{n}+"), &[], None, &[], &lown, snap, cfg) {
                Ok(o) => out.absorb(o),
                Err(x) => bad = Some(x),
            }
        }
        b.quit();
    }
    if let Some(x) = bad {
        return Err(x);
    }
    // --go: the stop has won something (an object, a flag). Where the scripts name it, there it is asked for: a
    // walk to each such room over the clicks seen to lead from room to room, one trial a walk; the nearest room's
    // stop last, so that it is the first to be expanded. The walk through every room stays, for what the scripts
    // do not say
    if !news.is_empty() {
        let mut rooms: Vec<i32> = Vec::new();
        for k in news {
            let named = if k.kind() == K_BIT { snap.mentions.bits.get(&(k.a() as u32)) } else { snap.mentions.objs.get(&(k.a() as u32)) };
            for &r in named.into_iter().flatten() {
                if r != r0.room && !rooms.contains(&r) {
                    rooms.push(r);
                }
            }
        }
        let mut ways: Vec<Vec<(i32, &Vec<Hit>)>> = rooms.iter().filter_map(|&t| way(snap.edges, r0.room, t)).collect();
        ways.sort_by_key(|w| (w.len(), w.last().map(|x| x.0)));
        ways.truncate(GO_ROOMS);
        trace(&format!("  room {} go: rooms named {:?}, ways {:?}", r0.room, rooms, ways.iter().map(|w| w.iter().map(|x| format!("{}:{}", x.0, x.1.iter().map(|h| h.name()).collect::<String>())).collect::<Vec<_>>().join(" ")).collect::<Vec<_>>()));
        for w in ways.iter().rev() {
            out.trials += 1;
            if let Some(r) = walk(e, w, ego, cfg, &mut out.bad)? {
                out.results.push(r);
            }
        }
    }
    let locals0: HashMap<u64, i32> = r0.locals.iter().copied().collect();
    for r in out.results.iter_mut() {
        r.changed = r0.facts.diff(&r.facts).into_iter().filter(|d| world_kind(d.0) || d.0.kind() == K_VAR).map(|d| (d.0, d.2)).collect();
        r.locals.retain(|l| locals0.get(&l.0) != Some(&l.1));
        if r.name.starts_with("wait") && !r.name.contains('+') {
            // what a run with no input moved
            out.lclocks.extend(r.locals.iter().map(|l| l.0));
        }
    }
    // stops that are one stop for all the search counts (the room, the interface, the picture, the floor, the
    // scripts' locals, and the world against the expanded stop's): the first stands for the rest. Most inputs of a
    // stop end where another did, and the commit, which is not made side by side, need not read them again
    let before = out.results.len();
    let mut seen: HashSet<u64> = HashSet::new();
    out.results.retain(|r| {
        if r.name.starts_with("wait") || r.name.starts_with("go") {
            return true;
        }
        let mut h = DefaultHasher::new();
        (r.room, r.iface, &r.vis, r.walkbox, r.cam, r.inside, &r.locals, &r.own).hash(&mut h);
        for c in r.changed.iter().filter(|c| snap.policy.world(c.0)) {
            c.hash(&mut h);
        }
        seen.insert(h.finish())
    });
    trace(&format!("  room {}: {} stops, {} of them not one another's like", r0.room, before, out.results.len()));
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
        let level0 = self.nodes[id].level;
        for &k in &out.ambient {
            *self.motion.entry((level0, room0, out.walkbox, k)).or_default() += 1;
        }
        self.noise.extend(out.noise.iter().copied());
        for f in &out.followed {
            *self.wins.entry(*f).or_default() += 1;
        }
        self.policy.iface_vars.extend(out.iface_vars.iter().copied());
        self.policy.clocks.extend(out.clocks.iter().copied());
        self.lclocks.extend(out.lclocks.iter().copied());
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
        let (line0, room00) = (self.nodes[id].line, self.nodes[id].room);
        // a way whose clicks a walk made and that did not lead on: forgotten (seen again, it is learned again, twice
        // at most)
        for k in &out.bad {
            if let Some(h) = self.edges.remove(k) {
                let how: String = h.iter().map(|x| x.name()).collect();
                if let Some(f) = self.edges_log.as_mut() {
                    let _ = writeln!(f, "{}", json!({"from": k.0, "to": k.1, "how": how, "failed": true}));
                }
                *self.dead.entry((k.0, k.1, how)).or_default() += 1;
            }
        }
        // the rooms the stop comes to with no input at all: a click that ends there did not lead there
        let drift: HashSet<i32> = results.iter().filter(|r| r.name.starts_with("wait") && !r.name.contains('+') && r.room != room00).map(|r| r.room).collect();
        let mut keep: Vec<(usize, usize, usize, usize, bool)> = Vec::new(); // (result, node, level, queue, to the stack)
        for (i, r) in results.iter().enumerate() {
            let world = self.policy.world_of(&r.facts);
            let mut novel: Vec<String> = Vec::new();
            let mut class = QUEUES;
            let ego = r.facts.val(Key::pack(K_EGO, 0, 0, 0));
            let owned: Vec<u32> = r.facts.iter().filter(|p| p.0.kind() == K_OWNER && p.1 == ego).map(|p| p.0.a() as u32).filter(|o| !self.root_owned.contains(o)).collect();
            let mut ever = ever0.clone();
            for &o in &owned {
                if let Err(at) = ever.binary_search(&o) {
                    ever.insert(at, o);
                }
            }
            // --lines gains: a line is what was gained, a loss stays in its line (what it changed is new there,
            // the rest is not)
            let (level, gline) = if self.gains_only { (ever.len() as u32, line_of(&ever, &[])) } else { ((2 * ever.len() - owned.len()) as u32, line_of(&ever, &owned)) };
            // the flags the stop holds set (but those seen unset again): a count no stop of the line has held is new
            // (a stop that holds two flags which two other stops hold one each is the first with both)
            // a flag here is a bit or a class of an object (an object made into something else keeps its number and
            // gets a class): both are set once and stay, or are seen unset again and count no more
            let flaglike = |k: Key| k.kind() == K_BIT || k.kind() == K_CLASS;
            for c in r.changed.iter().filter(|c| flaglike(c.0) && c.1 == 0) {
                self.toggles.insert(c.0);
            }
            let mut set: Vec<u64> = Vec::new();
            let mut flags = 0u32;
            for p in world.iter().filter(|p| flaglike(p.0) && p.1 != 0 && !self.toggles.contains(&p.0)) {
                let n = if p.0.kind() == K_BIT { p.0.a() as usize } else { 4096 + (p.0.a() as usize) * 32 + (p.0.b() as usize & 31) };
                if set.len() <= n / 64 {
                    set.resize(n / 64 + 1, 0);
                }
                set[n / 64] |= 1 << (n % 64);
                flags += 1;
            }
            // a set of flags that no kept stop of the line holds all of: new, and followed before anything else of
            // its level (the last such stop first: what is won one flag at a time is won on one path)
            let within = |a: &[u64], b: &[u64]| a.iter().enumerate().all(|(i, w)| w & !b.get(i).copied().unwrap_or(0) == 0);
            let tops = self.tops.entry(gline).or_default();
            let mut dive = false;
            // only a stop that changed a flag, or stands in another line of gains than the expanded stop, can hold
            // a set that is new
            let moved = gline != self.nodes[id].gline || r.changed.iter().any(|c| flaglike(c.0));
            if moved && !tops.iter().any(|m| within(&set, m)) {
                tops.retain(|m| !within(m, &set));
                tops.push(set);
                dive = true;
            }
            // what the stop won, where it begins a line: the objects it gained, the flags it set
            let news: Vec<Key> = if dive {
                r.changed
                    .iter()
                    .filter(|c| (c.0.kind() == K_OWNER && c.1 == ego) || (flaglike(c.0) && c.1 != 0 && !self.toggles.contains(&c.0)))
                    .map(|c| c.0)
                    .collect()
            } else {
                Vec::new()
            };
            // the clicks that led from the expanded stop's room to another: the way between the two, for a walk
            if r.room != room00 && !drift.contains(&r.room) && !r.name.contains('+') && !r.name.contains('>') && !r.name.starts_with("go") {
                if let Some(h) = hits_of(&r.name) {
                    let how = r.name.trim_end_matches('~').to_string();
                    if !self.edges.contains_key(&(room00, r.room)) && self.dead.get(&(room00, r.room, how)).copied().unwrap_or(0) < 2 {
                        // kept beside the stops (OUT/edges.jsonl), for a run that starts from one of them
                        if let Some(f) = self.edges_log.as_mut() {
                            let _ = writeln!(f, "{}", json!({"from": room00, "to": r.room, "how": r.name.trim_end_matches('~')}));
                        }
                        self.edges.insert((room00, r.room), h);
                    }
                }
            }
            // --flag-lines: such a stop starts a line of its own, as a gain does: every room and input counts
            // again from it (a flag is carried to where it matters by the stops that are new on the way)
            let fline = if dive { self.nodes.len() } else { self.nodes[id].fline };
            let line = if self.flag_lines {
                let mut h = DefaultHasher::new();
                (gline, fline).hash(&mut h);
                h.finish()
            } else {
                gline
            };
            if dive {
                novel.push(format!("flags={flags}"));
                class = 0;
            }
            // in the expanded stop's line and room only what changed can be new; elsewhere every atom may be
            let atoms: Vec<(Key, Val)> = if line == line0 && r.room == room00 {
                r.changed.iter().filter(|p| self.policy.world(p.0)).copied().collect()
            } else {
                world.clone()
            };
            for &(k, val) in &atoms {
                let guide = self.policy.guide.contains(&k);
                if k.kind() == K_VAR {
                    if self.seen.insert((line, k, val)) {
                        novel.push(format!("{k}={val}"));
                        class = class.min(2);
                        let vals = self.var_vals.entry(k).or_default();
                        if !vals.contains(&val) {
                            vals.push(val);
                        }
                        if vals.len() > VAR_CAP {
                            self.policy.capped.insert(k);
                        }
                    }
                    if self.seen_at.insert((line, r.room, k, val)) {
                        class = class.min(3);
                    }
                    continue;
                }
                if self.seen.insert((line, k, val)) {
                    novel.push(format!("{k}={val}"));
                    class = 0;
                }
                if self.seen_at.insert((line, r.room, k, val)) {
                    class = class.min(if guide { 3 } else { 4 });
                }
            }
            // a local variable of a running script with a value new in the line (a count a dialog keeps of what
            // was said): like a variable's
            let mut fresh = false;
            for &(k, v) in &r.locals {
                if self.lclocks.contains(&k) || self.lcapped.contains(&k) {
                    continue;
                }
                if self.lseen.insert((line, k, v)) {
                    fresh = true;
                    let vals = self.lvals.entry(k).or_default();
                    if !vals.contains(&v) {
                        vals.push(v);
                    }
                    if vals.len() > LOCAL_CAP {
                        self.lcapped.insert(k);
                    }
                }
            }
            if fresh {
                novel.push("local".into());
                class = class.min(2);
            }
            for &t in &r.vis {
                if self.sees.insert((line, r.room, t)) {
                    novel.push(format!("sees={t}"));
                    class = class.min(1);
                }
            }
            if self.ifaces.insert((line, r.iface)) {
                novel.push("iface".into());
                class = class.min(1);
            }
            if self.boxes.insert((line, r.room, r.walkbox)) {
                novel.push(format!("box={}", r.walkbox));
                class = class.min(2);
            }
            if self.cams.insert((line, r.room, r.cam)) {
                novel.push(format!("cam={}", r.cam));
                class = class.min(2);
            }
            if self.cells.insert((line, cell(r.room, r.iface, &world, &self.ambient))) {
                class = class.min(4);
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
            let text = json!({"id": nid, "parent": id, "depth": depth + 1, "frame": r.st.frame, "room": r.room, "level": level,
                "owned": owned.len(), "class": class, "how": r.name, "novel": novel, "t": (secs * 10.0).round() / 10.0,
                "news": news.iter().map(|k| format!("{k}")).collect::<Vec<_>>(),
                "events": events.iter().map(fmi_explore::events::event_json).collect::<Vec<Value>>()});
            if let Some(f) = self.log.as_mut() {
                let _ = writeln!(f, "{text}");
            }
            let mut pending = r.own.clone();
            for &(k, old) in &self.nodes[id].pending {
                if r.facts.val(k) != old && !pending.iter().any(|p| p.0 == k) {
                    pending.push((k, old));
                }
            }
            pending.truncate(PENDING);
            let inside = r.inside || (self.nodes[id].inside && r.name.starts_with("wait"));
            self.nodes.push(Node { events, frame: r.st.frame, depth: depth + 1, ever, level, line, room: r.room, pending, inside, flags, fline, gline, news });
            self.kept[class] += 1;
            self.top = self.top.max(level);
            // with --flag-lines every stop of queue 0 goes on the stack: the newest line is the one worked on
            keep.push((i, nid, level as usize, class, dive || (self.flag_lines && class == 0)));
        }
        if let Some(f) = self.offers.as_mut() {
            let _ = writeln!(f, "{}", json!({"node": id, "room": room0, "offer": out.offer, "trials": out.trials, "skipped": out.skipped, "mids": out.mids, "windows": out.windows,
                "stops": results.len(), "kept": keep.len(), "refused": out.refused, "not_idle": out.not_idle}));
        }
        let mut make = Vec::new();
        for (i, nid, level, class, dive) in keep {
            if self.queues.len() <= level {
                self.queues.resize_with(level + 1, Default::default);
            }
            if self.stacks.len() <= level {
                self.stacks.resize_with(level + 1, Default::default);
            }
            if dive {
                self.stacks[level].push(nid);
            } else {
                self.queues[level][class].push_back(nid);
            }
            if class < LIVE_QUEUES {
                make.push((nid, results[i].events.clone(), results[i].st.frame));
            }
        }
        // a kept engine for LIVE_EACH of them: the last ones where the newest stop is taken first, else the first
        if make.len() > LIVE_EACH {
            if self.flag_lines {
                make.drain(..make.len() - LIVE_EACH);
            } else {
                make.truncate(LIVE_EACH);
            }
        }
        make
    }

    fn progress(&self, last: &mut u64, dt: f64) {
        let secs = self.t0.map(|t| t.elapsed().as_secs_f64()).unwrap_or(0.0);
        let rungs: Vec<&str> = self.rungs.iter().filter(|g| g.at.is_some()).map(|g| g.name.as_str()).collect();
        let queued: Vec<usize> = (0..QUEUES).map(|c| self.queues.iter().map(|q| q[c].len()).sum()).collect();
        let at_top: usize = self.queues.last().map_or(0, |q| q.iter().map(|x| x.len()).sum()) + self.stacks.last().map_or(0, |x| x.len());
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
        .boot(&BootCfg { seed: 1, events, frame: Some(frame), dump_locals: true, lean: true, out_dir: eng_dir(out) })
        .expect("the boot");
    let o = e.obs(&ObsReq::default()).expect("obs");
    let base = facts_of(&o);
    let ego = base.val(Key::pack(K_EGO, 0, 0, 0)) as i32;
    let of = afford::offer_cfg(&mut e, &o, false, true).expect("offer");
    println!("stop: frame {frame} room {} offer {}", e.status().room, of.json());
    println!("  ego {:?} xstart {} room width {}", o.ego_pos, e.status().xstart, e.status().room_w);
    for r in o.room_objects.as_deref().unwrap_or(&[]) {
        println!("  object {} state {} at ({}, {}) size {}x{} walk {:?}", r.id, r.state, r.x, r.y, r.w, r.h, r.walk);
    }
    for a in &o.actors {
        println!("  actor {} at ({}, {}) moving {}", a.id, a.x, a.y, a.moving);
    }
    println!("  targets: {}", of.targets.iter().map(|p| format!("{}@({},{})", p.hit.name(), p.x, p.y)).collect::<Vec<_>>().join(" "));
    println!("  verbs: {}", of.verbs.iter().map(|p| format!("{}@({},{})", p.hit.name(), p.x, p.y)).collect::<Vec<_>>().join(" "));
    if let Ok(bx) = e.boxes() {
        for (i, w) in bx.boxes.iter().enumerate() {
            println!("  box {i} flags {:#x} {:?} {:?} {:?} {:?}", w.flags, w.ul, w.ur, w.lr, w.ll);
        }
    }
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
        .boot(&BootCfg { seed: 1, events: events.clone(), frame: Some(frame), dump_locals: true, lean: true, out_dir: eng_dir(out) })
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
            .boot(&BootCfg { seed: 1, events: Vec::new(), frame: None, dump_locals: true, lean: true, out_dir: eng_dir(&out) })
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
    if a.first().map(String::as_str) == Some("seq") {
        // `direct seq NODES ID OUT KEYS STEP...`: a measurement tool. The kept stop's movie from boot, then each STEP
        // "X,Y,N": a click at (X, Y) on the next frame and N frames run (N = "s": to where ego stands and the bridge
        // is idle); after each step, and every 5 frames for 120 more after the last, the KEYS' values.
        let text = std::fs::read_to_string(&a[1]).expect("NODES");
        let id: u64 = a[2].parse().expect("ID");
        let line = text.lines().filter_map(|l| serde_json::from_str::<Value>(l).ok()).find(|v| v["id"].as_u64() == Some(id)).expect("the node");
        let events: Vec<Event> = line["events"].as_array().unwrap().iter().filter_map(fmi_explore::events::event_of).collect();
        let frame = line["frame"].as_u64().unwrap();
        let out = PathBuf::from(&a[3]);
        std::fs::create_dir_all(&out).expect("OUT");
        let host = Host::new(HostCfg::from_env()).expect("the host");
        let mut e = host.boot(&BootCfg { seed: 1, events, frame: Some(frame), dump_locals: true, lean: true, out_dir: eng_dir(&out) }).expect("the boot");
        let keys: Vec<Key> = a[4].split(',').map(Key::parse).collect();
        let ego = e.facts().expect("facts").val(Key::pack(K_EGO, 0, 0, 0)) as i32;
        let say = |e: &mut Box<dyn Engine>, what: &str| {
            let f = e.facts().expect("facts");
            let st = e.status().clone();
            println!("+{:<4} {what:<14} room {} xstart {} ego {:?} idle {:?} mv {:?} box {:?} {}", st.frame - frame, st.room, st.xstart, st.ego, st.idle, st.mv, st.walkbox,
                keys.iter().map(|k| format!("{k}={}", f.val(*k))).collect::<Vec<_>>().join(" "));
            if std::env::var_os("DIRECT_SEQ_VERBS").is_some() {
                // the slots shown (id, point, the text's length; the text itself only with DIRECT_SEQ_TEXT, for a
                // look by hand) and the running scripts' locals that are not 0
                let o = e.obs(&NAMES).expect("obs");
                let text = std::env::var_os("DIRECT_SEQ_TEXT").is_some();
                println!("      slots: {}", o.verbs.iter().map(|v| format!("v{}@({},{}):{}", v.id, v.x, v.y, if text { v.name.clone().unwrap_or_default() } else { v.name.as_deref().unwrap_or("").len().to_string() })).collect::<Vec<_>>().join(" "));
                println!("      room {} cs {} up {} scripts: {}", st.room, st.cs, st.up, o.slots.iter().filter_map(|sl| sl.locals.as_ref().map(|ls| format!("{}:{}{:?}", sl.where_, sl.number, ls.iter().enumerate().filter(|x| *x.1 != 0).map(|x| (x.0, *x.1)).collect::<Vec<_>>()))).collect::<Vec<_>>().join(" "));
            }
        };
        say(&mut e, "start");
        for step in &a[5..] {
            let p: Vec<&str> = step.split(',').collect();
            let at = e.status().frame;
            if p[0] == "k" {
                // "k,ON,N": the skips (text and cutscenes) set on or off
                let on = p[1] == "1";
                e.inputs(&[Event { frame: at + 1, what: Input::Skips { text: on, cutscenes: on, spare: None } }]).expect("skips");
                let _ = go(&mut e, RunSpec { frame: Some(at + p[2].parse::<u64>().unwrap()), ..Default::default() });
                say(&mut e, step);
                continue;
            }
            let (x, y): (i32, i32) = (p[0].parse().unwrap(), p[1].parse().unwrap());
            if x >= 0 {
                e.inputs(&[Event { frame: at + 1, what: Input::Click(x, y) }]).expect("inputs");
            }
            if p[2] == "s" {
                let _ = go(&mut e, RunSpec { frame: Some(at + 2), ..Default::default() });
                let _ = settle(&mut e, at + 2000, at + 300, ego);
            } else {
                let _ = go(&mut e, RunSpec { frame: Some(at + p[2].parse::<u64>().unwrap()), ..Default::default() });
            }
            say(&mut e, step);
        }
        for _ in 0..24 {
            let at = e.status().frame;
            if go(&mut e, RunSpec { frame: Some(at + 5), ..Default::default() }).map_or(true, |r| r.is_err()) {
                break;
            }
            say(&mut e, "");
        }
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
        slices: opt("--slices").map(|v| v.parse().unwrap()).unwrap_or(1),
        batch: opt("--batch").map(|v| v.parse().unwrap()).unwrap_or(BATCH),
        at_once: opt("--at-once").map(|v| v.parse().unwrap()).unwrap_or(0),
    };
    let host = Host::new(HostCfg::from_env()).expect("the host");
    let root_events = if a.iter().any(|x| x == "--no-skips") {
        Vec::new()
    } else {
        vec![Event { frame: 1, what: Input::Skips { text: true, cutscenes: true, spare: None } }]
    };
    // --from NODES ID (a measurement aid): the search starts at that kept stop of another run, not at the boot
    let from: Option<(Vec<Event>, u64, Vec<Key>)> = opt("--from").map(|nodes| {
        let id: u64 = a[a.iter().position(|x| x == "--from").unwrap() + 2].parse().expect("--from NODES ID");
        let text = std::fs::read_to_string(&nodes).expect("--from NODES");
        let line = text.lines().filter_map(|l| serde_json::from_str::<Value>(l).ok()).find(|v| v["id"].as_u64() == Some(id)).expect("the node");
        let news = line["news"].as_array().map(|v| v.iter().filter_map(|x| x.as_str()).map(Key::parse).collect()).unwrap_or_default();
        (line["events"].as_array().unwrap().iter().filter_map(fmi_explore::events::event_of).collect(), line["frame"].as_u64().unwrap(), news)
    });
    let (root_events, root_frame, from_news) = match from {
        Some((ev, f, n)) => (ev, Some(f), n),
        None => (root_events, None, Vec::new()),
    };
    let t0 = Instant::now();
    let mut e = host
        .boot(&BootCfg { seed: 1, events: root_events.clone(), frame: root_frame, dump_locals: true, lean: true, out_dir: eng_dir(&out) })
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
    s.gains_only = opt("--lines").as_deref() == Some("gains");
    s.flag_lines = a.iter().any(|x| x == "--flag-lines");
    s.go = a.iter().any(|x| x == "--go");
    let mut loaded: Vec<String> = Vec::new();
    if s.go {
        if let Ok(d) = std::env::var("FMI_GAME_SCRIPTS") {
            s.mentions = mentions_of(Path::new(&d));
        }
        // --edges FILE: the ways from room to room an earlier run saw (its OUT/edges.jsonl)
        if let Some(p) = opt("--edges") {
            for v in std::fs::read_to_string(&p).expect("--edges").lines().filter_map(|l| serde_json::from_str::<Value>(l).ok()) {
                if let (Some(a), Some(b), Some(h)) = (v["from"].as_i64(), v["to"].as_i64(), v["how"].as_str().and_then(hits_of)) {
                    if v["failed"].as_bool() == Some(true) {
                        // a way that run found false
                        s.edges.remove(&(a as i32, b as i32));
                        *s.dead.entry((a as i32, b as i32, v["how"].as_str().unwrap_or("").to_string())).or_default() += 1;
                    } else if s.dead.get(&(a as i32, b as i32, v["how"].as_str().unwrap_or("").to_string())).copied().unwrap_or(0) < 2 {
                        s.edges.entry((a as i32, b as i32)).or_insert(h);
                    }
                    loaded.push(v.to_string());
                }
            }
        }
        // the ways the scripts advise (an object whose code leads ego to another room), where none was seen: a click
        // on the object. Advice only: a walk that fails on one forgets it
        let seen = s.edges.len();
        for &(a, b, o) in &s.mentions.exits {
            if !s.dead.contains_key(&(a, b, format!("t{o}"))) {
                s.edges.entry((a, b)).or_insert_with(|| vec![Hit::Object(o)]);
            }
        }
        eprintln!("mentions: {} objects, {} flags named in rooms' scripts; {} ways from room to room seen, {} more the scripts advise", s.mentions.objs.len(), s.mentions.bits.len(), seen, s.edges.len() - seen);
    }
    s.edges_log = std::fs::File::create(out.join("edges.jsonl")).ok();
    if let Some(f) = s.edges_log.as_mut() {
        for l in &loaded {
            let _ = writeln!(f, "{l}");
        }
    }
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
    let gline = line_of(&[], &[]);
    let line = if s.flag_lines {
        let mut h = DefaultHasher::new();
        (gline, 0usize).hash(&mut h);
        h.finish()
    } else {
        gline
    };
    for p in r.facts.iter().filter(|p| world_kind(p.0) || p.0.kind() == K_VAR) {
        s.seen.insert((line, p.0, p.1));
        s.seen_at.insert((line, r.room, p.0, p.1));
    }
    for &t in &r.vis {
        s.sees.insert((line, r.room, t));
    }
    s.ifaces.insert((line, r.iface));
    let flags0 = r.facts.iter().filter(|p| p.0.kind() == K_BIT && p.1 != 0).count() as u32;
    s.counts.insert((line, flags0));
    s.boxes.insert((line, r.room, r.walkbox));
    s.cams.insert((line, r.room, r.cam));
    s.rooms.push(r.room);
    for g in s.rungs.iter_mut().filter(|g| g.conds.is_empty()) {
        g.at = Some((0, 0.0));
    }
    // what the starting stop won (its line of nodes.jsonl, or --news KEY,KEY for a stop of an older run): it is
    // walked from like any stop that begins a line
    let root_news: Vec<Key> = match opt("--news") {
        Some(t) => t.split(',').filter(|x| !x.is_empty()).map(Key::parse).collect(),
        None => from_news.clone(),
    };
    s.nodes.push(Node { events: root_events, frame: st.frame, depth: 0, ever: Vec::new(), level: 0, line, room: r.room, pending: Vec::new(), inside: false, flags: flags0, fline: 0, gline, news: root_news });
    s.queues.push(Default::default());
    s.stacks.push(Vec::new());
    s.queues[0][0].push_back(0);
    s.live.insert(0, e);
    let limit = Duration::from_secs_f64(minutes * 60.0);
    let (mut last, mut printed) = (0u64, Instant::now());
    type Job = (usize, Option<Box<dyn Engine>>, Vec<Event>, u64, u32, Vec<(Key, Val)>, bool, Vec<Key>);
    type Done = (usize, Result<(Box<dyn Engine>, Out), String>, bool);
    type Parent = (Box<dyn Engine>, Vec<(usize, Vec<Event>, u64)>);
    while t0.elapsed() < limit {
        // a batch: the next stops in the queues' order, each with its kept engine if it has one
        let mut batch: Vec<Mutex<Option<Job>>> = Vec::new();
        let at_once = if cfg.at_once > 0 { cfg.at_once } else { (cfg.workers / cfg.slices.max(1)).max(1) };
        while batch.len() < at_once * cfg.batch {
            // the highest level first and in it the first queue, over the first queues of every level; then the same
            // over the later ones
            // with --flag-lines the newest line's stops go before any older line's (a line is as new as the stop
            // with a new set of flags it began at): of that line the stack's first (its newest stop: the walk
            // through the rooms), then queue 1, 2 and 3, each its earliest stop first. An older line's stops, the
            // stack's too, wait until the newest line has none: when no new set of flags is left to follow, what
            // the newest line saw on its way is gone through, not the walks of the lines left behind
            let newest = s.flag_lines;
            let first = (0..s.queues.len()).rev().find_map(|l| {
                if !newest {
                    if let Some(id) = s.stacks.get_mut(l).and_then(|st| st.pop()) {
                        return Some(id);
                    }
                    return s.queues[l][..FIRST_QUEUES].iter_mut().find_map(|c| c.pop_front());
                }
                let nodes = &s.nodes;
                // (the line's age, the queue, the order in it) of the best stop in the stack and in each queue
                let top = s.stacks.get(l).and_then(|st| {
                    (0..st.len()).max_by_key(|&i| (nodes[st[i]].fline, st[i])).map(|i| ((std::cmp::Reverse(nodes[st[i]].fline), 0usize, std::cmp::Reverse(st[i]).0), usize::MAX, i))
                });
                let rest = (0..FIRST_QUEUES)
                    .flat_map(|c| (0..s.queues[l][c].len()).map(move |i| (c, i)))
                    .map(|(c, i)| {
                        let id = s.queues[l][c][i];
                        ((std::cmp::Reverse(nodes[id].fline), c + 1, id), c, i)
                    })
                    .min_by_key(|x| x.0);
                // the stack's best is its newest stop of the newest line; a queue's best its earliest of that line
                let pick = match (top, rest) {
                    (Some(t), Some(r)) => {
                        if (t.0 .0, t.0 .1) <= (r.0 .0, r.0 .1) {
                            t
                        } else {
                            r
                        }
                    }
                    (Some(t), None) => t,
                    (None, Some(r)) => r,
                    (None, None) => return None,
                };
                if pick.1 == usize::MAX {
                    Some(s.stacks[l].remove(pick.2))
                } else {
                    s.queues[l][pick.1].remove(pick.2)
                }
            });
            // the last queue: of the highest level that has one, the stop that holds the most flags (the earliest of
            // them): what is won one flag at a time is carried on by the stop that has won most
            let nodes = &s.nodes;
            let last = || {
                s.queues.iter_mut().rev().find_map(|q| {
                    let c = &mut q[QUEUES - 1];
                    let i = (0..c.len()).max_by_key(|&i| (nodes[c[i]].flags, std::cmp::Reverse(c[i])))?;
                    c.remove(i)
                })
            };
            let Some(id) = first.or_else(last) else { break };
            let n = &s.nodes[id];
            batch.push(Mutex::new(Some((id, s.live.remove(&id), n.events.clone(), n.frame, n.level, n.pending.clone(), n.inside, if s.go { n.news.clone() } else { Vec::new() }))));
        }
        if batch.is_empty() {
            break;
        }
        let snap = Snap { motion: &s.motion, wins: &s.wins, noise: &s.noise, lclocks: &s.lclocks, policy: &s.policy, ambient: &s.ambient, mentions: &s.mentions, edges: &s.edges };
        // side by side: each stop explored, from the search's state at the batch's start
        let n = batch.len();
        let t_batch = Instant::now();
        let done: Vec<Mutex<Option<Done>>> = (0..n).map(|_| Mutex::new(None)).collect();
        let next = AtomicUsize::new(0);
        std::thread::scope(|sc| {
            for _ in 0..at_once.min(n) {
                sc.spawn(|| loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    if i >= n {
                        break;
                    }
                    let (id, engine, events, frame, level, pending, inside, news) = batch[i].lock().unwrap().take().unwrap();
                    let booted = engine.is_none();
                    let e = match engine {
                        Some(e) => Ok(e),
                        None => host.boot(&BootCfg { seed: 1, events, frame: Some(frame), dump_locals: true, lean: true, out_dir: eng_dir(&cfg.out) }),
                    };
                    let r = match e {
                        Ok(mut e) => match { trace(&format!("node {id}: explore")); let r = explore(&mut e, level, &pending, inside, &news, &snap, &cfg); trace(&format!("node {id}: explored")); r } {
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
        let t_explored = t_batch.elapsed().as_secs_f64();
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
        let t_committed = t_batch.elapsed().as_secs_f64();
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
        trace(&format!("batch of {n}: explored in {t_explored:.1} s, committed in {:.1} s, engines in {:.1} s", t_committed - t_explored, t_batch.elapsed().as_secs_f64() - t_committed));
        for (nid, b) in made.into_inner().unwrap() {
            s.live.insert(nid, b);
        }
        // the oldest kept engines go when there are too many: their stops wait longest
        while s.live.len() > s.live_max {
            let Some(&old) = s.live.keys().min() else { break };
            if let Some(e) = s.live.remove(&old) {
                e.quit();
            }
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
